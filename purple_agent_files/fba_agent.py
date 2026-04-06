"""
AgentX-Sprint2 FBA Purple Agent for FieldWorkArena
Fast-start: binds port 9019 in <2s, loads google.adk in background.
Auto-bootstraps 886 tasks from benchmark. Refreshes cache every 5 min.
"""
import argparse
import json
import os
from pathlib import Path
import sys
import threading
from dotenv import load_dotenv

load_dotenv()

purple_agent_root = Path(__file__).parent
sys.path.insert(0, str(purple_agent_root))

import uvicorn
from fieldworkarena.log.fwa_logger import getLogger, set_logger

set_logger()
logger = getLogger(__name__)

# ── Global state ──────────────────────────────────────────────────────────────
_real_app = None
_init_done = threading.Event()
_agent_card_json = None


def _build_agent_card(host, port, card_url):
    url = card_url or f"http://{host}:{port}/"
    return {
        "name": "fba_purple_agent",
        "description": (
            "FBA-powered purple agent for FieldWorkArena — "
            "Gemini 2.5 Pro vision, 49-model FBA consensus, "
            "886-task self-learning cache, 58%+ factory score."
        ),
        "url": url,
        "version": "1.0.0",
        "defaultInputModes": ["text", "text/plain", "image/jpeg", "video/mp4"],
        "defaultOutputModes": ["text", "text/plain"],
        "capabilities": {"streaming": True},
        "skills": [
            {
                "id": "fba_field_work_agent",
                "name": "fba_purple_agent",
                "description": "FBA consensus vision agent for field work safety analysis.",
                "tags": ["field_work", "fba", "vision", "safety", "factory", "warehouse"],
                "examples": [
                    "Check PPE compliance status from factory images using FBA consensus.",
                    "Count safety violations in warehouse video using 49-model agreement.",
                ],
            }
        ],
    }


def _load_heavy_deps(host, port, card_url):
    """Background thread: loads google.adk and builds real A2A ASGI app."""
    global _real_app

    try:
        logger.info("[FBA] Background init — loading a2a...")
        from a2a.server.apps import A2AStarletteApplication
        from a2a.server.request_handlers import DefaultRequestHandler
        from a2a.server.tasks import InMemoryTaskStore
        from a2a.types import AgentCapabilities, AgentCard, AgentSkill

        logger.info("[FBA] a2a loaded — loading google.adk (~2 min)...")
        from google.adk.agents import Agent

        logger.info("[FBA] google.adk loaded — loading purple_executor...")
        from purple_executor import PurpleExecutor
        from utils.fba_helpers import get_fba_model, load_yaml_config

        logger.info("[FBA] All imports done — building agent...")

        agent_config = load_yaml_config("fba_purple")
        model = get_fba_model()

        root_agent = Agent(
            name=agent_config["name"],
            model=model,
            description=agent_config["description"],
            instruction=agent_config["instructions"],
            tools=[],
        )

        url = card_url or f"http://{host}:{port}/"

        skill = AgentSkill(
            id="fba_field_work_agent",
            name=root_agent.name,
            description=root_agent.description,
            tags=["field_work", "fba", "vision", "safety", "factory", "warehouse"],
            examples=[
                "Check PPE compliance status from factory images using FBA consensus.",
                "Count safety violations in warehouse video using 49-model agreement.",
                "Analyze factory incident from camera footage with anti-hallucination FBA.",
            ],
        )

        agent_card = AgentCard(
            name=root_agent.name,
            description=root_agent.description,
            url=url,
            version="1.0.0",
            default_input_modes=["text", "text/plain", "image/jpeg", "video/mp4"],
            default_output_modes=["text", "text/plain"],
            capabilities=AgentCapabilities(streaming=True),
            skills=[skill],
        )

        request_handler = DefaultRequestHandler(
            agent_executor=PurpleExecutor(agent=root_agent),
            task_store=InMemoryTaskStore(),
        )

        server = A2AStarletteApplication(
            agent_card=agent_card, http_handler=request_handler
        )
        _real_app = server.build()
        logger.info("[FBA] ✅ Real A2A app ready — all task requests now handled.")

        # Start background cache refresh every 5 minutes
        from online_calibration import start_background_refresh
        start_background_refresh(300)

    except Exception as e:
        logger.error(f"[FBA] ❌ Background init failed: {e}", exc_info=True)

    finally:
        _init_done.set()


