use crate::domain::{models::tenant::Tenant, ports::TenantRepository};
use crate::error::AppError;
use async_trait::async_trait;
use sqlx::SqlitePool;

pub struct SqliteTenantRepo {
    pool: SqlitePool,
}

impl SqliteTenantRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TenantRepository for SqliteTenantRepo {
    async fn create(&self, tenant: &Tenant) -> Result<Tenant, AppError> {
        sqlx::query_as::<_, Tenant>(
            "INSERT INTO tenants (id, name, slug, logo_url, ai_api_key, ai_provider, created_at) VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING *"
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
            "SELECT * FROM tenants WHERE id = ?",
        )
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn find_by_slug(&self, slug: &str) -> Result<Option<Tenant>, AppError> {
        sqlx::query_as::<_, Tenant>(
            "SELECT * FROM tenants WHERE slug = ?",
        )
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map_err(AppError::Database)
    }

    async fn update(&self, tenant: &Tenant) -> Result<Tenant, AppError> {
        sqlx::query_as::<_, Tenant>(
            "UPDATE tenants SET name=?, logo_url=?, ai_api_key=? WHERE id=? RETURNING *"
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