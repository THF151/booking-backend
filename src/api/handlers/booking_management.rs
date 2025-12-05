use axum::{extract::{State, Path}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::dtos::requests::RescheduleBookingRequest;
use crate::domain::services::availability::calculate_slots;
use crate::domain::models::job::Job;
use crate::error::AppError;
use std::sync::Arc;
use chrono::{NaiveDate, NaiveTime, Utc, TimeZone, Duration};
use chrono_tz::Tz;
use tracing::info;

pub async fn get_booking_by_token(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let booking = state.booking_repo.find_by_token(&token).await?
        .ok_or(AppError::NotFound("Booking not found".into()))?;

    if booking.status == "CANCELLED" {
        return Err(AppError::Validation("Booking is already cancelled".into()));
    }

    let event = state.event_repo.find_by_id(&booking.tenant_id, &booking.event_id).await?
        .ok_or(AppError::Internal)?;

    let response = serde_json::json!({
        "booking": booking,
        "event": event
    });

    Ok(Json(response))
}

pub async fn cancel_booking(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let booking = state.booking_repo.find_by_token(&token).await?
        .ok_or(AppError::NotFound("Booking not found".into()))?;

    if booking.status == "CANCELLED" {
        return Ok(Json(booking));
    }

    let event = state.event_repo.find_by_id(&booking.tenant_id, &booking.event_id).await?
        .ok_or(AppError::Internal)?;

    if !event.allow_customer_cancel {
        return Err(AppError::Forbidden("Cancellation is disabled for this event.".into()));
    }

    let cancelled = state.booking_repo.cancel(&booking).await?;
    info!("Booking cancelled via management token: {}", booking.id);

    let rules = state.communication_repo.get_rules_by_trigger(&booking.tenant_id, Some(&event.id), "ON_CANCEL").await?;
    if !rules.is_empty() {
        for _rule in rules {
            let job = Job::new("CANCELLATION", booking.id.clone(), booking.tenant_id.clone(), Utc::now());
            state.job_repo.create(&job).await?;
        }
    }

    state.job_repo.cancel_jobs_for_booking(&booking.id).await?;

    Ok(Json(cancelled))
}

pub async fn reschedule_booking(
    State(state): State<Arc<AppState>>,
    Path(token): Path<String>,
    Json(payload): Json<RescheduleBookingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let booking = state.booking_repo.find_by_token(&token).await?
        .ok_or(AppError::NotFound("Booking not found".into()))?;

    if booking.status == "CANCELLED" {
        return Err(AppError::Validation("Cannot reschedule a cancelled booking.".into()));
    }

    let event = state.event_repo.find_by_id(&booking.tenant_id, &booking.event_id).await?
        .ok_or(AppError::Internal)?;

    if !event.allow_customer_reschedule {
        return Err(AppError::Forbidden("Rescheduling is disabled for this event.".into()));
    }

    let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);
    let date = NaiveDate::parse_from_str(&payload.date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date".into()))?;

    let time = if payload.time.contains('T') {
        let dt = chrono::DateTime::parse_from_rfc3339(&payload.time)
            .map_err(|_| AppError::Validation("Invalid ISO time".into()))?;
        dt.with_timezone(&tz).time()
    } else {
        NaiveTime::parse_from_str(&payload.time, "%H:%M")
            .map_err(|_| AppError::Validation("Invalid time".into()))?
    };

    let naive_dt = date.and_time(time);
    let new_start = tz.from_local_datetime(&naive_dt).single().unwrap().with_timezone(&Utc);
    let mut new_end = new_start + Duration::minutes(event.duration_min as i64);

    let day_start_tz = tz.from_local_datetime(&date.and_hms_opt(0,0,0).unwrap()).single().unwrap();
    let day_end_tz = tz.from_local_datetime(&date.and_hms_opt(23,59,59).unwrap()).single().unwrap();
    let day_start_utc = day_start_tz.with_timezone(&Utc);
    let day_end_utc = day_end_tz.with_timezone(&Utc);

    let existing_bookings = state.booking_repo.list_by_range(&event.id, day_start_utc, day_end_utc).await?;
    let override_rule = if event.schedule_type == "MANUAL" {
        None
    } else {
        state.event_override_repo.find_by_date(&event.id, date).await?
    };
    if let Some(ref rule) = override_rule && rule.is_unavailable {
        return Err(AppError::Conflict("Date is unavailable".into()));
    }

    let manual_sessions = if event.schedule_type == "MANUAL" {
        let sessions = state.session_repo.list_by_range(&event.id, day_start_utc, day_end_utc).await?;
        if let Some(session) = sessions.iter().find(|s| s.start_time == new_start) {
            new_end = session.end_time;
        } else {
            return Err(AppError::Conflict("Invalid session".into()));
        }
        Some(sessions)
    } else {
        None
    };

    let valid_slots = calculate_slots(&event, date, &existing_bookings, override_rule.as_ref(), manual_sessions.as_deref());
    if !valid_slots.contains(&new_start.to_rfc3339()) {
        return Err(AppError::Conflict("New slot is not available.".into()));
    }

    let location = override_rule.and_then(|r| r.location)
        .or_else(|| manual_sessions.as_ref().and_then(|s|
            s.iter().find(|sess| sess.start_time == new_start).and_then(|sess| sess.location.clone())
        ));

    let mut booking_to_update = booking.clone();
    booking_to_update.start_time = new_start;
    booking_to_update.end_time = new_end;
    booking_to_update.location = location;

    let updated = state.booking_repo.update(&booking_to_update).await?;

    // Cancel old reminders
    state.job_repo.cancel_jobs_for_booking(&updated.id).await?;

    // Trigger Reschedule Email
    let rules = state.communication_repo.get_rules_by_trigger(&updated.tenant_id, Some(&event.id), "ON_RESCHEDULE").await?;
    for _rule in rules {
        let job = Job::new("RESCHEDULE", updated.id.clone(), updated.tenant_id.clone(), Utc::now());
        state.job_repo.create(&job).await?;
    }

    // Schedule New Reminders
    let reminder_rules = state.communication_repo.get_rules_by_event(&event.id).await?;
    for rule in reminder_rules {
        let mut remind_at = None;

        if rule.trigger_type == "REMINDER_24H" {
            remind_at = Some(updated.start_time - Duration::hours(24));
        } else if rule.trigger_type == "REMINDER_1H" {
            remind_at = Some(updated.start_time - Duration::hours(1));
        } else if rule.trigger_type.starts_with("REMINDER_") && rule.trigger_type.ends_with("M")
            && let Ok(minutes) = rule.trigger_type[9..rule.trigger_type.len()-1].parse::<i64>() {
                remind_at = Some(updated.start_time - Duration::minutes(minutes));
            }

        if let Some(at) = remind_at
            && at > Utc::now() {
                let job = Job::new(&rule.trigger_type, updated.id.clone(), updated.tenant_id.clone(), at);
                state.job_repo.create(&job).await?;
            }
    }

    info!("Rescheduled booking {}", updated.id);
    Ok(Json(updated))
}