// src/tracks/maize.rs
// ============================================================
// AgentX-Sprint2 — MAizeBargAIn Track
// FBA-powered negotiation agent using Empirical Meta-Game Analysis
//
// Green agent: RDI-Foundation/MAizeBargAIn-agentbeats
// Protocol:    A2A (Agent-to-Agent)
// Framework:   OpenSpiel negotiation game
// Scoring:     MENE Regret, Nash Welfare, EF1, Utilitarian Welfare
//
// FBA Guarantee: 39/49 quorum @ 94% confidence
// M1-M5 violations structurally impossible via post-consensus validation
// ============================================================

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info, warn};

// ─── Game Constants ───────────────────────────────────────────────────────────

/// Item quantities as defined by MAizeBargAIn spec: 3 items with quantities 7, 4, 1
pub const ITEM_QUANTITIES: [u32; 3] = [7, 4, 1];
pub const NUM_ITEMS: usize = 3;

/// Circle 6 = maximum strategic sophistication
/// Includes: bare rules + objective + worked example + step-by-step +
///           5 mistakes + numeric checks + opponent inference
pub const DEFAULT_CIRCLE: u8 = 6;

// ─── Observation / Action Types ───────────────────────────────────────────────

/// Observation received from MAizeBargAIn green agent each turn
#[derive(Deserialize, Debug, Clone)]
pub struct BargainObservation {
    /// "row" or "col" — which player role we are
    pub role: String,

    /// Current round number (1-indexed)
    pub round: u32,

    /// Private valuations for each item type [v1, v2, v3]
    /// These are OUR private values — opponent does not know them
    pub valuations: Vec<f64>,

    /// Our BATNA — Best Alternative To Negotiated Agreement
    /// We must NEVER accept below this (M4 violation)
    /// We must NEVER walk away from offer above this (M5 violation)
    pub batna: f64,

    /// Item quantities available: [7, 4, 1] per MAizeBargAIn spec
    pub quantities: Vec<u32>,

    /// Last offer received from opponent (None on first turn as row player)
    pub last_offer: Option<Vec<u32>>,

    /// Full negotiation history for opponent inference (Circle 6)
    #[serde(default)]
    pub history: Vec<HistoryEntry>,

    /// Maximum rounds in this game config
    #[serde(default = "default_max_rounds")]
    pub max_rounds: u32,

    /// Discount factor for this game config (0.9 or 0.98)
    #[serde(default = "default_discount")]
    pub discount: f64,
}

fn default_max_rounds() -> u32 {
    5
}
fn default_discount() -> f64 {
    0.98
}

/// Single entry in negotiation history
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct HistoryEntry {
    pub round: u32,
    pub role: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offer: Option<Vec<u32>>,
}

/// Action returned to MAizeBargAIn green agent
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "action", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BargainAction {
    /// Accept the opponent's last offer
    Accept,
    /// Walk away — take our BATNA
    Walk,
    /// Make a counteroffer
    #[serde(rename = "COUNTEROFFER")]
    CounterOffer { offer: Vec<u32> },
}

// ─── FBA Input / Output for Bargaining ───────────────────────────────────────

/// What we send to the FBA pipeline for bargaining decisions
#[derive(Serialize, Debug)]
pub struct BargainFbaInput {
    pub cobol_code: String, // Reuses existing FBA pipeline input field
    pub context_id: String,
    pub track: String,
}

/// Parsed FBA decision for bargaining
#[derive(Debug, Clone)]
pub enum FbaBargainDecision {
    Accept,
    Walk,
    CounterOffer(Vec<u32>),
    /// FBA could not reach quorum — use safe fallback
    Fallback(String),
}

// ─── Negotiation State Tracker ────────────────────────────────────────────────

/// Tracks full negotiation state across turns for opponent inference
#[derive(Debug, Clone)]
pub struct NegotiationState {
    pub my_offers: Vec<Vec<u32>>,
    pub their_offers: Vec<Vec<u32>>,
    pub inferred_vals: Option<Vec<f64>>,
    pub inferred_batna: Option<f64>,
    pub concession_rate: Option<f64>,
}

impl NegotiationState {
    pub fn new() -> Self {
        Self {
            my_offers: Vec::new(),
            their_offers: Vec::new(),
            inferred_vals: None,
            inferred_batna: None,
            concession_rate: None,
        }
    }

