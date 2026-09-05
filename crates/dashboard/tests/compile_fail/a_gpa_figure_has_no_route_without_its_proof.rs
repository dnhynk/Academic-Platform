//! `a_gpa_figure_has_no_route_without_its_proof`.
//!
//! Section 25.4's first line asks each average for its own proof.
//! `GpaFigure::publish` takes one by value and is the only producer.
//!
//! The figure arrives as a parameter for the same reason the plan snapshot case
//! gives: an error reported against `Result<GpaFigure, _>` would say nothing
//! about `GpaFigure`.

use academic_dashboard::{GpaFigure, GpaProof, GpaScope};
use academic_record::views::GpaValue;

fn routes(figure: &mut GpaFigure) {
    // There is no constructor taking only a value.
    let _value_only = GpaFigure::publish(GpaScope::Cumulative, GpaValue::NoGradedAttempts);

    // Nor a `Default`, which would be an average over nothing.
    let _default = GpaFigure::default();

    // Nor a `From<GpaValue>`.
    let _converted = GpaFigure::from(GpaValue::NoGradedAttempts);

    // Nor a way to take the proof back off one.
    figure.clear_proof();

    // Nor a mutable view of the proof it carries.
    let _proof: &mut GpaProof = figure.proof_mut();

    // Nor a literal, because every field is private.
    let _literal = GpaFigure {
        scope: GpaScope::Major,
        value: GpaValue::NoGradedAttempts,
    };
}

fn main() {}
