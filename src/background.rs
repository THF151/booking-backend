use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use tokio::time::sleep;
use tracing::{error, info, warn, info_span, Instrument};
use crate::state::AppState;
use crate::domain::services::calendar::generate_ics;
use crate::domain::services::communication_service::CommunicationService;
use chrono_tz::Tz;
use serde_json::json;

pub async fn start_background_worker(state: Arc<AppState>) {
    info!("Starting background job worker...");

    let comm_service = CommunicationService::new(state.communication_repo.clone());

    loop {
        match state.job_repo.find_pending(10).await {
            Ok(jobs) => {
                for job in jobs {
                    let job_id = job.id.clone();
                    let job_type = job.job_type.clone();
                    let tenant_id = job.payload.tenant_id.clone();

                    let span = info_span!(
                        "background_job",
                        job_id = %job_id,
                        job_type = %job_type,
                        tenant_id = %tenant_id
                    );

                    let state = state.clone();
                    let comm_service_ref = &comm_service;

                    async move {
                        info!("Processing job: {}", job_type);
                        match process_job(&state, comm_service_ref, &job).await {
                            Ok(_) => {
                                info!("Job completed successfully");
                                if let Err(e) = state.job_repo.update_status(&job.id, "COMPLETED", None).await {
                                    error!("Failed to mark job as completed: {:?}", e);
                                }
                            },
                            Err(e) => {
                                let err_msg = format!("{}", e);
                                error!("Job failed with error: {}", err_msg);
                                if let Err(up_err) = state.job_repo.update_status(&job.id, "FAILED", Some(err_msg)).await {
                                    error!("Failed to mark job as failed: {:?}", up_err);
                                }
                            }
                        }
                    }
                        .instrument(span)
                        .await;
                }
            }
            Err(e) => error!("Failed to fetch pending jobs: {:?}", e),
        }
        sleep(Duration::from_secs(5)).await;
    }
}

fn render_email_body(template_type: &str, body_content: &str) -> Result<String, crate::error::AppError> {
    if template_type == "mjml" {
        match mrml::parse(body_content) {
            Ok(root) => {
                let opts = mrml::prelude::render::RenderOptions::default();
                match root.element.render(&opts) {
                    Ok(html) => Ok(html),
                    Err(e) => {
                        error!("MJML Render Error: {:?}", e);
                        Err(crate::error::AppError::InternalWithMsg(format!("MJML Render Error: {:?}", e)))
                    }
                }
            },
            Err(e) => {
                error!("MJML Parse Error: {:?}", e);
                Err(crate::error::AppError::InternalWithMsg(format!("MJML Parse Error: {:?}", e)))
            }
        }
    } else {
        Ok(body_content.to_string())
    }
}