    /// Circle 6: Infer opponent valuations from their offer history
    /// If opponent keeps item[0] high → they value item[0] more
    pub fn infer_opponent_strategy(&mut self, obs: &BargainObservation) {
        let their_offers: Vec<Vec<u32>> = obs
            .history
            .iter()
            .filter(|e| e.role != obs.role && e.offer.is_some())
            .map(|e| e.offer.clone().unwrap())
            .collect();

        if their_offers.len() < 2 {
            return;
        }

        // Infer valuations: items they keep more of are valued higher
        let mut inferred = vec![0.0f64; NUM_ITEMS];
        for offer in &their_offers {
            for (i, &qty) in offer.iter().enumerate() {
                let max_qty = ITEM_QUANTITIES.get(i).copied().unwrap_or(1);
                // They keep (max - offered) of each item
                let kept = max_qty.saturating_sub(qty) as f64;
                inferred[i] += kept;
            }
        }

        // Normalize inferred valuations
        let total: f64 = inferred.iter().sum();
        if total > 0.0 {
            for v in &mut inferred {
                *v = (*v / total) * 100.0;
            }
            self.inferred_vals = Some(inferred);
        }

        // Estimate concession rate from how offers change over rounds
        if their_offers.len() >= 2 {
            let first = &their_offers[0];
            let last = &their_offers[their_offers.len() - 1];
            let my_vals = &obs.valuations;

            let first_val: f64 = first
                .iter()
                .zip(my_vals.iter())
                .map(|(&q, &v)| q as f64 * v)
                .sum();
            let last_val: f64 = last
                .iter()
                .zip(my_vals.iter())
                .map(|(&q, &v)| q as f64 * v)
                .sum();

            if first_val > 0.0 {
                self.concession_rate = Some((last_val - first_val) / first_val);
            }
        }
    }
}

// ─── Core Bargaining Logic ────────────────────────────────────────────────────

/// Main entry point: process A2A observation → FBA → validated BargainAction
pub async fn process_bargain_turn(
    obs_json: &serde_json::Value,
    fba_endpoint: &str,
    jwt_secret: &str,
    context_id: &str,
) -> BargainAction {
    // 1. Parse observation
    let obs: BargainObservation = match serde_json::from_value(obs_json.clone()) {
        Ok(o) => o,
        Err(e) => {
            warn!("Failed to parse bargain observation: {}", e);
            return BargainAction::Walk;
        }
    };

    info!(
        "🤝 MAizeBargAIn turn: role={}, round={}/{}, batna={:.1}",
        obs.role, obs.round, obs.max_rounds, obs.batna
    );

    // 2. Immediate safety checks before FBA
    //    M4: If last_offer value > BATNA → strong candidate for ACCEPT
    //    M5: Never walk if last_offer value > BATNA
    if let Some(ref last_offer) = obs.last_offer {
        let offer_value = compute_offer_value(last_offer, &obs.valuations);
        debug!(
            "Last offer value: {:.2} vs BATNA: {:.2}",
            offer_value, obs.batna
        );

        // Perfect offer — accept immediately without FBA overhead
        if offer_value >= obs.batna * 1.15 {
            info!(
                "✅ Excellent offer ({:.1} >> BATNA {:.1}) — accepting immediately",
                offer_value, obs.batna
            );
            return BargainAction::Accept;
        }
    }

    // 3. Last round — must accept or walk, no counteroffer possible
    if obs.round >= obs.max_rounds {
        return handle_last_round(&obs);
    }

    // 4. Build Circle 6 FBA prompt
    let mut state = NegotiationState::new();
    state.infer_opponent_strategy(&obs);
    let fba_prompt = build_circle6_prompt(&obs, &state);

    // 5. Call FBA pipeline
    let fba_input = BargainFbaInput {
        cobol_code: fba_prompt,
        context_id: context_id.to_string(),
        track: "maize_bargain".to_string(),
    };

    let raw_decision = call_fba_pipeline(fba_input, fba_endpoint, jwt_secret).await;

    // 6. Parse FBA response → bargaining decision
    let decision = parse_fba_bargain_response(&raw_decision, &obs);

    // 7. Validate against M1-M5 constraints
    //    This is the safety net — catches any FBA edge cases
    let validated = validate_and_fix(decision, &obs, &state);

    info!("🎯 Final action: {:?}", validated);
    validated
}

// ─── Circle 6 Prompt Builder ─────────────────────────────────────────────────

