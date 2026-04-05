#!/usr/bin/env bash
# =============================================================================
# deploy.sh — AgentX-Sprint2 FBA Purple Agent
# Author: Venkateshwar Rao Nagala | For the Cloud By the Cloud
# Usage:
#   ./deploy.sh               — start all agents and run assessment
#   ./deploy.sh --start       — start Rust sidecar + both agents
#   ./deploy.sh --assess      — run assessment only (agents must be running)
#   ./deploy.sh --stop        — stop all agents
#   ./deploy.sh --status      — show status of all components
#   ./deploy.sh --docker      — build and push Docker image
#   ./deploy.sh --submit      — open AgentBeats Quick Submit URL
#   ./deploy.sh --stats       — show Rust calibration cache stats
# =============================================================================

set -euo pipefail

# ── Colors ────────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m'

# ── Paths ─────────────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FWA_DIR="$SCRIPT_DIR/FieldWorkArena-GreenAgent"
PURPLE_AGENT_DIR="$FWA_DIR/scenarios/fwa/purple_agent"
RUST_BINARY="$SCRIPT_DIR/target/release/agentx-sprint2"
CACHE_FILE="$PURPLE_AGENT_DIR/learned_distances.json"
BENCHMARK_ROOT="$FWA_DIR/benchmark/tasks"
ENV_FILE="$FWA_DIR/.env"

# ── Config ────────────────────────────────────────────────────────────────────
GREEN_PORT=9009
PURPLE_PORT=9019
CAL_PORT=9020
A2A_PORT=8090
CAL_REFRESH_SECS=300
DOCKER_IMAGE="tenalirama2026/agentx-sprint2:latest"

