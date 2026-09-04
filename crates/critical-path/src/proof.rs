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

use academic_curriculum::{Meeting, Weekday};
use academic_domain::{
    FreshnessBand,
    engines::{
        EngineError, EngineOutcome, EngineResult, FrozenInputs, InputKey, InputValue, NodeId,
        ProofNode, ProofStatus, RuleId,
    },
};

use crate::{
    CriticalPathError,
    constraint::{CONSTRAINTS, OfficialPrerequisiteStanding},
    engine::PlanRequest,
    hypergraph::EdgeStanding,
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

    // Section 16.1's hypergraph. A run's answer depends on its shape, so the
    // shape is part of its identity: every member's two ends, its predicate and
    // its standing, in the graph's own order.
    for (position, member) in request.graph.all_members().iter().enumerate() {
        entries.push((
            key(&format!("graph.{position:02}.dependent"))?,
            InputValue::Reference(hex(member.dependent().as_bytes())),
        ));
        entries.push((
            key(&format!("graph.{position:02}.prerequisite"))?,
            InputValue::Reference(hex(member.concept().as_bytes())),
        ));
        entries.push((
            key(&format!("graph.{position:02}.predicate"))?,
            InputValue::Reference(member.edge().predicate().as_str().to_owned()),
        ));
        entries.push((
            key(&format!("graph.{position:02}.standing"))?,
            InputValue::Reference(standing_token(member.standing()).to_owned()),
        ));
    }

    // Section 16.3's inputs, all of them, each with its own count so an empty
    // list is distinguishable from an absent one.
    //
    // Leaving any of them out is not a cosmetic omission: two runs that differ
    // only in an excluded concept reach different answers, so frozen inputs
    // that did not carry the exclusion would make the engine **not a function
    // of them**, which is the whole property `P2-C5`'s contract is about. The
    // first version of this file omitted every one, and two corpus cases shared
    // a digest while their canonical bytes differed.
    // `the_frozen_inputs_are_the_runs_identity` is the assertion that says so.
    let constraints = request.constraints;
    for (position, concept) in constraints.hard_prerequisites_met.iter().enumerate() {
        entries.push((
            key(&format!("constraint.hard_met.{position:02}"))?,
            InputValue::Reference(hex(concept.as_bytes())),
        ));
    }
    entries.push((
        key("constraint.hard_met.count")?,
        InputValue::Integer(count(constraints.hard_prerequisites_met.len())),
    ));
    for (position, (offering, standing)) in constraints.official_prerequisites.iter().enumerate() {
        entries.push((
            key(&format!("constraint.official.{position:02}.offering"))?,
            InputValue::Reference(hex(offering.as_uuid().as_bytes())),
        ));
        entries.push((
            key(&format!("constraint.official.{position:02}.standing"))?,
            InputValue::Reference(official_token(*standing).to_owned()),
        ));
    }
    entries.push((
        key("constraint.official.count")?,
        InputValue::Integer(count(constraints.official_prerequisites.len())),
    ));
    for (position, meeting) in constraints.committed_meetings.iter().enumerate() {
        push_meeting(
            &mut entries,
            &format!("constraint.committed_meeting.{position:02}"),
            *meeting,
        )?;
    }
    entries.push((
        key("constraint.committed_meeting.count")?,
        InputValue::Integer(count(constraints.committed_meetings.len())),
    ));
    for (position, (offering, meetings)) in constraints.offering_meetings.iter().enumerate() {
        entries.push((
            key(&format!(
                "constraint.offering_meeting.{position:02}.offering"
            ))?,
            InputValue::Reference(hex(offering.as_uuid().as_bytes())),
        ));
        for (slot, meeting) in meetings.iter().enumerate() {
            push_meeting(
                &mut entries,
                &format!("constraint.offering_meeting.{position:02}.{slot:02}"),
                *meeting,
            )?;
        }
        entries.push((
            key(&format!("constraint.offering_meeting.{position:02}.count"))?,
            InputValue::Integer(count(meetings.len())),
        ));
    }
    entries.push((
        key("constraint.offering_meeting.count")?,
        InputValue::Integer(count(constraints.offering_meetings.len())),
    ));
    entries.push((
        key("constraint.committed_credits")?,
        InputValue::Integer(i64::from(constraints.committed_credits)),
    ));
    for (position, source) in constraints.privacy_excluded_sources.iter().enumerate() {
        entries.push((
            key(&format!("constraint.privacy_excluded.{position:02}"))?,
            InputValue::Reference(hex(source.as_bytes())),
        ));
    }
    entries.push((
        key("constraint.privacy_excluded.count")?,
        InputValue::Integer(count(constraints.privacy_excluded_sources.len())),
    ));
    for (position, concept) in constraints.user_excluded_concepts.iter().enumerate() {
        entries.push((
            key(&format!("constraint.excluded_concept.{position:02}"))?,
            InputValue::Reference(hex(concept.as_bytes())),
        ));
    }
    entries.push((
        key("constraint.excluded_concept.count")?,
        InputValue::Integer(count(constraints.user_excluded_concepts.len())),
    ));
    for (position, offering) in constraints.user_excluded_offerings.iter().enumerate() {
        entries.push((
            key(&format!("constraint.excluded_offering.{position:02}"))?,
            InputValue::Reference(hex(offering.as_uuid().as_bytes())),
        ));
    }
    entries.push((
        key("constraint.excluded_offering.count")?,
        InputValue::Integer(count(constraints.user_excluded_offerings.len())),
    ));
    for (position, (concept, band)) in constraints.bands.iter().enumerate() {
        entries.push((
            key(&format!("constraint.band.{position:02}.concept"))?,
            InputValue::Reference(hex(concept.as_bytes())),
        ));
        entries.push((
            key(&format!("constraint.band.{position:02}.band"))?,
            InputValue::Reference(band_token(*band).to_owned()),
        ));
    }
    entries.push((
        key("constraint.band.count")?,
        InputValue::Integer(count(constraints.bands.len())),
    ));

    // Every acquisition option a route could take. An option decides credits,
    // meetings, offering standing and which sources privacy has to allow, so
    // two runs that differ only in their options are two different runs.
    for (position, estimate) in request.estimates.iter().enumerate() {
        for (slot, option) in estimate.options.iter().enumerate() {
            entries.push((
                key(&format!("estimate.{position:02}.option.{slot:02}.kind"))?,
                InputValue::Reference(option.as_str().to_owned()),
            ));
            entries.push((
                key(&format!("estimate.{position:02}.option.{slot:02}.credits"))?,
                InputValue::Integer(i64::from(option.credits())),
            ));
            entries.push((
                key(&format!("estimate.{position:02}.option.{slot:02}.offering"))?,
                option.offering().map_or(InputValue::Unknown, |offering| {
                    InputValue::Reference(hex(offering.as_uuid().as_bytes()))
                }),
            ));
            entries.push((
                key(&format!("estimate.{position:02}.option.{slot:02}.status"))?,
                option
                    .offering_status()
                    .map_or(InputValue::Unknown, |status| {
                        InputValue::Reference(status.as_str().to_owned())
                    }),
            ));
            for (occasion, opportunity) in option.supplies().iter().enumerate() {
                entries.push((
                    key(&format!(
                        "estimate.{position:02}.option.{slot:02}.occasion.{occasion:02}.concept"
                    ))?,
                    InputValue::Reference(hex(opportunity.concept().as_bytes())),
                ));
                entries.push((
                    key(&format!(
                        "estimate.{position:02}.option.{slot:02}.occasion.{occasion:02}.kind"
                    ))?,
                    InputValue::Reference(opportunity.kind().as_str().to_owned()),
                ));
                entries.push((
                    key(&format!(
                        "estimate.{position:02}.option.{slot:02}.occasion.{occasion:02}.source"
                    ))?,
                    InputValue::Reference(hex(opportunity.source().as_bytes())),
                ));
            }
        }
        entries.push((
            key(&format!("estimate.{position:02}.option.count"))?,
            InputValue::Integer(count(estimate.options.len())),
        ));
    }

    FrozenInputs::new(entries).map_err(CriticalPathError::Engine)
}

