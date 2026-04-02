// src/a2a/types.rs
// A2A Protocol v0.2 — https://a2a-protocol.org/latest/
// Shared across all 4 benchmark tracks

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─── Agent Card (GET /.well-known/agent.json) ────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub url: String,
    pub version: String,
    pub capabilities: AgentCapabilities,
    pub skills: Vec<AgentSkill>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentCapabilities {
    pub streaming: bool,
    pub push_notifications: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AgentSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
}

// ─── Task (POST /a2a/tasks/send) ─────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct A2ATask {
    pub id: String,
    pub message: A2AMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct A2AMessage {
    pub role: String, // "user" | "agent"
    pub parts: Vec<A2APart>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum A2APart {
    /// Text part: carries policy document + conversation history
    Text { text: String },
    /// Data part: carries available tools (JSON schema) or tool call results
    Data { data: serde_json::Value },
}

// ─── Response ─────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct A2AResponse {
    pub id: String,
    pub status: A2AStatus,
    pub artifacts: Vec<A2AArtifact>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct A2AStatus {
    pub state: TaskState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<A2AMessage>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    Submitted,
    Working,
    Completed,
    Failed,
    Canceled,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct A2AArtifact {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub parts: Vec<A2APart>,
}

// ─── FBA Pipeline Request / Response ─────────────────────────────────────────

/// What we send to the existing purple_agent FBA pipeline
#[derive(Serialize, Deserialize, Debug)]
pub struct FbaRequest {
    /// Encoded as COBOL-style input for pipeline reuse
    pub cobol_code: String,
    pub context_id: String,
    pub available_tools: Vec<ToolDefinition>,
    pub policy: String,
    pub conversation: Vec<ConversationTurn>,
    pub track: BenchmarkTrack,
}

/// FBA pipeline response
#[derive(Serialize, Deserialize, Debug)]
pub struct FbaResponse {
    pub consensus_reached: bool,
    pub confidence: f64, // target: 94%+
    pub quorum: u32,     // target: 39/49
    pub action: FbaAction,
    pub reasoning_steps: u32, // target: 89 per model
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FbaAction {
    /// Agent speaks to user
    TextResponse { text: String },
    /// Agent calls a tool
    ToolCall {
        name: String,
        arguments: HashMap<String, serde_json::Value>,
    },
    /// Agent refuses (hallucination protection — quorum not reached)
    Abstain { reason: String },
    /// Agent asks user for clarification (disambiguation)
    Clarify { question: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConversationTurn {
    pub role: String, // "user" | "agent" | "tool"
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value, // JSON Schema
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkTrack {
    CarBench,
    Tau2Bench,
    MaizeBargain,
    OsWorld,
}
