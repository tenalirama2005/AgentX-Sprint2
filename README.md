# AgentX-Sprint2

**FBA-powered A2A Purple Agent — AgentBeats Phase 2, Sprint 2**
**Deadline: April 12, 2026**

> AgentX-Sprint2 replaces single-LLM hallucination risk with Federated Byzantine Agreement
> consensus across 49 heterogeneous AI models. An action is taken only when **39 of 49 models**
> agree at ≥94% confidence across 4,361 verified reasoning steps — making hallucination
> structurally impossible at the architectural level, not just probabilistically suppressed.

---

## Tracks Targeted

| Track | Benchmark | Leaderboard Status |
|---|---|---|
| Multi-Agent | τ²-Bench (telecom) | 2 entries — wide open |
| Computer Use | CAR-bench | Active competition |
| Multi-Agent | MAizeBargAIn | Active competition |
| Computer Use | OSWorld-Verified | Active competition |

---

## Project Structure

```
AgentX-Sprint2/
├── Cargo.toml
├── Dockerfile
├── .env.example
├── .github/
│   └── workflows/
│       └── agentx-sprint2.yml
├── src/
│   ├── main.rs                  # Axum server — all 4 tracks
│   ├── a2a/
│   │   ├── mod.rs
│   │   ├── types.rs             # A2A protocol structs
│   │   ├── handler.rs           # POST /a2a/tasks/send
│   │   └── card.rs              # GET /.well-known/agent.json
│   ├── fba/
│   │   ├── mod.rs
│   │   └── client.rs            # HTTP client → purple_agent FBA
│   └── tracks/
│       ├── mod.rs
│       ├── car_bench.rs         # 58 tool mappings
│       ├── tau2.rs              # Telecom tool mappings
│       ├── maize.rs             # Bargaining tool mappings
│       └── osworld.rs           # GUI action mappings
└── tests/
    └── a2a_conformance.rs
```

---

## Local Setup (WSL)

```bash
# 1. Create project on D:\ drive
mkdir -p /mnt/d/AgentX-Sprint2
cd /mnt/d/AgentX-Sprint2

# 2. Copy all generated files into place
# (see file placement guide below)

# 3. Create GitHub repo
gh repo create tenalirama2005/AgentX-Sprint2 \
  --public \
  --description "FBA-powered A2A purple agent — CAR-bench, τ²-Bench, MAizeBargAIn, OSWorld"

# 4. Push initial commit
git init
git remote add origin https://github.com/tenalirama2005/AgentX-Sprint2.git
git add .
git commit -m "feat: AgentX-Sprint2 — FBA A2A adapter for 4 benchmark tracks"
git push -u origin main

# 5. Set GitHub Secrets
echo -n "$GATEWAY_JWT_SECRET" | gh secret set GATEWAY_JWT_SECRET --repo tenalirama2005/AgentX-Sprint2
echo -n "$FBA_ENDPOINT"       | gh secret set FBA_ENDPOINT       --repo tenalirama2005/AgentX-Sprint2

# 6. Build locally
cargo build --release

# 7. Test agent card
GATEWAY_JWT_SECRET=test FBA_ENDPOINT=http://localhost:8081 \
  AGENT_URL=http://localhost:8090 \
  cargo run --release &
sleep 3
curl http://localhost:8090/.well-known/agent.json | jq .
curl http://localhost:8090/health | jq .
```

---

## File Placement Guide

```
Cargo.toml          → /mnt/d/AgentX-Sprint2/Cargo.toml
Dockerfile          → /mnt/d/AgentX-Sprint2/Dockerfile
main.rs             → /mnt/d/AgentX-Sprint2/src/main.rs
types.rs            → /mnt/d/AgentX-Sprint2/src/a2a/types.rs
handler.rs          → /mnt/d/AgentX-Sprint2/src/a2a/handler.rs
modules.rs          → split into:
                      /mnt/d/AgentX-Sprint2/src/a2a/card.rs      (agent card section)
                      /mnt/d/AgentX-Sprint2/src/fba/client.rs    (FBA client section)
                      /mnt/d/AgentX-Sprint2/src/tracks/car_bench.rs
                      /mnt/d/AgentX-Sprint2/src/tracks/tau2.rs
                      /mnt/d/AgentX-Sprint2/src/tracks/maize.rs
                      /mnt/d/AgentX-Sprint2/src/tracks/osworld.rs
agentx-sprint2.yml  → /mnt/d/AgentX-Sprint2/.github/workflows/agentx-sprint2.yml
```

---

## AgentBeats Submission Steps

### 1. Register on agentbeats.dev
- URL: `https://ghcr.io/tenalirama2005/agentx-sprint2:latest`
- Register once, use for all 4 tracks

### 2. CAR-bench Submission
```bash
git clone https://github.com/CAR-bench/car-bench-leaderboard-agentbeats.git
# Edit scenario.toml with your agent ID
# Add GitHub Secrets, push PR
```

### 3. τ²-Bench Submission
```bash
# Quick Submit:
# https://agentbeats.dev/agentbeater/tau2-bench/submit
# OR manual PR to: RDI-Foundation/tau2-agentbeats-leaderboard
```

### 4. MAizeBargAIn + OSWorld
- Check agentbeats.dev for leaderboard repo links as they go live

---

## FBA Parameters

```
Total models  : 49 (1 Claude Opus 4.6 + 48 Nebius)
Quorum        : 39 of 49  (Byzantine fault tolerance: f=10)
Confidence    : ≥94%
Reasoning     : 49 × 89 = 4,361 verified steps
Paper         : arxiv:2507.11768
GitHub (P2)   : github.com/tenalirama2005/AgentX-Phase2 (frozen — in review)
```

---

## Key Differentiator: Structural Anti-Hallucination

For CAR-bench Hallucination tasks (98 tasks):
- Required tool is deliberately REMOVED
- Single-LLM agents: fabricate a response → score 0
- AgentX-Sprint2: FBA quorum of 39/49 NOT reached → `FbaAction::Abstain` → correct refusal → score 1

For τ²-Bench Pass Rate:
- Single-LLM temperature variance → inconsistent across runs
- FBA at 94% confidence → near-deterministic → consistent pass rate
