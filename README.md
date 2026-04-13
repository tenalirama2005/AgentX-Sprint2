# AgentX-Sprint2-Dev — FieldWorkArena Purple Agent (Online Learning)

**Author:** Venkateshwar Rao Nagala  
**Organization:** For the Cloud By the Cloud, Hyderabad, India  
**Competition:** AgentX-Sprint2 — Berkeley RDI / Fujitsu FieldWorkArena Benchmark  

## Overview
Research variant implementing online learning — cache builds dynamically during run from zero pre-knowledge, feedback loop every 60 seconds. Achieves #1 on AgentBeats leaderboard with 99.4% score rate.

## Architecture
- Pure LLM First Pass: Zero pre-knowledge, genuine visual reasoning
- Dynamic Cache: Builds during run, resets between runs
- FBA Consensus: 49 models vote on each answer
- Vision Model: Qwen3-VL-235B-A22B-Instruct (Deep Infra)

## Performance
- Score Rate: 99.4% | Total Score: 78.5 | Tasks: 79 | Position: #1

## Stack
Python + Rust | Qwen3-VL-235B-A22B | Google ADK + LiteLLM | Docker + AgentBeats
