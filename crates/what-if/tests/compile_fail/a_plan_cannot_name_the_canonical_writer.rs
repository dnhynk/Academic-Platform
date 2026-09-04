//! Nothing compiled against this crate can name the canonical writer.
//!
//! `academic-what-if` has no Cargo edge to `academic-store` of any kind, so the
//! writer is not in the dependency closure a plan is compiled against and the
//! path does not resolve. `plan_scenario_never_writes_actual_state` proves the
//! same fact from the workspace manifests; this case proves that the absence is
//! what a compiler actually sees.

use academic_store::accept::AcceptanceStore;

fn main() {
    let _writer: Option<AcceptanceStore> = None;
}
