# AgentX-Phase2 → AgentX-Sprint2: Copy Decision Matrix

## ✅ COPY — Reuse directly

| File/Folder | From Phase2 | To Sprint2 | Purpose |
|---|---|---|---|
| `.env` | `/mnt/d/AgentX-Phase2/.env` | `/mnt/d/AgentX-Sprint2/.env` | API keys (ANTHROPIC, NEBIUS, JWT, AWS) |
| `kind-config.yaml` | root | root | Same kind cluster reused |
| `deploy.sh` | root | `scripts/deploy_phase2_ref.sh` | Reference for port-forwards |
| `.github/workflows/*.yml` | `.github/workflows/` | `phase2_ref/` | CI pattern reference |
| `k8s/` or `manifests/` | root | `k8s_phase2_ref/` | Service mesh config reference |
| `docker-compose*.yml` | root | `*_phase2_ref.yml` | Local testing reference |

## ❌ DO NOT COPY — Sprint2 replaces these

| File/Folder | Reason |
|---|---|
| `src/purple_agent/` | Sprint2 has new Rust A2A adapter instead |
| `src/green_agent/` | Not needed — green agents are on AgentBeats platform |
| `src/s3_mcp/` | S3 MCP not needed for benchmark tracks |
| `src/agent_gateway/` | Sprint2 uses AgentBeats gateway, not internal one |
| `Cargo.toml` (Phase2) | Sprint2 has new dependencies (axum, a2a types) |
| `Dockerfile` (Phase2) | Sprint2 has new single-binary Dockerfile |
| `.github/workflows/agentx-deploy.yml` | Sprint2 has agentx-sprint2.yml instead |
| `k8s/purple-agent-deployment.yaml` | Sprint2 deploys differently (AgentBeats) |

## 🔗 CALL VIA HTTP — Do not copy, just reference endpoint

| Component | Phase2 Location | Sprint2 Usage |
|---|---|---|
| FBA pipeline | `purple-agent:8081/modernize` | `FBA_ENDPOINT=http://localhost:8085` |
| 49-model consensus | Inside purple_agent pod | Called via `fba/client.rs` HTTP POST |
| JWT auth | `GATEWAY_JWT_SECRET` | Reused from `.env` — same secret |

## Key Insight

Sprint2's A2A adapter in Rust is a **thin wrapper** that:
```
AgentBeats green agent
        ↓ A2A protocol
  AgentX-Sprint2 (NEW Rust binary, port 8090)
        ↓ HTTP + JWT (existing auth pattern)
  AgentX-Phase2 purple_agent (UNCHANGED, port 8085)
        ↓ FBA consensus
  49 models × 89 steps → 4,361 verified reasoning steps
```

Phase2 stays completely frozen. Sprint2 just adds the A2A translation layer on top.
