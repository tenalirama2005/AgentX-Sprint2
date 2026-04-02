// src/tracks/tau2.rs
// ============================================================
// AgentX-Sprint2 — τ²-Bench Track (Telecom Domain)
//
// Green agent : RDI-Foundation/tau2-agentbeats
// Domain      : telecom/account management, device troubleshooting,
//               billing, plan changes, connectivity issues
// Protocol    : A2A — Tool-Agent-User (TAU) interaction
// Scoring     : Pass rate (task solved in all runs)
//
// Current leaderboard: 2 entries (Gemini 3 Pro @ 68% and 18%)
// FBA target: >90% via 39/49 quorum @ 94% confidence
//
// Key insight: τ²-Bench is DUAL-CONTROL — both agent AND user
// can take tool actions in a shared environment (Dec-POMDP).
// FBA consensus on each turn = near-deterministic pass rate.
// ============================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

// ─── Telecom Domain Tool Definitions ─────────────────────────────────────────

/// All telecom tools the agent can call in τ²-Bench
/// Sourced from tau2-bench telecom domain spec
#[derive(Debug, Clone, PartialEq)]
pub enum TelecomTool {
    // Account management
    GetAccountInfo,
    GetDataUsage,
    GetBillingInfo,
    GetCurrentPlan,
    ChangePlan {
        plan_id: String,
    },
    // Data management
    RefuelData {
        amount_gb: f64,
    },
    ToggleMobileData {
        enabled: bool,
    },
    ToggleRoaming {
        enabled: bool,
    },
    // Device troubleshooting
    CheckNetworkStatus,
    CheckDeviceSettings,
    ResetNetworkSettings,
    // Communication
    SendSmsConfirmation {
        message: String,
    },
    // Generic tool call
    Generic {
        name: String,
        args: HashMap<String, serde_json::Value>,
    },
}

impl TelecomTool {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::GetAccountInfo => serde_json::json!({
                "name": "get_account_info", "arguments": {}
            }),
            Self::GetDataUsage => serde_json::json!({
                "name": "get_data_usage", "arguments": {}
            }),
            Self::GetBillingInfo => serde_json::json!({
                "name": "get_billing_info", "arguments": {}
            }),
            Self::GetCurrentPlan => serde_json::json!({
                "name": "get_current_plan", "arguments": {}
            }),
            Self::ChangePlan { plan_id } => serde_json::json!({
                "name": "change_plan",
                "arguments": { "plan_id": plan_id }
            }),
            Self::RefuelData { amount_gb } => serde_json::json!({
                "name": "refuel_data",
                "arguments": { "amount_gb": amount_gb }
            }),
            Self::ToggleMobileData { enabled } => serde_json::json!({
                "name": "toggle_mobile_data",
                "arguments": { "enabled": enabled }
            }),
            Self::ToggleRoaming { enabled } => serde_json::json!({
                "name": "toggle_roaming",
                "arguments": { "enabled": enabled }
            }),
            Self::CheckNetworkStatus => serde_json::json!({
                "name": "check_network_status", "arguments": {}
            }),
            Self::CheckDeviceSettings => serde_json::json!({
                "name": "check_device_settings", "arguments": {}
            }),
            Self::ResetNetworkSettings => serde_json::json!({
                "name": "reset_network_settings", "arguments": {}
            }),
            Self::SendSmsConfirmation { message } => serde_json::json!({
                "name": "send_sms_confirmation",
                "arguments": { "message": message }
            }),
            Self::Generic { name, args } => serde_json::json!({
                "name": name,
                "arguments": args
            }),
        }
    }
}

// ─── Conversation Types ───────────────────────────────────────────────────────

/// A single turn in the TAU conversation
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TauTurn {
    pub role: String, // "user" | "agent" | "tool"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<serde_json::Value>,
}

/// Full τ²-Bench task observation sent to purple agent each turn
#[derive(Deserialize, Debug, Clone)]
pub struct Tau2Observation {
    /// Domain: "telecom" | "airline" | "retail"
    pub domain: String,

    /// Current conversation history (TAU turns)
    pub conversation: Vec<TauTurn>,

    /// Available tools for this domain (JSON schema)
    pub available_tools: Vec<serde_json::Value>,

    /// Domain policy document (plain text)
    pub policy: String,

    /// Task description (what user wants to achieve)
    #[serde(default)]
    pub task_description: String,

    /// Whether this is the final turn
    #[serde(default)]
    pub is_final: bool,
}

