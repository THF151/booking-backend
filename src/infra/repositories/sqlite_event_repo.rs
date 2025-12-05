use crate::domain::{models::event::Event, ports::EventRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::SqlitePool;

pub struct SqliteEventRepo {
    pool: SqlitePool,
}

impl SqliteEventRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EventRepository for SqliteEventRepo {
    async fn create(&self, event: &Event) -> Result<Event, AppError> {
        sqlx::query_as::<_, Event>(
            r#"INSERT INTO events (
                id, tenant_id, slug, title_en, title_de, desc_en, desc_de,
                location, payout, host_name, timezone, min_notice_general, min_notice_first,
                active_start, active_end, duration_min, interval_min, max_participants,
                image_url, config_json, access_mode, schedule_type, allow_customer_cancel, allow_customer_reschedule, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            RETURNING *"#
        )
            .bind(&event.id)
            .bind(&event.tenant_id)
            .bind(&event.slug)
            .bind(&event.title_en)
            .bind(&event.title_de)
            .bind(&event.desc_en)
            .bind(&event.desc_de)
            .bind(&event.location)
            .bind(&event.payout)
            .bind(&event.host_name)
            .bind(&event.timezone)
            .bind(event.min_notice_general)
            .bind(event.min_notice_first)
            .bind(event.active_start)
            .bind(event.active_end)
            .bind(event.duration_min)
            .bind(event.interval_min)
            .bind(event.max_participants)
            .bind(&event.image_url)
            .bind(&event.config_json)
            .bind(&event.access_mode)
            .bind(&event.schedule_type)
            .bind(event.allow_customer_cancel)
            .bind(event.allow_customer_reschedule)
            .bind(event.created_at)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_slug(&self, tenant_id: &str, slug: &str) -> Result<Option<Event>, AppError> {
        sqlx::query_as::<_, Event>(
            "SELECT * FROM events WHERE tenant_id = ? AND slug = ?",
        )
            .bind(tenant_id)
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<Event>, AppError> {
        sqlx::query_as::<_, Event>(
            "SELECT * FROM events WHERE tenant_id = ? AND id = ?",
        )
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<Event>, AppError> {
        sqlx::query_as::<_, Event>(
            "SELECT * FROM events WHERE tenant_id = ?",
        )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update(&self, event: &Event) -> Result<Event, AppError> {
        sqlx::query_as::<_, Event>(
            r#"UPDATE events SET
                slug=?, title_en=?, title_de=?, desc_en=?, desc_de=?,
                location=?, payout=?, host_name=?, timezone=?,
                min_notice_general=?, min_notice_first=?,
                active_start=?, active_end=?, duration_min=?, interval_min=?,
                max_participants=?, image_url=?, config_json=?, access_mode=?, schedule_type=?,
                allow_customer_cancel=?, allow_customer_reschedule=?
               WHERE id=? AND tenant_id=? RETURNING *"#
        )
            .bind(&event.slug)
            .bind(&event.title_en)
            .bind(&event.title_de)
            .bind(&event.desc_en)
            .bind(&event.desc_de)
            .bind(&event.location)
            .bind(&event.payout)
            .bind(&event.host_name)
            .bind(&event.timezone)
            .bind(event.min_notice_general)
            .bind(event.min_notice_first)
            .bind(event.active_start)
            .bind(event.active_end)
            .bind(event.duration_min)
            .bind(event.interval_min)
            .bind(event.max_participants)
            .bind(&event.image_url)
            .bind(&event.config_json)
            .bind(&event.access_mode)
            .bind(&event.schedule_type)
            .bind(event.allow_customer_cancel)
            .bind(event.allow_customer_reschedule)
            .bind(&event.id)
            .bind(&event.tenant_id)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM events WHERE id = ? AND tenant_id = ?")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Event not found".into()));
        }
        Ok(())
    }
}
