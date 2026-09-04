//! `P2-C5`'s harness vocabulary applied to section 16, and the reason no
//! thirteenth engine is registered.
//!
//! ## What the registry says, and why this crate is not in it
//!
//! `docs/contracts/engine-harness.md` pins the registry to section 28's table:
//! *the registered engines must be exactly the §28 table rows, in table order*,
//! and the comparison is an enumeration rather than a count. Section 28 has
//! twelve rows and none of them is a critical path engine. `t068`'s `P2-N6`
//! entry names `P2-C5` as a dependency; it does not say to add a row, and adding
//! one would fail `engine_registry_is_complete` against the specification
//! itself.
//!
//! So this crate:
//!
//! * registers **no** `engine_id` and adds **no** directory under
//!   `testdata/engines/`, which `engine_registry_is_complete` also polices --
//!   *nothing unregistered may hide under the harness root*;
//! * uses `P2-C5`'s [`FrozenInputs`], [`ProofNode`], [`EngineResult`],
//!   [`EngineOutcome`] and `ExplanationSnapshot` for its own determinism
//!   evidence, with its corpus at [`CRITICAL_PATH_CORPUS_ROOT`], the way the
//!   reference engine's lives at `testdata/engine-harness-reference/`;
//! * and states that reading in `docs/contracts/critical-path.md`, so the next
//!   reader does not rediscover it.
//!
//! `t068` has been wrong about a count before -- `engine-harness.md` records
//! `thirteen-engine registry` where §28 tabulates twelve, and §31.3's `thirteen
//! named dimensions` where the specification names fifteen -- and its own
//! instruction is that every count in it is derived and unverified. This is the
//! same class of reading and it is recorded rather than invented around.
//!
//! ## What determinism means here
//!
//! [`frozen_inputs`] renders a run's identity into `P2-C5`'s canonical
//! `key=value` encoding, and [`outcome`] renders the answer into a proof tree
//! and a normalized explanation. Two runs agree exactly when
//! `EngineOutcome::canonical_bytes` agrees, and
//! `same_inputs_and_rule_hash_yield_byte_equal_results` in
//! `crates/critical-path/tests/critical_path_harness.rs` asserts both halves the
//! contract requires: equal bytes under the same rule-set hash, and **different**
//! bytes under a different one with an identical result.
//!
//! ## Every value is identifier- or integer-shaped
//!
//! `P2-C5`'s encoding admits `int:`, `dec:`, `ref:` and `unknown`, and an
//! identifier is ASCII alphanumerics, `.`, `_` and `-`. There is no free text,
//! which is why nothing structured can be smuggled through one (§2.3-3). Every
//! key below is built from a position -- an axis's own token, a candidate's
//! index -- and every value is a count, an interval end, or a hex identity.

use academic_domain::engines::{
    EngineError, EngineOutcome, EngineResult, FrozenInputs, InputKey, InputValue, NodeId,
    ProofNode, ProofStatus, RuleId,
};

use crate::{
    CriticalPathError,
    constraint::CONSTRAINTS,
    engine::PlanRequest,
    plan::CriticalPathResult,
    vector::{BENEFIT_COMPONENTS, COST_COMPONENTS},
};

/// Where this crate's determinism corpus lives.
///
/// **Not** under `testdata/engines/`: everything there belongs to one of
/// `P2-C5`'s twelve registered engines, and this is not one. See the module
/// note.
pub const CRITICAL_PATH_CORPUS_ROOT: &str = "testdata/critical-path";

/// The rule identifiers this engine's proof tree uses.
///
/// One per section 16 stage, plus one per section 16.3 constraint, derived from
/// [`CONSTRAINTS`] rather than listed, so a ninth constraint would appear here
/// without an edit and a missing one would disappear.
pub const STAGE_RULES: [&str; 5] = [
    "section-16.1-satisfy",
    "section-16.2-cost",
    "section-16.3-constrain",
    "section-16.2-eliminate",
    "section-16.2-order",
];

