// src/tracks/mle_bench.rs
// ============================================================
// AgentX-Sprint2 — MLE-Bench Track (Research Agent)
//
// Green agent : RDI-Foundation/mle-bench-green
// Track       : Research Agent
// Domain      : Kaggle ML competitions (75 total)
//               Current: spaceship-titanic (baseline 0.503)
//               Target : beat median human score ~0.79
//
// Interface (from agent.py analysis):
//   1. Green agent sends: instructions (TextPart) + competition.tar.gz (FilePart)
//   2. Purple agent explores data, trains model, generates submission.csv
//   3. Purple agent sends "validate" message with submission FilePart
//   4. Green agent validates + grades via mlebench.grade_csv()
//   5. Purple agent sends final submission.csv as A2A artifact
//
// FBA Strategy:
//   - 49 models debate feature engineering approach
//   - Qwen3-Coder-480B leads code generation
//   - Claude Opus 4.6 anchors reasoning consistency
//   - DeepSeek R1 contributes ML domain expertise
//   - 39/49 quorum before any model/feature decision
// ============================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

// ─── Competition Registry ─────────────────────────────────────────────────────

/// Known Kaggle competitions in MLE-bench
/// Source: mlebench registry (75 competitions)
#[derive(Debug, Clone, PartialEq)]
pub enum KaggleCompetition {
    SpaceshipTitanic, // Binary classification — current leaderboard task
    TitanicSurvival,  // Classic binary classification
    HousePrices,      // Regression
    DigitRecognizer,  // MNIST image classification
    NLPDisaster,      // Text classification
    StoreSales,       // Time series forecasting
    Unknown(String),  // Future competitions
}

impl KaggleCompetition {
    pub fn from_id(id: &str) -> Self {
        match id {
            "spaceship-titanic" => Self::SpaceshipTitanic,
            "titanic" => Self::TitanicSurvival,
            "house-prices-advanced-regression-techniques" => Self::HousePrices,
            "digit-recognizer" => Self::DigitRecognizer,
            "nlp-getting-started" => Self::NLPDisaster,
            "store-sales-time-series-forecasting" => Self::StoreSales,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn task_type(&self) -> &'static str {
        match self {
            Self::SpaceshipTitanic | Self::TitanicSurvival | Self::NLPDisaster => {
                "binary_classification"
            }
            Self::HousePrices | Self::StoreSales => "regression",
            Self::DigitRecognizer => "multiclass_classification",
            Self::Unknown(_) => "unknown",
        }
    }

    pub fn target_column(&self) -> &'static str {
        match self {
            Self::SpaceshipTitanic => "Transported",
            Self::TitanicSurvival => "Survived",
            Self::HousePrices => "SalePrice",
            Self::DigitRecognizer => "Label",
            Self::NLPDisaster => "target",
            Self::StoreSales => "sales",
            Self::Unknown(_) => "target",
        }
    }

    pub fn evaluation_metric(&self) -> &'static str {
        match self {
            Self::SpaceshipTitanic | Self::TitanicSurvival | Self::NLPDisaster => "accuracy",
            Self::HousePrices => "rmse_log",
            Self::DigitRecognizer => "accuracy",
            Self::StoreSales => "rmsle",
            Self::Unknown(_) => "accuracy",
        }
    }

    /// Known baseline scores to beat
    pub fn baseline_score(&self) -> f64 {
        match self {
            Self::SpaceshipTitanic => 0.503, // Current MLE-bench leaderboard entry
            Self::TitanicSurvival => 0.765,  // Typical LLM baseline
            Self::HousePrices => 0.15,       // RMSE log
            Self::DigitRecognizer => 0.990,
            Self::NLPDisaster => 0.780,
            Self::StoreSales => 0.5,
            Self::Unknown(_) => 0.5,
        }
    }

    /// Target score (above median Kaggle human)
    pub fn target_score(&self) -> f64 {
        match self {
            Self::SpaceshipTitanic => 0.81, // Medal territory
            Self::TitanicSurvival => 0.82,
            Self::HousePrices => 0.12, // Lower RMSE is better
            Self::DigitRecognizer => 0.995,
            Self::NLPDisaster => 0.83,
            Self::StoreSales => 0.4,
            Self::Unknown(_) => 0.75,
        }
    }
}

// ─── Task State ───────────────────────────────────────────────────────────────

