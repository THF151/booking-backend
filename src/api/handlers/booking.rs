use axum::{extract::{State, Path}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId};
use crate::api::dtos::requests::{CreateBookingRequest, UpdateBookingRequest};
use crate::domain::models::booking::{Booking, NewBookingParams};
use crate::domain::models::job::Job;
use crate::domain::services::availability::calculate_slots;
use crate::error::AppError;
use std::sync::Arc;
use chrono::{NaiveDate, NaiveTime, Utc, TimeZone, Duration};
use chrono_tz::Tz;
use tracing::{info, warn};

pub async fn create_booking(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    Path((_, slug)): Path<(String, String)>,
    Json(payload): Json<CreateBookingRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("create_booking: Starting for slug {}", slug);

    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    info!("create_booking: Event found: {}", event.id);

    let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);

    let mut invitee_id = None;
    let mut token_to_burn = None;

    match event.access_mode.as_str() {
        "CLOSED" => return Err(AppError::Forbidden("Event is closed".into())),
        "RESTRICTED" => {
            let token = payload.token.as_ref()
                .ok_or(AppError::Forbidden("Token required for restricted event".into()))?;

            let invitee = state.invitee_repo.find_by_token(token).await?
                .ok_or(AppError::Forbidden("Invalid token".into()))?;

            if invitee.event_id != event.id {
                return Err(AppError::Forbidden("Token invalid for this event".into()));
            }
            if invitee.status != "ACTIVE" {
                return Err(AppError::Conflict("Token already used".into()));
            }

            invitee_id = Some(invitee.id);
            token_to_burn = Some(token.clone());
        },
        "OPEN" => {}
        _ => return Err(AppError::Internal),
    }

    let date = NaiveDate::parse_from_str(&payload.date, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date format".into()))?;

    let time = if payload.time.contains('T') {
        let dt = chrono::DateTime::parse_from_rfc3339(&payload.time)
            .map_err(|_| AppError::Validation("Invalid ISO time format".into()))?;
        dt.with_timezone(&tz).time()
    } else {
        NaiveTime::parse_from_str(&payload.time, "%H:%M")
            .map_err(|_| AppError::Validation("Invalid time format (HH:MM)".into()))?
    };

    let naive_dt = date.and_time(time);

    let start_time = tz.from_local_datetime(&naive_dt)
        .single()
        .ok_or(AppError::Validation("Invalid local time (ambiguous or skipped due to DST)".into()))?
        .with_timezone(&Utc);

    let mut end_time = start_time + Duration::minutes(event.duration_min as i64);

    if start_time < Utc::now() {
        return Err(AppError::Validation("Cannot book in the past".into()));
    }
    if start_time < event.active_start || end_time > event.active_end {
        return Err(AppError::Validation("Booking outside of event active range".into()));
    }

    info!("create_booking: Checking availability for {} (UTC: {})", date, start_time);

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

    if let Some(ref rule) = override_rule
        && rule.is_unavailable {
        return Err(AppError::Conflict("Date is unavailable".into()));
    }

    let manual_sessions = if event.schedule_type == "MANUAL" {
        let sessions = state.session_repo.list_by_range(&event.id, day_start_utc, day_end_utc).await?;
        if let Some(session) = sessions.iter().find(|s| s.start_time == start_time) {
            end_time = session.end_time;
        } else {
            return Err(AppError::Conflict("Selected time slot does not exist (manual session missing)".into()));
        }
        Some(sessions)
    } else {
        None
    };

    let valid_slots_utc = calculate_slots(&event, date, &existing_bookings, override_rule.as_ref(), manual_sessions.as_deref());

    let requested_iso = start_time.to_rfc3339();

    if !valid_slots_utc.contains(&requested_iso) {
        warn!("Booking rejected: Slot {} (UTC) not available. Valid slots: {:?}", requested_iso, valid_slots_utc);
        return Err(AppError::Conflict("Selected time slot is not available or valid".into()));
    }

    let location = override_rule.and_then(|r| r.location)
        .or_else(|| manual_sessions.as_ref().and_then(|s|
            s.iter().find(|sess| sess.start_time == start_time).and_then(|sess| sess.location.clone())
        ));

    let booking = Booking::new(NewBookingParams {
        tenant_id: tenant_id.clone(),
        event_id: event.id.clone(),
        start: start_time,
        duration_min: (end_time - start_time).num_minutes() as i32,
        name: payload.name,
        email: payload.email,
        note: payload.notes,
        invitee_id,
        location
    });

    let mut jobs = Vec::new();

    let rules = state.communication_repo.get_rules_by_event(&event.id).await?;

    for rule in rules {
        match rule.trigger_type.as_str() {
            "ON_BOOKING" => {
                jobs.push(Job::new("CONFIRMATION", booking.id.clone(), tenant_id.clone(), Utc::now()));
            },
            "REMINDER_24H" => {
                let remind_at = booking.start_time - Duration::hours(24);
                if remind_at > Utc::now() {
                    jobs.push(Job::new("REMINDER", booking.id.clone(), tenant_id.clone(), remind_at));
                }
            },
            "REMINDER_1H" => {
                let remind_at = booking.start_time - Duration::hours(1);
                if remind_at > Utc::now() {
                    jobs.push(Job::new("REMINDER", booking.id.clone(), tenant_id.clone(), remind_at));
                }
            },
            _ => {
                if rule.trigger_type.starts_with("REMINDER_") && rule.trigger_type.ends_with("M")
                    && let Ok(minutes) = rule.trigger_type[9..rule.trigger_type.len()-1].parse::<i64>() {
                    let remind_at = booking.start_time - Duration::minutes(minutes);
                    if remind_at > Utc::now() {
                        jobs.push(Job::new("REMINDER", booking.id.clone(), tenant_id.clone(), remind_at));
                    }
                }
            }
        }
    }

    info!("create_booking: Inserting booking into DB...");
    let created = state.booking_repo.create_with_token(&booking, token_to_burn, jobs).await?;
    info!("create_booking: DB Insert success: {}", created.id);

    info!("Booking confirmed: {} for event {}", created.id, slug);
    Ok(Json(created))
}

