use crate::domain::{models::booking::BookingLabel, ports::BookingLabelRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::SqlitePool;

pub struct SqliteLabelRepo {
    pool: SqlitePool,
}

impl SqliteLabelRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BookingLabelRepository for SqliteLabelRepo {
    async fn create(&self, label: &BookingLabel) -> Result<BookingLabel, AppError> {
        sqlx::query_as::<_, BookingLabel>(
            "INSERT INTO booking_labels (id, tenant_id, name, color, payout, created_at) VALUES (?, ?, ?, ?, ?, ?) RETURNING *"
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
        sqlx::query_as::<_, BookingLabel>("SELECT * FROM booking_labels WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn update(&self, label: &BookingLabel) -> Result<BookingLabel, AppError> {
        sqlx::query_as::<_, BookingLabel>(
            "UPDATE booking_labels SET name=?, color=?, payout=? WHERE id=? RETURNING *"
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
            "SELECT * FROM booking_labels WHERE tenant_id = ? ORDER BY created_at ASC"
        )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM booking_labels WHERE id = ? AND tenant_id = ?")
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