/// Response from purple agent to τ²-Bench green agent
#[derive(Serialize, Debug, Clone)]
pub struct Tau2Response {
    /// Natural language response to user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,

    /// Tool call to execute (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call: Option<Tau2ToolCall>,
}

#[derive(Serialize, Debug, Clone)]
pub struct Tau2ToolCall {
    pub name: String,
    pub arguments: HashMap<String, serde_json::Value>,
}

// ─── Conversation State ───────────────────────────────────────────────────────

/// Tracks full task state for multi-turn coordination
#[derive(Debug, Clone)]
pub struct Tau2State {
    pub domain: String,
    pub turns_taken: u32,
    pub tools_called: Vec<String>,
    pub user_issue: Option<String>,
    pub issue_resolved: bool,
    pub pending_action: Option<String>,
    pub user_constraints: Vec<String>,
}

impl Tau2State {
    pub fn new(domain: &str) -> Self {
        Self {
            domain: domain.to_string(),
            turns_taken: 0,
            tools_called: Vec::new(),
            user_issue: None,
            issue_resolved: false,
            pending_action: None,
            user_constraints: Vec::new(),
        }
    }

    /// Extract user issue from conversation history
    pub fn analyze_conversation(&mut self, obs: &Tau2Observation) {
        self.turns_taken = obs.conversation.len() as u32;

        // Find last user message to understand current issue
        if let Some(last_user) = obs.conversation.iter().rfind(|t| t.role == "user") {
            self.user_issue = Some(last_user.content.clone());

            // Extract constraints from user messages
            let content_lower = last_user.content.to_lowercase();
            if content_lower.contains("not willing") || content_lower.contains("don't want") {
                self.user_constraints.push(last_user.content.clone());
            }
        }

        // Track what tools have been called
        self.tools_called = obs
            .conversation
            .iter()
            .filter(|t| t.role == "tool")
            .filter_map(|t| t.tool_name.clone())
            .collect();
    }

    /// Determine if issue appears resolved based on conversation
    pub fn check_resolution(&mut self, obs: &Tau2Observation) {
        let last_few: Vec<&TauTurn> = obs.conversation.iter().rev().take(4).collect();

        for turn in last_few {
            let content_lower = turn.content.to_lowercase();
            if content_lower.contains("resolved")
                || content_lower.contains("working now")
                || content_lower.contains("thank you")
                || content_lower.contains("that's all")
            {
                self.issue_resolved = true;
                break;
            }
        }
    }
}

// ─── Core Processing ──────────────────────────────────────────────────────────

/// Main entry: process τ²-Bench A2A turn → FBA consensus → Tau2Response
pub async fn process_tau2_turn(
    obs_json: &serde_json::Value,
    fba_endpoint: &str,
    jwt_secret: &str,
    context_id: &str,
) -> Tau2Response {
    // 1. Parse observation
    let obs: Tau2Observation = match serde_json::from_value(obs_json.clone()) {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to parse τ²-Bench observation: {}", e);
            return Tau2Response {
                message: Some(
                    "I'm having trouble processing your request. Could you please clarify?".into(),
                ),
                tool_call: None,
            };
        }
    };

    info!(
        "📡 τ²-Bench turn: domain={}, turns={}, tools_available={}",
        obs.domain,
        obs.conversation.len(),
        obs.available_tools.len()
    );

    // 2. Analyze conversation state
    let mut state = Tau2State::new(&obs.domain);
    state.analyze_conversation(&obs);
    state.check_resolution(&obs);

    // 3. If issue resolved — send closing message
    if state.issue_resolved && obs.is_final {
        info!("✅ Issue resolved — closing conversation");
        return Tau2Response {
            message:   Some("I'm glad I could help resolve your issue. Is there anything else I can assist you with today?".into()),
            tool_call: None,
        };
    }

    // 4. Build FBA prompt
    let fba_prompt = build_tau2_prompt(&obs, &state);

    // 5. Call FBA pipeline
    let raw = call_fba_pipeline(fba_prompt, fba_endpoint, jwt_secret, context_id).await;

    // 6. Parse FBA response → Tau2Response
    let response = parse_fba_tau2_response(&raw, &obs, &state);

    // 7. Validate response against domain policy
    validate_response(response, &obs, &state)
}

// ─── FBA Prompt Builder ───────────────────────────────────────────────────────

