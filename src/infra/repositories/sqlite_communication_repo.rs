use crate::domain::models::communication::{EmailTemplate, NotificationRule, MailLog, EmailTemplateVersion};
use crate::domain::ports::CommunicationRepository;
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::SqlitePool;

pub struct SqliteCommunicationRepo {
    pool: SqlitePool,
}

impl SqliteCommunicationRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }
}

#[async_trait]
impl CommunicationRepository for SqliteCommunicationRepo {
    async fn create_template(&self, t: &EmailTemplate) -> Result<EmailTemplate, AppError> {
        sqlx::query_as::<_, EmailTemplate>(
            "INSERT INTO email_templates (id, tenant_id, event_id, name, subject_template, body_template, template_type, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING *"
        )
            .bind(&t.id).bind(&t.tenant_id).bind(&t.event_id).bind(&t.name)
            .bind(&t.subject_template).bind(&t.body_template).bind(&t.template_type)
            .bind(t.created_at).bind(t.updated_at)
            .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_template(&self, id: &str) -> Result<Option<EmailTemplate>, AppError> {
        sqlx::query_as::<_, EmailTemplate>("SELECT * FROM email_templates WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn list_templates(&self, tenant_id: &str, event_id: Option<&str>) -> Result<Vec<EmailTemplate>, AppError> {
        let query = if let Some(_eid) = event_id {
            "SELECT * FROM email_templates WHERE tenant_id = ? AND event_id = ? ORDER BY name ASC"
        } else {
            "SELECT * FROM email_templates WHERE tenant_id = ? ORDER BY name ASC"
        };

        let mut q = sqlx::query_as::<_, EmailTemplate>(query).bind(tenant_id);
        if let Some(eid) = event_id {
            q = q.bind(eid);
        }
        q.fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn update_template(&self, t: &EmailTemplate) -> Result<EmailTemplate, AppError> {
        sqlx::query_as::<_, EmailTemplate>(
            "UPDATE email_templates SET name=?, subject_template=?, body_template=?, template_type=?, updated_at=? WHERE id=? RETURNING *"
        )
            .bind(&t.name).bind(&t.subject_template).bind(&t.body_template)
            .bind(&t.template_type).bind(t.updated_at).bind(&t.id)
            .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn delete_template(&self, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM email_templates WHERE id = ?")
            .bind(id)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }

    async fn create_template_version(&self, v: &EmailTemplateVersion) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO email_template_versions (id, template_id, subject_template, body_template, created_at)
             VALUES (?, ?, ?, ?, ?)"
        )
            .bind(&v.id).bind(&v.template_id).bind(&v.subject_template)
            .bind(&v.body_template).bind(v.created_at)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }

    async fn list_template_versions(&self, template_id: &str) -> Result<Vec<EmailTemplateVersion>, AppError> {
        sqlx::query_as::<_, EmailTemplateVersion>(
            "SELECT * FROM email_template_versions WHERE template_id = ? ORDER BY created_at DESC"
        )
            .bind(template_id)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_template_version(&self, version_id: &str) -> Result<Option<EmailTemplateVersion>, AppError> {
        sqlx::query_as::<_, EmailTemplateVersion>(
            "SELECT * FROM email_template_versions WHERE id = ?"
        )
            .bind(version_id)
            .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn create_rule(&self, r: &NotificationRule) -> Result<NotificationRule, AppError> {
        sqlx::query_as::<_, NotificationRule>(
            "INSERT INTO notification_rules (id, tenant_id, event_id, trigger_type, template_id, is_active, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *"
        )
            .bind(&r.id).bind(&r.tenant_id).bind(&r.event_id)
            .bind(&r.trigger_type).bind(&r.template_id).bind(r.is_active).bind(r.created_at)
            .fetch_one(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_rules_by_event(&self, event_id: &str) -> Result<Vec<NotificationRule>, AppError> {
        sqlx::query_as::<_, NotificationRule>(
            "SELECT * FROM notification_rules WHERE event_id = ? AND is_active = 1"
        )
            .bind(event_id)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn get_rules_by_trigger(&self, tenant_id: &str, event_id: Option<&str>, trigger: &str) -> Result<Vec<NotificationRule>, AppError> {
        sqlx::query_as::<_, NotificationRule>(
            "SELECT * FROM notification_rules
             WHERE tenant_id = ? AND trigger_type = ? AND is_active = 1
             AND (event_id = ? OR event_id IS NULL)
             ORDER BY event_id NULLS LAST"
        )
            .bind(tenant_id).bind(trigger).bind(event_id)
            .fetch_all(&self.pool).await.map_err(AppError::Database)
    }

    async fn delete_rule(&self, id: &str) -> Result<(), AppError> {
        let res = sqlx::query("DELETE FROM notification_rules WHERE id = ?")
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
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
            .bind(&log.id).bind(&log.job_id).bind(&log.recipient)
            .bind(&log.template_id).bind(&log.context_hash).bind(log.sent_at).bind(&log.status)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }

    async fn has_mail_been_sent(&self, recipient: &str, template_id: &str, context_hash: &str) -> Result<bool, AppError> {
        let count: i32 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM mail_logs WHERE recipient = ? AND template_id = ? AND context_hash = ? AND status = 'SENT'"
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
            WHERE json_extract(j.payload, '$.tenant_id') = ?
            AND (? IS NULL OR ml.recipient = ?)
            ORDER BY ml.sent_at DESC
        "#;

        sqlx::query_as::<_, MailLog>(query)
            .bind(tenant_id)
            .bind(recipient)
            .bind(recipient)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }
}