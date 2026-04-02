// src/fba/client.rs — FBA Pipeline HTTP Client
use crate::a2a::types::{FbaAction, FbaRequest, FbaResponse};
use anyhow::Result;
use tracing::warn;

pub struct FbaClient {
    endpoint: String,
    jwt_secret: String,
    http: reqwest::Client,
}

impl FbaClient {
    pub fn new(endpoint: &str, jwt_secret: &str) -> Self {
        Self {
            endpoint: endpoint.to_string(),
            jwt_secret: jwt_secret.to_string(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .expect("HTTP client failed"),
        }
    }

    pub async fn run_consensus(&self, req: FbaRequest) -> Result<FbaResponse> {
        let response = self
            .http
            .post(format!("{}/modernize", self.endpoint))
            .bearer_auth(self.mint_jwt())
            .json(&serde_json::json!({
                "cobol_code": req.cobol_code,
                "context_id": req.context_id,
                "track":      format!("{:?}", req.track),
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            warn!("FBA returned: {}", response.status());
            return Ok(FbaResponse {
                consensus_reached: false,
                confidence: 0.0,
                quorum: 0,
                action: FbaAction::Abstain {
                    reason: "FBA unavailable".into(),
                },
                reasoning_steps: 0,
            });
        }

        let raw: serde_json::Value = response.json().await?;
        let quorum = raw["consensus_nodes"].as_u64().unwrap_or(0) as u32;
        let confidence = raw["confidence"].as_f64().unwrap_or(0.0);
        let consensus = quorum >= 39 && confidence >= 0.94;

        let action = if !consensus {
            FbaAction::Abstain {
                reason: format!(
                    "Quorum {}/49 @ {:.1}% below 39/94% threshold",
                    quorum,
                    confidence * 100.0
                ),
            }
        } else if let Some(tc) = raw["tool_call"].as_object() {
            FbaAction::ToolCall {
                name: tc["name"].as_str().unwrap_or("").to_string(),
                arguments: serde_json::from_value(tc["arguments"].clone()).unwrap_or_default(),
            }
        } else {
            FbaAction::TextResponse {
                text: raw["response"]
                    .as_str()
                    .or_else(|| raw["rust_code"].as_str())
                    .unwrap_or("FBA consensus response")
                    .to_string(),
            }
        };

        Ok(FbaResponse {
            consensus_reached: consensus,
            confidence,
            quorum,
            action,
            reasoning_steps: raw["reasoning_steps"].as_u64().unwrap_or(89) as u32,
        })
    }

    fn mint_jwt(&self) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::{SystemTime, UNIX_EPOCH};
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let h = b64(r#"{"alg":"HS256","typ":"JWT"}"#);
        let p = b64(&format!(
            r#"{{"sub":"agentx-sprint2","role":"purple_agent","exp":{}}}"#,
            exp
        ));
        let mut hasher = DefaultHasher::new();
        format!("{}.{}{}", h, p, self.jwt_secret).hash(&mut hasher);
        format!("{}.{}.{}", h, p, b64(&format!("{:x}", hasher.finish())))
    }
}

fn b64(s: &str) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut r = String::new();
    for ch in s.as_bytes().chunks(3) {
        let (b0, b1, b2) = (
            ch[0] as usize,
            if ch.len() > 1 { ch[1] as usize } else { 0 },
            if ch.len() > 2 { ch[2] as usize } else { 0 },
        );
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