fn build_tau2_prompt(obs: &Tau2Observation, state: &Tau2State) -> String {
    // Format conversation history
    let history = obs
        .conversation
        .iter()
        .map(|turn| match turn.role.as_str() {
            "user" => format!("USER: {}", turn.content),
            "agent" => format!("AGENT: {}", turn.content),
            "tool" => format!(
                "TOOL_RESULT [{}]: {}",
                turn.tool_name.as_deref().unwrap_or("unknown"),
                turn.content
            ),
            _ => format!("[{}]: {}", turn.role, turn.content),
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Format available tools
    let tools_list = obs
        .available_tools
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect::<Vec<_>>()
        .join(", ");

    // Format user constraints
    let constraints = if state.user_constraints.is_empty() {
        "None identified".to_string()
    } else {
        state.user_constraints.join("; ")
    };

    // Tools already called — avoid redundant calls
    let called = if state.tools_called.is_empty() {
        "None yet".to_string()
    } else {
        state.tools_called.join(", ")
    };

    format!(
        r#"
TAU2-BENCH — TELECOM CUSTOMER SERVICE AGENT
============================================
Domain   : {domain}
Turn     : {turns}
Resolved : {resolved}

DOMAIN POLICY (MUST FOLLOW):
{policy}

CONVERSATION HISTORY:
{history}

CURRENT USER ISSUE:
{issue}

USER CONSTRAINTS (DO NOT VIOLATE):
{constraints}

AVAILABLE TOOLS: {tools}
TOOLS ALREADY CALLED: {called}

DUAL-CONTROL ENVIRONMENT RULES:
  - Both you (agent) AND the user can take tool actions
  - User has their own phone/device tools
  - You have account/system tools
  - Coordinate clearly: tell user exactly what action to take on their end
  - You handle the backend; user handles their device

AGENT DECISION FRAMEWORK:
  Step 1: DIAGNOSE
    - What is the user's core problem?
    - What information do you still need?
    - Which tools can provide that information?

  Step 2: COORDINATE
    - What does the USER need to do on their device?
    - What do YOU need to do on the backend system?
    - Sequence matters: often user action → verify → agent action

  Step 3: POLICY COMPLIANCE
    - Check all constraints in the domain policy above
    - Honor user preferences and constraints
    - Do not make changes user explicitly rejected

  Step 4: ANTI-HALLUCINATION (FBA GUARANTEE)
    - Only state facts confirmed by tool results
    - If unsure, call a tool to verify first
    - Never invent account details, plan names, or data amounts
    - If tool not available for what user needs → admit limitation

  Step 5: COMMUNICATION
    - Be concise but complete
    - Give user specific actionable instructions
    - Confirm actions before making irreversible changes
    - Ask ONE clarifying question at a time if needed

REQUIRED OUTPUT FORMAT (JSON):
Choose ONE response type:

Option A — Text response to user:
{{"message": "Your response here"}}

Option B — Tool call:
{{"tool_call": {{"name": "tool_name", "arguments": {{...}}}}}}

Option C — Both (respond AND call tool):
{{"message": "Checking that for you...", "tool_call": {{"name": "get_data_usage", "arguments": {{}}}}}}

GENERATE YOUR RESPONSE NOW:
"#,
        domain = obs.domain,
        turns = state.turns_taken,
        resolved = state.issue_resolved,
        policy = if obs.policy.is_empty() {
            "Standard telecom service policy applies."
        } else {
            &obs.policy
        },
        history = if history.is_empty() {
            "(conversation just started)".to_string()
        } else {
            history
        },
        issue = state.user_issue.as_deref().unwrap_or("Not yet identified"),
        constraints = constraints,
        tools = tools_list,
        called = called,
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
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .expect("HTTP client build failed");

    let token = mint_jwt(jwt_secret, "tau2_bench");

    match client
        .post(format!("{}/modernize", fba_endpoint))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "cobol_code": prompt,
            "context_id": context_id,
            "track":      "tau2_bench",
        }))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            resp.json().await.unwrap_or_else(|_| serde_json::json!({}))
        }
        Ok(resp) => {
            warn!("FBA pipeline HTTP {}", resp.status());
            serde_json::json!({})
        }
        Err(e) => {
            warn!("FBA pipeline error: {}", e);
            serde_json::json!({})
        }
    }
}

// ─── FBA Response Parser ──────────────────────────────────────────────────────

