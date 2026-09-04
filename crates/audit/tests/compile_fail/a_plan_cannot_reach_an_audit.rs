//! `plan_excluded_from_actual_audit`, the half a running test cannot observe.
//!
//! Section 6 binds a `DegreeAuditAggregate` to a profile, a requirement set and
//! a transcript snapshot. A plan is none of those, so there is no argument to
//! pass one as -- and the annotated view, which does see a plan, produces
//! labels and never an audit.

use academic_audit::{DegreeAudit, GraduationAuditEngine, PlanAnnotatedView, PlannedCoursework};
use academic_domain::engines::FrozenInputs;

fn main() {
    let engine: GraduationAuditEngine = unimplemented!();
    let inputs: FrozenInputs = unimplemented!();
    let plan = PlannedCoursework::none();

    // There is no third parameter.
    let _with_plan = DegreeAudit::evaluate(&engine, &inputs, &plan);

    // And no accessor on a plan yields anything an audit reads.
    let _facts = plan.as_transcript();
    let _attempts = plan.into_attempts();

    let audit: DegreeAudit = unimplemented!();
    let view = PlanAnnotatedView::new(&audit, &plan);

    // The view labels; it does not produce, replace, or mutate an audit.
    let _promoted = view.into_audit();
    let _status = view.status_for("4190.409");
}
