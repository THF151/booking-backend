use crate::domain::{models::invitee::Invitee, ports::InviteeRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::SqlitePool;

pub struct SqliteInviteeRepo {
    pool: SqlitePool,
}

impl SqliteInviteeRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InviteeRepository for SqliteInviteeRepo {
    async fn create(&self, invitee: &Invitee) -> Result<Invitee, AppError> {
        sqlx::query_as::<_, Invitee>(
            "INSERT INTO invitees (id, tenant_id, event_id, token, email, status, created_at) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id, tenant_id, event_id, token, email, status, created_at",
        )
            .bind(&invitee.id)
            .bind(&invitee.tenant_id)
            .bind(&invitee.event_id)
            .bind(&invitee.token)
            .bind(&invitee.email)
            .bind(&invitee.status)
            .bind(invitee.created_at)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_token(&self, token: &str) -> Result<Option<Invitee>, AppError> {
        sqlx::query_as::<_, Invitee>(
            "SELECT id, tenant_id, event_id, token, email, status, created_at FROM invitees WHERE token = ?",
        )
            .bind(token)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<Invitee>, AppError> {
        sqlx::query_as::<_, Invitee>(
            "SELECT id, tenant_id, event_id, token, email, status, created_at FROM invitees WHERE tenant_id = ? AND id = ?",
        )
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list_by_event(&self, tenant_id: &str, event_id: &str) -> Result<Vec<Invitee>, AppError> {
        sqlx::query_as::<_, Invitee>(
            "SELECT id, tenant_id, event_id, token, email, status, created_at FROM invitees WHERE tenant_id = ? AND event_id = ?",
        )
            .bind(tenant_id)
            .bind(event_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update(&self, invitee: &Invitee) -> Result<Invitee, AppError> {
        sqlx::query_as::<_, Invitee>(
            "UPDATE invitees SET status=?, email=? WHERE id=? AND tenant_id=? RETURNING *"
        )
            .bind(&invitee.status)
            .bind(&invitee.email)
            .bind(&invitee.id)
            .bind(&invitee.tenant_id)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn delete(&self, tenant_id: &str, id: &str) -> Result<(), AppError> {
        let result = sqlx::query("DELETE FROM invitees WHERE id = ? AND tenant_id = ?")
            .bind(id)
            .bind(tenant_id)
            .execute(&self.pool)
            .await
            .map_err(AppError::Database)?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Invitee not found".into()));
        }
        Ok(())
    }
}