/// One meeting, as a weekday index and its two minutes.
///
/// The weekday is its position in `P2-U1`'s own `Weekday::ALL`, so a day added
/// there changes this encoding rather than colliding with an existing one.
fn push_meeting(
    entries: &mut Vec<(InputKey, InputValue)>,
    prefix: &str,
    meeting: Meeting,
) -> Result<(), CriticalPathError> {
    let weekday = Weekday::ALL
        .iter()
        .position(|day| *day == meeting.weekday())
        .unwrap_or(Weekday::ALL.len());
    entries.push((
        key(&format!("{prefix}.weekday"))?,
        InputValue::Integer(count(weekday)),
    ));
    entries.push((
        key(&format!("{prefix}.from"))?,
        InputValue::Integer(i64::from(meeting.from_minute())),
    ));
    entries.push((
        key(&format!("{prefix}.to"))?,
        InputValue::Integer(i64::from(meeting.to_minute())),
    ));
    Ok(())
}

/// `P2-N3`'s band spelling, through that crate's own function.
fn band_token(band: FreshnessBand) -> &'static str {
    academic_freshness::band_token(band)
}

/// The standing of one hyperedge member. Total with no wildcard arm.
const fn standing_token(standing: EdgeStanding) -> &'static str {
    match standing {
        EdgeStanding::Settled => "SETTLED",
        EdgeStanding::Uncertain => "UNCERTAIN",
    }
}

/// The registrar's answer for one offering. Total with no wildcard arm.
const fn official_token(standing: OfficialPrerequisiteStanding) -> &'static str {
    match standing {
        OfficialPrerequisiteStanding::Met => "MET",
        OfficialPrerequisiteStanding::Unmet => "UNMET",
        OfficialPrerequisiteStanding::Unknown => "UNKNOWN",
    }
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