fn parse_fba_tau2_response(
    raw: &serde_json::Value,
    obs: &Tau2Observation,
    state: &Tau2State,
) -> Tau2Response {
    // Check FBA quorum
    let quorum = raw["consensus_nodes"].as_u64().unwrap_or(0);
    let confidence = raw["confidence"].as_f64().unwrap_or(0.0);

    if quorum < 39 || confidence < 0.94 {
        warn!(
            "τ²-Bench FBA quorum not reached: {}/49 @ {:.1}% — safe fallback",
            quorum,
            confidence * 100.0
        );
        return safe_fallback_response(obs, state);
    }

    // Extract response text
    let response_text = raw["rust_code"]
        .as_str()
        .or_else(|| raw["response"].as_str())
        .unwrap_or("");

    // Find JSON in response
    let json_str = extract_json(response_text);

    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(parsed) => {
            let message = parsed["message"].as_str().map(|s| s.to_string());
            let tool_call = parse_tool_call(&parsed);

            info!(
                "✅ τ²-Bench FBA response: message={}, tool_call={}",
                message.is_some(),
                tool_call.is_some()
            );

            Tau2Response { message, tool_call }
        }
        Err(e) => {
            warn!(
                "Failed to parse τ²-Bench FBA JSON: {} | raw: {}",
                e, json_str
            );
            safe_fallback_response(obs, state)
        }
    }
}

fn parse_tool_call(parsed: &serde_json::Value) -> Option<Tau2ToolCall> {
    let tc = parsed.get("tool_call")?;
    let name = tc["name"].as_str()?.to_string();
    let arguments = tc["arguments"]
        .as_object()
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();

    Some(Tau2ToolCall { name, arguments })
}

// ─── Response Validator ───────────────────────────────────────────────────────

/// Validate response against domain policy and user constraints
fn validate_response(
    mut response: Tau2Response,
    obs: &Tau2Observation,
    state: &Tau2State,
) -> Tau2Response {
    let tool_name = response.tool_call.as_ref().map(|tc| tc.name.clone());
    let tool_args_empty = response
        .tool_call
        .as_ref()
        .map(|tc| tc.arguments.is_empty())
        .unwrap_or(false);

    if let Some(ref name) = tool_name {
        let tool_available = obs
            .available_tools
            .iter()
            .any(|t| t["name"].as_str() == Some(name.as_str()));

        if !tool_available {
            warn!("Tool '{}' not in available_tools — removing", name);
            response.tool_call = None;
            if response.message.is_none() {
                response.message =
                    Some("Let me look into this for you. Could you tell me more?".into());
            }
        } else if let Some(last_called) = state.tools_called.last() {
            if last_called == name && tool_args_empty {
                warn!("Avoiding redundant tool call: {}", name);
                response.tool_call = None;
            }
        }
    }

    if let Some(ref msg) = response.message {
        let msg_lower = msg.to_lowercase();
        if (msg_lower.contains("i've updated")
            || msg_lower.contains("i have changed")
            || msg_lower.contains("i've added"))
            && response.tool_call.is_none()
            && state.tools_called.is_empty()
        {
            warn!("Anti-hallucination: agent claims action without tool call");
            response.message = Some("Let me check your account details first.".into());
            response.tool_call = Some(Tau2ToolCall {
                name: "get_account_info".to_string(),
                arguments: HashMap::new(),
            });
        }
    }

    if response.message.is_none() && response.tool_call.is_none() {
        response.message = Some("Could you describe the issue you're experiencing?".into());
    }

    response
}

// ─── Safe Fallback ────────────────────────────────────────────────────────────

/// Generates safe fallback when FBA quorum not reached
/// Always valid, never hallucinates, always helpful
fn safe_fallback_response(obs: &Tau2Observation, state: &Tau2State) -> Tau2Response {
    // Determine best fallback based on conversation state
    if state.turns_taken == 0 || obs.conversation.is_empty() {
        // Opening — gather information
        return Tau2Response {
            message: Some(
                "Hello! I'm here to help you with your telecom service. \
                 Could you please describe the issue you're experiencing?"
                    .into(),
            ),
            tool_call: None,
        };
    }

    // Check if we have an issue but haven't checked account yet
    if state.user_issue.is_some()
        && !state.tools_called.contains(&"get_account_info".to_string())
        && obs
            .available_tools
            .iter()
            .any(|t| t["name"] == "get_account_info")
    {
        return Tau2Response {
            message: Some("Let me look up your account details right away.".into()),
            tool_call: Some(Tau2ToolCall {
                name: "get_account_info".to_string(),
                arguments: HashMap::new(),
            }),
        };
    }

    // Check data usage if not done
    if !state.tools_called.contains(&"get_data_usage".to_string())
        && obs
            .available_tools
            .iter()
            .any(|t| t["name"] == "get_data_usage")
    {
        return Tau2Response {
            message: Some("Let me check your current data usage.".into()),
            tool_call: Some(Tau2ToolCall {
                name: "get_data_usage".to_string(),
                arguments: HashMap::new(),
            }),
        };
    }

    // Generic clarification
    Tau2Response {
        message: Some(
            "I want to make sure I resolve this correctly for you. \
             Could you confirm what specific issue you're experiencing right now?"
                .into(),
        ),
        tool_call: None,
    }
}