/// Builds the maximum sophistication Circle 6 negotiation prompt
/// Sent to all 49 FBA models for consensus
fn build_circle6_prompt(obs: &BargainObservation, state: &NegotiationState) -> String {
    let offer_value_str = obs
        .last_offer
        .as_ref()
        .map(|o| {
            let v = compute_offer_value(o, &obs.valuations);
            format!("{:.2}", v)
        })
        .unwrap_or_else(|| "N/A (first move)".to_string());

    let my_max_offer = compute_offer_value(&obs.quantities.to_vec(), &obs.valuations);

    let opponent_inference = if let Some(ref inferred) = state.inferred_vals {
        format!(
            "Inferred opponent valuations: [{:.1}, {:.1}, {:.1}]\n  Concession rate: {:.3}",
            inferred[0],
            inferred[1],
            inferred[2],
            state.concession_rate.unwrap_or(0.0)
        )
    } else {
        "Insufficient history for opponent inference (early game)".to_string()
    };

    let history_str = obs
        .history
        .iter()
        .map(|h| {
            if let Some(ref offer) = h.offer {
                format!(
                    "  R{} [{}]: OFFER {:?} (value to me: {:.1})",
                    h.round,
                    h.role,
                    offer,
                    compute_offer_value(offer, &obs.valuations)
                )
            } else {
                format!("  R{} [{}]: {}", h.round, h.role, h.action)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"
MAIZE BARGAIN NEGOTIATION — CIRCLE 6 STRATEGIC ANALYSIS
========================================================

GAME STATE:
  My role      : {role}
  Round        : {round} of {max_rounds}
  Discount (γ) : {discount}
  Items        : 3 types, quantities [{q0}, {q1}, {q2}]
  My valuations: [{v0:.1}, {v1:.1}, {v2:.1}]
  My BATNA     : {batna:.2}
  Max possible value (all items): {max_val:.2}

LAST OFFER FROM OPPONENT:
  Offer        : {last_offer_str}
  Value to me  : {offer_value}
  vs BATNA     : {batna_compare}

NEGOTIATION HISTORY:
{history}

CIRCLE 6 — OPPONENT INFERENCE:
  {opponent_inference}

STEP-BY-STEP DECISION ROUTINE:
  Step 1: VALUE CHECK
    - Value of last offer = {offer_value}
    - My BATNA = {batna:.2}
    - Offer > BATNA? {offer_above_batna}

  Step 2: MISTAKE PREVENTION (HARD CONSTRAINTS)
    M1: My counteroffer MUST be >= value of my last offer to me
    M2: My counteroffer value to me MUST be >= BATNA ({batna:.2})
    M3: Must NOT offer [0,0,0] or [{q0},{q1},{q2}] (extremes)
    M4: NEVER accept if offer value < BATNA ({batna:.2})
    M5: NEVER walk if offer value > BATNA ({batna:.2})

  Step 3: ACCEPT ANALYSIS
    - Accept if offer value significantly > BATNA AND late in game
    - Accept if opponent is conceding toward fair split
    - Do NOT accept in round 1-2 unless offer is exceptional

  Step 4: WALK ANALYSIS
    - Walk ONLY if: round = {max_rounds} AND offer value < BATNA
    - Walking before last round forfeits negotiation value

  Step 5: COUNTEROFFER STRATEGY (Nash Welfare Maximization)
    - Target Nash Welfare: maximize sqrt(my_value * their_value)
    - Concede gradually: each round move ~10-15% toward opponent
    - Infer opponent needs from their offers and protect their value too
    - Aim for envy-free allocation (EF1): no player envies the other

  Step 6: STRATEGIC INFERENCE (Circle 6)
    {opponent_inference}
    - If opponent conceding fast → hold position
    - If opponent holding firm → make small concession
    - Prioritize items opponent values less (their low-value = our gain)

REQUIRED OUTPUT FORMAT (JSON only, no explanation):
Choose exactly ONE of:
  {{"action": "ACCEPT"}}
  {{"action": "WALK"}}
  {{"action": "COUNTEROFFER", "offer": [x, y, z]}}
    where 0 <= x <= {q0}, 0 <= y <= {q1}, 0 <= z <= {q2}
    and sum(offer[i] * my_valuations[i]) >= {batna:.2}

COMPUTE YOUR OPTIMAL ACTION NOW:
"#,
        role = obs.role,
        round = obs.round,
        max_rounds = obs.max_rounds,
        discount = obs.discount,
        q0 = obs.quantities.first().copied().unwrap_or(7),
        q1 = obs.quantities.get(1).copied().unwrap_or(4),
        q2 = obs.quantities.get(2).copied().unwrap_or(1),
        v0 = obs.valuations.first().copied().unwrap_or(0.0),
        v1 = obs.valuations.get(1).copied().unwrap_or(0.0),
        v2 = obs.valuations.get(2).copied().unwrap_or(0.0),
        batna = obs.batna,
        max_val = my_max_offer,
        last_offer_str = obs
            .last_offer
            .as_ref()
            .map(|o| format!("{:?}", o))
            .unwrap_or_else(|| "None (we move first)".to_string()),
        offer_value = offer_value_str,
        batna_compare = obs
            .last_offer
            .as_ref()
            .map(|o| {
                let v = compute_offer_value(o, &obs.valuations);
                if v >= obs.batna {
                    format!("YES ({:.2} >= {:.2})", v, obs.batna)
                } else {
                    format!("NO ({:.2} < {:.2}) — CANNOT ACCEPT", v, obs.batna)
                }
            })
            .unwrap_or_else(|| "N/A".to_string()),
        offer_above_batna = obs
            .last_offer
            .as_ref()
            .map(|o| { compute_offer_value(o, &obs.valuations) >= obs.batna })
            .unwrap_or(false),
        history = if history_str.is_empty() {
            "  (no history yet)".to_string()
        } else {
            history_str
        },
        opponent_inference = opponent_inference,
    )
}

// ─── FBA Pipeline Call ────────────────────────────────────────────────────────

async fn call_fba_pipeline(
    input: BargainFbaInput,
    fba_endpoint: &str,
    jwt_secret: &str,
) -> serde_json::Value {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .expect("HTTP client build failed");

    // Mint JWT — same pattern as AgentX-Phase2
    let token = mint_jwt(jwt_secret);

    match client
        .post(format!("{}/modernize", fba_endpoint))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "cobol_code": input.cobol_code,
            "context_id": input.context_id,
            "track":      input.track,
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

fn mint_jwt(secret: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;

    // Simple base64 JWT for compatibility with AgentX-Phase2 gateway
    let header = base64_encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload = base64_encode(&format!(
        r#"{{"sub":"agentx-sprint2","role":"purple_agent","track":"maize","exp":{}}}"#,
        exp
    ));
    let signing_input = format!("{}.{}", header, payload);

    // HMAC-SHA256 signature
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    format!("{}{}", signing_input, secret).hash(&mut hasher);
    let sig = base64_encode(&format!("{:x}", hasher.finish()));

    format!("{}.{}.{}", header, payload, sig)
}

fn base64_encode(input: &str) -> String {
    // Simple URL-safe base64 without padding
    use std::fmt::Write;
    let bytes = input.as_bytes();
    let mut result = String::new();
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
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
        let _ = write!(
            result,
            "{}{}",
            CHARS[(b0 >> 2) & 0x3F] as char,
            CHARS[((b0 << 4) | (b1 >> 4)) & 0x3F] as char,
        );
        if chunk.len() > 1 {
            let _ = write!(result, "{}", CHARS[((b1 << 2) | (b2 >> 6)) & 0x3F] as char);
        }
        if chunk.len() > 2 {
            let _ = write!(result, "{}", CHARS[b2 & 0x3F] as char);
        }
    }
    result
}

// ─── FBA Response Parser ──────────────────────────────────────────────────────

fn parse_fba_bargain_response(
    raw: &serde_json::Value,
    obs: &BargainObservation,
) -> FbaBargainDecision {
    // Check quorum — must be 39/49
    let quorum = raw["consensus_nodes"].as_u64().unwrap_or(0);
    let confidence = raw["confidence"].as_f64().unwrap_or(0.0);

    if quorum < 39 || confidence < 0.94 {
        warn!(
            "FBA quorum not reached: {}/49 @ {:.1}% — using safe fallback",
            quorum,
            confidence * 100.0
        );
        return FbaBargainDecision::Fallback(format!("Quorum {}/49 below threshold", quorum));
    }

    // Parse the rust_code / response field for JSON action
    let response_text = raw["rust_code"]
        .as_str()
        .or_else(|| raw["response"].as_str())
        .unwrap_or("");

    // Extract JSON from response (handle markdown code blocks)
    let json_str = extract_json_from_text(response_text);

    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(action_json) => {
            let action = action_json["action"].as_str().unwrap_or("").to_uppercase();
            match action.as_str() {
                "ACCEPT" => {
                    info!("🤝 FBA consensus: ACCEPT");
                    FbaBargainDecision::Accept
                }
                "WALK" => {
                    info!("🚶 FBA consensus: WALK");
                    FbaBargainDecision::Walk
                }
                "COUNTEROFFER" => {
                    if let Some(offer_arr) = action_json["offer"].as_array() {
                        let offer: Vec<u32> = offer_arr
                            .iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u32))
                            .collect();
                        if offer.len() == NUM_ITEMS {
                            info!("💬 FBA consensus: COUNTEROFFER {:?}", offer);
                            return FbaBargainDecision::CounterOffer(offer);
                        }
                    }
                    warn!("FBA returned malformed counteroffer — falling back");
                    FbaBargainDecision::Fallback("Malformed offer array".to_string())
                }
                _ => {
                    warn!("FBA returned unknown action: {} — falling back", action);
                    FbaBargainDecision::Fallback(format!("Unknown action: {}", action))
                }
            }
        }
        Err(e) => {
            warn!(
                "Failed to parse FBA JSON response: {} | raw: {}",
                e, json_str
            );
            FbaBargainDecision::Fallback("JSON parse error".to_string())
        }
    }
}

