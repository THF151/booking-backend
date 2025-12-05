use crate::domain::{models::event_override::EventOverride, ports::EventOverrideRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;
use chrono::NaiveDate;

pub struct PostgresEventOverrideRepo {
    pool: PgPool,
}

impl PostgresEventOverrideRepo {
    pub fn new(pool: PgPool) -> Self { Self { pool } }
}

#[async_trait]
impl EventOverrideRepository for PostgresEventOverrideRepo {
    async fn upsert(&self, entity: &EventOverride) -> Result<EventOverride, AppError> {
        sqlx::query_as::<_, EventOverride>(
            r#"INSERT INTO event_overrides (id, event_id, date, is_unavailable, override_config_json, override_max_participants, location, host_name, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
               ON CONFLICT(event_id, date) DO UPDATE SET
               is_unavailable=excluded.is_unavailable,
               override_config_json=excluded.override_config_json,
               override_max_participants=excluded.override_max_participants,
               location=excluded.location,
               host_name=excluded.host_name
               RETURNING *"#
        )
            .bind(&entity.id)
            .bind(&entity.event_id)
            .bind(entity.date)
            .bind(entity.is_unavailable)
            .bind(&entity.override_config_json)
            .bind(entity.override_max_participants)
            .bind(&entity.location)
            .bind(&entity.host_name)
            .bind(entity.created_at)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_date(&self, event_id: &str, date: NaiveDate) -> Result<Option<EventOverride>, AppError> {
        sqlx::query_as::<_, EventOverride>(
            "SELECT * FROM event_overrides WHERE event_id = $1 AND date = $2"
        )
            .bind(event_id)
            .bind(date)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list_by_range(&self, event_id: &str, start: NaiveDate, end: NaiveDate) -> Result<Vec<EventOverride>, AppError> {
        sqlx::query_as::<_, EventOverride>(
            "SELECT * FROM event_overrides WHERE event_id = $1 AND date >= $2 AND date <= $3"
        )
            .bind(event_id)
            .bind(start)
            .bind(end)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn delete(&self, event_id: &str, date: NaiveDate) -> Result<(), AppError> {
        let res = sqlx::query("DELETE FROM event_overrides WHERE event_id = $1 AND date = $2")
            .bind(event_id)
            .bind(date)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if res.rows_affected() == 0 {
            return Err(AppError::NotFound("Override not found".into()));
        }
        Ok(())
    }
}