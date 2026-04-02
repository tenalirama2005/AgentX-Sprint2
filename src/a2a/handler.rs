// src/a2a/handler.rs
// Core A2A task handler — routes to correct track, calls FBA pipeline

use axum::{extract::State, Json};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    a2a::types::*,
    fba::client::FbaClient,
    tracks::{car_bench, maize, osworld, tau2},
    AppState,
};

pub async fn handle_task(
    State(state): State<Arc<AppState>>,
    Json(task): Json<A2ATask>,
) -> Json<A2AResponse> {
    info!("📥 A2A task received: {}", task.id);

    // 1. Parse the incoming A2A message
    let (policy, conversation, available_tools) = match parse_task(&task) {
        Ok(parsed) => parsed,
        Err(e) => {
            warn!("Failed to parse A2A task: {}", e);
            return Json(error_response(&task.id, &e.to_string()));
        }
    };

    // 2. Detect which benchmark track this task belongs to
    let track = detect_track(&available_tools, &policy);
    info!("🎯 Detected track: {:?}", track);

    // 3. Build FBA request — domain-agnostic pipeline reuse
    let fba_input = encode_for_fba(&policy, &conversation, &available_tools, &track);

    // 4. Call existing FBA pipeline (purple_agent:8081)
    let fba_client = FbaClient::new(&state.fba_endpoint, &state.jwt_secret);
    let fba_result = match fba_client.run_consensus(fba_input).await {
        Ok(r) => r,
        Err(e) => {
            warn!("FBA pipeline error: {}", e);
            return Json(error_response(&task.id, "FBA pipeline unavailable"));
        }
    };

    info!(
        "🧠 FBA result: consensus={}, quorum={}/49, confidence={:.1}%",
        fba_result.consensus_reached,
        fba_result.quorum,
        fba_result.confidence * 100.0
    );

    // 5. Map FBA action → A2A response
    let response = match fba_result.action {
        FbaAction::TextResponse { text } => {
            info!("💬 Agent text response");
            completed_response(&task.id, text, None)
        }

        FbaAction::ToolCall { name, arguments } => {
            info!("🔧 Tool call: {}", name);
            // Apply track-specific tool call formatting
            let tool_call = match track {
                BenchmarkTrack::CarBench => car_bench::format_tool_call(&name, &arguments),
                BenchmarkTrack::Tau2Bench => tau2::format_tool_call(&name, &arguments),
                BenchmarkTrack::MaizeBargain => maize::format_tool_call(&name, &arguments),
                BenchmarkTrack::OsWorld => osworld::format_tool_call(&name, &arguments),
            };
            completed_response(&task.id, String::new(), Some(tool_call))
        }

        // ── HALLUCINATION PROTECTION ──────────────────────────────────────
        // FBA quorum of 39/49 NOT reached → agent correctly abstains
        // This is the structural anti-hallucination guarantee
        FbaAction::Abstain { reason } => {
            warn!("🛑 FBA abstain (quorum not reached): {}", reason);
            let refusal = format!("I'm unable to complete this request. {}", reason);
            completed_response(&task.id, refusal, None)
        }

        // ── DISAMBIGUATION ────────────────────────────────────────────────
        // FBA consensus: clarify before acting
        FbaAction::Clarify { question } => {
            info!("❓ Disambiguation required");
            completed_response(&task.id, question, None)
        }
    };

    Json(response)
}

// ─── Parse incoming A2A task ─────────────────────────────────────────────────

fn parse_task(
    task: &A2ATask,
) -> anyhow::Result<(String, Vec<ConversationTurn>, Vec<ToolDefinition>)> {
    let mut policy = String::new();
    let mut conversation: Vec<ConversationTurn> = Vec::new();
    let mut available_tools: Vec<ToolDefinition> = Vec::new();

    for part in &task.message.parts {
        match part {
            A2APart::Text { text } => {
                // Text part contains policy + conversation history (newline-separated)
                // Format: "POLICY:\n{policy}\nCONVERSATION:\n{turns}"
                if let Some(pol) = extract_policy(text) {
                    policy = pol;
                }
                conversation.extend(extract_conversation(text));
            }
            A2APart::Data { data } => {
                // Data part contains available tools as JSON Schema array
                if let Ok(tools) = serde_json::from_value::<Vec<ToolDefinition>>(data.clone()) {
                    available_tools = tools;
                }
            }
        }
    }

    Ok((policy, conversation, available_tools))
}

fn extract_policy(text: &str) -> Option<String> {
    text.split("POLICY:").nth(1).map(|s| {
        s.split("CONVERSATION:")
            .next()
            .unwrap_or(s)
            .trim()
            .to_string()
    })
}