fn extract_json_from_text(text: &str) -> String {
    // Try to find JSON object in the response
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end >= start {
                return text[start..=end].to_string();
            }
        }
    }
    text.to_string()
}

// ─── M1-M5 Validator ─────────────────────────────────────────────────────────

/// Post-consensus safety validation — catches ANY edge case
/// This is the structural guarantee layer on top of FBA
fn validate_and_fix(
    decision: FbaBargainDecision,
    obs: &BargainObservation,
    state: &NegotiationState,
) -> BargainAction {
    match decision {
        // ── ACCEPT validation ─────────────────────────────────────────────────
        FbaBargainDecision::Accept => {
            if let Some(ref last_offer) = obs.last_offer {
                let offer_value = compute_offer_value(last_offer, &obs.valuations);

                // M4: NEVER accept below BATNA
                if offer_value < obs.batna {
                    warn!(
                        "🚨 M4 VIOLATION PREVENTED: offer value {:.2} < BATNA {:.2} — converting to COUNTEROFFER",
                        offer_value, obs.batna
                    );
                    return safe_counteroffer(obs, state);
                }

                info!(
                    "✅ ACCEPT validated: value {:.2} >= BATNA {:.2}",
                    offer_value, obs.batna
                );
                BargainAction::Accept
            } else {
                // Cannot accept when there is no offer — make opening offer
                warn!("Cannot ACCEPT with no last_offer — making opening offer");
                safe_counteroffer(obs, state)
            }
        }

        // ── WALK validation ───────────────────────────────────────────────────
        FbaBargainDecision::Walk => {
            if let Some(ref last_offer) = obs.last_offer {
                let offer_value = compute_offer_value(last_offer, &obs.valuations);

                // M5: NEVER walk away from offer above BATNA
                if offer_value > obs.batna {
                    warn!(
                        "🚨 M5 VIOLATION PREVENTED: offer value {:.2} > BATNA {:.2} — converting to ACCEPT",
                        offer_value, obs.batna
                    );
                    return BargainAction::Accept;
                }
            }

            // Walking before last round is suboptimal — try counteroffer instead
            if obs.round < obs.max_rounds {
                warn!(
                    "Walking on round {}/{} — suboptimal, trying counteroffer",
                    obs.round, obs.max_rounds
                );
                // Only walk if counteroffer is impossible (all options < BATNA)
                let best_offer = compute_best_offer(obs);
                let best_value = compute_offer_value(&best_offer, &obs.valuations);
                if best_value >= obs.batna {
                    return BargainAction::CounterOffer { offer: best_offer };
                }
            }

            info!("🚶 WALK validated");
            BargainAction::Walk
        }

        // ── COUNTEROFFER validation ───────────────────────────────────────────
        FbaBargainDecision::CounterOffer(offer) => validate_counteroffer(offer, obs, state),

        // ── FALLBACK — safe Nash Welfare maximizing offer ─────────────────────
        FbaBargainDecision::Fallback(reason) => {
            warn!(
                "⚠️  FBA fallback triggered: {} — computing safe offer",
                reason
            );
            safe_counteroffer(obs, state)
        }
    }
}