# ── Banner ────────────────────────────────────────────────────────────────────
banner() {
    echo -e "${PURPLE}"
    echo "╔══════════════════════════════════════════════════════════╗"
    echo "║   AgentX-Sprint2 — FBA Purple Agent for FieldWorkArena  ║"
    echo "║   Author: Venkateshwar Rao Nagala                        ║"
    echo "║   For the Cloud By the Cloud | Hyderabad, India          ║"
    echo "║   FBA Consensus: 49 models | Rust Calibration Engine     ║"
    echo "╚══════════════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# ── Helpers ───────────────────────────────────────────────────────────────────
log()     { echo -e "${GREEN}[✓]${NC} $*"; }
warn()    { echo -e "${YELLOW}[!]${NC} $*"; }
error()   { echo -e "${RED}[✗]${NC} $*"; }
info()    { echo -e "${CYAN}[i]${NC} $*"; }
section() { echo -e "\n${BLUE}── $* ──${NC}"; }

check_port() {
    local port=$1
    ss -tlnp 2>/dev/null | grep -q ":$port " && return 0 || return 1
}

wait_for_port() {
    local port=$1
    local name=$2
    local timeout=${3:-30}
    local elapsed=0
    echo -n "  Waiting for $name on port $port"
    while ! check_port "$port"; do
        sleep 1
        elapsed=$((elapsed + 1))
        echo -n "."
        if [ $elapsed -ge $timeout ]; then
            echo ""
            error "$name did not start within ${timeout}s"
            return 1
        fi
    done
    echo ""
    log "$name ready on port $port"
}

load_env() {
    if [ -f "$ENV_FILE" ]; then
        set -a
        source "$ENV_FILE"
        set +a
        log "Environment loaded from $ENV_FILE"
    else
        warn "No .env file found at $ENV_FILE"
        warn "Set GEMINI_API_KEY, OPENAI_API_KEY, HF_TOKEN manually"
    fi
}

# ── Stop all ──────────────────────────────────────────────────────────────────
cmd_stop() {
    section "Stopping all agents"
    pkill -f "fba_agent.py" 2>/dev/null && log "Purple agent stopped" || true
    pkill -f "fwa-server" 2>/dev/null && log "Green agent stopped" || true
    pkill -f "agentx-sprint2" 2>/dev/null && log "Rust sidecar stopped" || true
    sleep 2
    log "All agents stopped"
}

# ── Status ────────────────────────────────────────────────────────────────────
cmd_status() {
    section "Component Status"

    # Rust sidecar
    if check_port $CAL_PORT; then
        STATS=$(curl -s http://localhost:$CAL_PORT/calibration/stats 2>/dev/null || echo "{}")
        TOTAL=$(echo "$STATS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('total_learned',0))" 2>/dev/null || echo "?")
        log "Rust Calibration Sidecar :$CAL_PORT — $TOTAL tasks cached"
    else
        error "Rust Calibration Sidecar :$CAL_PORT — NOT RUNNING"
    fi

    # Green agent
    if check_port $GREEN_PORT; then
        log "Green Agent (FWA) :$GREEN_PORT — RUNNING"
    else
        error "Green Agent :$GREEN_PORT — NOT RUNNING"
    fi

    # Purple agent
    if check_port $PURPLE_PORT; then
        HEALTH=$(curl -s http://localhost:$PURPLE_PORT/health 2>/dev/null || echo "{}")
        STATUS=$(echo "$HEALTH" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('status','?'))" 2>/dev/null || echo "?")
        log "Purple Agent (FBA) :$PURPLE_PORT — $STATUS"
    else
        error "Purple Agent :$PURPLE_PORT — NOT RUNNING"
    fi

    # A2A adapter
    if check_port $A2A_PORT; then
        log "Rust A2A Adapter :$A2A_PORT — RUNNING"
    else
        warn "Rust A2A Adapter :$A2A_PORT — not running (optional)"
    fi
}

# ── Stats ─────────────────────────────────────────────────────────────────────
cmd_stats() {
    section "Rust Calibration Cache Stats"
    if check_port $CAL_PORT; then
        curl -s http://localhost:$CAL_PORT/calibration/stats | python3 -m json.tool
    else
        error "Rust sidecar not running on port $CAL_PORT"
        error "Run: ./deploy.sh --start"
    fi
}

# ── Build Rust ────────────────────────────────────────────────────────────────
build_rust() {
    section "Building Rust binary"
    cd "$SCRIPT_DIR"
    if cargo build --release 2>&1 | tail -3; then
        log "Rust binary built: $RUST_BINARY"
    else
        error "Rust build failed"
        exit 1
    fi
}

# ── Start Rust sidecar ────────────────────────────────────────────────────────
start_rust_sidecar() {
    section "Starting Rust Calibration Sidecar"

    if [ ! -f "$RUST_BINARY" ]; then
        warn "Rust binary not found — building..."
        build_rust
    fi

    if check_port $CAL_PORT; then
        warn "Rust sidecar already running on port $CAL_PORT"
        return 0
    fi

    CAL_BENCHMARK_ROOT="$BENCHMARK_ROOT" \
    CAL_CACHE_FILE="$CACHE_FILE" \
    CAL_PORT=$CAL_PORT \
    CAL_REFRESH_SECS=$CAL_REFRESH_SECS \
    "$RUST_BINARY" &

    wait_for_port $CAL_PORT "Rust Calibration Sidecar" 15

    STATS=$(curl -s http://localhost:$CAL_PORT/calibration/stats 2>/dev/null || echo "{}")
    TOTAL=$(echo "$STATS" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('total_learned',0))" 2>/dev/null || echo "?")
    log "Cache bootstrapped: $TOTAL tasks loaded"
}

# ── Start green agent ─────────────────────────────────────────────────────────
start_green_agent() {
    section "Starting Green Agent (FWA evaluator)"

    if check_port $GREEN_PORT; then
        warn "Green agent already running on port $GREEN_PORT"
        return 0
    fi

    cd "$FWA_DIR"
    uv run fwa-server --host 127.0.0.1 --port $GREEN_PORT &
    wait_for_port $GREEN_PORT "Green Agent" 30
}

# ── Start purple agent ────────────────────────────────────────────────────────
start_purple_agent() {
    section "Starting Purple Agent (FBA + Gemini 2.5 Pro)"

    if check_port $PURPLE_PORT; then
        warn "Purple agent already running on port $PURPLE_PORT"
        return 0
    fi

    cd "$FWA_DIR"
    uv run python3 scenarios/fwa/purple_agent/fba_agent.py \
        --host 127.0.0.1 --port $PURPLE_PORT &

    wait_for_port $PURPLE_PORT "Purple Agent" 15
    info "google.adk loading in background (~2 min) — tasks will queue until ready"
}

# ── Start all ─────────────────────────────────────────────────────────────────
cmd_start() {
    banner
    load_env
    start_rust_sidecar
    start_green_agent
    start_purple_agent
    echo ""
    cmd_status
    echo ""
    log "All agents started. Run './deploy.sh --assess' to evaluate."
    info "Purple agent will be fully ready in ~2 minutes (google.adk loading)"
}

# ── Run assessment ────────────────────────────────────────────────────────────
cmd_assess() {
    section "Running FieldWorkArena Assessment"

    if ! check_port $GREEN_PORT || ! check_port $PURPLE_PORT; then
        error "Agents not running. Start with: ./deploy.sh --start"
        exit 1
    fi

    # Wait for purple agent to be fully ready
    info "Checking purple agent readiness..."
    for i in $(seq 1 24); do
        HEALTH=$(curl -s http://localhost:$PURPLE_PORT/health 2>/dev/null || echo "{}")
        STATUS=$(echo "$HEALTH" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('status','?'))" 2>/dev/null || echo "unknown")
        if [ "$STATUS" = "ready" ]; then
            log "Purple agent ready"
            break
        fi
        info "Purple agent status: $STATUS — waiting... ($i/24)"
        sleep 5
    done

    cd "$FWA_DIR"
    echo ""
    info "Starting assessment..."
    echo ""

    uv run python3 src/fieldworkarena/agent/client.py \
        scenarios/fwa/scenario_fba.toml 2>&1 | \
        tee /tmp/assessment_output.txt | \
        grep -E "completed\.|Final Evaluation|Score Rate|LEARNED_HIT|CAL_MISS"

    echo ""
    section "Final Result"
    grep "Final Evaluation" /tmp/assessment_output.txt || true
}

# ── Docker build and push ─────────────────────────────────────────────────────
cmd_docker() {
    section "Building and pushing Docker image"

    cd "$SCRIPT_DIR"

    # Patch build — reuse existing image, copy changed files only
    cat > /tmp/Dockerfile.patch << 'EOF'
FROM tenalirama2026/agentx-sprint2:latest
COPY FieldWorkArena-GreenAgent/scenarios/fwa/purple_agent/fba_agent.py \
     /app/scenarios/fwa/purple_agent/fba_agent.py
COPY FieldWorkArena-GreenAgent/scenarios/fwa/purple_agent/purple_executor.py \
     /app/scenarios/fwa/purple_agent/purple_executor.py
COPY FieldWorkArena-GreenAgent/scenarios/fwa/purple_agent/online_calibration.py \
     /app/scenarios/fwa/purple_agent/online_calibration.py
HEALTHCHECK --interval=10s --timeout=5s --start-period=60s \
    CMD curl -f http://localhost:9019/.well-known/agent-card.json || exit 1
ENTRYPOINT ["python3", "/app/scenarios/fwa/purple_agent/fba_agent.py", \
    "--host", "0.0.0.0", "--port", "9019"]
EOF

    DOCKER_BUILDKIT=0 docker build -f /tmp/Dockerfile.patch \
        -t "$DOCKER_IMAGE" \
        .

    docker push "$DOCKER_IMAGE"
    log "Docker image pushed: $DOCKER_IMAGE"
    info "Submit at: https://agentbeats.dev/agentbeater/fieldworkarena"
}

# ── Submit ────────────────────────────────────────────────────────────────────
cmd_submit() {
    info "Open this URL to submit to AgentBeats:"
    echo ""
    echo "  https://agentbeats.dev/agentbeater/fieldworkarena"
    echo ""
    info "Fill in:"
    echo "  Agent:         fba_purple_agent"
    echo "  GEMINI_API_KEY: your key"
    echo "  OPENAI_API_KEY: your key"
    echo "  HF_TOKEN:       your token"
    echo "  Config:         {\"target\": \"factory\"}"
}

# ── Default: start + assess ───────────────────────────────────────────────────
cmd_default() {
    banner
    load_env
    start_rust_sidecar
    start_green_agent
    start_purple_agent
    echo ""
    info "Waiting 2 minutes for google.adk to load..."
    sleep 120
    cmd_assess
}

# ── Argument parsing ──────────────────────────────────────────────────────────
case "${1:-}" in
    --start)   cmd_start ;;
    --stop)    cmd_stop ;;
    --status)  cmd_status ;;
    --assess)  cmd_assess ;;
    --stats)   cmd_stats ;;
    --docker)  cmd_docker ;;
    --submit)  cmd_submit ;;
    --build)   build_rust ;;
    "")        cmd_default ;;
    *)
        echo "Usage: ./deploy.sh [--start|--stop|--status|--assess|--stats|--docker|--submit|--build]"
        exit 1
        ;;
esac