pub async fn list_bookings(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let bookings = state.booking_repo.list_by_event(&tenant_id, &event.id).await?;
    Ok(Json(bookings))
}

pub async fn list_all_bookings(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let bookings = state.booking_repo.list_by_tenant(&tenant_id).await?;
    Ok(Json(bookings))
}

pub async fn get_booking(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, booking_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let booking = state.booking_repo.find_by_id(&tenant_id, &booking_id).await?
        .ok_or(AppError::NotFound("Booking not found".into()))?;
    Ok(Json(booking))
}

pub async fn update_booking(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, booking_id)): Path<(String, String)>,
    Json(payload): Json<UpdateBookingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut booking = state.booking_repo.find_by_id(&tenant_id, &booking_id).await?
        .ok_or(AppError::NotFound("Booking not found".into()))?;

    if let Some(name) = payload.name { booking.customer_name = name; }
    if let Some(email) = payload.email { booking.customer_email = email; }

    if let Some(label_id) = payload.label_id {
        if label_id.is_empty() {
            booking.label_id = None;
        } else {
            booking.label_id = Some(label_id);
        }
    }

    if let Some(payout) = payload.payout {
        booking.payout = Some(payout);
    }

    if let Some(t) = payload.token {
        if t.is_empty() {
            booking.token = None;
        } else {
            booking.token = Some(t);
        }
    }

    if let (Some(date_str), Some(time_str)) = (payload.date, payload.time) {
        let event = state.event_repo.find_by_id(&tenant_id, &booking.event_id).await?
            .ok_or(AppError::Internal)?;

        let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);

        let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
            .map_err(|_| AppError::Validation("Invalid date".into()))?;

        let time = if time_str.contains('T') {
            let dt = chrono::DateTime::parse_from_rfc3339(&time_str)
                .map_err(|_| AppError::Validation("Invalid ISO time".into()))?;
            dt.with_timezone(&tz).time()
        } else {
            NaiveTime::parse_from_str(&time_str, "%H:%M")
                .map_err(|_| AppError::Validation("Invalid time".into()))?
        };

        let naive_dt = date.and_time(time);
        let new_start = tz.from_local_datetime(&naive_dt)
            .single()
            .ok_or(AppError::Validation("Invalid local time".into()))?
            .with_timezone(&Utc);

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

        let manual_sessions = if event.schedule_type == "MANUAL" {
            Some(state.session_repo.list_by_range(&event.id, day_start_utc, day_end_utc).await?)
        } else {
            None
        };

        let valid_slots_utc = calculate_slots(&event, date, &existing_bookings, override_rule.as_ref(), manual_sessions.as_deref());
        let requested_iso = new_start.to_rfc3339();

        if !valid_slots_utc.contains(&requested_iso) {
            return Err(AppError::Conflict("Target slot is unavailable or invalid".into()));
        }

        let mut new_end = new_start + Duration::minutes(event.duration_min as i64);
        if let Some(sessions) = manual_sessions
            && let Some(session) = sessions.iter().find(|s| s.start_time == new_start) {
            new_end = session.end_time;
        }

        booking.start_time = new_start;
        booking.end_time = new_end;
    }

    let updated = state.booking_repo.update(&booking).await?;
    info!("Booking updated: {}", updated.id);
    Ok(Json(updated))
}

pub async fn delete_booking(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, booking_id)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    state.booking_repo.delete(&tenant_id, &booking_id).await?;
    info!("Booking cancelled: {}", booking_id);
    Ok(Json(serde_json::json!({"status": "cancelled"})))
}