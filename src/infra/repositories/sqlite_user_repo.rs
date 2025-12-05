use crate::domain::{models::user::User, ports::UserRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::error;

pub struct SqliteUserRepo {
    pool: SqlitePool,
}

impl SqliteUserRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for SqliteUserRepo {
    async fn create(&self, user: &User) -> Result<User, AppError> {
        sqlx::query_as::<_, User>(
            "INSERT INTO users (id, tenant_id, username, password_hash, role, created_at) VALUES (?, ?, ?, ?, ?, ?) RETURNING id, tenant_id, username, password_hash, role, created_at",
        )
            .bind(&user.id)
            .bind(&user.tenant_id)
            .bind(&user.username)
            .bind(&user.password_hash)
            .bind(&user.role)
            .bind(user.created_at)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_username(&self, tenant_id: &str, username: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>(
            "SELECT id, tenant_id, username, password_hash, role, created_at FROM users WHERE tenant_id = ? AND username = ?",
        )
            .bind(tenant_id)
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>(
            "SELECT id, tenant_id, username, password_hash, role, created_at FROM users WHERE tenant_id = ? AND id = ?",
        )
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<User>, AppError> {
        sqlx::query_as::<_, User>(
            "SELECT id, tenant_id, username, password_hash, role, created_at FROM users WHERE tenant_id = ? ORDER BY username ASC"
        )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError> {

        let _result = sqlx::query("DELETE FROM users WHERE tenant_id = ? AND id = ?")
            .bind(tenant_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("SQLite User Deletion Failed: {:?}", e);
                AppError::Database(e)
            })?;
        Ok(())
    }
}