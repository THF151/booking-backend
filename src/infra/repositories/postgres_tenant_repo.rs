use crate::domain::{models::tenant::Tenant, ports::TenantRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::PgPool;

pub struct PostgresTenantRepo {
    pool: PgPool,
}

impl PostgresTenantRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TenantRepository for PostgresTenantRepo {
    async fn create(&self, tenant: &Tenant) -> Result<Tenant, AppError> {
        sqlx::query_as::<_, Tenant>(
            "INSERT INTO tenants (id, name, slug, logo_url, ai_api_key, ai_provider, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING *"
        )
            .bind(&tenant.id)
            .bind(&tenant.name)
            .bind(&tenant.slug)
            .bind(&tenant.logo_url)
            .bind(&tenant.ai_api_key)
            .bind(&tenant.ai_provider)
            .bind(tenant.created_at)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<Tenant>, AppError> {
        sqlx::query_as::<_, Tenant>(
            "SELECT * FROM tenants WHERE id = $1",
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_slug(&self, slug: &str) -> Result<Option<Tenant>, AppError> {
        sqlx::query_as::<_, Tenant>(
            "SELECT * FROM tenants WHERE slug = $1",
        )
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update(&self, tenant: &Tenant) -> Result<Tenant, AppError> {
        sqlx::query_as::<_, Tenant>(
            "UPDATE tenants SET name=$1, logo_url=$2, ai_api_key=$3 WHERE id=$4 RETURNING *"
        )
            .bind(&tenant.name)
            .bind(&tenant.logo_url)
            .bind(&tenant.ai_api_key)
            .bind(&tenant.id)
            .fetch_one(&self.pool)
            .await
            .map_err(AppError::Database)
    }
}