/// Tracks MLE-bench evaluation pipeline state
#[derive(Debug, Clone)]
pub struct MleBenchState {
    pub competition_id: String,
    pub competition: KaggleCompetition,
    pub phase: MlePhase,
    pub data_explored: bool,
    pub features_chosen: Vec<String>,
    pub model_chosen: Option<String>,
    pub iterations: u32,
    pub best_cv_score: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MlePhase {
    DataExploration,
    FeatureEngineering,
    ModelSelection,
    Training,
    Validation,
    Submission,
}

impl MleBenchState {
    pub fn new(competition_id: &str) -> Self {
        let competition = KaggleCompetition::from_id(competition_id);
        Self {
            competition_id: competition_id.to_string(),
            competition,
            phase: MlePhase::DataExploration,
            data_explored: false,
            features_chosen: Vec::new(),
            model_chosen: None,
            iterations: 0,
            best_cv_score: None,
        }
    }
}

// ─── Message Types ────────────────────────────────────────────────────────────

/// Incoming message from MLE-bench green agent
#[derive(Debug, Clone)]
pub struct MleMessage {
    pub instructions: String,
    pub competition_tar: Option<Vec<u8>>, // base64-decoded .tar.gz
    pub validation_msg: Option<String>,   // validation result from green agent
    pub is_validation: bool,
}

/// Response from purple agent to green agent
#[derive(Serialize, Debug, Clone)]
pub struct MleResponse {
    /// Message text to green agent
    pub message: String,
    /// Submission CSV bytes (when ready)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submission_csv: Option<Vec<u8>>,
    /// Whether this is a validation request
    pub is_validation_request: bool,
}

// ─── ML Pipeline Steps ────────────────────────────────────────────────────────

/// A complete ML solution step decided by FBA consensus
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MlPipelineStep {
    pub step_type: String, // "explore" | "features" | "model" | "train" | "submit"
    pub description: String,
    pub code: String,      // Python code to execute
    pub rationale: String, // FBA reasoning
}

// ─── Core Processing ──────────────────────────────────────────────────────────

/// Main entry: process MLE-bench A2A message → FBA → MleResponse  
pub async fn process_mle_turn(
    text_parts: &[String],
    file_parts: &[serde_json::Value],
    fba_endpoint: &str,
    jwt_secret: &str,
    context_id: &str,
) -> MleResponse {
    info!(
        "🔬 MLE-bench turn received: {} text parts, {} file parts",
        text_parts.len(),
        file_parts.len()
    );

    // Extract instructions and competition data
    let instructions = text_parts.join("\n");
    let has_tar = file_parts
        .iter()
        .any(|f| f["name"].as_str().unwrap_or("").ends_with(".tar.gz"));

    // Detect phase from instructions
    let phase = detect_phase(&instructions);
    info!("📊 MLE phase detected: {:?}", phase);

    // Parse competition ID from instructions
    let competition_id =
        extract_competition_id(&instructions).unwrap_or_else(|| "spaceship-titanic".to_string());
    let state = MleBenchState::new(&competition_id);

    info!(
        "🏆 Competition: {} (type: {}, metric: {})",
        competition_id,
        state.competition.task_type(),
        state.competition.evaluation_metric()
    );

    // Build FBA prompt based on phase
    let fba_prompt = build_mle_prompt(&instructions, &state, &phase, has_tar);

    // Call FBA pipeline
    let raw = call_fba_pipeline(fba_prompt, fba_endpoint, jwt_secret, context_id).await;

    // Parse response
    parse_mle_response(&raw, &state, &phase)
}

// ─── Phase Detection ──────────────────────────────────────────────────────────

fn detect_phase(instructions: &str) -> MlePhase {
    let lower = instructions.to_lowercase();
    if lower.contains("validate") || lower.contains("validation") {
        MlePhase::Validation
    } else if lower.contains("submit") || lower.contains("submission.csv") {
        MlePhase::Submission
    } else if lower.contains("train") || lower.contains("fit") {
        MlePhase::Training
    } else if lower.contains("feature") || lower.contains("engineer") {
        MlePhase::FeatureEngineering
    } else if lower.contains("model") || lower.contains("algorithm") {
        MlePhase::ModelSelection
    } else {
        MlePhase::DataExploration
    }
}

fn extract_competition_id(instructions: &str) -> Option<String> {
    // Look for competition ID patterns
    let patterns = [
        "spaceship-titanic",
        "titanic",
        "house-prices",
        "digit-recognizer",
        "nlp-getting-started",
        "store-sales",
    ];
    for pattern in &patterns {
        if instructions.contains(pattern) {
            return Some(pattern.to_string());
        }
    }
    // Look for competition_id in JSON-like content
    if let Some(start) = instructions.find("competition_id") {
        let slice = &instructions[start..];
        if let Some(id_start) = slice.find('"') {
            let after = &slice[id_start + 1..];
            if let Some(id_end) = after.find('"') {
                return Some(after[..id_end].to_string());
            }
        }
    }
    None
}

// ─── FBA Prompt Builder ───────────────────────────────────────────────────────

fn build_mle_prompt(
    instructions: &str,
    state: &MleBenchState,
    phase: &MlePhase,
    has_data_tar: bool,
) -> String {
    let phase_guidance = match phase {
        MlePhase::DataExploration => {
            r#"
PHASE: DATA EXPLORATION
  1. Load train.csv and test.csv from /home/data/
  2. Examine shape, dtypes, missing values, target distribution
  3. Identify numerical, categorical, and text features
  4. Check for class imbalance
  5. Report key statistics that will guide feature engineering
  
  Generate Python code to explore the data comprehensively.
  Use pandas, numpy, and standard libraries only."#
        }

        MlePhase::FeatureEngineering => {
            r#"
PHASE: FEATURE ENGINEERING
  1. Handle missing values (impute, not drop)
  2. Encode categorical features (LabelEncoder or OneHotEncoder)
  3. Create interaction features if domain-relevant
  4. Scale numerical features if needed
  5. Generate feature importance estimates
  
  Prioritize features that maximize signal for the target variable.
  Use scikit-learn preprocessing pipeline."#
        }

        MlePhase::ModelSelection => {
            r#"
PHASE: MODEL SELECTION (FBA Consensus)
  Vote on best model for this task type.
  Consider: RandomForest, GradientBoosting, XGBoost, LightGBM, LogisticRegression
  
  For classification: prefer ensemble methods
  For regression: prefer gradient boosting
  Use 5-fold cross-validation to evaluate.
  Report CV score before selecting."#
        }

        MlePhase::Training => {
            r#"
PHASE: MODEL TRAINING
  1. Train selected model on full training data
  2. Use cross-validation to estimate generalization
  3. Tune top 3 hyperparameters only (time budget)
  4. Generate predictions on test set
  5. Format predictions as competition requires
  
  Generate complete, runnable Python code."#
        }

        MlePhase::Validation => {
            r#"
PHASE: SUBMISSION VALIDATION
  The green agent has returned a validation result.
  Analyze the result and either:
  - If valid: proceed to final submission
  - If invalid: fix the submission format issues
  
  Common issues: wrong column names, wrong prediction format,
  missing PassengerId, wrong number of rows."#
        }

        MlePhase::Submission => {
            r#"
PHASE: FINAL SUBMISSION
  Generate the final submission.csv with:
  - Correct ID column (e.g., PassengerId)
  - Correct target column (e.g., Transported)
  - Correct format (True/False for classification, float for regression)
  - Correct number of rows (match test set exactly)
  
  Send as FilePart with name "submission.csv"."#
        }
    };

    format!(
        r#"
MLE-BENCH — KAGGLE COMPETITION AGENT
=====================================
Competition : {competition_id}
Task Type   : {task_type}
Target Col  : {target}
Metric      : {metric}
Baseline    : {baseline:.3} (current best on leaderboard)
Our Target  : {target_score:.3} (above median human)
Data Ready  : {has_data}

INSTRUCTIONS FROM GREEN AGENT:
{instructions}

{phase_guidance}

FBA STRATEGY (49-MODEL CONSENSUS):
  - DeepSeek R1-0528    : ML domain expertise, algorithm selection
  - Qwen3-Coder-480B    : Code generation, implementation
  - Claude Opus 4.6     : Reasoning anchor, prevents hallucinated scores
  - Kimi-K2-Thinking    : Feature engineering creativity
  - 45 additional models: Vote on approach
  - Quorum: 39/49 @ 94% confidence before any decision

ANTI-HALLUCINATION RULES:
  1. NEVER invent CV scores — only report actual computed scores
  2. NEVER claim model achieved X% without running the code
  3. NEVER skip validation step before submission
  4. If uncertain about data format — explore first, claim second
  5. Code must be complete and runnable — no pseudocode

SPACESHIP TITANIC SPECIFIC KNOWLEDGE (if applicable):
  Key features: HomePlanet, CryoSleep, Cabin, Destination, Age, VIP
  + Amenity spend features: RoomService, FoodCourt, ShoppingMall, Spa, VRDeck
  Target: Transported (bool → True/False in submission)
  Insight: CryoSleep strongly predicts Transported (FBA consensus from prior runs)
  Cabin: split into Deck/Num/Side for better signal
  Group: extract from PassengerId (GroupId_PersonNum)

REQUIRED OUTPUT FORMAT:
Generate a complete Python ML pipeline as a code block.
Then state your recommendation in JSON:
{{
  "phase": "{phase_name}",
  "action": "explore|train|validate|submit",
  "code": "python code here",
  "cv_score": null_or_float,
  "ready_to_submit": true_or_false,
  "message": "explanation for green agent"
}}

GENERATE YOUR ML SOLUTION NOW:
"#,
        competition_id = state.competition_id,
        task_type = state.competition.task_type(),
        target = state.competition.target_column(),
        metric = state.competition.evaluation_metric(),
        baseline = state.competition.baseline_score(),
        target_score = state.competition.target_score(),
        has_data = if has_data_tar {
            "YES (tar.gz received)"
        } else {
            "Pending"
        },
        instructions = &instructions[..instructions.len().min(500)],
        phase_guidance = phase_guidance,
        phase_name = format!("{:?}", phase).to_lowercase(),
    )
}

// ─── FBA Pipeline Call ────────────────────────────────────────────────────────

async fn call_fba_pipeline(
    prompt: String,
    fba_endpoint: &str,
    jwt_secret: &str,
    context_id: &str,
) -> serde_json::Value {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("HTTP client failed");

    let token = mint_jwt(jwt_secret, "mle_bench");

    match client
        .post(format!("{}/modernize", fba_endpoint))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "cobol_code": prompt,
            "context_id": context_id,
            "track":      "mle_bench",
        }))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r.json().await.unwrap_or_default(),
        Ok(r) => {
            warn!("FBA HTTP {}", r.status());
            serde_json::json!({})
        }
        Err(e) => {
            warn!("FBA error: {}", e);
            serde_json::json!({})
        }
    }
}

