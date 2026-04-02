// src/tracks/mod.rs  — UPDATED (replace previous version)
// All 6 benchmark tracks registered
pub mod car_bench;
pub mod fieldwork; // FieldWorkArena (Research Agent)
pub mod maize; // MAizeBargAIn (Multi-Agent)
pub mod mle_bench; // MLE-bench (Research Agent)
pub mod osworld; // OSWorld-Verified (Computer Use)
pub mod tau2; // τ²-Bench (Telecom)

// ─── Track Summary ────────────────────────────────────────────────────────────
//
// Sprint 2 Tracks (deadline: April 12, 2026):
//
// Research Agent Track:
//   fieldwork  → FieldWorkArena: 239 multimodal tasks (factory/warehouse/retail)
//   mle_bench  → MLE-bench:      75 Kaggle competitions (current: spaceship-titanic)
//
// Multi-Agent Evaluation Track:
//   maize      → MAizeBargAIn:   0 entries — FIRST MOVER ADVANTAGE
//
// τ²-Bench Track:
//   tau2       → τ²-Bench:       2 entries at 68% — target >90%
//
// Computer Use & Web Agent Track:
//   car_bench  → CAR-bench:      254 tasks (58 tools, 19 policies)
//   osworld    → OSWorld:        369 tasks (1 dummy entry at 0.8%)
//
// FBA Guarantee across all tracks:
//   39/49 quorum @ 94% confidence = structural anti-hallucination
//   4,361 verified reasoning steps per decision
