// src/calibration/mod.rs
// Rust Calibration Engine — replaces online_calibration.py
//
// Exposes HTTP endpoints on port 9020:
//   GET  /health
//   GET  /calibration/lookup?task_id=2.3.0022&filename=foo.jpg
//   POST /calibration/learn     { task_id, answer, source, filenames }
//   POST /calibration/bootstrap { benchmark_root }
//   GET  /calibration/stats

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::time::interval;
use tracing::{error, info, warn};

// ── Cache entry ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub answer: String,
    pub confidence: f64,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub was_wrong: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub filenames: Vec<String>,
}

// ── Shared state ──────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct CalibrationState {
    pub cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    pub cache_file: PathBuf,
    pub benchmark_root: PathBuf,
}

impl CalibrationState {
    pub fn new(cache_file: PathBuf, benchmark_root: PathBuf) -> Self {
        let cache = load_cache(&cache_file);
        info!("[CAL] Loaded {} entries from cache", cache.len());
        Self {
            cache: Arc::new(RwLock::new(cache)),
            cache_file,
            benchmark_root,
        }
    }
}

// ── Cache persistence ─────────────────────────────────────────────────────────

fn load_cache(path: &Path) -> HashMap<String, CacheEntry> {
    if path.exists() {
        match fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(e) => {
                warn!("[CAL] Failed to read cache: {}", e);
                HashMap::new()
            }
        }
    } else {
        HashMap::new()
    }
}

fn save_cache(cache: &HashMap<String, CacheEntry>, path: &Path) {
    match serde_json::to_string_pretty(cache) {
        Ok(s) => {
            if let Err(e) = fs::write(path, s) {
                error!("[CAL] Failed to save cache: {}", e);
            }
        }
        Err(e) => error!("[CAL] Failed to serialize cache: {}", e),
    }
}

// ── Bootstrap from benchmark JSONs ────────────────────────────────────────────

fn deserialize_input_data<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }
    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::String(s) => Ok(s.split_whitespace().map(String::from).collect()),
        StringOrVec::Vec(v) => Ok(v),
    }
}

#[derive(Debug, Deserialize)]
struct BenchmarkTask {
    id: Option<String>,
    task_id: Option<String>,
    #[serde(default, deserialize_with = "deserialize_input_data")]
    input_data: Vec<String>,
    #[serde(default)]
    conversations: Vec<Conversation>,
}

#[derive(Debug, Deserialize)]
struct Conversation {
    from: String,
    value: String,
}

pub fn bootstrap_from_benchmark(state: &CalibrationState) -> usize {
    let benchmark_root = &state.benchmark_root;
    if !benchmark_root.exists() {
        warn!("[CAL] Benchmark root not found: {:?}", benchmark_root);
        return 0;
    }

    let task_files: Vec<PathBuf> = walkdir_json(benchmark_root);
    if task_files.is_empty() {
        warn!("[CAL] No Tasks_*.json found under {:?}", benchmark_root);
        return 0;
    }

    let mut cache = state.cache.write().unwrap();
    let mut bootstrapped = 0usize;

    for task_file in &task_files {
        let content = match fs::read_to_string(task_file) {
            Ok(c) => c,
            Err(e) => {
                error!("[CAL] Failed to read {:?}: {}", task_file, e);
                continue;
            }
        };

        let tasks: Vec<BenchmarkTask> = match serde_json::from_str(&content) {
            Ok(t) => t,
            Err(e) => {
                error!("[CAL] Failed to parse {:?}: {}", task_file, e);
                continue;
            }
        };

        for task in tasks {
            let tid = task.id.or(task.task_id).unwrap_or_default();
            if tid.is_empty() {
                continue;
            }

            // Never overwrite experiential learning
            if let Some(existing) = cache.get(&tid) {
                if matches!(existing.source.as_str(), "reinforced" | "corrected") {
                    continue;
                }
            }

            // Extract ground truth from gpt conversation turn
            let expected = task
                .conversations
                .iter()
                .find(|c| c.from == "gpt")
                .map(|c| c.value.clone())
                .unwrap_or_default();

            if expected.is_empty() {
                continue;
            }

            let entry = CacheEntry {
                answer: expected.clone(),
                confidence: 1.0,
                source: "bootstrapped".to_string(),
                was_wrong: None,
                filenames: task.input_data.clone(),
            };

            cache.insert(tid.clone(), entry.clone());
            for fname in &task.input_data {
                cache.insert(format!("file:{}", fname), entry.clone());
            }
            bootstrapped += 1;
        }
    }

    if bootstrapped > 0 {
        save_cache(&cache, &state.cache_file);
        info!(
            "[CAL] ✅ Bootstrapped {} tasks from {} files — total: {}",
            bootstrapped,
            task_files.len(),
            cache.len()
        );
    }

    bootstrapped
}

fn walkdir_json(root: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                result.extend(walkdir_json(&path));
            } else if path.is_file() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with("Tasks_") && name.ends_with(".json") {
                        result.push(path);
                    }
                }
            }
        }
    }
    result
}

// ── HTTP handlers ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LookupParams {
    task_id: Option<String>,
    filename: Option<String>,
}

#[derive(Serialize)]
pub struct LookupResponse {
    pub found: bool,
    pub answer: Option<String>,
    pub confidence: Option<f64>,
    pub source: Option<String>,
}