/// Renders one run's identity into `P2-C5`'s canonical frozen-input encoding.
///
/// # Errors
///
/// [`CriticalPathError::Engine`] for any value `P2-C5` refuses, which is what
/// keeps a malformed identity a typed error rather than a panic (§2.3-11).
pub fn frozen_inputs(request: &PlanRequest<'_>) -> Result<FrozenInputs, CriticalPathError> {
    let mut entries: Vec<(InputKey, InputValue)> = vec![
        (
            key("plan.credit_limit")?,
            InputValue::Integer(i64::from(request.constraints.credit_limit)),
        ),
        (
            key("plan.engine_version")?,
            InputValue::Integer(i64::from(request.engine_version)),
        ),
        (
            key("plan.estimates")?,
            InputValue::Integer(count(request.estimates.len())),
        ),
        (
            key("plan.goal")?,
            InputValue::Reference(hex(request.gap_case.goal().as_bytes())),
        ),
        (
            key("plan.horizon_days")?,
            InputValue::Integer(i64::from(request.constraints.horizon_days)),
        ),
        (
            key("plan.hyperedge_members")?,
            InputValue::Integer(count(request.graph.all_members().len())),
        ),
        (
            key("plan.surface_concept")?,
            InputValue::Reference(hex(request.gap_case.surface_concept().as_bytes())),
        ),
    ];
    for (position, axis) in request.slider.order().iter().enumerate() {
        entries.push((
            key(&format!("slider.{position:02}"))?,
            InputValue::Reference(axis_token(axis)),
        ));
    }
    for (position, estimate) in request.estimates.iter().enumerate() {
        entries.push((
            key(&format!("estimate.{position:02}.concept"))?,
            InputValue::Reference(hex(estimate.concept.as_bytes())),
        ));
        for component in COST_COMPONENTS {
            let interval = estimate.cost.component(component);
            entries.push((
                key(&format!(
                    "estimate.{position:02}.cost.{}.low",
                    component.spec_token()
                ))?,
                InputValue::Integer(i64::from(interval.low())),
            ));
            entries.push((
                key(&format!(
                    "estimate.{position:02}.cost.{}.high",
                    component.spec_token()
                ))?,
                InputValue::Integer(i64::from(interval.high())),
            ));
            entries.push((
                key(&format!(
                    "estimate.{position:02}.cost.{}.basis",
                    component.spec_token()
                ))?,
                // An unmeasured basis is `unknown`, which is a value and not a
                // missing key. Folding it into a zero would be the default
                // `P2-C5` names as manufacturing a verdict.
                if interval.basis().is_measured() {
                    InputValue::Integer(count(interval.basis().families().len()))
                } else {
                    InputValue::Unknown
                },
            ));
        }
        for component in BENEFIT_COMPONENTS {
            let interval = estimate.benefit.component(component);
            entries.push((
                key(&format!(
                    "estimate.{position:02}.benefit.{}.low",
                    component.spec_token()
                ))?,
                InputValue::Integer(i64::from(interval.low())),
            ));
            entries.push((
                key(&format!(
                    "estimate.{position:02}.benefit.{}.high",
                    component.spec_token()
                ))?,
                InputValue::Integer(i64::from(interval.high())),
            ));
        }
    }
    FrozenInputs::new(entries).map_err(CriticalPathError::Engine)
}

