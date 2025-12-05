use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use sqlx::{postgres::{PgPoolOptions, PgConnectOptions}, sqlite::{SqlitePoolOptions, SqliteJournalMode, SqliteConnectOptions}};
use sqlx::{PgPool, SqlitePool, ConnectOptions};
use tracing::info;
use tracing::log::LevelFilter;
use tera::Tera;

use crate::config::Config;
use crate::state::AppState;
use crate::infra::email::http_email_service::HttpEmailService;
use crate::infra::ai::gemini_service::GeminiService;
use crate::domain::services::auth_service::AuthService;
use crate::infra::repositories::{
    postgres_booking_repo::PostgresBookingRepo, postgres_event_repo::PostgresEventRepo,
    postgres_invitee_repo::PostgresInviteeRepo, postgres_tenant_repo::PostgresTenantRepo,
    postgres_user_repo::PostgresUserRepo, postgres_job_repo::PostgresJobRepo,
    postgres_event_override_repo::PostgresEventOverrideRepo, postgres_auth_repo::PostgresAuthRepo,
    postgres_label_repo::PostgresLabelRepo, postgres_session_repo::PostgresSessionRepo,
    postgres_communication_repo::PostgresCommunicationRepo,
    sqlite_booking_repo::SqliteBookingRepo, sqlite_event_repo::SqliteEventRepo,
    sqlite_invitee_repo::SqliteInviteeRepo, sqlite_tenant_repo::SqliteTenantRepo,
    sqlite_user_repo::SqliteUserRepo, sqlite_job_repo::SqliteJobRepo,
    sqlite_event_override_repo::SqliteEventOverrideRepo, sqlite_auth_repo::SqliteAuthRepo,
    sqlite_label_repo::SqliteLabelRepo, sqlite_session_repo::SqliteSessionRepo,
    sqlite_communication_repo::SqliteCommunicationRepo,
};

pub async fn bootstrap_state(config: &Config) -> AppState {
    let database_url = &config.database_url;
    let email_service = Arc::new(HttpEmailService::new(
        config.mail_service_url.clone(),
        config.mail_service_token.clone(),
    ));

    let llm_service = Arc::new(GeminiService::new());

    let mut tera = Tera::default();
    tera.add_raw_template("confirmation.html", include_str!("../templates/confirmation.html"))
        .expect("Failed to load confirmation template");
    tera.add_raw_template("reminder.html", include_str!("../templates/reminder.html"))
        .expect("Failed to load reminder template");
    let templates = Arc::new(tera);

    if database_url.starts_with("postgres://") || database_url.starts_with("postgresql://") {
        info!("Initializing PostgreSQL connection...");

        let mut opts: PgConnectOptions = database_url.parse().expect("Invalid Postgres URL");
        opts = opts.log_statements(LevelFilter::Debug)
            .log_slow_statements(LevelFilter::Warn, Duration::from_millis(500));

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(opts)
            .await
            .expect("Failed to connect to Postgres");

        run_postgres_migrations(&pool).await;

        let auth_repo = Arc::new(PostgresAuthRepo::new(pool.clone()));
        let auth_service = Arc::new(AuthService::new(auth_repo.clone(), config.clone()));

        AppState {
            config: config.clone(),
            tenant_repo: Arc::new(PostgresTenantRepo::new(pool.clone())),
            user_repo: Arc::new(PostgresUserRepo::new(pool.clone())),
            event_repo: Arc::new(PostgresEventRepo::new(pool.clone())),
            booking_repo: Arc::new(PostgresBookingRepo::new(pool.clone())),
            invitee_repo: Arc::new(PostgresInviteeRepo::new(pool.clone())),
            job_repo: Arc::new(PostgresJobRepo::new(pool.clone())),
            event_override_repo: Arc::new(PostgresEventOverrideRepo::new(pool.clone())),
            auth_repo,
            label_repo: Arc::new(PostgresLabelRepo::new(pool.clone())),
            session_repo: Arc::new(PostgresSessionRepo::new(pool.clone())),
            communication_repo: Arc::new(PostgresCommunicationRepo::new(pool.clone())),
            auth_service,
            email_service,
            llm_service,
            templates,
        }
    } else {
        info!("Initializing SQLite connection with WAL Mode...");

        let opts = SqliteConnectOptions::from_str(database_url)
            .expect("Invalid SQLite connection string")
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5))
            .log_statements(LevelFilter::Debug)
            .log_slow_statements(LevelFilter::Warn, Duration::from_millis(500));

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await
            .expect("Failed to connect to SQLite");

        run_sqlite_migrations(&pool).await;

        let auth_repo = Arc::new(SqliteAuthRepo::new(pool.clone()));
        let auth_service = Arc::new(AuthService::new(auth_repo.clone(), config.clone()));

        AppState {
            config: config.clone(),
            tenant_repo: Arc::new(SqliteTenantRepo::new(pool.clone())),
            user_repo: Arc::new(SqliteUserRepo::new(pool.clone())),
            event_repo: Arc::new(SqliteEventRepo::new(pool.clone())),
            booking_repo: Arc::new(SqliteBookingRepo::new(pool.clone())),
            invitee_repo: Arc::new(SqliteInviteeRepo::new(pool.clone())),
            job_repo: Arc::new(SqliteJobRepo::new(pool.clone())),
            event_override_repo: Arc::new(SqliteEventOverrideRepo::new(pool.clone())),
            auth_repo,
            label_repo: Arc::new(SqliteLabelRepo::new(pool.clone())),
            session_repo: Arc::new(SqliteSessionRepo::new(pool.clone())),
            communication_repo: Arc::new(SqliteCommunicationRepo::new(pool.clone())),
            auth_service,
            email_service,
            llm_service,
            templates,
        }
    }
}

async fn run_postgres_migrations(pool: &PgPool) {
    sqlx::migrate!("./migrations/postgres")
        .run(pool)
        .await
        .expect("Failed to run Postgres migrations");
}

async fn run_sqlite_migrations(pool: &SqlitePool) {
    sqlx::migrate!("./migrations/sqlite")
        .run(pool)
        .await
        .expect("Failed to run SQLite migrations");
}