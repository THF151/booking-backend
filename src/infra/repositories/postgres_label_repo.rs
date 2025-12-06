use crate::domain::{models::booking::BookingLabel, ports::BookingLabelRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;

pub struct PostgresLabelRepo {
    pool: PgPool,
}

impl PostgresLabelRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BookingLabelRepository for PostgresLabelRepo {
    async fn create(&self, label: &BookingLabel) -> Result<BookingLabel, AppError> {
        sqlx::query_as::<_, BookingLabel>(
            "INSERT INTO booking_labels (id, tenant_id, name, color, payout, created_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING *"
        )
            .bind(&label.id)
            .bind(&label.tenant_id)
            .bind(&label.name)
            .bind(&label.color)
            .bind(label.payout)
            .bind(label.created_at)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<BookingLabel>, AppError> {
        sqlx::query_as::<_, BookingLabel>("SELECT * FROM booking_labels WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn update(&self, label: &BookingLabel) -> Result<BookingLabel, AppError> {
        sqlx::query_as::<_, BookingLabel>(
            "UPDATE booking_labels SET name=$1, color=$2, payout=$3 WHERE id=$4 RETURNING *"
        )
            .bind(&label.name)
            .bind(&label.color)
            .bind(label.payout)
            .bind(&label.id)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list(&self, tenant_id: &str) -> Result<Vec<BookingLabel>, AppError> {
        sqlx::query_as::<_, BookingLabel>(
            "SELECT * FROM booking_labels WHERE tenant_id = $1 ORDER BY created_at ASC"
        )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM booking_labels WHERE id = $1 AND tenant_id = $2")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Label not found".into()));
        }
        Ok(())
    }
}