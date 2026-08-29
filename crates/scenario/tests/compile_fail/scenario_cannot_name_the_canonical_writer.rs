//! Nothing compiled against this crate can name the canonical writer.
//!
//! `academic-scenario` has no Cargo edge to `academic-store` of any kind, so
//! the writer is not in the dependency closure a projection is compiled
//! against and the path does not resolve. `scenario_crate_has_no_writer_dependency`
//! proves the same fact from the Cargo metadata graph; this case proves that
//! the absence is what a compiler actually sees.

use academic_store::accept::AcceptanceStore;

fn main() {
    let _writer: Option<AcceptanceStore> = None;
}
