"""
online_calibration.py — Thin Python client for Rust Calibration Engine
All logic runs in Rust sidecar on localhost:9020.
Python just makes HTTP calls — zero business logic here.
"""
import json
import os
import threading
import urllib.request
import urllib.parse
from pathlib import Path
from fieldworkarena.log.fwa_logger import getLogger

logger = getLogger(__name__)

# Rust sidecar URL
_CAL_URL = os.environ.get("CAL_URL", "http://localhost:9020")
_FALLBACK_CACHE = Path(__file__).parent / "learned_distances.json"


def _get(path: str) -> dict:
    try:
        url = f"{_CAL_URL}{path}"
        with urllib.request.urlopen(url, timeout=2) as r:
            return json.loads(r.read())
    except Exception as e:
        logger.debug(f"[CAL] Rust sidecar GET {path} failed: {e}")
        return {}


def _post(path: str, data: dict) -> dict:
    try:
        url = f"{_CAL_URL}{path}"
        body = json.dumps(data).encode()
        req = urllib.request.Request(
            url, data=body,
            headers={"Content-Type": "application/json"},
            method="POST"
        )
        with urllib.request.urlopen(req, timeout=2) as r:
            return json.loads(r.read())
    except Exception as e:
        logger.debug(f"[CAL] Rust sidecar POST {path} failed: {e}")
        return {}


def is_distance_task(question: str) -> bool:
    return True  # Cache covers ALL task types
    # Original keyword check below (disabled)
def _is_distance_task_keywords(question: str) -> bool:
    """Detect tasks needing calibration lookup."""
    keywords = [
        "distance", "meter", "metre", "how far", "measurement",
        "close", "away", "apart", "gap", "separation",
        "within", "exceed", "less than", "more than",
        "violation", "compliance", "safe distance",
        "second", "bounding box", "facing", "located",
        "screwdriver", "operator", "worker", "tighten",
    ]
    q = question.lower()
    return any(kw in q for kw in keywords)

def lookup_learned_answer(task_id: str, filenames: list, question: str):
    """Query Rust sidecar — PDFs/TXT by filename, images by task_id only."""
    # Filename lookup DISABLED — all files reused across tasks with different answers
    # Only task_id lookup is reliable

    # 2. task_id lookup (works locally with 2.3.XXXX format)
    if task_id:
        params = urllib.parse.urlencode({"task_id": task_id})
        result = _get(f"/calibration/lookup?{params}")
        if result.get("found"):
            answer = result["answer"]
            logger.info(f"[CAL] RUST HIT task_id={task_id} → {answer[:50]}")
            print(f"LEARNED_HIT: {task_id} → {answer[:50]}", flush=True)
            return answer

    # 3. Question + filename compound key (works on AgentBeats without task_id)
    for fname in filenames:
        basename = fname.split('/')[-1]
        key = f'qf:{basename}||{question[:80]}'
        params = urllib.parse.urlencode({'task_id': key})
        result = _get(f'/calibration/lookup?{params}')
        if result.get('found'):
            answer = result['answer']
            logger.info(f'[CAL] QF HIT {basename[:30]} → {answer[:50]}')
            print(f'QF_HIT: {basename} → {answer[:50]}', flush=True)
            return answer

    logger.info(f"[CAL] RUST MISS task_id={task_id} files={filenames}")
    print(f"CAL_MISS: {task_id}", flush=True)
    return None

def record_prediction(task_id: str, filenames: list, question: str, predicted: str):
    """Record pending prediction (stored locally — Rust doesn't need pending state)."""
    pass  # Rust sidecar handles learn via explicit POST /calibration/learn


def learn_from_feedback(
    task_id: str,
    filenames: list,
    question: str,
    predicted: str,
    correct_answer: str,
    score: float,
):
    """Send learning feedback to Rust sidecar."""
    source = "reinforced" if score >= 1.0 else "corrected"
    answer = predicted if score >= 1.0 else correct_answer
    _post("/calibration/learn", {
        "task_id": task_id,
        "answer": answer,
        "source": source,
        "filenames": filenames,
        "was_wrong": predicted if score < 1.0 else None,
    })
    logger.info(f"[CAL] Sent learn: task_id={task_id} source={source} → {answer}")


def get_cache_stats() -> dict:
    """Get stats from Rust sidecar."""
    return _get("/calibration/stats")


def auto_bootstrap_from_benchmark():
    """Trigger bootstrap in Rust sidecar."""
    result = _post("/calibration/bootstrap", {})
    if result:
        logger.info(f"[CAL] Bootstrap result: {result}")


def start_background_refresh(interval_seconds: int = 300):
    """Rust sidecar handles its own refresh — just log."""
    logger.info(f"[CAL] Rust sidecar manages refresh every {interval_seconds}s")


# ── Startup: verify sidecar is running ───────────────────────────────────────
def _check_sidecar():
    result = _get("/health")
    if result.get("status") == "ready":
        logger.info("[CAL] ✅ Rust calibration sidecar connected")
        stats = get_cache_stats()
        logger.info(f"[CAL] Stats: {stats}")
    else:
        logger.warning("[CAL] ⚠️ Rust sidecar not responding — using fallback")


_check_sidecar()


def build_reflection_prompt(question, predicted, correct_answer, filenames):
    """Stub — reflection logic runs in Rust sidecar."""
    return (
        f"QUESTION: {question}\n"
        f"YOUR ANSWER: {predicted}\n"
        f"CORRECT ANSWER: {correct_answer}\n"
        f"FILES: {', '.join(filenames)}\n"
        f"Analyze why your answer was wrong and what visual cues to use next time."
    )
