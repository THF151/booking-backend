use chrono::{Duration as ChronoDuration, Utc};
use colored::*;
use governor::{Quota, RateLimiter};
use hdrhistogram::Histogram;
use reqwest::Client;
use serde_json::{json, Value};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use uuid::Uuid;

const DURATION_SECS: u64 = 20;
const BASE_URL: &str = "http://localhost:8000";

struct Target {
    name: &'static str,
    method: &'static str,
    url: String,
    body: Option<serde_json::Value>,
}

#[tokio::main]
async fn main() {
    println!("{}", "🚀 Starting Benchmark Suite".bold().green());
    println!("Target URL: {}", BASE_URL);

    let client = Client::builder()
        .pool_max_idle_per_host(1000)
        .timeout(Duration::from_secs(10))
        .cookie_store(true)
        .build()
        .unwrap();

    if client.get(format!("{}/health", BASE_URL)).send().await.is_err() {
        eprintln!("{}", "❌ Server is NOT reachable at localhost:8000. Please start it first.".red().bold());
        return;
    }

    println!("\n{}", "⚙️  Setting up benchmark data...".yellow());
    let (tenant_id, admin_secret) = setup_tenant(&client).await;
    let event_slug = "benchmark-event";
    setup_event(&client, &tenant_id, &admin_secret, event_slug).await;

    println!("{}", "✅ Data created successfully.".green());
    println!("   Tenant ID: {}", tenant_id);
    println!("   Admin Sec: {}", admin_secret);

    let targets = vec![
        Target {
            name: "Health Check (Public)",
            method: "GET",
            url: format!("{}/health", BASE_URL),
            body: None,
        },
        Target {
            name: "Get Event Details (Public Read)",
            method: "GET",
            url: format!("{}/api/v1/{}/events/{}", BASE_URL, tenant_id, event_slug),
            body: None,
        },
        Target {
            name: "Login Flow (Crypto Intensive)",
            method: "POST",
            url: format!("{}/api/v1/auth/login", BASE_URL),
            body: Some(json!({
                "tenant_id": tenant_id,
                "username": "admin",
                "password": admin_secret
            })),
        },
    ];

    let rps_stages = vec![10, 50, 200, 1000];

    for target in targets {
        println!("\n{}", "=".repeat(60));
        println!("Benchmarking Endpoint: {}", target.name.cyan().bold());
        println!("URL: {}", target.url);
        println!("{}", "=".repeat(60));

        println!("{:<10} | {:<15} | {:<15} | {:<15}", "RPS", "Mean (ms)", "P99 (ms)", "Success Rate");
        println!("{:-<10}-+-{:-<15}-+-{:-<15}-+-{:-<15}", "", "", "", "");

        for &rps in &rps_stages {
            run_stage(&client, &target, rps).await;
        }
    }
}

async fn setup_tenant(client: &Client) -> (String, String) {
    let slug = format!("bench-{}", Uuid::new_v4());
    let res = client.post(format!("{}/api/v1/tenants", BASE_URL))
        .json(&json!({
            "name": "Benchmark Corp",
            "slug": slug
        }))
        .send()
        .await
        .expect("Failed to send tenant create request");

    if !res.status().is_success() {
        panic!("Failed to create tenant: status {}", res.status());
    }

    let body: Value = res.json().await.expect("Failed to parse tenant response");
    let id = body["tenant_id"].as_str().expect("No tenant_id").to_string();
    let secret = body["admin_secret"].as_str().expect("No admin_secret").to_string();
    (id, secret)
}

async fn setup_event(client: &Client, tenant_id: &str, admin_secret: &str, slug: &str) {
    let event_payload = json!({
        "slug": slug,
        "title_en": "Benchmark Meeting",
        "title_de": "Benchmark Treffen",
        "desc_en": "Load testing",
        "desc_de": "Lasttest",
        "location": "Server",
        "timezone": "Europe/Berlin",
        "payout": "0",
        "host_name": "Bot",
        "active_start": Utc::now().to_rfc3339(),
        "active_end": (Utc::now() + ChronoDuration::days(365)).to_rfc3339(),
        "duration_min": 30,
        "interval_min": 30,
        "max_participants": 1,
        "image_url": "http://localhost",
        "config": { "monday": [{"start": "09:00", "end": "17:00"}] },
        "access_mode": "OPEN"
    });

    let login_res = client.post(format!("{}/api/v1/auth/login", BASE_URL))
        .json(&json!({
            "tenant_id": tenant_id,
            "username": "admin",
            "password": admin_secret
        }))
        .send()
        .await
        .expect("Login failed during setup");

    if !login_res.status().is_success() {
        panic!("Login failed. Status: {}", login_res.status());
    }

    let auth_body: Value = login_res.json().await.unwrap();
    let csrf_token = auth_body["csrf_token"].as_str().unwrap();

    let res = client.post(format!("{}/api/v1/{}/events", BASE_URL, tenant_id))
        .header("X-CSRF-Token", csrf_token)
        .json(&event_payload)
        .send()
        .await
        .expect("Failed to create event");

    if !res.status().is_success() {
        let status = res.status();
        let txt = res.text().await.unwrap_or_default();
        panic!("Failed to create event data. Status: {}. Body: {}", status, txt);
    }
}

async fn run_stage(client: &Client, target: &Target, rps: u32) {
    let limiter = Arc::new(RateLimiter::direct(
        Quota::per_second(NonZeroU32::new(rps).unwrap())
    ));

    let (tx, mut rx) = mpsc::channel(50000);
    let start_time = Instant::now();
    let duration = Duration::from_secs(DURATION_SECS);

    loop {
        if start_time.elapsed() > duration {
            break;
        }

        if limiter.check().is_ok() {
            let client = client.clone();
            let url = target.url.clone();
            let body = target.body.clone();
            let method = target.method;
            let tx = tx.clone();

            tokio::spawn(async move {
                let req_start = Instant::now();
                let res = match method {
                    "GET" => client.get(&url).send().await,
                    "POST" => {
                        let mut req = client.post(&url);
                        if let Some(b) = body {
                            req = req.json(&b);
                        }
                        req.send().await
                    },
                    _ => client.get(&url).send().await,
                };
                let latency = req_start.elapsed();

                let success = match res {
                    Ok(r) => r.status().is_success(),
                    Err(_) => false,
                };

                let _ = tx.send((latency, success)).await;
            });
        } else {
            tokio::task::yield_now().await;
        }
    }

    drop(tx);

    let mut histogram = Histogram::<u64>::new(3).unwrap();
    let mut successes = 0;
    let mut total = 0;

    while let Some((latency, success)) = rx.recv().await {
        total += 1;
        if success { successes += 1; }
        histogram.record(latency.as_micros() as u64).unwrap();
    }

    let mean_ms = histogram.mean() / 1000.0;
    let p99_ms = histogram.value_at_quantile(0.99) as f64 / 1000.0;
    let success_rate = if total > 0 { (successes as f64 / total as f64) * 100.0 } else { 0.0 };

    println!(
        "{:<10} | {:<15.2} | {:<15.2} | {:<14.1}%",
        rps,
        mean_ms,
        p99_ms,
        success_rate
    );

    tokio::time::sleep(Duration::from_millis(500)).await;
}