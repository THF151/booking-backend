use axum::{extract::{State, Path, Query}, response::IntoResponse, Json};
use crate::state::AppState;
use crate::api::extractors::{auth::AuthUser, tenant::TenantId, maybe_auth::MaybeAuthUser};
use crate::api::dtos::{
    requests::{CreateEventRequest, UpdateEventRequest},
    responses::SlotsResponse
};
use crate::domain::models::{event::Event, communication::{EmailTemplate, NotificationRule, EmailTemplateVersion}};
use crate::domain::services::{availability::calculate_slots, defaults};
use crate::error::AppError;
use std::sync::Arc;
use uuid::Uuid;
use chrono::{Utc, NaiveDate, Duration, TimeZone};
use chrono_tz::Tz;
use tracing::info;
use std::collections::HashMap;

pub async fn create_event(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    AuthUser(_user): AuthUser,
    Json(payload): Json<CreateEventRequest>,
) -> Result<impl IntoResponse, AppError> {
    info!("Creating event: {} for tenant: {}", payload.slug, tenant_id);

    match payload.access_mode.as_str() {
        "OPEN" | "RESTRICTED" | "CLOSED" => {},
        _ => return Err(AppError::Validation("Invalid access_mode".into()))
    }

    let schedule_type = payload.schedule_type.unwrap_or_else(|| "RECURRING".to_string());
    match schedule_type.as_str() {
        "RECURRING" | "MANUAL" => {},
        _ => return Err(AppError::Validation("Invalid schedule_type".into()))
    }

    if payload.active_end < payload.active_start {
        return Err(AppError::Validation("End date must be after start date".into()));
    }

    if payload.timezone.parse::<Tz>().is_err() {
        return Err(AppError::Validation("Invalid timezone".into()));
    }

    let config_json = serde_json::to_string(&payload.config)
        .map_err(|_| AppError::Validation("Invalid config JSON".into()))?;

    let event = Event {
        id: Uuid::new_v4().to_string(),
        tenant_id: tenant_id.clone(),
        slug: payload.slug.clone(),
        title_en: payload.title_en,
        title_de: payload.title_de,
        desc_en: payload.desc_en,
        desc_de: payload.desc_de,
        location: payload.location,
        payout: payload.payout,
        host_name: payload.host_name,
        timezone: payload.timezone,
        min_notice_general: payload.min_notice_general.unwrap_or(0),
        min_notice_first: payload.min_notice_first.unwrap_or(0),
        active_start: payload.active_start,
        active_end: payload.active_end,
        duration_min: payload.duration_min,
        interval_min: payload.interval_min,
        max_participants: payload.max_participants,
        image_url: payload.image_url,
        config_json,
        access_mode: payload.access_mode,
        schedule_type,
        allow_customer_cancel: payload.allow_customer_cancel.unwrap_or(true),
        allow_customer_reschedule: payload.allow_customer_reschedule.unwrap_or(true),
        created_at: Utc::now(),
    };

    let created_event = state.event_repo.create(&event).await?;

    let templates_to_create = vec![
        ("Confirmation", defaults::DEFAULT_CONFIRMATION_SUBJECT, defaults::get_default_template("confirmation"), Some("ON_BOOKING")),
        ("Reminder 24h", defaults::DEFAULT_REMINDER_SUBJECT, defaults::get_default_template("reminder"), Some("REMINDER_24H")),
        ("Cancellation", defaults::DEFAULT_CANCELLATION_SUBJECT, defaults::get_default_template("cancellation"), Some("ON_CANCEL")),
        ("Reschedule", defaults::DEFAULT_RESCHEDULE_SUBJECT, defaults::get_default_template("reschedule"), Some("ON_RESCHEDULE")),
        ("Invitation", defaults::DEFAULT_INVITATION_SUBJECT, defaults::get_default_template("invitation"), None),
    ];

    for (suffix, subj, body, trigger_opt) in templates_to_create {
        let name = format!("{} - {}", payload.slug, suffix);
        let tmpl = EmailTemplate::new(
            tenant_id.clone(),
            Some(created_event.id.clone()),
            name,
            subj.to_string(),
            body,
            "mjml".to_string()
        );
        let saved_tmpl = state.communication_repo.create_template(&tmpl).await?;

        let ver = EmailTemplateVersion::new(saved_tmpl.id.clone(), saved_tmpl.subject_template.clone(), saved_tmpl.body_template.clone());
        let _ = state.communication_repo.create_template_version(&ver).await;

        if let Some(trigger) = trigger_opt {
            let rule = NotificationRule::new(
                tenant_id.clone(),
                Some(created_event.id.clone()),
                trigger.to_string(),
                saved_tmpl.id
            );
            let _ = state.communication_repo.create_rule(&rule).await;
        }
    }

    Ok(Json(created_event))
}