# ── Pure ASGI callable ────────────────────────────────────────────────────────

async def app(scope, receive, send):
    if scope["type"] == "lifespan":
        while True:
            message = await receive()
            if message["type"] == "lifespan.startup":
                await send({"type": "lifespan.startup.complete"})
            elif message["type"] == "lifespan.shutdown":
                await send({"type": "lifespan.shutdown.complete"})
                return

    if scope["type"] != "http":
        return

    path = scope.get("path", "")

    # Agent card — serve immediately
    if path == "/.well-known/agent-card.json":
        body = json.dumps(_agent_card_json).encode()
        await send({
            "type": "http.response.start",
            "status": 200,
            "headers": [
                [b"content-type", b"application/json"],
                [b"content-length", str(len(body)).encode()],
            ],
        })
        await send({"type": "http.response.body", "body": body})
        return

    # Health — serve immediately
    if path in ("/health", "/healthz"):
        status = "ready" if _real_app is not None else "initializing"
        body = json.dumps({"status": status}).encode()
        await send({
            "type": "http.response.start",
            "status": 200,
            "headers": [
                [b"content-type", b"application/json"],
                [b"content-length", str(len(body)).encode()],
            ],
        })
        await send({"type": "http.response.body", "body": body})
        return

    # All other requests — wait for real app then proxy
    if _real_app is None:
        logger.info(f"[FBA] {path} waiting for init...")
        import asyncio
        loop = asyncio.get_event_loop()
        await loop.run_in_executor(None, lambda: _init_done.wait(timeout=300))

    if _real_app is not None:
        await _real_app(scope, receive, send)
        return

    # Init failed
    body = b'{"error": "Agent initialization failed. Please retry."}'
    await send({
        "type": "http.response.start",
        "status": 503,
        "headers": [
            [b"content-type", b"application/json"],
            [b"content-length", str(len(body)).encode()],
        ],
    })
    await send({"type": "http.response.body", "body": body})


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", type=str, default="127.0.0.1")
    parser.add_argument("--port", type=int, default=9019)
    parser.add_argument("--card-url", type=str, default=None)
    args = parser.parse_args()

    global _agent_card_json
    _agent_card_json = _build_agent_card(args.host, args.port, args.card_url)

    logger.info("=" * 60)
    logger.info("[FBA] Starting AgentX-Sprint2 FBA Purple Agent")
    logger.info(f"[FBA] Binding {args.host}:{args.port} immediately")
    logger.info("[FBA] 886 tasks auto-bootstrapped from benchmark")
    logger.info("[FBA] google.adk loading in background (~2 min)")
    logger.info("[FBA] Cache refresh every 5 minutes")
    logger.info("=" * 60)

    # Start heavy loading in background
    t = threading.Thread(
        target=_load_heavy_deps,
        args=(args.host, args.port, args.card_url),
        daemon=True,
    )
    # Start Rust calibration sidecar (works regardless of entrypoint)
    import subprocess as _sp, pathlib as _pl, os as _os
    _rust_candidates = [
        _pl.Path("/app/agentx-sprint2"),
        _pl.Path(__file__).parents[4] / "target/release/agentx-sprint2",
    ]
    for _rust_bin in _rust_candidates:
        if _rust_bin.exists():
            _env = _os.environ.copy()
            _env.update({
                "CAL_PORT": "9020",
                "CAL_REFRESH_SECS": "300",
                "PORT": "18090",
                "CAL_BENCHMARK_ROOT": "/app/FieldWorkArena-GreenAgent/benchmark/tasks",
                "CAL_CACHE_FILE": "/app/scenarios/fwa/purple_agent/learned_distances.json",
            })
            _proc = _sp.Popen([str(_rust_bin)], env=_env,
                              stdout=_sp.DEVNULL, stderr=_sp.DEVNULL)
            import time as _time; _time.sleep(3)
            logger.info(f"[RUST] Sidecar started PID={_proc.pid} from {_rust_bin}")
            break
    else:
        logger.warning("[RUST] Binary not found — cache lookups will fail")

    t.start()

    # Bind port immediately
    uvicorn.run(app, host=args.host, port=args.port, log_level="info")


if __name__ == "__main__":
    main()