// ─── Response Parser ──────────────────────────────────────────────────────────

fn parse_mle_response(
    raw: &serde_json::Value,
    state: &MleBenchState,
    phase: &MlePhase,
) -> MleResponse {
    let quorum = raw["consensus_nodes"].as_u64().unwrap_or(0);
    let confidence = raw["confidence"].as_f64().unwrap_or(0.0);

    if quorum < 39 || confidence < 0.94 {
        warn!(
            "MLE-bench FBA quorum not reached: {}/49 @ {:.1}%",
            quorum,
            confidence * 100.0
        );
        return safe_mle_response(state, phase);
    }

    let response_text = raw["rust_code"]
        .as_str()
        .or_else(|| raw["response"].as_str())
        .unwrap_or("");

    // Extract JSON decision block
    let json_str = extract_json(response_text);
    let code_block = extract_code_block(response_text);

    let parsed = serde_json::from_str::<serde_json::Value>(&json_str).unwrap_or_default();

    let action = parsed["action"].as_str().unwrap_or("explore");
    let message = parsed["message"]
        .as_str()
        .unwrap_or("Processing ML pipeline...")
        .to_string();
    let ready = parsed["ready_to_submit"].as_bool().unwrap_or(false);
    let cv = parsed["cv_score"].as_f64();

    if let Some(score) = cv {
        info!(
            "📈 CV Score reported: {:.4} (target: {:.3})",
            score,
            state.competition.target_score()
        );
    }

    // Determine if this is a validation request
    let is_validation = action == "validate" || message.to_lowercase().contains("validate");

    // Build full response message including code
    let full_message = if code_block.is_empty() {
        message.clone()
    } else {
        format!("{}\n\n```python\n{}\n```", message, code_block)
    };

    info!(
        "✅ MLE-bench response: action={}, validation={}, ready={}",
        action, is_validation, ready
    );

    MleResponse {
        message: full_message,
        submission_csv: None, // Set by submission handler
        is_validation_request: is_validation,
    }
}

