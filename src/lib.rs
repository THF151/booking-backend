pub mod api;
pub mod config;
pub mod domain;
pub mod error;
pub mod infra;
pub mod state;
pub mod background;

use crate::config::Config;
use crate::infra::factory::bootstrap_state;
use api::router::create_router;
use std::sync::Arc;
use tracing::info;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};
use crate::background::start_background_worker;

pub fn init_logging() -> WorkerGuard {
    let file_appender = tracing_appender::rolling::daily("./logs", "booking-service.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .json()
        .with_writer(non_blocking)
        .with_filter(EnvFilter::new("info,booking_backend=debug"));

    let stdout_layer = tracing_subscriber::fmt::layer()
        .pretty()
        .with_target(false)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()));

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();

    info!("Logging initialized. Writing JSON logs to ./logs/");
    guard
}

pub async fn run() {
    let _guard = init_logging();

    let config = Config::from_env();
    let state = bootstrap_state(&config).await;
    let state_arc = Arc::new(state);

    let worker_state = state_arc.clone();
    tokio::spawn(async move {
        start_background_worker(worker_state).await;
    });

    let app = create_router(state_arc);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", config.port))
        .await
        .unwrap();

    info!("🚀 Server running on port {}", config.port);
    axum::serve(listener, app).await.unwrap();
}