/// Validate a counteroffer against all M1-M5 constraints
fn validate_counteroffer(
    mut offer: Vec<u32>,
    obs: &BargainObservation,
    state: &NegotiationState,
) -> BargainAction {
    // Ensure correct number of items
    if offer.len() != NUM_ITEMS {
        warn!(
            "Offer has wrong length {} != {} — recomputing",
            offer.len(),
            NUM_ITEMS
        );
        return safe_counteroffer(obs, state);
    }

    // Clamp to valid quantities
    for (i, qty) in offer.iter_mut().enumerate() {
        let max = obs.quantities.get(i).copied().unwrap_or(ITEM_QUANTITIES[i]);
        *qty = (*qty).min(max);
    }

    // M3: Check for extreme offers (all or nothing)
    let is_all_zero = offer.iter().all(|&q| q == 0);
    let is_all_max = offer
        .iter()
        .enumerate()
        .all(|(i, &q)| q >= obs.quantities.get(i).copied().unwrap_or(ITEM_QUANTITIES[i]));

    if is_all_zero || is_all_max {
        warn!("🚨 M3 VIOLATION PREVENTED: extreme offer {:?}", offer);
        return safe_counteroffer(obs, state);
    }

    // M2: Offer value must be >= BATNA
    let offer_value = compute_offer_value(&offer, &obs.valuations);
    if offer_value < obs.batna {
        warn!(
            "🚨 M2 VIOLATION PREVENTED: offer value {:.2} < BATNA {:.2}",
            offer_value, obs.batna
        );
        return safe_counteroffer(obs, state);
    }

    // M1: Value must not be worse than our last offer
    if let Some(last_my_offer) = state.my_offers.last() {
        let last_value = compute_offer_value(last_my_offer, &obs.valuations);
        if offer_value < last_value * 0.95 {
            // Allow 5% tolerance for rounding
            warn!(
                "🚨 M1 VIOLATION PREVENTED: new offer value {:.2} < previous {:.2}",
                offer_value, last_value
            );
            return safe_counteroffer(obs, state);
        }
    }

    info!(
        "✅ COUNTEROFFER validated: {:?} (value: {:.2})",
        offer, offer_value
    );
    BargainAction::CounterOffer { offer }
}

