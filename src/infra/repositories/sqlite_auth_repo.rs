use crate::domain::{models::auth::RefreshTokenRecord, ports::AuthRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::SqlitePool;
use uuid::Uuid;

pub struct SqliteAuthRepo { pool: SqlitePool }
impl SqliteAuthRepo { pub fn new(pool: SqlitePool) -> Self { Self { pool } } }

#[async_trait]
impl AuthRepository for SqliteAuthRepo {
    async fn create_refresh_token(&self, record: &RefreshTokenRecord) -> Result<(), AppError> {
        sqlx::query(
            "INSERT INTO refresh_tokens (token_hash, user_id, tenant_id, family_id, generation_id, expires_at, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
            .bind(&record.token_hash)
            .bind(&record.user_id)
            .bind(&record.tenant_id)
            .bind(record.family_id)
            .bind(record.generation_id)
            .bind(record.expires_at)
            .bind(record.created_at)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }

    async fn find_refresh_token(&self, token_hash: &str) -> Result<Option<RefreshTokenRecord>, AppError> {
        sqlx::query_as::<_, RefreshTokenRecord>(
            "SELECT token_hash, user_id, tenant_id, family_id, generation_id, expires_at, created_at
             FROM refresh_tokens WHERE token_hash = ?"
        )
            .bind(token_hash)
            .fetch_optional(&self.pool).await.map_err(AppError::Database)
    }

    async fn delete_refresh_token(&self, token_hash: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM refresh_tokens WHERE token_hash = ?")
            .bind(token_hash)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }

    async fn delete_refresh_family(&self, family_id: Uuid) -> Result<(), AppError> {
        sqlx::query("DELETE FROM refresh_tokens WHERE family_id = ?")
            .bind(family_id)
            .execute(&self.pool).await.map_err(AppError::Database)?;
        Ok(())
    }
}