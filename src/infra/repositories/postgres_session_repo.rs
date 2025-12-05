use crate::domain::{models::session::EventSession, ports::SessionRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;
use chrono::{DateTime, Utc};

pub struct PostgresSessionRepo {
    pool: PgPool,
}

impl PostgresSessionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepo {
    async fn create(&self, session: &EventSession) -> Result<EventSession, AppError> {
        sqlx::query_as::<_, EventSession>(
            r#"INSERT INTO event_sessions (id, event_id, start_time, end_time, max_participants, location, host_name, created_at)
               VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
               RETURNING *"#
        )
            .bind(&session.id)
            .bind(&session.event_id)
            .bind(session.start_time)
            .bind(session.end_time)
            .bind(session.max_participants)
            .bind(&session.location)
            .bind(&session.host_name)
            .bind(session.created_at)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<EventSession>, AppError> {
        sqlx::query_as::<_, EventSession>(
            "SELECT * FROM event_sessions WHERE id = $1"
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list_by_event(&self, event_id: &str) -> Result<Vec<EventSession>, AppError> {
        sqlx::query_as::<_, EventSession>(
            "SELECT * FROM event_sessions WHERE event_id = $1 ORDER BY start_time ASC"
        )
            .bind(event_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list_by_range(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<EventSession>, AppError> {
        sqlx::query_as::<_, EventSession>(
            "SELECT * FROM event_sessions WHERE event_id = $1 AND start_time < $2 AND end_time > $3 ORDER BY start_time ASC"
        )
            .bind(event_id)
            .bind(end)
            .bind(start)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update(&self, session: &EventSession) -> Result<EventSession, AppError> {
        sqlx::query_as::<_, EventSession>(
            r#"UPDATE event_sessions SET max_participants=$1, location=$2, host_name=$3 WHERE id=$4 RETURNING *"#
        )
            .bind(session.max_participants)
            .bind(&session.location)
            .bind(&session.host_name)
            .bind(&session.id)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn delete(&self, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM event_sessions WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Session not found".into()));
        }
        Ok(())
    }

    async fn find_overlap(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<EventSession>, AppError> {
        sqlx::query_as::<_, EventSession>(
            "SELECT * FROM event_sessions WHERE event_id = $1 AND start_time < $2 AND end_time > $3"
        )
            .bind(event_id)
            .bind(end)
            .bind(start)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }
}