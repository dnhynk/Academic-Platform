//! `no_academic_surface_type_has_a_public_field`.
//!
//! One probe per closed type and nothing else in the file, because **E0451 is
//! reported by the privacy pass and that pass does not run once type checking
//! has failed.** The first version of this suite put each literal beside the
//! method probes for its own type, and two of the four produced no diagnostic
//! at all: the committed `.stderr` for the plan snapshot and the percentage
//! held only the method errors, so those two probes were carrying no load.
//! `GpaFigure`'s did report, and only because it omitted a field, which is a
//! resolution-phase error rather than a privacy-phase one.
//!
//! Here every probe is a struct literal and there is nothing before them to
//! stop the privacy pass running, so each one appears in the committed
//! diagnostic. If a field of any of the three ever became public, the
//! corresponding line would disappear from that diagnostic and this case would
//! fail.

use academic_dashboard::{
    DragOutcome, GpaFigure, GpaProof, GpaScope, PlanSnapshot, RequirementBreakdown,
    SecondaryPercentage,
};
use academic_record::views::GpaValue;

fn literals(outcome: DragOutcome, proof: GpaProof, breakdown: RequirementBreakdown) {
    let _snapshot = PlanSnapshot {
        label: String::new(),
        placed: Vec::new(),
        outcome,
    };
    let _percentage = SecondaryPercentage {
        breakdown,
        permille: 720,
    };
    let _figure = GpaFigure {
        scope: GpaScope::Cumulative,
        value: GpaValue::NoGradedAttempts,
        proof,
    };
}

fn main() {}
