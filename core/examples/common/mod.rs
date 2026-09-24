//! Shared parts of the gold-set examples `export_gold` and `match_eval` (private real-data
//! evaluation, `docs/PLAN.md` "Evaluation"): the gold folder and its `jobs.json`, TXT
//! reading, profiles, blind labels and the metric wiring on top of `tests/common/eval.rs`.
//! Unit-tested on synthetic data by `core/tests/gold_eval.rs`.
#![allow(dead_code)] // Each example and the test use a different part.

#[path = "../../tests/common/eval.rs"]
pub mod eval;
pub mod gold;
pub mod labels;
pub mod metrics;