/// Generate safe fallback ML response
fn safe_mle_response(state: &MleBenchState, phase: &MlePhase) -> MleResponse {
    let (message, is_validation) = match phase {
        MlePhase::DataExploration => (
            format!(
                "Exploring {} competition data.\n\n```python\nimport pandas as pd\nimport numpy as np\n\ntrain = pd.read_csv('/home/data/train.csv')\ntest  = pd.read_csv('/home/data/test.csv')\n\nprint('Train shape:', train.shape)\nprint('Test shape:', test.shape)\nprint('\\nMissing values:\\n', train.isnull().sum())\nprint('\\nTarget distribution:\\n', train['{}'].value_counts())\nprint('\\nData types:\\n', train.dtypes)\n```",
                state.competition_id,
                state.competition.target_column()
            ),
            false
        ),
        MlePhase::Training => (
            format!(
                "Training model for {} competition.\n\n```python\nimport pandas as pd\nimport numpy as np\nfrom sklearn.ensemble import GradientBoostingClassifier\nfrom sklearn.model_selection import cross_val_score\nfrom sklearn.preprocessing import LabelEncoder\n\ntrain = pd.read_csv('/home/data/train.csv')\ntest  = pd.read_csv('/home/data/test.csv')\n\n# Basic preprocessing\nfor col in train.select_dtypes('object').columns:\n    if col != '{}':\n        le = LabelEncoder()\n        train[col] = le.fit_transform(train[col].astype(str))\n        test[col]  = le.transform(test[col].astype(str))\n\nfeatures = [c for c in train.columns if c not in ['{}', 'PassengerId']]\nX = train[features].fillna(0)\ny = train['{}']\n\nmodel = GradientBoostingClassifier(n_estimators=200, random_state=42)\ncv_scores = cross_val_score(model, X, y, cv=5)\nprint(f'CV Score: {{cv_scores.mean():.4f}} +/- {{cv_scores.std():.4f}}')\n\nmodel.fit(X, y)\npreds = model.predict(test[features].fillna(0))\n```",
                state.competition_id,
                state.competition.target_column(),
                state.competition.target_column(),
                state.competition.target_column()
            ),
            false
        ),
        MlePhase::Validation => (
            "Validation result received. Checking submission format...".to_string(),
            true
        ),
        _ => (
            format!("Processing {} competition, phase: {:?}", state.competition_id, phase),
            false
        ),
    };

    MleResponse {
        message,
        submission_csv: None,
        is_validation_request: is_validation,
    }
}

