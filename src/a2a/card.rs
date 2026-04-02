use crate::a2a::types::{AgentCapabilities, AgentCard, AgentSkill};
use crate::AppState;
use axum::extract::State;
use axum::Json;
use std::sync::Arc;

pub async fn agent_card(State(state): State<Arc<AppState>>) -> Json<AgentCard> {
    Json(AgentCard {
        name: "AgentX-Sprint2".into(),
        description: "FBA-powered purple agent: 49 models, 39/49 quorum @ 94% confidence. Anti-hallucination by architecture.".into(),
        url: format!("{}/a2a/tasks/send", state.agent_url),
        version: "0.1.0".into(),
        capabilities: AgentCapabilities {
            streaming: false,
            push_notifications: false,
        },
        skills: vec![
            AgentSkill { id: "maize".into(), name: "MAizeBargAIn".into(), description: "FBA bargaining".into(), tags: vec!["bargaining".into()] },
            AgentSkill { id: "tau2".into(),  name: "τ²-Bench".into(),     description: "FBA telecom".into(),    tags: vec!["telecom".into()] },
            AgentSkill { id: "car".into(),   name: "CAR-bench".into(),    description: "FBA automotive".into(), tags: vec!["automotive".into()] },
            AgentSkill { id: "osw".into(),   name: "OSWorld".into(),      description: "FBA GUI".into(),        tags: vec!["gui".into()] },
            AgentSkill { id: "mle".into(),   name: "MLE-bench".into(),    description: "FBA ML".into(),         tags: vec!["ml".into()] },
            AgentSkill { id: "fwa".into(),   name: "FieldWorkArena".into(), description: "FBA vision".into(),   tags: vec!["vision".into()] },
        ],
    })
}
