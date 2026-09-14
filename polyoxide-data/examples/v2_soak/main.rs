//! Measures, then validates, the rate limits for Data API v2 routes.
//!
//! Built up across the Phase 3 plan; the modules land before the entry point.

#[path = "../common/mod.rs"]
mod common;
#[allow(dead_code)] // Used by the entry point, which lands in the next task.
mod probes;
#[allow(dead_code)] // Used by the entry point, which lands in the next task.
mod verdict;

fn main() {}
