use crate::domain::models::communication::{EmailTemplate, NotificationRule, MailLog, EmailTemplateVersion};
use crate::domain::ports::CommunicationRepository;
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;

pub struct PostgresCommunicationRepo {
    pool: PgPool,
}

impl PostgresCommunicationRepo {
    pub fn new(pool: PgPool) -> Self { Self { pool } }
}

#[async_trait]
impl CommunicationRepository for PostgresCommunicationRepo {
    async fn create_template(&self, t: &EmailTemplate) -> Result<EmailTemplate, AppError> {
        sqlx::query_as::<_, EmailTemplate>(
            "INSERT INTO email_templates (id, tenant_id, event_id, name, subject_template, body_template, template_type, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING *"
        )
            .bind(&t.id).bind(&t.tenant_id).bind(&t.event_id).bind(&t.name)
            .bind(&t.subject_template).bind(&t.body_template).bind(&t.template_type)
            .bind(t.created_at).bind(t.updated_at)
            .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_template(&self, id: &str) -> Result<Option<EmailTemplate>, AppError> {
        sqlx::query_as::<_, EmailTemplate>("SELECT * FROM email_templates WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn list_templates(&self, tenant_id: &str, event_id: Option<&str>) -> Result<Vec<EmailTemplate>, AppError> {
        let query = if event_id.is_some() {
            "SELECT * FROM email_templates WHERE tenant_id = $1 AND event_id = $2 ORDER BY name ASC"
        } else {
            "SELECT * FROM email_templates WHERE tenant_id = $1 ORDER BY name ASC"
        };

        let mut q = sqlx::query_as::<_, EmailTemplate>(query).bind(tenant_id);
        if let Some(eid) = event_id {
            q = q.bind(eid);
        }
        q.fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn update_template(&self, t: &EmailTemplate) -> Result<EmailTemplate, AppError> {
        sqlx::query_as::<_, EmailTemplate>(
            "UPDATE email_templates SET name=$1, subject_template=$2, body_template=$3, template_type=$4, updated_at=$5 WHERE id=$6 RETURNING *"
        )
            .bind(&t.name).bind(&t.subject_template).bind(&t.body_template)
            .bind(&t.template_type).bind(t.updated_at).bind(&t.id)
            .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn delete_template(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM email_templates WHERE id = $1")
            .bind(id)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }

    async fn create_template_version(&self, v: &EmailTemplateVersion) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO email_template_versions (id, template_id, subject_template, body_template, created_at)
             VALUES ($1, $2, $3, $4, $5)"
        )
            .bind(&v.id).bind(&v.template_id).bind(&v.subject_template)
            .bind(&v.body_template).bind(v.created_at)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }

    async fn list_template_versions(&self, template_id: &str) -> Result<Vec<EmailTemplateVersion>, AppError> {
        sqlx::query_as::<_, EmailTemplateVersion>(
            "SELECT * FROM email_template_versions WHERE template_id = $1 ORDER BY created_at DESC"
        )
            .bind(template_id)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_template_version(&self, version_id: &str) -> Result<Option<EmailTemplateVersion>, AppError> {
        sqlx::query_as::<_, EmailTemplateVersion>(
            "SELECT * FROM email_template_versions WHERE id = $1"
        )
            .bind(version_id)
            .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn create_rule(&self, r: &NotificationRule) -> Result<NotificationRule, AppError> {
        sqlx::query_as::<_, NotificationRule>(
            "INSERT INTO notification_rules (id, tenant_id, event_id, trigger_type, template_id, is_active, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"
        )
            .bind(&r.id).bind(&r.tenant_id).bind(&r.event_id)
            .bind(&r.trigger_type).bind(&r.template_id).bind(r.is_active).bind(r.created_at)
            .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_rules_by_event(&self, event_id: &str) -> Result<Vec<NotificationRule>, AppError> {
        sqlx::query_as::<_, NotificationRule>(
            "SELECT * FROM notification_rules WHERE event_id = $1 AND is_active = true"
        )
            .bind(event_id)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_rules_by_trigger(&self, tenant_id: &str, event_id: Option<&str>, trigger: &str) -> Result<Vec<NotificationRule>, AppError> {
        sqlx::query_as::<_, NotificationRule>(
            "SELECT * FROM notification_rules
             WHERE tenant_id = $1 AND trigger_type = $2 AND is_active = true
             AND (event_id = $3 OR event_id IS NULL)
             ORDER BY event_id NULLS LAST"
        )
            .bind(tenant_id).bind(trigger).bind(event_id)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn delete_rule(&self, id: &str) -> Result<(), AppError> {
        let res = sqlx::query("DELETE FROM notification_rules WHERE id = $1")
            .bind(id)
            .execute(&self.pool).await.map_err(AppError::Database)?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Rule not found".into()));
        }
        Ok(())
    }

    async fn log_mail(&self, log: &MailLog) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO mail_logs (id, job_id, recipient, template_id, context_hash, sent_at, status)
             VALUES ($1, $2, $3, $4, $5, $6, $7)"
        )
            .bind(&log.id).bind(&log.job_id).bind(&log.recipient)
            .bind(&log.template_id).bind(&log.context_hash).bind(log.sent_at).bind(&log.status)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }

    async fn has_mail_been_sent(&self, recipient: &str, template_id: &str, context_hash: &str) -> Result<bool, AppError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mail_logs WHERE recipient = $1 AND template_id = $2 AND context_hash = $3 AND status = 'SENT'"
        )
            .bind(recipient).bind(template_id).bind(context_hash)
            .fetch_one(&self.pool).await.map_err(AppError::Database)?;

        Ok(count > 0)
    }

    async fn list_logs(&self, tenant_id: &str, recipient: Option<&str>) -> Result<Vec<MailLog>, AppError> {
        let query = r#"
            SELECT ml.*
            FROM mail_logs ml
            JOIN jobs j ON ml.job_id = j.id
            WHERE j.payload->>'tenant_id' = $1
            AND ($2::text IS NULL OR ml.recipient = $2)
            ORDER BY ml.sent_at DESC
        "#;

        sqlx::query_as::<_, MailLog>(query)
            .bind(tenant_id)
            .bind(recipient)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }
}