pub async fn list_events(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    let events = state.event_repo.list(&tenant_id).await?;
    Ok(Json(events))
}

pub async fn update_event(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
    Json(payload): Json<UpdateEventRequest>,
) -> Result<impl IntoResponse, AppError> {
    let mut event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    if let Some(val) = payload.slug { event.slug = val; }
    if let Some(val) = payload.title_en { event.title_en = val; }
    if let Some(val) = payload.title_de { event.title_de = val; }
    if let Some(val) = payload.desc_en { event.desc_en = val; }
    if let Some(val) = payload.desc_de { event.desc_de = val; }
    if let Some(val) = payload.location { event.location = val; }
    if let Some(val) = payload.payout { event.payout = val; }
    if let Some(val) = payload.host_name { event.host_name = val; }
    if let Some(val) = payload.timezone {
        if val.parse::<Tz>().is_err() {
            return Err(AppError::Validation("Invalid timezone".into()));
        }
        event.timezone = val;
    }
    if let Some(val) = payload.min_notice_general { event.min_notice_general = val; }
    if let Some(val) = payload.min_notice_first { event.min_notice_first = val; }
    if let Some(val) = payload.active_start { event.active_start = val; }
    if let Some(val) = payload.active_end { event.active_end = val; }
    if let Some(val) = payload.duration_min { event.duration_min = val; }
    if let Some(val) = payload.interval_min { event.interval_min = val; }
    if let Some(val) = payload.max_participants { event.max_participants = val; }
    if let Some(val) = payload.image_url { event.image_url = val; }
    if let Some(val) = payload.access_mode { event.access_mode = val; }
    if let Some(val) = payload.schedule_type { event.schedule_type = val; }
    if let Some(val) = payload.allow_customer_cancel { event.allow_customer_cancel = val; }
    if let Some(val) = payload.allow_customer_reschedule { event.allow_customer_reschedule = val; }
    if let Some(val) = payload.config {
        event.config_json = serde_json::to_string(&val)
            .map_err(|_| AppError::Validation("Invalid config".into()))?;
    }

    let updated = state.event_repo.update(&event).await?;
    info!("Event updated: {}", slug);
    Ok(Json(updated))
}

pub async fn delete_event(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    _user: AuthUser,
    Path((_, slug)): Path<(String, String)>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    state.event_repo.delete(&tenant_id, &event.id).await?;
    info!("Event deleted: {}", slug);
    Ok(Json(serde_json::json!({"status": "deleted"})))
}

