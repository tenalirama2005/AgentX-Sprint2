#![allow(dead_code, unused_imports, unused_variables, unused_mut)]
// src/main.rs
// AgentX-Sprint2 — A2A Purple Agent Server + Calibration Sidecar
use axum::{
    routing::{get, post},
    Router,
};
use std::{path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod a2a;
mod calibration;
mod fba;
mod tracks;

use a2a::{card, handler};

// ── App State ─────────────────────────────────────────────────────────────────
#[derive(Clone)]
pub struct AppState {
    pub fba_endpoint: String,
    pub jwt_secret: String,
    pub agent_url: String,
}

// ── Main ──────────────────────────────────────────────────────────────────────
#[tokio::main]
async fn main() {
    // Logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    // ── Config ────────────────────────────────────────────────────────────────
    let fba_endpoint = std::env::var("FBA_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:8081".into());
    let jwt_secret = std::env::var("GATEWAY_JWT_SECRET")
        .unwrap_or_else(|_| "agentx-internal-token".into());
    let agent_url = std::env::var("AGENT_URL")
        .unwrap_or_else(|_| "http://localhost:8090".into());
    let port = std::env::var("PORT").unwrap_or_else(|_| "8090".into());

    // Calibration sidecar config
    let cal_port: u16 = std::env::var("CAL_PORT")
        .unwrap_or_else(|_| "9020".into())
        .parse()
        .unwrap_or(9020);

    let cache_file = PathBuf::from(
        std::env::var("CAL_CACHE_FILE")
            .unwrap_or_else(|_| "/app/scenarios/fwa/purple_agent/learned_distances.json".into()),
    );

    let benchmark_root = PathBuf::from(
        std::env::var("CAL_BENCHMARK_ROOT")
            .unwrap_or_else(|_| "/app/FieldWorkArena-GreenAgent/benchmark/tasks".into()),
    );

    let refresh_secs: u64 = std::env::var("CAL_REFRESH_SECS")
        .unwrap_or_else(|_| "300".into())
        .parse()
        .unwrap_or(300);

    // ── Spawn Calibration Sidecar ─────────────────────────────────────────────
    info!("🦀 Starting Rust Calibration Sidecar on port {}", cal_port);
    tokio::spawn(calibration::run_calibration_sidecar(
        cache_file,
        benchmark_root,
        cal_port,
        refresh_secs,
    ));

    // ── Main A2A Server ───────────────────────────────────────────────────────
    let state = Arc::new(AppState {
        fba_endpoint,
        jwt_secret,
        agent_url,
    });

    let app = Router::new()
        .route("/", post(handler::handle_task))
        .route("/.well-known/agent-card.json", get(card::agent_card))
        .route("/health", get(health))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    info!("🚀 AgentX-Sprint2 A2A adapter listening on {}", addr);
    let listener = TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "service": "agentx-sprint2",
        "calibration_sidecar": "localhost:9020"
    }))
}
