use crate::domain::{models::{booking::Booking, job::Job}, ports::BookingRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::{SqlitePool, Row};
use chrono::{DateTime, Utc};

pub struct SqliteBookingRepo {
    pool: SqlitePool,
}

impl SqliteBookingRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BookingRepository for SqliteBookingRepo {
    async fn create(&self, booking: &Booking) -> Result<Booking, AppError> {
        self.create_with_token(booking, None, vec![]).await
    }
    async fn create_with_token(&self, booking: &Booking, token_to_burn: Option<String>, jobs: Vec<Job>) -> Result<Booking, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::Database)?;
        if let Some(token) = token_to_burn {
            let result = sqlx::query("UPDATE invitees SET status = 'USED' WHERE token = ? AND status = 'ACTIVE'").bind(token).execute(&mut *tx).await.map_err(AppError::Database)?;
            if result.rows_affected() == 0 { return Err(AppError::Conflict("Token invalid or already used".to_string())); }
        }
        let created = sqlx::query_as::<_, Booking>(
            "INSERT INTO bookings (id, tenant_id, event_id, invitee_id, start_time, end_time, customer_name, customer_email, customer_note, location, label_id, status, management_token, token, payout, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             RETURNING *"
        )
            .bind(&booking.id).bind(&booking.tenant_id).bind(&booking.event_id).bind(&booking.invitee_id)
            .bind(booking.start_time).bind(booking.end_time).bind(&booking.customer_name).bind(&booking.customer_email)
            .bind(&booking.customer_note).bind(&booking.location).bind(&booking.label_id).bind(&booking.status)
            .bind(&booking.management_token).bind(&booking.token).bind(booking.payout).bind(booking.created_at)
            .fetch_one(&mut *tx).await.map_err(AppError::Database)?;

        for job in jobs {
            sqlx::query("INSERT INTO jobs (id, job_type, payload, execute_at, status, error_message, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)").bind(&job.id).bind(&job.job_type).bind(&job.payload).bind(job.execute_at).bind(&job.status).bind(&job.error_message).bind(job.created_at).execute(&mut *tx).await.map_err(AppError::Database)?;
        }
        tx.commit().await.map_err(AppError::Database)?;
        Ok(created)
    }
    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE tenant_id = ? AND id = ?").bind(tenant_id).bind(id).fetch_optional(&self.pool).await.map_err(AppError::Database)
    }
    async fn find_by_token(&self, token: &str) -> Result<Option<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE management_token = ?").bind(token).fetch_optional(&self.pool).await.map_err(AppError::Database)
    }
    async fn list_by_event(&self, tenant_id: &str, event_id: &str) -> Result<Vec<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE tenant_id = ? AND event_id = ?").bind(tenant_id).bind(event_id).fetch_all(&self.pool).await.map_err(AppError::Database)
    }
    async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE tenant_id = ? ORDER BY start_time ASC").bind(tenant_id).fetch_all(&self.pool).await.map_err(AppError::Database)
    }
    async fn list_by_range(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Vec<Booking>, AppError> {
        sqlx::query_as::<_, Booking>("SELECT * FROM bookings WHERE event_id = ? AND start_time < ? AND end_time > ? AND status != 'CANCELLED'").bind(event_id).bind(end).bind(start).fetch_all(&self.pool).await.map_err(AppError::Database)
    }
    async fn update(&self, booking: &Booking) -> Result<Booking, AppError> {
        sqlx::query_as::<_, Booking>(
            "UPDATE bookings SET start_time=?, end_time=?, customer_name=?, customer_email=?, location=?, label_id=?, token=?, payout=?
             WHERE id=? AND tenant_id=?
             RETURNING *"
        )
            .bind(booking.start_time).bind(booking.end_time).bind(&booking.customer_name).bind(&booking.customer_email)
            .bind(&booking.location).bind(&booking.label_id).bind(&booking.token).bind(booking.payout)
            .bind(&booking.id).bind(&booking.tenant_id)
            .fetch_one(&self.pool).await.map_err(AppError::Database)
    }
    async fn cancel(&self, booking: &Booking) -> Result<Booking, AppError> {
        let mut tx = self.pool.begin().await.map_err(AppError::Database)?;
        let cancelled = sqlx::query_as::<_, Booking>("UPDATE bookings SET status = 'CANCELLED' WHERE id = ? RETURNING *").bind(&booking.id).fetch_one(&mut *tx).await.map_err(AppError::Database)?;
        if let Some(invitee_id) = &booking.invitee_id { sqlx::query("UPDATE invitees SET status = 'ACTIVE' WHERE id = ?").bind(invitee_id).execute(&mut *tx).await.map_err(AppError::Database)?; }
        tx.commit().await.map_err(AppError::Database)?;
        Ok(cancelled)
    }
    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM bookings WHERE id = ? AND tenant_id = ?").bind(id).bind(tenant_id).execute(&self.pool).await.map_err(AppError::Database)?;
        if result.rows_affected() == 0 { return Err(AppError::NotFound("Booking not found".into())); }
        Ok(())
    }
    async fn count_overlap(&self, event_id: &str, start: DateTime<Utc>, end: DateTime<Utc>) -> Result<i64, AppError> {
        let result = sqlx::query("SELECT COUNT(*) as count FROM bookings WHERE event_id = ? AND start_time < ? AND end_time > ? AND status != 'CANCELLED'").bind(event_id).bind(end).bind(start).fetch_one(&self.pool).await.map_err(AppError::Database)?;
        Ok(result.get::<i64, _>("count") as i64)
    }

    async fn find_future_active_bookings(&self, event_id: &str) -> Result<Vec<Booking>, AppError> {
        sqlx::query_as::<_, Booking>(
            "SELECT * FROM bookings WHERE event_id = ? AND start_time > ? AND status != 'CANCELLED'"
        )
            .bind(event_id)
            .bind(Utc::now())
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }
}