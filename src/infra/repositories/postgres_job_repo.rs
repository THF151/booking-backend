use crate::domain::{models::job::Job, models::booking::Booking, ports::JobRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;
use chrono::Utc;

pub struct PostgresJobRepo {
    pool: PgPool,
}

impl PostgresJobRepo {
    pub fn new(pool: PgPool) -> Self { Self { pool } }
}

#[async_trait]
impl JobRepository for PostgresJobRepo {
    async fn create(&self, job: &Job) -> Result<Job, AppError> {
        sqlx::query_as::<_, Job>(
            "INSERT INTO jobs (id, job_type, payload, execute_at, status, error_message, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"
        )
            .bind(&job.id)
            .bind(&job.job_type)
            .bind(&job.payload)
            .bind(job.execute_at)
            .bind(&job.status)
            .bind(&job.error_message)
            .bind(job.created_at)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_pending(&self, limit: i32) -> Result<Vec<Job>, AppError> {
        let now = Utc::now();
        let jobs = sqlx::query_as::<_, Job>(
            r#"
            UPDATE jobs
            SET status = 'PROCESSING'
            WHERE id IN (
                SELECT id
                FROM jobs
                WHERE status = 'PENDING' AND execute_at <= $1
                ORDER BY execute_at ASC
                LIMIT $2
                FOR UPDATE SKIP LOCKED
            )
            RETURNING *
            "#
        )
            .bind(now)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)?;

        Ok(jobs)
    }

    async fn list_jobs(&self, tenant_id: &str) -> Result<Vec<Job>, AppError> {
        sqlx::query_as::<_, Job>(
            "SELECT * FROM jobs WHERE payload->>'tenant_id' = $1 ORDER BY created_at DESC LIMIT 100"
        )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update_status(&self, id: &str, status: &str, error_message: Option<String>) -> Result<(), AppError> {
        sqlx::query("UPDATE jobs SET status = $1, error_message = $2 WHERE id = $3")
            .bind(status)
            .bind(error_message)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
        Ok(())
    }

    async fn cancel_jobs_for_booking(&self, booking_id: &str) -> Result<(), AppError> {
        sqlx::query("UPDATE jobs SET status = 'CANCELLED' WHERE payload->>'booking_id' = $1 AND status = 'PENDING'")
            .bind(booking_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
        Ok(())
    }

    async fn delete_jobs_by_type_and_event(&self, event_id: &str, job_type: &str) -> Result<(), AppError> {
        let query = r#"
            DELETE FROM jobs
            WHERE status = 'PENDING'
            AND job_type = $1
            AND payload->>'booking_id' IN (
                SELECT id FROM bookings WHERE event_id = $2
            )
        "#;
        sqlx::query(query)
            .bind(job_type)
            .bind(event_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;
        Ok(())
    }

    async fn find_future_bookings_for_event(&self, _event_id: &str) -> Result<Vec<Booking>, AppError> {
        Ok(vec![])
    }
}