pub async fn get_event(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    MaybeAuthUser(maybe_user): MaybeAuthUser,
    Path((_, slug)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or_else(|| AppError::NotFound(format!("Event '{}' not found", slug)))?;

    if maybe_user.is_some() {
        return Ok(Json(serde_json::to_value(event).unwrap()));
    }

    let mut invitee_email = None;

    match event.access_mode.as_str() {
        "CLOSED" => {
            return Err(AppError::Forbidden("This event is closed.".into()));
        },
        "RESTRICTED" => {
            let token = params.get("token")
                .ok_or(AppError::Forbidden("Access restricted. Token required.".into()))?;

            let invitee = state.invitee_repo.find_by_token(token).await?
                .ok_or(AppError::Forbidden("Invalid token.".into()))?;

            if invitee.event_id != event.id {
                return Err(AppError::Forbidden("Token does not belong to this event.".into()));
            }
            if invitee.status != "ACTIVE" {
                return Err(AppError::Conflict("This invitation token has already been used.".into()));
            }

            invitee_email = invitee.email;
        },
        _ => {}
    }

    let mut event_json = serde_json::to_value(&event).map_err(|_| AppError::Internal)?;

    if let Some(email) = invitee_email {
        event_json["invitee_email"] = serde_json::Value::String(email);
    }

    Ok(Json(event_json))
}

pub async fn get_available_dates(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    Path((_, slug)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    let start_str = params.get("start").ok_or(AppError::Validation("start required".into()))?;
    let end_str = params.get("end").ok_or(AppError::Validation("end required".into()))?;

    let start_date = NaiveDate::parse_from_str(start_str, "%Y-%m-%d").map_err(|_| AppError::Validation("Invalid start".into()))?;
    let end_date = NaiveDate::parse_from_str(end_str, "%Y-%m-%d").map_err(|_| AppError::Validation("Invalid end".into()))?;

    let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);

    let range_start_tz = tz.from_local_datetime(&start_date.and_hms_opt(0,0,0).unwrap()).single().unwrap();
    let range_end_tz = tz.from_local_datetime(&end_date.and_hms_opt(23,59,59).unwrap()).single().unwrap();

    let range_start_utc = range_start_tz.with_timezone(&Utc);
    let range_end_utc = range_end_tz.with_timezone(&Utc);

    let all_bookings = state.booking_repo.list_by_range(&event.id, range_start_utc, range_end_utc).await?;

    let overrides = if event.schedule_type == "MANUAL" {
        vec![]
    } else {
        state.event_override_repo.list_by_range(&event.id, start_date, end_date).await?
    };

    let manual_sessions = if event.schedule_type == "MANUAL" {
        Some(state.session_repo.list_by_range(&event.id, range_start_utc, range_end_utc).await?)
    } else {
        None
    };

    let mut available_dates = Vec::new();
    let mut current_date = start_date;

    while current_date <= end_date {
        let day_start_tz = tz.from_local_datetime(&current_date.and_hms_opt(0,0,0).unwrap()).single().unwrap();
        let day_end_tz = tz.from_local_datetime(&current_date.and_hms_opt(23,59,59).unwrap()).single().unwrap();

        let day_start_utc = day_start_tz.with_timezone(&Utc);
        let day_end_utc = day_end_tz.with_timezone(&Utc);

        if day_end_utc >= event.active_start && day_start_utc <= event.active_end {
            let day_bookings: Vec<_> = all_bookings.iter()
                .filter(|b| b.start_time < day_end_utc && b.end_time > day_start_utc)
                .cloned()
                .collect();

            let override_rule = overrides.iter().find(|o| o.date == current_date);
            let slots = calculate_slots(&event, current_date, &day_bookings, override_rule, manual_sessions.as_deref());
            if !slots.is_empty() {
                available_dates.push(current_date.to_string());
            }
        }
        current_date += Duration::days(1);
    }

    Ok(Json(available_dates))
}

pub async fn get_slots(
    State(state): State<Arc<AppState>>,
    TenantId(tenant_id): TenantId,
    Path((_, slug)): Path<(String, String)>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, AppError> {
    let event = state.event_repo.find_by_slug(&tenant_id, &slug).await?
        .ok_or(AppError::NotFound("Event not found".into()))?;

    if event.access_mode == "CLOSED" {
        return Err(AppError::Forbidden("Event is closed".into()));
    }

    let date_str = params.get("date").ok_or(AppError::Validation("Date required".into()))?;
    let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|_| AppError::Validation("Invalid date format".into()))?;

    let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);

    let day_start_tz = tz.from_local_datetime(&date.and_hms_opt(0,0,0).unwrap()).single().unwrap();
    let day_end_tz = tz.from_local_datetime(&date.and_hms_opt(23,59,59).unwrap()).single().unwrap();

    let day_start_utc = day_start_tz.with_timezone(&Utc);
    let day_end_utc = day_end_tz.with_timezone(&Utc);

    let bookings = state.booking_repo.list_by_range(&event.id, day_start_utc, day_end_utc).await?;

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

    let slots = calculate_slots(&event, date, &bookings, override_rule.as_ref(), manual_sessions.as_deref());

    Ok(Json(SlotsResponse {
        date: date_str.to_string(),
        slots,
    }))
}