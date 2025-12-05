use axum::{
    extract::{FromRequestParts, Path},
    http::{request::Parts, StatusCode},
};
use std::collections::HashMap;
use crate::state::AppState;
use std::sync::Arc;

pub struct TenantId(pub String);

impl FromRequestParts<Arc<AppState>> for TenantId {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &Arc<AppState>) -> Result<Self, Self::Rejection> {
        let params: Path<HashMap<String, String>> = Path::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::BAD_REQUEST)?;

        let tenant_id = params.get("tenant_id").ok_or(StatusCode::BAD_REQUEST)?;

        match state.tenant_repo.find_by_id(tenant_id).await {
            Ok(Some(_)) => Ok(TenantId(tenant_id.clone())),
            Ok(None) => Err(StatusCode::NOT_FOUND),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}