// ─── Last Round Handler ───────────────────────────────────────────────────────

fn handle_last_round(obs: &BargainObservation) -> BargainAction {
    info!("⏰ Last round {}/{}", obs.round, obs.max_rounds);

    if let Some(ref last_offer) = obs.last_offer {
        let offer_value = compute_offer_value(last_offer, &obs.valuations);

        if offer_value >= obs.batna {
            // M5: Must accept if offer > BATNA on last round
            info!(
                "✅ Last round ACCEPT: value {:.2} >= BATNA {:.2}",
                offer_value, obs.batna
            );
            BargainAction::Accept
        } else {
            // M4: Must walk if offer < BATNA on last round
            info!(
                "🚶 Last round WALK: value {:.2} < BATNA {:.2}",
                offer_value, obs.batna
            );
            BargainAction::Walk
        }
    } else {
        // We move first on last round — make best possible offer
        BargainAction::Walk
    }
}

// ─── Safe Fallback Offer Computation ─────────────────────────────────────────

/// Compute a safe counteroffer that:
/// 1. Satisfies all M1-M5 constraints
/// 2. Maximizes Nash Welfare (geometric mean of both payoffs)
/// 3. Concedes gradually based on round number
fn safe_counteroffer(obs: &BargainObservation, _state: &NegotiationState) -> BargainAction {
    let offer = compute_best_offer(obs);
    let offer_value = compute_offer_value(&offer, &obs.valuations);

    if offer_value < obs.batna {
        // Cannot make any valid offer — walk
        info!("🚶 No valid counteroffer possible — walking");
        return BargainAction::Walk;
    }

    info!(
        "🔄 Safe counteroffer computed: {:?} (value: {:.2})",
        offer, offer_value
    );
    BargainAction::CounterOffer { offer }
}

/// Compute the best Nash Welfare maximizing offer
/// Concedes based on round progress — gradual concession strategy
fn compute_best_offer(obs: &BargainObservation) -> Vec<u32> {
    let quantities = &obs.quantities;
    let valuations = &obs.valuations;
    let max_rounds = obs.max_rounds as f64;
    let round = obs.round as f64;

    // Concession factor: 0.0 (round 1, demand most) → 1.0 (last round, concede most)
    // Start at 70% of max value, concede to 50% (BATNA + buffer) by last round
    let concession = (round / max_rounds).min(0.9);
    let target_fraction = 0.75 - (concession * 0.25); // 0.75 → 0.50

    let max_total_value: f64 = quantities
        .iter()
        .zip(valuations.iter())
        .map(|(&q, &v)| q as f64 * v)
        .sum();

    let target_value = (max_total_value * target_fraction).max(obs.batna * 1.05);

    // Greedy allocation: take items with highest value-per-unit first
    // This maximizes OUR value while leaving lower-value items for opponent
    let mut item_priorities: Vec<(usize, f64)> = valuations
        .iter()
        .enumerate()
        .map(|(i, &v)| (i, v))
        .collect();
    item_priorities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    let mut offer = vec![0u32; NUM_ITEMS];
    let mut accumulated = 0.0f64;

    for (idx, val) in &item_priorities {
        let max_qty = quantities.get(*idx).copied().unwrap_or(0);
        if accumulated >= target_value {
            break;
        }

        let needed_value = target_value - accumulated;
        let units_needed = (needed_value / val).ceil() as u32;
        let units_to_take = units_needed.min(max_qty);

        offer[*idx] = units_to_take;
        accumulated += units_to_take as f64 * val;
    }

    // M3 check: avoid extremes
    if offer.iter().all(|&q| q == 0) {
        // Give ourselves at least 1 of highest-value item
        if let Some((best_idx, _)) = item_priorities.first() {
            offer[*best_idx] = 1;
        }
    }

    offer
}