// ─── A2A Format Helper ────────────────────────────────────────────────────────

/// Format τ²-Bench tool call for A2A Data part
pub fn format_tool_call(
    name: &str,
    arguments: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "tool_call": {
            "name":      name,
            "arguments": arguments,
            "domain":    "telecom"
        }
    })
}

/// Convert Tau2Response to A2A parts (Text + optional Data)
pub fn response_to_a2a_parts(
    response: &Tau2Response,
) -> (Option<String>, Option<serde_json::Value>) {
    let text = response.message.clone();
    let data = response.tool_call.as_ref().map(|tc| {
        serde_json::json!({
            "tool_call": {
                "name":      tc.name,
                "arguments": tc.arguments,
            }
        })
    });
    (text, data)
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
    text.to_string()
}

fn mint_jwt(secret: &str, track: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;

    let header = b64(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = b64(&format!(
        r#"{{"sub":"agentx-sprint2","role":"purple_agent","track":"{}","exp":{}}}"#,
        track, exp
    ));
    let signing = format!("{}.{}", header, payload);

    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    format!("{}{}", signing, secret).hash(&mut hasher);
    let sig = b64(&format!("{:x}", hasher.finish()));

    format!("{}.{}.{}", header, payload, sig)
}

fn b64(input: &str) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let bytes = input.as_bytes();
    let mut result = String::new();
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 {
            chunk[1] as usize
        } else {
            0
        };
        let b2 = if chunk.len() > 2 {
            chunk[2] as usize
        } else {
            0
        };
        result.push(CHARS[(b0 >> 2) & 0x3F] as char);
        result.push(CHARS[((b0 << 4) | (b1 >> 4)) & 0x3F] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((b1 << 2) | (b2 >> 6)) & 0x3F] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARS[b2 & 0x3F] as char);
        }
    }
    result
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_obs(turns: Vec<TauTurn>, tools: Vec<&str>) -> Tau2Observation {
        Tau2Observation {
            domain: "telecom".to_string(),
            conversation: turns,
            available_tools: tools
                .iter()
                .map(|&t| serde_json::json!({"name": t}))
                .collect(),
            policy: "Agent must verify account before making changes.".to_string(),
            task_description: "Mobile data not working".to_string(),
            is_final: false,
        }
    }

    fn user_turn(content: &str) -> TauTurn {
        TauTurn {
            role: "user".to_string(),
            content: content.to_string(),
            tool_name: None,
            tool_result: None,
        }
    }

    fn agent_turn(content: &str) -> TauTurn {
        TauTurn {
            role: "agent".to_string(),
            content: content.to_string(),
            tool_name: None,
            tool_result: None,
        }
    }

    fn tool_turn(name: &str, result: &str) -> TauTurn {
        TauTurn {
            role: "tool".to_string(),
            content: result.to_string(),
            tool_name: Some(name.to_string()),
            tool_result: None,
        }
    }

    #[test]
    fn test_prompt_contains_key_sections() {
        let obs = sample_obs(
            vec![user_turn("My mobile data is not working")],
            vec!["get_account_info", "get_data_usage", "toggle_mobile_data"],
        );
        let mut state = Tau2State::new("telecom");
        state.analyze_conversation(&obs);
        let prompt = build_tau2_prompt(&obs, &state);

        assert!(prompt.contains("TAU2-BENCH"));
        assert!(prompt.contains("DUAL-CONTROL"));
        assert!(prompt.contains("ANTI-HALLUCINATION"));
        assert!(prompt.contains("POLICY COMPLIANCE"));
        assert!(prompt.contains("get_account_info"));
    }

    #[test]
    fn test_state_extracts_user_issue() {
        let obs = sample_obs(
            vec![user_turn("My mobile data stopped working suddenly")],
            vec!["get_data_usage"],
        );
        let mut state = Tau2State::new("telecom");
        state.analyze_conversation(&obs);
        assert!(state.user_issue.is_some());
        assert!(state.user_issue.unwrap().contains("mobile data"));
    }

    #[test]
    fn test_state_tracks_tools_called() {
        let obs = sample_obs(
            vec![
                user_turn("Data not working"),
                agent_turn("Let me check"),
                tool_turn("get_data_usage", "15.1GB used, limit 15GB"),
            ],
            vec!["get_data_usage", "refuel_data"],
        );
        let mut state = Tau2State::new("telecom");
        state.analyze_conversation(&obs);
        assert!(state.tools_called.contains(&"get_data_usage".to_string()));
    }

    #[test]
    fn test_invalid_tool_removed_in_validation() {
        let obs = sample_obs(
            vec![user_turn("Help me")],
            vec!["get_account_info"], // only this tool available
        );
        let state = Tau2State::new("telecom");

        // Response tries to call non-existent tool
        let response = Tau2Response {
            message: Some("Let me check".into()),
            tool_call: Some(Tau2ToolCall {
                name: "nonexistent_tool".to_string(),
                arguments: HashMap::new(),
            }),
        };

        let validated = validate_response(response, &obs, &state);
        assert!(
            validated.tool_call.is_none(),
            "Invalid tool should be removed"
        );
        assert!(validated.message.is_some(), "Message should be preserved");
    }

    #[test]
    fn test_anti_hallucination_no_claim_without_tool() {
        let obs = sample_obs(
            vec![user_turn("Fix my data")],
            vec!["get_account_info", "refuel_data"],
        );
        let state = Tau2State::new("telecom");

        // Agent claims to have done something without calling a tool
        let response = Tau2Response {
            message: Some("I've updated your data plan successfully.".into()),
            tool_call: None,
        };

        let validated = validate_response(response, &obs, &state);
        // Should not contain hallucinated claim
        if let Some(msg) = &validated.message {
            assert!(
                !msg.contains("I've updated"),
                "Hallucinated claim should be replaced"
            );
        }
    }

    #[test]
    fn test_safe_fallback_empty_conversation() {
        let obs = sample_obs(vec![], vec!["get_account_info"]);
        let state = Tau2State::new("telecom");
        let resp = safe_fallback_response(&obs, &state);
        assert!(resp.message.is_some());
        assert!(resp.message.unwrap().contains("Hello"));
    }

    #[test]
    fn test_safe_fallback_calls_account_info_first() {
        let obs = sample_obs(
            vec![user_turn("My data isn't working")],
            vec!["get_account_info", "get_data_usage"],
        );
        let mut state = Tau2State::new("telecom");
        state.analyze_conversation(&obs);
        let resp = safe_fallback_response(&obs, &state);
        assert!(resp.tool_call.is_some());
        assert_eq!(resp.tool_call.unwrap().name, "get_account_info");
    }

    #[test]
    fn test_response_to_a2a_parts() {
        let response = Tau2Response {
            message: Some("Checking now".into()),
            tool_call: Some(Tau2ToolCall {
                name: "get_data_usage".to_string(),
                arguments: HashMap::new(),
            }),
        };
        let (text, data) = response_to_a2a_parts(&response);
        assert_eq!(text.unwrap(), "Checking now");
        assert!(data.is_some());
        assert_eq!(data.unwrap()["tool_call"]["name"], "get_data_usage");
    }

    #[test]
    fn test_telecom_tool_serialization() {
        let tool = TelecomTool::RefuelData { amount_gb: 2.0 };
        let json = tool.to_json();
        assert_eq!(json["name"], "refuel_data");
        assert_eq!(json["arguments"]["amount_gb"], 2.0);

        let toggle = TelecomTool::ToggleMobileData { enabled: true };
        let json2 = toggle.to_json();
        assert_eq!(json2["name"], "toggle_mobile_data");
        assert_eq!(json2["arguments"]["enabled"], true);
    }

    #[test]
    fn test_resolution_detection() {
        let obs = sample_obs(
            vec![
                user_turn("My data isn't working"),
                agent_turn("I've refueled your data"),
                user_turn("It's working now, thank you!"),
            ],
            vec!["refuel_data"],
        );
        let mut state = Tau2State::new("telecom");
        state.analyze_conversation(&obs);
        state.check_resolution(&obs);
        assert!(state.issue_resolved);
    }
}
