#!/bin/bash
echo "[START] AgentX-Sprint2 container starting..."

# Create prompts directory
mkdir -p /app/scenarios/fwa/purple_agent/prompts

# Start Rust calibration sidecar
if [ -f /app/agentx-sprint2 ]; then
    GATEWAY_JWT_SECRET=agentx-internal-token \
    CAL_BENCHMARK_ROOT=/app/FieldWorkArena-GreenAgent/benchmark/tasks \
    CAL_CACHE_FILE=/app/scenarios/fwa/purple_agent/learned_distances.json \
    CAL_PORT=9020 \
    CAL_REFRESH_SECS=300 \
    PORT=18090 \
    /app/agentx-sprint2 &
    sleep 3
    echo "[START] ✅ Rust sidecar started"
else
    echo "[START] WARNING: Rust binary not found"
fi

# Start Python FBA agent
echo "[START] Starting Python FBA agent on port 9019..."
exec python3 /app/scenarios/fwa/purple_agent/fba_agent.py \
    --host 0.0.0.0 --port 9019