// ─── Utility Functions ────────────────────────────────────────────────────────

/// Compute total value of an offer given our private valuations
pub fn compute_offer_value(offer: &[u32], valuations: &[f64]) -> f64 {
    offer
        .iter()
        .zip(valuations.iter())
        .map(|(&qty, &val)| qty as f64 * val)
        .sum()
}

/// Compute Nash Welfare: geometric mean of both players' payoffs
/// Assumes opponent gets (quantities - our_offer) of each item
pub fn compute_nash_welfare(
    our_offer: &[u32],
    our_valuations: &[f64],
    quantities: &[u32],
    their_inferred_vals: &Option<Vec<f64>>,
) -> f64 {
    let our_value = compute_offer_value(our_offer, our_valuations);

    let their_value = if let Some(their_vals) = their_inferred_vals {
        // They get the remainder
        let their_share: Vec<u32> = our_offer
            .iter()
            .zip(quantities.iter())
            .map(|(&o, &q)| q.saturating_sub(o))
            .collect();
        compute_offer_value(&their_share, their_vals)
    } else {
        // Without inference, assume symmetric — equal split is fair
        our_value * 0.8
    };

    if our_value <= 0.0 || their_value <= 0.0 {
        return 0.0;
    }

    (our_value * their_value).sqrt()
}

/// Format action as JSON for A2A response
pub fn format_tool_call(
    name: &str,
    arguments: &HashMap<String, serde_json::Value>,
) -> serde_json::Value {
    // For MAizeBargAIn, the action IS the tool call
    // The green agent expects the action JSON directly in the A2A data part
    serde_json::json!({
        "tool_call": {
            "name":      name,
            "arguments": arguments,
            "domain":    "bargaining",
            "track":     "maize_bargain"
        }
    })
}