// ─── Submission Builder ───────────────────────────────────────────────────────

/// Build submission CSV content for Spaceship Titanic
/// FBA consensus on predictions → format as required
pub fn build_spaceship_titanic_submission(test_ids: &[String], predictions: &[bool]) -> String {
    let mut csv = String::from("PassengerId,Transported\n");
    for (id, &pred) in test_ids.iter().zip(predictions.iter()) {
        csv.push_str(&format!("{},{}\n", id, pred));
    }
    csv
}

/// Validate submission format before sending
pub fn validate_submission_format(
    csv_content: &str,
    competition: &KaggleCompetition,
    expected_rows: usize,
) -> Result<(), String> {
    let lines: Vec<&str> = csv_content.lines().collect();

    if lines.is_empty() {
        return Err("Submission is empty".to_string());
    }

    // Check header
    let header = lines[0];
    let target = competition.target_column();
    if !header.contains(target) {
        return Err(format!(
            "Header missing target column '{}'. Got: {}",
            target, header
        ));
    }

    // Check row count
    let data_rows = lines.len() - 1; // exclude header
    if data_rows != expected_rows {
        return Err(format!(
            "Wrong number of rows: got {} expected {}",
            data_rows, expected_rows
        ));
    }

    // Check no empty predictions
    for (i, line) in lines[1..].iter().enumerate() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 2 || parts[1].trim().is_empty() {
            return Err(format!("Empty prediction at row {}", i + 1));
        }
    }

    Ok(())
}

// ─── A2A Format Helper ────────────────────────────────────────────────────────

pub fn format_tool_call(
    name: &str,
    arguments: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "tool_call": {
            "name":      name,
            "arguments": arguments,
            "domain":    "ml_engineering"
        }
    })
}

/// Build A2A artifact for submission CSV
pub fn build_submission_artifact(csv_bytes: &[u8]) -> serde_json::Value {
    use base64::{engine::general_purpose, Engine as _};
    let encoded = general_purpose::STANDARD.encode(csv_bytes);
    serde_json::json!({
        "type": "file",
        "file": {
            "name":      "submission.csv",
            "mime_type": "text/csv",
            "bytes":     encoded
        }
    })
}

// ─── Utilities ────────────────────────────────────────────────────────────────

fn extract_json(text: &str) -> String {
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end >= start {
                return text[start..=end].to_string();
            }
        }
    }
    "{}".to_string()
}

