use booking_backend::{
    api::router::create_router,
    state::AppState,
    config::Config,
    infra::repositories::{
        sqlite_booking_repo::SqliteBookingRepo,
        sqlite_event_repo::SqliteEventRepo,
        sqlite_invitee_repo::SqliteInviteeRepo,
        sqlite_tenant_repo::SqliteTenantRepo,
        sqlite_user_repo::SqliteUserRepo,
        sqlite_job_repo::SqliteJobRepo,
        sqlite_event_override_repo::SqliteEventOverrideRepo,
        sqlite_auth_repo::SqliteAuthRepo,
        sqlite_label_repo::SqliteLabelRepo,
        sqlite_session_repo::SqliteSessionRepo,
        sqlite_communication_repo::SqliteCommunicationRepo,
    },
    domain::services::auth_service::AuthService,
    domain::ports::{EmailService, LlmService},
    background::start_background_worker,
    error::AppError,
};
use sqlx::{sqlite::{SqliteConnectOptions, SqlitePoolOptions}, Pool, Sqlite};
use std::sync::Arc;
use uuid::Uuid;
use axum::{
    body::Body,
    http::{Request, header},
    Router,
};
use std::str::FromStr;
use async_trait::async_trait;
use tera::Tera;
use tower::ServiceExt;
use serde_json::Value;

pub struct MockEmailService;

#[async_trait]
impl EmailService for MockEmailService {
    async fn send(
        &self,
        _recipient: &str,
        _subject: &str,
        _html_body: &str,
        _attachment_name: Option<&str>,
        _attachment_data: Option<&[u8]>
    ) -> Result<(), AppError> {
        Ok(())
    }
}

pub struct MockLlmService;

#[async_trait]
impl LlmService for MockLlmService {
    async fn generate(
        &self,
        _api_key: &str,
        _prompt: &str,
        _system_instruction: &str
    ) -> Result<String, AppError> {
        Ok("Mock AI Response: Content generated successfully.".to_string())
    }
}

pub struct AuthHeaders {
    pub access_token: String,
    pub csrf_token: String,
}

#[allow(dead_code)]
pub struct TestApp {
    pub router: Router,
    pub pool: Pool<Sqlite>,
    pub db_filename: String,
    pub state: Arc<AppState>,
}

impl TestApp {
    pub async fn new() -> Self {
        let db_filename = format!("test_{}.db", Uuid::new_v4());
        let db_url = format!("sqlite://{}?mode=rwc", db_filename);

        let connection_options = SqliteConnectOptions::from_str(&db_url)
            .unwrap()
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .connect_with(connection_options)
            .await
            .expect("Failed to connect to test db");

        sqlx::migrate!("./migrations/sqlite")
            .run(&pool)
            .await
            .expect("Failed to migrate test db");

        let mut tera = Tera::default();
        tera.add_raw_template("confirmation.html", "<html>Mock Confirmation for {{ user_name }}</html>").unwrap();
        tera.add_raw_template("reminder.html", "<html>Mock Reminder for {{ user_name }}</html>").unwrap();
        let templates = Arc::new(tera);

        let priv_key_pem = include_str!("../tests/keys/test_private.pem");
        let pub_key_pem = include_str!("../tests/keys/test_public.pem");

        let config = Config {
            database_url: db_url.clone(),
            port: 0,
            mail_service_url: "http://localhost".to_string(),
            mail_service_token: "token".to_string(),
            jwt_secret_key: priv_key_pem.to_string(),
            jwt_public_key: pub_key_pem.to_string(),
            auth_issuer: "test-issuer".to_string(),
        };

        let auth_repo = Arc::new(SqliteAuthRepo::new(pool.clone()));
        let auth_service = Arc::new(AuthService::new(auth_repo.clone(), config.clone()));

        let state = Arc::new(AppState {
            config: config.clone(),
            tenant_repo: Arc::new(SqliteTenantRepo::new(pool.clone())),
            user_repo: Arc::new(SqliteUserRepo::new(pool.clone())),
            event_repo: Arc::new(SqliteEventRepo::new(pool.clone())),
            booking_repo: Arc::new(SqliteBookingRepo::new(pool.clone())),
            invitee_repo: Arc::new(SqliteInviteeRepo::new(pool.clone())),
            job_repo: Arc::new(SqliteJobRepo::new(pool.clone())),
            event_override_repo: Arc::new(SqliteEventOverrideRepo::new(pool.clone())),
            label_repo: Arc::new(SqliteLabelRepo::new(pool.clone())),
            session_repo: Arc::new(SqliteSessionRepo::new(pool.clone())),
            communication_repo: Arc::new(SqliteCommunicationRepo::new(pool.clone())),
            auth_repo,
            auth_service,
            email_service: Arc::new(MockEmailService),
            llm_service: Arc::new(MockLlmService),
            templates,
        });

        // Start Background Worker
        let worker_state = state.clone();
        tokio::spawn(async move {
            start_background_worker(worker_state).await;
        });

        let router = create_router(state.clone());

        Self {
            router,
            pool,
            db_filename,
            state,
        }
    }

    pub async fn login(&self, tenant_id: &str, username: &str, password: &str) -> AuthHeaders {
        let payload = serde_json::json!({
            "tenant_id": tenant_id,
            "username": username,
            "password": password
        });

        let response = self.router.clone().oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(payload.to_string()))
                .unwrap()
        ).await.unwrap();

        if !response.status().is_success() {
            panic!("Login failed in test helper: status {}", response.status());
        }

        let cookies: Vec<String> = response.headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|h| h.to_str().unwrap().to_string())
            .collect();

        let access_token_cookie = cookies.iter()
            .find(|c| c.contains("access_token="))
            .expect("No access_token cookie returned");

        let start = access_token_cookie.find("access_token=").unwrap() + 13;
        let end = access_token_cookie[start..].find(';').unwrap_or(access_token_cookie.len() - start);
        let access_token = access_token_cookie[start..start+end].to_string();

        let body_bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
        let csrf_token = body_json["csrf_token"].as_str().expect("No csrf_token in body").to_string();

        AuthHeaders {
            access_token,
            csrf_token
        }
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_filename);
    }
}