use crate::domain::{models::invitee::Invitee, ports::InviteeRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;

pub struct PostgresInviteeRepo {
    pool: PgPool,
}

impl PostgresInviteeRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl InviteeRepository for PostgresInviteeRepo {
    async fn create(&self, invitee: &Invitee) -> Result<Invitee, AppError> {
        sqlx::query_as::<_, Invitee>(
            "INSERT INTO invitees (id, tenant_id, event_id, token, email, status, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, tenant_id, event_id, token, email, status, created_at",
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
            "SELECT id, tenant_id, event_id, token, email, status, created_at FROM invitees WHERE token = $1",
        )
            .bind(token)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_id(&self, tenant_id: &str, id: &str) -> Result<Option<Invitee>, AppError> {
        sqlx::query_as::<_, Invitee>(
            "SELECT id, tenant_id, event_id, token, email, status, created_at FROM invitees WHERE tenant_id = $1 AND id = $2",
        )
            .bind(tenant_id)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn list_by_event(&self, tenant_id: &str, event_id: &str) -> Result<Vec<Invitee>, AppError> {
        sqlx::query_as::<_, Invitee>(
            "SELECT id, tenant_id, event_id, token, email, status, created_at FROM invitees WHERE tenant_id = $1 AND event_id = $2",
        )
            .bind(tenant_id)
            .bind(event_id)
            .fetch_all(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update(&self, invitee: &Invitee) -> Result<Invitee, AppError> {
        sqlx::query_as::<_, Invitee>(
            "UPDATE invitees SET status=$1, email=$2 WHERE id=$3 AND tenant_id=$4 RETURNING *"
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
        let result = sqlx::query("DELETE FROM invitees WHERE id = $1 AND tenant_id = $2")
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