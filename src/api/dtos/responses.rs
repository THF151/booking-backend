use serde::Serialize;

#[derive(Serialize)]
pub struct TenantCreatedResponse {
    pub tenant_id: String,
    pub admin_username: String,
    pub admin_secret: String,
}

#[derive(Serialize)]
pub struct SlotsResponse {
    pub date: String,
    pub slots: Vec<String>,
}