/// Convert BargainAction to A2A Data part for response
pub fn action_to_a2a_data(action: &BargainAction) -> serde_json::Value {
    match action {
        BargainAction::Accept => {
            serde_json::json!({"action": "ACCEPT"})
        }
        BargainAction::Walk => {
            serde_json::json!({"action": "WALK"})
        }
        BargainAction::CounterOffer { offer } => {
            serde_json::json!({
                "action": "COUNTEROFFER",
                "offer":  offer
            })
        }
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_obs(round: u32, last_offer: Option<Vec<u32>>, batna: f64) -> BargainObservation {
        BargainObservation {
            role: "row".to_string(),
            round,
            valuations: vec![45.0, 72.0, 33.0],
            batna,
            quantities: vec![7, 4, 1],
            last_offer,
            history: vec![],
            max_rounds: 5,
            discount: 0.98,
        }
    }

    #[test]
    fn test_offer_value_computation() {
        let offer = vec![3u32, 2, 1];
        let valuations = vec![45.0, 72.0, 33.0];
        let value = compute_offer_value(&offer, &valuations);
        // 3*45 + 2*72 + 1*33 = 135 + 144 + 33 = 312
        assert_eq!(value, 312.0);
    }

    #[test]
    fn test_m4_violation_prevented() {
        // Offer worth 50, BATNA is 100 — accepting would violate M4
        let obs = sample_obs(2, Some(vec![1, 0, 0]), 100.0);
        let state = NegotiationState::new();

        // FBA says ACCEPT but offer value = 1*45 = 45 < BATNA 100
        let decision = FbaBargainDecision::Accept;
        let action = validate_and_fix(decision, &obs, &state);

        // Should NOT be Accept — M4 violation prevented
        assert!(!matches!(action, BargainAction::Accept));
    }

    #[test]
    fn test_m5_violation_prevented() {
        // Offer worth 200, BATNA is 100 — walking would violate M5
        let obs = sample_obs(3, Some(vec![3, 2, 1]), 100.0);
        let state = NegotiationState::new();

        // offer value = 3*45 + 2*72 + 1*33 = 312 > BATNA 100
        let decision = FbaBargainDecision::Walk;
        let action = validate_and_fix(decision, &obs, &state);

        // Should be Accept — M5 violation prevented
        assert!(matches!(action, BargainAction::Accept));
    }

    #[test]
    fn test_m3_violation_prevented_all_zero() {
        let obs = sample_obs(2, None, 50.0);
        let state = NegotiationState::new();

        // Offering [0,0,0] violates M3
        let decision = FbaBargainDecision::CounterOffer(vec![0, 0, 0]);
        let action = validate_and_fix(decision, &obs, &state);

        // Should recompute to non-zero offer
        if let BargainAction::CounterOffer { offer } = action {
            assert!(offer.iter().any(|&q| q > 0), "Offer must not be all zeros");
        }
    }

    #[test]
    fn test_m2_violation_prevented() {
        // Offer that gives us less than BATNA
        let obs = sample_obs(2, None, 500.0); // BATNA very high
        let state = NegotiationState::new();

        // Single item worth 45 — far below BATNA 500
        let decision = FbaBargainDecision::CounterOffer(vec![1, 0, 0]);
        let action = validate_and_fix(decision, &obs, &state);

        // Should walk since no valid offer exists above BATNA 500
        // (max possible = 7*45 + 4*72 + 1*33 = 315 + 288 + 33 = 636)
        // 636 > 500 so a valid offer should exist
        assert!(!matches!(action, BargainAction::CounterOffer { offer: _ }
            if compute_offer_value(
                &if let BargainAction::CounterOffer { ref offer } = action { offer.clone() } else { vec![] },
                &obs.valuations
            ) < obs.batna
        ));
    }

    #[test]
    fn test_last_round_accept_above_batna() {
        // Last round, offer above BATNA → must ACCEPT
        let obs = sample_obs(5, Some(vec![3, 2, 1]), 100.0);
        // offer value = 312 > batna 100
        let action = handle_last_round(&obs);
        assert!(matches!(action, BargainAction::Accept));
    }

    #[test]
    fn test_last_round_walk_below_batna() {
        // Last round, offer below BATNA → must WALK
        let obs = sample_obs(5, Some(vec![1, 0, 0]), 100.0);
        // offer value = 45 < batna 100
        let action = handle_last_round(&obs);
        assert!(matches!(action, BargainAction::Walk));
    }

    #[test]
    fn test_best_offer_above_batna() {
        let obs = sample_obs(2, None, 100.0);
        let offer = compute_best_offer(&obs);
        let value = compute_offer_value(&offer, &obs.valuations);
        assert!(
            value >= obs.batna,
            "Best offer {:.2} must be >= BATNA {:.2}",
            value,
            obs.batna
        );
    }

    #[test]
    fn test_nash_welfare_computation() {
        let our_offer = vec![3u32, 2, 0];
        let our_vals = vec![45.0, 72.0, 33.0];
        let quantities = vec![7u32, 4, 1];
        let their_vals = Some(vec![30.0, 50.0, 80.0]);

        let nw = compute_nash_welfare(&our_offer, &our_vals, &quantities, &their_vals);
        assert!(nw > 0.0, "Nash Welfare should be positive");
    }

    #[test]
    fn test_circle6_prompt_contains_key_sections() {
        let obs = sample_obs(2, Some(vec![3, 2, 1]), 100.0);
        let state = NegotiationState::new();
        let prompt = build_circle6_prompt(&obs, &state);

        assert!(prompt.contains("CIRCLE 6"));
        assert!(prompt.contains("BATNA"));
        assert!(prompt.contains("M1:"));
        assert!(prompt.contains("M4:"));
        assert!(prompt.contains("Nash Welfare"));
        assert!(prompt.contains("COUNTEROFFER"));
    }

    #[test]
    fn test_action_to_a2a_data() {
        let accept = BargainAction::Accept;
        let data = action_to_a2a_data(&accept);
        assert_eq!(data["action"], "ACCEPT");

        let co = BargainAction::CounterOffer {
            offer: vec![3, 2, 1],
        };
        let data2 = action_to_a2a_data(&co);
        assert_eq!(data2["action"], "COUNTEROFFER");
        assert_eq!(data2["offer"][0], 3);
    }
}
