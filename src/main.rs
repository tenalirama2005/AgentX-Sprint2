#![allow(dead_code, unused_imports, unused_variables, unused_mut)]
// src/main.rs
// AgentX-Sprint2 — A2A Purple Agent Server
// Serves all 4 benchmark tracks from a single binary

use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod a2a;
mod fba;
mod tracks;

use a2a::{card, handler};

// ─── App State ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct AppState {
    /// URL of existing purple_agent FBA pipeline
    /// e.g. http://localhost:8081  (or k8s service in cluster)
    pub fba_endpoint: String,
    pub jwt_secret: String,
    pub agent_url: String,
}

// ─── Main ────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env
    dotenvy::dotenv().ok();

    // Tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "agentx_sprint2=debug,tower_http=info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let state = Arc::new(AppState {
        fba_endpoint: std::env::var("FBA_ENDPOINT")
            .unwrap_or_else(|_| "http://purple-agent:8081".into()),
        jwt_secret: std::env::var("GATEWAY_JWT_SECRET").expect("GATEWAY_JWT_SECRET must be set"),
        agent_url: std::env::var("AGENT_URL").unwrap_or_else(|_| "http://localhost:8090".into()),
    });

    let port = std::env::var("PORT").unwrap_or_else(|_| "8090".into());
    let addr = format!("0.0.0.0:{}", port);

    let app = build_router(state);

    info!("🚀 AgentX-Sprint2 A2A adapter listening on {}", addr);
    info!("   Tracks: CAR-bench | τ²-Bench | MAizeBargAIn | OSWorld");
    info!("   FBA:    39/49 quorum @ 94% confidence");

    let listener = TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ─── Router ──────────────────────────────────────────────────────────────────

fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        // A2A standard endpoints
        .route("/.well-known/agent.json", get(card::agent_card))
        .route("/a2a/tasks/send", post(handler::handle_task))
        // Track-specific health / info endpoints
        .route("/health", get(health))
        .route("/tracks", get(tracks_info))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "agent":  "AgentX-Sprint2",
        "fba":    "39/49 @ 94%",
        "tracks": ["car-bench", "tau2-bench", "maize-bargain", "osworld"]
    }))
}

async fn tracks_info() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "tracks": {
            "car-bench": {
                "tools": 58,
                "policies": 19,
                "tasks": 254,
                "deadline": "2026-04-12"
            },
            "tau2-bench": {
                "domain": "telecom",
                "leaderboard_entries": 2,
                "top_score": "68%",
                "deadline": "2026-04-12"
            },
            "maize-bargain": {
                "type": "multi-round bargaining",
                "deadline": "2026-04-12"
            },
            "osworld": {
                "type": "computer-use GUI",
                "deadline": "2026-04-12"
            }
        }
    }))
}
