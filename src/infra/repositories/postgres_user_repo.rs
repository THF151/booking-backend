use crate::domain::{models::user::User, ports::UserRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;
use tracing::error;

pub struct PostgresUserRepo {
    pool: PgPool,
}

impl PostgresUserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepo {
    async fn create(&self, user: &User) -> Result<User, AppError> {
        sqlx::query_as::<_, User>(
            "INSERT INTO users (id, tenant_id, username, password_hash, role, created_at) VALUES ($1, $2, $3, $4, $5, $6) RETURNING id, tenant_id, username, password_hash, role, created_at",
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
            "SELECT id, tenant_id, username, password_hash, role, created_at FROM users WHERE tenant_id = $1 AND username = $2",
        )
            .bind(tenant_id)
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<User>, AppError> {
        sqlx::query_as::<_, User>(
            "SELECT id, tenant_id, username, password_hash, role, created_at FROM users WHERE tenant_id = $1 AND id = $2",
        )
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list_by_tenant(&self, tenant_id: &str) -> Result<Vec<User>, AppError> {
        sqlx::query_as::<_, User>(
            "SELECT id, tenant_id, username, password_hash, role, created_at FROM users WHERE tenant_id = $1 ORDER BY username ASC"
        )
            .bind(tenant_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError> {
        sqlx::query("DELETE FROM users WHERE tenant_id = $1 AND id = $2")
            .bind(tenant_id)
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                error!("Postgres User Deletion Failed: {:?}", e);
                AppError::Database(e)
            })?;
        Ok(())
    }
}