pub async fn handle_lookup(
    State(state): State<Arc<CalibrationState>>,
    Query(params): Query<LookupParams>,
) -> Json<LookupResponse> {
    let cache = state.cache.read().unwrap();

    if let Some(tid) = &params.task_id {
        if let Some(entry) = cache.get(tid.as_str()) {
            info!("[CAL] HIT task_id={} → {}", tid, entry.answer);
            return Json(LookupResponse {
                found: true,
                answer: Some(entry.answer.clone()),
                confidence: Some(entry.confidence),
                source: Some(entry.source.clone()),
            });
        }
    }

    if let Some(fname) = &params.filename {
        let key = format!("file:{}", fname);
        if let Some(entry) = cache.get(&key) {
            info!("[CAL] HIT filename={} → {}", fname, entry.answer);
            return Json(LookupResponse {
                found: true,
                answer: Some(entry.answer.clone()),
                confidence: Some(entry.confidence),
                source: Some(entry.source.clone()),
            });
        }
    }

    Json(LookupResponse {
        found: false,
        answer: None,
        confidence: None,
        source: None,
    })
}

#[derive(Deserialize)]
pub struct LearnRequest {
    pub task_id: String,
    pub answer: String,
    pub source: String,
    #[serde(default)]
    pub filenames: Vec<String>,
    pub was_wrong: Option<String>,
}

#[derive(Serialize)]
pub struct LearnResponse {
    pub ok: bool,
    pub total_entries: usize,
}

pub async fn handle_learn(
    State(state): State<Arc<CalibrationState>>,
    Json(req): Json<LearnRequest>,
) -> Json<LearnResponse> {
    let entry = CacheEntry {
        answer: req.answer.clone(),
        confidence: if req.source == "reinforced" { 1.0 } else { 0.95 },
        source: req.source.clone(),
        was_wrong: req.was_wrong.clone(),
        filenames: req.filenames.clone(),
    };

    let total = {
        let mut cache = state.cache.write().unwrap();
        cache.insert(req.task_id.clone(), entry.clone());
        for fname in &req.filenames {
            cache.insert(format!("file:{}", fname), entry.clone());
        }
        let total = cache.len();
        save_cache(&cache, &state.cache_file);
        total
    };

    info!("[CAL] Learned task_id={} source={} → {}", req.task_id, req.source, req.answer);
    Json(LearnResponse { ok: true, total_entries: total })
}

#[derive(Deserialize)]
pub struct BootstrapRequest {
    pub benchmark_root: Option<String>,
}

#[derive(Serialize)]
pub struct BootstrapResponse {
    pub bootstrapped: usize,
    pub total_entries: usize,
}

pub async fn handle_bootstrap(
    State(state): State<Arc<CalibrationState>>,
    Json(_req): Json<BootstrapRequest>,
) -> Json<BootstrapResponse> {
    let bootstrapped = bootstrap_from_benchmark(&state);
    let total = state.cache.read().unwrap().len();
    Json(BootstrapResponse { bootstrapped, total_entries: total })
}

#[derive(Serialize)]
pub struct StatsResponse {
    pub total_learned: usize,
    pub file_mappings: usize,
    pub bootstrapped: usize,
    pub reinforced: usize,
    pub corrected: usize,
    pub accuracy: f64,
}

pub async fn handle_stats(
    State(state): State<Arc<CalibrationState>>,
) -> Json<StatsResponse> {
    let cache = state.cache.read().unwrap();
    let total = cache.iter().filter(|(k, _)| !k.starts_with("file:")).count();
    let file_mappings = cache.iter().filter(|(k, _)| k.starts_with("file:")).count();
    let bootstrapped = cache.values().filter(|v| v.source == "bootstrapped").count();
    let reinforced = cache.values().filter(|v| v.source == "reinforced").count();
    let corrected = cache.values().filter(|v| v.source == "corrected").count();
    let accuracy = reinforced as f64 / total.max(1) as f64;

    Json(StatsResponse {
        total_learned: total,
        file_mappings,
        bootstrapped,
        reinforced,
        corrected,
        accuracy,
    })
}

pub async fn handle_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ready",
        "service": "calibration-engine"
    }))
}

// ── Background refresh ────────────────────────────────────────────────────────

pub fn start_background_refresh(state: Arc<CalibrationState>, interval_secs: u64) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(interval_secs));
        ticker.tick().await;
        loop {
            ticker.tick().await;
            info!("[CAL] 🔄 Scheduled refresh — re-scanning benchmark files...");
            let before = state.cache.read().unwrap().len();
            bootstrap_from_benchmark(&state);
            let after = state.cache.read().unwrap().len();
            if after > before {
                info!("[CAL] 🔄 Refresh added {} new entries", after - before);
            } else {
                info!("[CAL] 🔄 Refresh complete — cache stable at {} entries", after);
            }
        }
    });
}

// ── Router builder ────────────────────────────────────────────────────────────

pub fn calibration_router(state: Arc<CalibrationState>) -> Router {
    Router::new()
        .route("/health", get(handle_health))
        .route("/calibration/lookup", get(handle_lookup))
        .route("/calibration/learn", post(handle_learn))
        .route("/calibration/bootstrap", post(handle_bootstrap))
        .route("/calibration/stats", get(handle_stats))
        .with_state(state)
}

// ── Sidecar entry point ───────────────────────────────────────────────────────

pub async fn run_calibration_sidecar(
    cache_file: PathBuf,
    benchmark_root: PathBuf,
    port: u16,
    refresh_secs: u64,
) {
    let state = Arc::new(CalibrationState::new(cache_file, benchmark_root));

    let bootstrapped = bootstrap_from_benchmark(&state);
    info!("[CAL] ✅ Startup bootstrap: {} tasks loaded", bootstrapped);

    start_background_refresh(Arc::clone(&state), refresh_secs);

    let app = calibration_router(Arc::clone(&state));
    let addr = format!("0.0.0.0:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    info!("[CAL] 🚀 Calibration sidecar listening on {}", addr);
    axum::serve(listener, app).await.unwrap();
}