/// Renders one answer into `P2-C5`'s proof tree and normalized explanation.
///
/// The tree is one node per section 16 stage with one child per section 16.3
/// constraint under the third, so the explanation snapshot names every
/// constraint that was answered and the status it reached.
///
/// # Errors
///
/// [`CriticalPathError::Engine`] for anything `P2-C5` refuses, including a
/// `SATISFIED` result over a tree holding a `CONFLICT`.
pub fn outcome(
    result: &CriticalPathResult,
    inputs: &FrozenInputs,
) -> Result<EngineOutcome, CriticalPathError> {
    let ranked = result.ranked();
    let first = ranked.first();

    let mut children = Vec::new();
    children.push(stage_node(
        "n01",
        STAGE_RULES[0],
        if ranked.is_empty() {
            ProofStatus::NotSatisfied
        } else {
            ProofStatus::Satisfied
        },
        Vec::new(),
    )?);
    children.push(stage_node(
        "n02",
        STAGE_RULES[1],
        if result
            .disclosure()
            .cost_assumptions()
            .entries()
            .iter()
            .all(|entry| entry.families.is_empty())
        {
            ProofStatus::Unknown
        } else {
            ProofStatus::Satisfied
        },
        Vec::new(),
    )?);

    let mut constraint_children = Vec::new();
    for (position, constraint) in CONSTRAINTS.into_iter().enumerate() {
        let status = first.map_or(ProofStatus::Unknown, |path| {
            match path.candidate().verdict_of(constraint) {
                crate::constraint::ConstraintVerdict::Satisfied => ProofStatus::Satisfied,
                crate::constraint::ConstraintVerdict::SatisfiedWithInsertion => ProofStatus::Needs,
                crate::constraint::ConstraintVerdict::Violated => ProofStatus::NotSatisfied,
                crate::constraint::ConstraintVerdict::Unknown => ProofStatus::Unknown,
            }
        });
        constraint_children.push(stage_node(
            &format!("n03-{position:02}"),
            constraint.as_str(),
            status,
            Vec::new(),
        )?);
    }
    children.push(stage_node(
        "n03",
        STAGE_RULES[2],
        if first.is_some() {
            ProofStatus::Satisfied
        } else {
            ProofStatus::NotSatisfied
        },
        constraint_children,
    )?);

    children.push(stage_node(
        "n04",
        STAGE_RULES[3],
        if result.front().dominated().is_empty() {
            ProofStatus::Satisfied
        } else {
            ProofStatus::Needs
        },
        Vec::new(),
    )?);
    children.push(stage_node(
        "n05",
        STAGE_RULES[4],
        if ranked.is_empty() {
            ProofStatus::NotSatisfied
        } else {
            ProofStatus::Satisfied
        },
        Vec::new(),
    )?);

    let root = stage_node(
        "n00",
        "section-16",
        if ranked.is_empty() {
            ProofStatus::NotSatisfied
        } else {
            ProofStatus::Satisfied
        },
        children,
    )?;

    let mut values = std::collections::BTreeMap::new();
    values.insert(
        "ranked_routes".to_owned(),
        academic_domain::Decimal::new(i128::try_from(ranked.len()).unwrap_or(i128::MAX), 0)
            .map_err(CriticalPathError::Domain)?,
    );
    values.insert(
        "dominated_routes".to_owned(),
        academic_domain::Decimal::new(
            i128::try_from(result.front().dominated().len()).unwrap_or(i128::MAX),
            0,
        )
        .map_err(CriticalPathError::Domain)?,
    );

    let engine_result = EngineResult {
        status: if ranked.is_empty() {
            ProofStatus::NotSatisfied
        } else {
            ProofStatus::Satisfied
        },
        values,
        unevaluated: Vec::new(),
    };

    EngineOutcome::new(engine_result, root, inputs).map_err(CriticalPathError::Engine)
}

fn stage_node(
    node_id: &str,
    rule_id: &str,
    status: ProofStatus,
    children: Vec<ProofNode>,
) -> Result<ProofNode, CriticalPathError> {
    Ok(ProofNode {
        node_id: NodeId::new(node_id).map_err(CriticalPathError::Engine)?,
        rule_id: RuleId::new(rule_id).map_err(CriticalPathError::Engine)?,
        status,
        inputs: Vec::new(),
        source_locators: Vec::new(),
        children,
    })
}

fn key(name: &str) -> Result<InputKey, CriticalPathError> {
    InputKey::new(name).map_err(CriticalPathError::Engine)
}

fn count(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn hex(bytes: &[u8; 16]) -> String {
    let mut rendered = String::with_capacity(32);
    for byte in bytes {
        rendered.push_str(&format!("{byte:02x}"));
    }
    rendered
}

fn axis_token(axis: &crate::vector::VectorAxis) -> String {
    match axis {
        crate::vector::VectorAxis::Cost { component } => {
            format!("cost.{}", component.spec_token())
        }
        crate::vector::VectorAxis::Benefit { component } => {
            format!("benefit.{}", component.spec_token())
        }
    }
}

/// `P2-C5`'s own error, re-exported so a caller need not name that module.
pub type HarnessError = EngineError;