fn extract_conversation(text: &str) -> Vec<ConversationTurn> {
    let mut turns = Vec::new();
    if let Some(conv_section) = text.split("CONVERSATION:").nth(1) {
        for line in conv_section.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix("USER:") {
                turns.push(ConversationTurn {
                    role: "user".into(),
                    content: line[5..].trim().to_string(),
                    tool_name: None,
                });
            } else if let Some(stripped) = line.strip_prefix("AGENT:") {
                turns.push(ConversationTurn {
                    role: "agent".into(),
                    content: line[6..].trim().to_string(),
                    tool_name: None,
                });
            } else if let Some(stripped) = line.strip_prefix("TOOL:") {
                let parts: Vec<&str> = stripped.splitn(2, ':').collect();
                turns.push(ConversationTurn {
                    role: "tool".into(),
                    content: parts.get(1).unwrap_or(&"").trim().to_string(),
                    tool_name: parts.first().map(|s| s.trim().to_string()),
                });
            }
        }
    }
    turns
}

// ─── Track detection ─────────────────────────────────────────────────────────

fn detect_track(tools: &[ToolDefinition], policy: &str) -> BenchmarkTrack {
    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();

    // CAR-bench: navigation/vehicle tools
    if tool_names
        .iter()
        .any(|n| n.contains("sunroof") || n.contains("navigation") || n.contains("charging"))
    {
        return BenchmarkTrack::CarBench;
    }

    // τ²-Bench: telecom tools
    if tool_names.iter().any(|n| {
        n.contains("data_plan")
            || n.contains("roaming")
            || n.contains("mobile")
            || n.contains("refuel")
            || n.contains("telecom")
    }) {
        return BenchmarkTrack::Tau2Bench;
    }

    // MAizeBargAIn: bargaining/negotiation
    if policy.to_lowercase().contains("bargain")
        || policy.to_lowercase().contains("negotiat")
        || tool_names
            .iter()
            .any(|n| n.contains("offer") || n.contains("bid"))
    {
        return BenchmarkTrack::MaizeBargain;
    }

    // OSWorld: computer use / GUI
    if tool_names.iter().any(|n| {
        n.contains("click")
            || n.contains("screenshot")
            || n.contains("keyboard")
            || n.contains("mouse")
    }) {
        return BenchmarkTrack::OsWorld;
    }

    // Default to CAR-bench (most structured)
    BenchmarkTrack::CarBench
}

// ─── Encode for FBA pipeline ─────────────────────────────────────────────────

fn encode_for_fba(
    policy: &str,
    conversation: &[ConversationTurn],
    tools: &[ToolDefinition],
    track: &BenchmarkTrack,
) -> FbaRequest {
    // Encode the full context as structured text for FBA pipeline
    // The pipeline is domain-agnostic — it reasons over whatever input it gets
    let context = format!(
        "TRACK: {:?}\n\nPOLICY:\n{}\n\nAVAILABLE_TOOLS:\n{}\n\nCONVERSATION:\n{}",
        track,
        policy,
        serde_json::to_string_pretty(tools).unwrap_or_default(),
        conversation
            .iter()
            .map(|t| format!("[{}] {}", t.role.to_uppercase(), t.content))
            .collect::<Vec<_>>()
            .join("\n"),
    );

    FbaRequest {
        cobol_code: context, // reuses existing pipeline input field
        context_id: Uuid::new_v4().to_string(),
        available_tools: tools.to_vec(),
        policy: policy.to_string(),
        conversation: conversation.to_vec(),
        track: track.clone(),
    }
}

// ─── Response builders ───────────────────────────────────────────────────────

fn completed_response(
    task_id: &str,
    text: String,
    tool_call: Option<serde_json::Value>,
) -> A2AResponse {
    let mut parts = Vec::new();

    if !text.is_empty() {
        parts.push(A2APart::Text { text });
    }
    if let Some(tc) = tool_call {
        parts.push(A2APart::Data { data: tc });
    }

    A2AResponse {
        id: task_id.to_string(),
        status: A2AStatus {
            state: TaskState::Completed,
            message: Some(A2AMessage {
                role: "agent".into(),
                parts,
            }),
        },
        artifacts: vec![],
    }
}

fn error_response(task_id: &str, msg: &str) -> A2AResponse {
    A2AResponse {
        id: task_id.to_string(),
        status: A2AStatus {
            state: TaskState::Failed,
            message: Some(A2AMessage {
                role: "agent".into(),
                parts: vec![A2APart::Text {
                    text: msg.to_string(),
                }],
            }),
        },
        artifacts: vec![],
    }
}
