// ============================================================
// FILE 1: src/a2a/mod.rs
// ============================================================
pub mod card;
pub mod handler;
pub mod types;


// ============================================================
// FILE 2: src/fba/mod.rs
// ============================================================
pub mod client;


// ============================================================
// FILE 3: src/tracks/mod.rs
// ============================================================
pub mod car_bench;
pub mod maize;
pub mod osworld;
pub mod tau2;


// ============================================================
// FILE 4: src/tracks/car_bench.rs  (stub — full file separate)
// ============================================================
// CAR-bench: 58 tools, 19 policies, 254 tasks
// Hallucination-resistant via FBA abstain when quorum not reached
// Pass^3 advantage: FBA near-deterministic across all 3 runs

use std::collections::HashMap;

pub fn format_tool_call(
    name:      &str,
    arguments: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "tool_call": {
            "name":      name,
            "arguments": arguments,
            "domain":    "automotive"
        }
    })
}

/// CAR-bench specific: check if tool is in available set
pub fn is_tool_available(name: &str, available: &[serde_json::Value]) -> bool {
    available.iter().any(|t| t["name"].as_str() == Some(name))
}

/// CAR-bench hallucination task: if required tool missing → abstain
pub fn should_abstain(required_tool: &str, available: &[serde_json::Value]) -> bool {
    !is_tool_available(required_tool, available)
}


// ============================================================
// FILE 5: src/tracks/osworld.rs  (stub — full file separate)
// ============================================================
// OSWorld-Verified: 369 tasks, GUI interaction
// Current leaderboard: 1 dummy entry at 0.8%
// Vision: Gemini 2.5 Pro (screenshots) + Qwen3.5-397B (GUI)

use std::collections::HashMap as OSHashMap;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct OsWorldAction {
    pub action_type: String,   // "click" | "type" | "scroll" | "key" | "screenshot"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinate:  Option<(u32, u32)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text:        Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key:         Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction:   Option<String>,
}

pub fn format_tool_call(
    name:      &str,
    arguments: &OSHashMap<String, serde_json::Value>,
) -> serde_json::Value {
    serde_json::json!({
        "tool_call": {
            "name":      name,
            "arguments": arguments,
            "domain":    "computer_use"
        }
    })
}

/// Parse screenshot observation for vision processing
pub fn parse_screenshot(data_part: &serde_json::Value) -> Option<String> {
    // Extract base64 screenshot for Gemini/Qwen vision processing
    data_part["screenshot"].as_str().map(|s| s.to_string())
}

/// Format GUI action as A2A tool call
pub fn gui_action_to_tool_call(action: &OsWorldAction) -> serde_json::Value {
    serde_json::json!({
        "tool_call": {
            "name": action.action_type,
            "arguments": {
                "coordinate": action.coordinate,
                "text":       action.text,
                "key":        action.key,
                "direction":  action.direction,
            }
        }
    })
}


// ============================================================
// FILE 6: src/main.rs  (updated imports section only)
// Add these use statements to existing main.rs
// ============================================================
/*
mod a2a;
mod fba;
mod tracks;

use a2a::{card, handler};
*/


// ============================================================
// FILE 7: src/a2a/handler.rs  (updated track dispatch)
// Replace the tracks section in existing handler.rs with this
// ============================================================
// The key update: tau2 and maize now call their async processors
// rather than just format_tool_call

/*
// In handle_task(), replace the tool call dispatch block with:

FbaAction::ToolCall { name, arguments } => {
    info!("🔧 Tool call: {} on track {:?}", name, track);
    let tool_data = match track {
        BenchmarkTrack::CarBench =>
            car_bench::format_tool_call(&name, &arguments),
        BenchmarkTrack::Tau2Bench =>
            tau2::format_tool_call(&name, &arguments),
        BenchmarkTrack::MaizeBargain =>
            maize::format_tool_call(&name, &arguments),
        BenchmarkTrack::OsWorld =>
            osworld::format_tool_call(&name, &arguments),
    };
    completed_response(&task.id, String::new(), Some(tool_data))
}

// For MAizeBargAIn: use the full async processor directly
// In handle_task(), add before the FBA call:
if track == BenchmarkTrack::MaizeBargain {
    if let Some(data_part) = task.message.parts.iter()
        .find(|p| matches!(p, A2APart::Data { .. }))
    {
        if let A2APart::Data { data } = data_part {
            let action = maize::process_bargain_turn(
                data,
                &state.fba_endpoint,
                &state.jwt_secret,
                &task.id,
            ).await;
            let action_data = maize::action_to_a2a_data(&action);
            return Json(completed_response(&task.id, String::new(), Some(action_data)));
        }
    }
}

// For τ²-Bench: use the full async processor directly
if track == BenchmarkTrack::Tau2Bench {
    if let Some(data_part) = task.message.parts.iter()
        .find(|p| matches!(p, A2APart::Data { .. }))
    {
        if let A2APart::Data { data } = data_part {
            let response = tau2::process_tau2_turn(
                data,
                &state.fba_endpoint,
                &state.jwt_secret,
                &task.id,
            ).await;
            let (text, data_out) = tau2::response_to_a2a_parts(&response);
            return Json(completed_response(
                &task.id,
                text.unwrap_or_default(),
                data_out,
            ));
        }
    }
}
*/