fn extract_code_block(text: &str) -> String {
    if let Some(start) = text.find("```python") {
        let after = &text[start + 9..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    if let Some(start) = text.find("```") {
        let after = &text[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim().to_string();
        }
    }
    String::new()
}

fn mint_jwt(secret: &str, track: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let h = b64(r#"{"alg":"HS256","typ":"JWT"}"#);
    let p = b64(&format!(
        r#"{{"sub":"agentx-sprint2","role":"purple_agent","track":"{}","exp":{}}}"#,
        track, exp
    ));
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    format!("{}.{}{}", h, p, secret).hash(&mut hasher);
    format!("{}.{}.{}", h, p, b64(&format!("{:x}", hasher.finish())))
}

fn b64(s: &str) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut r = String::new();
    for ch in s.as_bytes().chunks(3) {
        let b0 = ch[0] as usize;
        let b1 = if ch.len() > 1 { ch[1] as usize } else { 0 };
        let b2 = if ch.len() > 2 { ch[2] as usize } else { 0 };
        r.push(C[(b0 >> 2) & 0x3F] as char);
        r.push(C[((b0 << 4) | (b1 >> 4)) & 0x3F] as char);
        if ch.len() > 1 {
            r.push(C[((b1 << 2) | (b2 >> 6)) & 0x3F] as char);
        }
        if ch.len() > 2 {
            r.push(C[b2 & 0x3F] as char);
        }
    }
    r
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_competition_from_id() {
        assert_eq!(
            KaggleCompetition::from_id("spaceship-titanic"),
            KaggleCompetition::SpaceshipTitanic
        );
        assert_eq!(
            KaggleCompetition::from_id("titanic"),
            KaggleCompetition::TitanicSurvival
        );
    }

    #[test]
    fn test_spaceship_titanic_properties() {
        let c = KaggleCompetition::SpaceshipTitanic;
        assert_eq!(c.task_type(), "binary_classification");
        assert_eq!(c.target_column(), "Transported");
        assert_eq!(c.evaluation_metric(), "accuracy");
        assert!(
            c.baseline_score() < c.target_score(),
            "Target must exceed baseline"
        );
    }

    #[test]
    fn test_submission_builder() {
        let ids = vec!["001_01".to_string(), "002_01".to_string()];
        let preds = vec![true, false];
        let csv = build_spaceship_titanic_submission(&ids, &preds);
        assert!(csv.starts_with("PassengerId,Transported\n"));
        assert!(csv.contains("001_01,true"));
        assert!(csv.contains("002_01,false"));
    }

    #[test]
    fn test_submission_validation_correct() {
        let csv = "PassengerId,Transported\n001_01,True\n002_01,False\n";
        let result = validate_submission_format(csv, &KaggleCompetition::SpaceshipTitanic, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_submission_validation_wrong_rows() {
        let csv = "PassengerId,Transported\n001_01,True\n";
        let result = validate_submission_format(csv, &KaggleCompetition::SpaceshipTitanic, 5);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Wrong number of rows"));
    }

    #[test]
    fn test_submission_validation_missing_target() {
        let csv = "PassengerId,WrongColumn\n001_01,True\n";
        let result = validate_submission_format(csv, &KaggleCompetition::SpaceshipTitanic, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing target column"));
    }

    #[test]
    fn test_phase_detection() {
        assert_eq!(
            detect_phase("please validate submission"),
            MlePhase::Validation
        );
        assert_eq!(
            detect_phase("generate submission.csv"),
            MlePhase::Submission
        );
        assert_eq!(detect_phase("train the model"), MlePhase::Training);
        assert_eq!(
            detect_phase("explore the dataset"),
            MlePhase::DataExploration
        );
    }

    #[test]
    fn test_competition_id_extraction() {
        let instructions = "Run MLE-bench for spaceship-titanic competition";
        assert_eq!(
            extract_competition_id(instructions),
            Some("spaceship-titanic".to_string())
        );
    }

    #[test]
    fn test_prompt_contains_key_sections() {
        let state = MleBenchState::new("spaceship-titanic");
        let prompt = build_mle_prompt("explore data", &state, &MlePhase::DataExploration, true);
        assert!(prompt.contains("FBA STRATEGY"));
        assert!(prompt.contains("ANTI-HALLUCINATION"));
        assert!(prompt.contains("SPACESHIP TITANIC"));
        assert!(prompt.contains("CryoSleep"));
    }

    #[test]
    fn test_code_block_extraction() {
        let text = "Here is code:\n```python\nprint('hello')\n```\nDone.";
        let code = extract_code_block(text);
        assert_eq!(code, "print('hello')");
    }
}
