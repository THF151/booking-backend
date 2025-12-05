use booking_backend::{
    domain::models::job::Job,
    domain::ports::JobRepository,
    infra::repositories::postgres_job_repo::PostgresJobRepo,
};
use chrono::{Duration, Utc};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use tokio::task::JoinSet;
use uuid::Uuid;

#[tokio::test]
async fn test_job_queue_race_conditions() {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for concurrency test");
    if !db_url.starts_with("postgres") {
        println!("Skipping concurrency test (not targeting Postgres)");
        return;
    }

    let opts = PgConnectOptions::from_str(&db_url)
        .unwrap()
        .log_statements(tracing::log::LevelFilter::Debug);

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect_with(opts)
        .await
        .expect("Failed to connect to DB");

    sqlx::query("DELETE FROM jobs").execute(&pool).await.unwrap();

    let repo = Arc::new(PostgresJobRepo::new(pool.clone()));

    // 2. Seed Data
    let total_jobs = 100;
    let now = Utc::now();

    for i in 0..total_jobs {
        let exec_time = now - Duration::minutes(1) + Duration::milliseconds(i as i64);
        let job = Job::new(
            "TEST_EMAIL",
            Uuid::new_v4().to_string(),
            "test-tenant".to_string(),
            exec_time
        );

        sqlx::query(
            "INSERT INTO jobs (id, job_type, payload, execute_at, status, created_at) VALUES ($1, $2, $3, $4, $5, $6)"
        )
            .bind(&job.id)
            .bind(&job.job_type)
            .bind(&job.payload)
            .bind(job.execute_at)
            .bind("PENDING")
            .bind(job.created_at)
            .execute(&pool).await.unwrap();
    }

    // 3. Simulate Distributed Workers
    let worker_count = 10;
    let mut set = JoinSet::new();

    for i in 0..worker_count {
        let repo_clone = repo.clone();
        set.spawn(async move {
            let mut claimed_jobs = Vec::new();
            let mut empty_streaks = 0;

            while empty_streaks < 10 {
                let batch = repo_clone.find_pending(5).await.expect("Failed to fetch jobs");
                if batch.is_empty() {
                    empty_streaks += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                } else {
                    empty_streaks = 0;
                    for job in batch {
                        claimed_jobs.push(job.id);
                    }
                }
            }
            println!("Worker {} claimed {} jobs", i, claimed_jobs.len());
            claimed_jobs
        });
    }

    // 4. Verify Results
    let mut all_claimed_ids = Vec::new();
    while let Some(res) = set.join_next().await {
        let worker_claimed = res.unwrap();
        all_claimed_ids.extend(worker_claimed);
    }

    let unique_ids: HashSet<String> = all_claimed_ids.iter().cloned().collect();

    println!("Total seeded: {}", total_jobs);
    println!("Total claimed: {}", all_claimed_ids.len());
    println!("Unique claimed: {}", unique_ids.len());

    assert_eq!(
        unique_ids.len(),
        all_claimed_ids.len(),
        "Duplicate jobs detected! Race condition exists."
    );

    assert_eq!(
        all_claimed_ids.len(),
        total_jobs,
        "Not all jobs were processed"
    );

    sqlx::query("DELETE FROM jobs").execute(&pool).await.unwrap();
}