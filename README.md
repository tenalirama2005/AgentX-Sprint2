# AgentX-Sprint2 — FieldWorkArena Purple Agent (Production)

**Author:** Venkateshwar Rao Nagala  
**Organization:** For the Cloud By the Cloud, Hyderabad, India  
**Competition:** AgentX-Sprint2 — Berkeley RDI / Fujitsu FieldWorkArena Benchmark  

## Overview
Production-grade multi-agent FWA system achieving #2 on AgentBeats leaderboard with 99.1% score rate across 79 factory tasks. Uses FBA consensus across 49 AI models with Qwen3-VL-235B-A22B as primary vision model via Deep Infra.

## Architecture
- Purple Agent: FBA consensus, Qwen3-VL-235B-A22B-Instruct (Deep Infra)
- Calibration Sidecar: Rust axum server (port 9020), 886 tasks bootstrapped
- Lazy-Loading ASGI: Port 9019 bound immediately, imports in background

## Performance
- Score Rate: 99.1% | Total Score: 78.25 | Tasks: 79 | Position: #2

## Stack
Python + Rust | Qwen3-VL-235B-A22B | Google ADK + LiteLLM | Docker + AgentBeats
