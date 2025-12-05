use crate::domain::{models::{booking::Booking, job::Job}, ports::BookingRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use chrono::{DateTime, Utc};

pub struct PostgresBookingRepo {
    pool: PgPool,
}

impl PostgresBookingRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BookingRepository for PostgresBookingRepo {

    async fn create(&self, booking: &Booking) -> Result<Booking, AppError> {
        self.create_with_token(booking, None, vec![]).await
    }
    async fn create_with_token(&self, booking: &Booking, token_to_burn: Option<String>, jobs: Vec<Job>) -> Result<Booking, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::Database)?;
        if let Some(token) = token_to_burn {
            let result = sqlx::query("UPDATE invitees SET status = 'USED' WHERE token = $1 AND status = 'ACTIVE'").bind(token).execute(&mut *tx).await.map_err(AppError::Database)?;
            if result.rows_affected() == 0 { return Err(AppError::Conflict("Token invalid or already used".to_string())); }
        }
        let created = sqlx::query_as::<_, Booking>("INSERT INTO bookings (id, tenant_id, event_id, invitee_id, start_time, end_time, customer_name, customer_email, customer_note, location, label_id, status, management_token, token, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15) RETURNING *").bind(&booking.id).bind(&booking.tenant_id).bind(&booking.event_id).bind(&booking.invitee_id).bind(booking.start_time).bind(booking.end_time).bind(&booking.customer_name).bind(&booking.customer_email).bind(&booking.customer_note).bind(&booking.location).bind(&booking.label_id).bind(&booking.status).bind(&booking.management_token).bind(&booking.token).bind(booking.created_at).fetch_one(&mut *tx).await.map_err(AppError::Database)?;
        for job in jobs {
            sqlx::query("INSERT INTO jobs (id, job_type, payload, execute_at, status, error_message, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7)").bind(&job.id).bind(&job.job_type).bind(&job.payload).bind(job.execute_at).bind(&job.status).bind(&job.error_message).bind(job.created_at).execute(&mut *tx).await.map_err(AppError::Database)?;
        }
        tx.commit().await.map_err(AppError::Database)?;
        Ok(created)
    }
    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE tenant_id = $1 AND id = $2").bind(tenant_id).bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
    }
    async fn find_by_token(&self, token: &str) -> Result<Option<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE management_token = $1").bind(token).fetch_optional(&self.pool).await.map_err(AppError::Database)
    }
    async fn list_by_event(&self, tenant_id: &str, event_id: &str) -> Result<Vec<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE tenant_id = $1 AND event_id = $2").bind(tenant_id).bind(event_id).fetch_all(&self.pool).await.map_err(AppError::Database)
    }
    async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE tenant_id = $1 ORDER BY start_time ASC").bind(tenant_id).fetch_all(&self.pool).await.map_err(AppError::Database)
    }
    async fn list_by_range(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE event_id = $1 AND start_time < $2 AND end_time > $3 AND status != 'CANCELLED'").bind(event_id).bind(end).bind(start).fetch_all(&self.pool).await.map_err(AppError::Database)
    }
    async fn update(&self, booking: &Booking) -> Result<Booking, AppError> {
        sqlx::query_as::<_, Booking>("UPDATE bookings SET start_time=$1, end_time=$2, customer_name=$3, customer_email=$4, location=$5, label_id=$6, token=$7 WHERE id=$8 AND tenant_id=$9 RETURNING *").bind(booking.start_time).bind(booking.end_time).bind(&booking.customer_name).bind(&booking.customer_email).bind(&booking.location).bind(&booking.label_id).bind(&booking.token).bind(&booking.id).bind(&booking.tenant_id).fetch_one(&self.pool).await.map_err(AppError::Database)
    }
    async fn cancel(&self, booking: &Booking) -> Result<Booking, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::Database)?;
        let cancelled = sqlx::query_as::<_, Booking>("UPDATE bookings SET status = 'CANCELLED' WHERE id = $1 RETURNING *").bind(&booking.id).fetch_one(&mut *tx).await.map_err(AppError::Database)?;
        if let Some(invitee_id) = &booking.invitee_id { sqlx::query("UPDATE invitees SET status = 'ACTIVE' WHERE id = $1").bind(invitee_id).execute(&mut *tx).await.map_err(AppError::Database)?; }
        tx.commit().await.map_err(AppError::Database)?;
        Ok(cancelled)
    }
    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM bookings WHERE id = $1 AND tenant_id = $2").bind(id).bind(tenant_id).execute(&self.pool).await.map_err(AppError::Database)?;
        if result.rows_affected() == 0 { return Err(AppError::NotFound("Booking not found".into())); }
        Ok(())
    }
    async fn count_overlap(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64, AppError> {
        let result = sqlx::query("SELECT COUNT(*) as count FROM bookings WHERE event_id = $1 AND start_time < $2 AND end_time > $3 AND status != 'CANCELLED'").bind(event_id).bind(end).bind(start).fetch_one(&self.pool).await.map_err(AppError::Database)?;
        Ok(result.get::<i64, _>("count"))
    }

    async fn find_future_active_bookings(&self, event_id: &str) -> Result<Vec<Booking>, AppError> {
        sqlx::query_as::<_, Booking>(
            "SELECT * FROM bookings WHERE event_id = $1 AND start_time > $2 AND status != 'CANCELLED'"
        )
            .bind(event_id)
            .bind(Utc::now())
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }
}