async fn process_job(
    state: &Arc<AppState>,
    comm_service: &CommunicationService,
    job: &crate::domain::models::job::Job
) -> Result<(), crate::error::AppError> {
    let payload_id = &job.payload.booking_id;
    let tenant_id = &job.payload.tenant_id;

    let tenant = state.tenant_repo.find_by_id(tenant_id).await?
        .ok_or(crate::error::AppError::NotFound(format!("Tenant {} not found", tenant_id)))?;

    if job.job_type.starts_with("CAMPAIGN:") {
        info!("Handling CAMPAIGN job type: {}", job.job_type);
        let parts: Vec<&str> = job.job_type.split(':').collect();
        if parts.len() < 3 { return Err(crate::error::AppError::InternalWithMsg("Invalid Campaign Job Type Format".to_string())); }

        let target_type = parts[1];
        let template_id = parts[2];

        let mut email = String::new();
        let mut context_map = serde_json::Map::new();
        let event_id;

        if target_type == "BOOKING" {
            let booking = state.booking_repo.find_by_id(tenant_id, payload_id).await?
                .ok_or(crate::error::AppError::NotFound(format!("Booking {} not found", payload_id)))?;
            email = booking.customer_email;
            event_id = booking.event_id;
            context_map.insert("user_name".to_string(), json!(booking.customer_name));
        } else if target_type == "INVITEE" {
            let invitee = state.invitee_repo.find_by_id(tenant_id, payload_id).await?
                .ok_or(crate::error::AppError::NotFound(format!("Invitee {} not found", payload_id)))?;
            email = invitee.email.ok_or(crate::error::AppError::Validation("Invitee has no email".into()))?;
            event_id = invitee.event_id;
            context_map.insert("token".to_string(), json!(invitee.token));
            context_map.insert("user_name".to_string(), json!(""));
        } else {
            return Err(crate::error::AppError::InternalWithMsg(format!("Unknown target type {}", target_type)));
        }

        let event = state.event_repo.find_by_id(tenant_id, &event_id).await?
            .ok_or(crate::error::AppError::NotFound(format!("Event {} not found", event_id)))?;

        context_map.insert("event_title".to_string(), json!(event.title_en));
        context_map.insert("event_description".to_string(), json!(event.desc_en));
        context_map.insert("location".to_string(), json!(event.location));
        context_map.insert("duration".to_string(), json!(event.duration_min));
        context_map.insert("timezone".to_string(), json!(event.timezone));
        context_map.insert("payout".to_string(), json!(event.payout));
        context_map.insert("tenant_name".to_string(), json!(tenant.name));
        context_map.insert("logo_url".to_string(), json!(tenant.logo_url.unwrap_or_default()));

        let base_url = &state.config.frontend_url;

        if target_type == "BOOKING" {
            let booking = state.booking_repo.find_by_id(tenant_id, payload_id).await?.unwrap();
            let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);
            let event_time = booking.start_time.with_timezone(&tz);
            context_map.insert("start_time".to_string(), json!(event_time.format("%Y-%m-%d %H:%M").to_string()));
            let manage_link = format!("{}/en/manage/{}", base_url, booking.management_token);
            context_map.insert("manage_link".to_string(), json!(manage_link));
            if let Some(loc) = booking.location {
                context_map.insert("location".to_string(), json!(loc));
            }
        } else {
            context_map.insert("start_time".to_string(), json!(""));
            context_map.insert("manage_link".to_string(), json!(""));
            if let Some(token_val) = context_map.get("token") {
                let token_str = token_val.as_str().unwrap_or("");
                let book_link = format!("{}/en/book/{}/{}?accessToken={}", base_url, tenant_id, event.slug, token_str);
                context_map.insert("link".to_string(), json!(book_link));
                context_map.insert("book_link".to_string(), json!(book_link));
                context_map.insert("booking_link".to_string(), json!(book_link));
            }
        }

        let context_data = serde_json::Value::Object(context_map);

        info!("Loading template {}", template_id);
        let template = state.communication_repo.get_template(template_id).await?
            .ok_or(crate::error::AppError::NotFound(format!("Template {} not found", template_id)))?;

        let mut tera = tera::Tera::default();
        tera.add_raw_template(&template.name, &template.body_template).map_err(|e| crate::error::AppError::InternalWithMsg(format!("Tera parse error: {:?}", e)))?;
        let body_with_vars = tera.render(&template.name, &tera::Context::from_value(context_data.clone()).unwrap()).map_err(|e| crate::error::AppError::InternalWithMsg(format!("Tera render error: {:?}", e)))?;
        let final_html = render_email_body(&template.template_type, &body_with_vars)?;

        let subject_tmpl_name = format!("{}_subject", template.name);
        tera.add_raw_template(&subject_tmpl_name, &template.subject_template).map_err(|e| crate::error::AppError::InternalWithMsg(format!("Tera subject parse error: {:?}", e)))?;
        let final_subject = tera.render(&subject_tmpl_name, &tera::Context::from_value(context_data.clone()).unwrap()).map_err(|e| crate::error::AppError::InternalWithMsg(format!("Tera subject render error: {:?}", e)))?;

        info!("Sending campaign email to {}", email);
        state.email_service.send(&email, &final_subject, &final_html, None, None).await?;
        comm_service.record_success(&job.id, &email, &template.name, &context_data).await?;

        return Ok(());
    }

    // Standard Flow (Confirmation/Reminder)
    let booking = state.booking_repo.find_by_id(tenant_id, payload_id).await?
        .ok_or(crate::error::AppError::NotFound(format!("Booking {} not found", payload_id)))?;
    let event = state.event_repo.find_by_id(tenant_id, &booking.event_id).await?
        .ok_or(crate::error::AppError::NotFound(format!("Event {} not found", booking.event_id)))?;

    let mut context = tera::Context::new();
    context.insert("user_name", &booking.customer_name);
    context.insert("event_title", &event.title_en);
    context.insert("event_description", &event.desc_en);

    // New Variables
    context.insert("tenant_name", &tenant.name);
    context.insert("logo_url", &tenant.logo_url.unwrap_or_default());
    context.insert("payout", &event.payout);

    let tz: Tz = event.timezone.parse().unwrap_or(chrono_tz::UTC);
    let event_time = booking.start_time.with_timezone(&tz);

    context.insert("start_time", &event_time.format("%Y-%m-%d %H:%M").to_string());
    context.insert("timezone", &event.timezone);

    let display_location = booking.location.as_deref().unwrap_or(&event.location);
    context.insert("location", display_location);
    context.insert("duration", &event.duration_min);

    let base_url = &state.config.frontend_url;
    let manage_link = format!("{}/en/manage/{}", base_url, booking.management_token);
    context.insert("manage_link", &manage_link);
    let book_link = format!("{}/en/book/{}/{}", base_url, tenant_id, event.slug);
    context.insert("book_link", &book_link);
    context.insert("booking_link", &book_link); // Alias

    let mut resolved_trigger = job.job_type.clone();
    if resolved_trigger == "CONFIRMATION" { resolved_trigger = "ON_BOOKING".to_string(); }
    else if resolved_trigger == "CANCELLATION" { resolved_trigger = "ON_CANCEL".to_string(); }
    else if resolved_trigger == "RESCHEDULE" { resolved_trigger = "ON_RESCHEDULE".to_string(); }
    else if resolved_trigger == "REMINDER" {
        let diff = booking.start_time - job.execute_at;
        if diff.num_hours() >= 23 { resolved_trigger = "REMINDER_24H".to_string(); }
        else { resolved_trigger = "REMINDER_1H".to_string(); }
    }

    let rules = state.communication_repo.get_rules_by_trigger(tenant_id, Some(&event.id), &resolved_trigger).await?;
    let context_val = context.into_json();

    if let Some(rule) = rules.first() {
        info!("Using custom template rule {} for trigger {}", rule.id, resolved_trigger);
        let template = state.communication_repo.get_template(&rule.template_id).await?
            .ok_or(crate::error::AppError::NotFound(format!("Template {} not found", rule.template_id)))?;

        // Idempotency Check
        use sha2::{Sha256, Digest};
        let context_json = serde_json::to_string(&context_val).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(template.name.as_bytes());
        hasher.update(context_json.as_bytes());
        let hash = hex::encode(hasher.finalize());

        if state.communication_repo.has_mail_been_sent(&booking.customer_email, &template.name, &hash).await? {
            info!("Email skipped (idempotency) for job {}. Recipient: {}, Template: {}", job.id, booking.customer_email, template.name);
            let log = crate::domain::models::communication::MailLog {
                id: uuid::Uuid::new_v4().to_string(),
                job_id: job.id.clone(),
                recipient: booking.customer_email.clone(),
                template_id: template.name.clone(),
                context_hash: hash,
                sent_at: Utc::now(),
                status: "SKIPPED_DUPLICATE".to_string(),
            };
            state.communication_repo.log_mail(&log).await?;
            return Ok(());
        }

        let mut tera = tera::Tera::default();
        tera.add_raw_template(&template.name, &template.body_template).map_err(|e| {
            error!("Tera/MJML parse error: {:?}", e);
            crate::error::AppError::InternalWithMsg(format!("Tera parse error: {:?}", e))
        })?;

        let body_with_vars = tera.render(&template.name, &tera::Context::from_value(context_val.clone()).unwrap())
            .map_err(|e| {
                error!("Tera render error: {:?}", e);
                crate::error::AppError::InternalWithMsg(format!("Tera render error: {:?}", e))
            })?;

        let final_html = render_email_body(&template.template_type, &body_with_vars)?;

        let subject_tmpl_name = format!("{}_subject", template.name);
        tera.add_raw_template(&subject_tmpl_name, &template.subject_template).map_err(|e| crate::error::AppError::InternalWithMsg(format!("Tera subject error: {:?}", e)))?;
        let final_subject = tera.render(&subject_tmpl_name, &tera::Context::from_value(context_val.clone()).unwrap()).map_err(|e| crate::error::AppError::InternalWithMsg(format!("Tera subject render error: {:?}", e)))?;

        let (attachment_name, attachment_data) = if job.job_type == "CONFIRMATION" {
            let ics_string = generate_ics(&event, &booking);
            (Some("invite.ics"), Some(ics_string.into_bytes()))
        } else {
            (None, None)
        };

        info!("Sending custom email to {}", booking.customer_email);
        state.email_service.send(&booking.customer_email, &final_subject, &final_html, attachment_name, attachment_data.as_deref()).await?;
        comm_service.record_success(&job.id, &booking.customer_email, &template.name, &context_val).await?;
    } else {
        warn!("No notification rule found for event {} trigger {}. Skipping email.", event.id, resolved_trigger);
        return Ok(());
    }

    Ok(())
}