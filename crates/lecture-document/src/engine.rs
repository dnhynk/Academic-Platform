//! `TRANSCRIPT_COVERAGE`, the `P2-C5` deterministic engine, and the encoding
//! that makes a coverage answer a byte comparison.
//!
//! # Why the validator is an engine
//!
//! Section 12.6's first sentence is "CoverageValidator는 deterministic하다".
//! This repository already has the shape that makes such a sentence executable:
//! `P2-C5`'s harness fixes the signature `(frozen_inputs, rule_set_hash,
//! engine_version) -> (result, proof_tree, explanation_snapshot)`, the
//! canonical byte encoding, and the CI rule that an engine registered as
//! `IMPLEMENTED` ships a committed corpus. `TRANSCRIPT_COVERAGE` is entry seven
//! of that registry and was `PLANNED` until this task; running the coverage
//! answer through it is what turns "deterministic" into a committed
//! `.expected` file that a fresh render is compared against.
//!
//! # Two encodings, and why the report has its own
//!
//! [`CoverageReport::canonical_bytes`] is the whole report, including every
//! segment's evidence. The frozen inputs here are the *engine's* view: the
//! counts and statuses a completeness verdict is a function of, in `P2-C5`'s
//! `key=value` grammar, which admits integers and identifier-shaped references
//! and no free text at all. Both are total functions of the report, and
//! `coverage_determinism` compares both.

use std::collections::BTreeMap;

use academic_domain::{
    ContentDigest, Decimal,
    engines::{
        DeterministicEngine, EngineError, EngineOutcome, EngineResult, EngineVersion, FrozenInputs,
        InputKey, InputValue, NodeId as ProofNodeId, ProofNode, ProofStatus, RuleId, RuleSetHash,
    },
};

use crate::{coverage::CoverageReport, render::RenderQaReport};

/// The registry identifier. `schemas/registry/engine-registry-v1.json` is the
/// source of truth and this constant is compared against it.
pub const TRANSCRIPT_COVERAGE_ENGINE_ID: &str = "engine.transcript.coverage";

/// The engine's own version.
pub const TRANSCRIPT_COVERAGE_ENGINE_VERSION: u16 = 1;

/// The published rules the proof tree cites.
///
/// One per section 12.6 check, plus the partition rule `INV-C-003` states and
/// the root the four combine under.
pub const RULE_PARTITION: &str = "coverage.every-segment-has-exactly-one-status";
/// Section 12.6's segment coverage line.
pub const RULE_SEGMENT_COVERAGE: &str = "coverage.segment-coverage-is-whole";
/// Section 12.6's token coverage line.
pub const RULE_TOKEN_COVERAGE: &str = "coverage.token-coverage-is-whole";
/// Section 12.6's ordering line.
pub const RULE_ORDERING: &str = "coverage.order-is-monotonic-or-cross-referenced";
/// Section 12.6's capture line.
pub const RULE_CAPTURES: &str = "coverage.every-capture-is-placed-or-excluded";
/// Section 12.6's gap line.
pub const RULE_GAPS: &str = "coverage.no-unexplained-hole-above-threshold";
/// Section 12.6's render QA line.
pub const RULE_RENDER: &str = "coverage.render-has-no-defect";
/// The root.
pub const RULE_COMPLETE: &str = "coverage.document-is-complete";

/// Every rule, in proof-tree order.
pub const RULES: [&str; 8] = [
    RULE_COMPLETE,
    RULE_PARTITION,
    RULE_SEGMENT_COVERAGE,
    RULE_TOKEN_COVERAGE,
    RULE_ORDERING,
    RULE_CAPTURES,
    RULE_GAPS,
    RULE_RENDER,
];

/// The published rule set, verbatim.
///
/// `docs/contracts/engine-harness.md` says `ruleset.txt` in a harness directory
/// *is* the published rule set and its SHA-256 is the hash every case is
/// evaluated under. This constant is that file's bytes, so the committed
/// artifact and the engine cannot disagree: the harness renders the file from
/// here, and [`ruleset_hash`] is the digest of the same bytes.
pub const RULESET_TEXT: &str = concat!(
    "coverage.document-is-complete\n",
    "coverage.every-segment-has-exactly-one-status\n",
    "coverage.segment-coverage-is-whole\n",
    "coverage.token-coverage-is-whole\n",
    "coverage.order-is-monotonic-or-cross-referenced\n",
    "coverage.every-capture-is-placed-or-excluded\n",
    "coverage.no-unexplained-hole-above-threshold\n",
    "coverage.render-has-no-defect\n",
);

/// The hash of the published rule set.
#[must_use]
pub fn ruleset_hash() -> RuleSetHash {
    RuleSetHash::new(ContentDigest::sha256(RULESET_TEXT.as_bytes()))
}

/// The engine's frozen input view of one coverage run.
///
/// A total function of the report and the render QA report. Every value is an
/// integer or an identifier-shaped reference; `P2-C5`'s grammar has no free
/// text, which is why no segment identifier that is not identifier-shaped can
/// reach it.
///
/// # Errors
///
/// [`EngineError`] when a count does not fit or an identifier is not
/// identifier-shaped.
pub fn freeze(report: &CoverageReport, qa: &RenderQaReport) -> Result<FrozenInputs, EngineError> {
    let mut entries: Vec<(InputKey, InputValue)> = vec![
        (
            InputKey::new("coverage.captures.excluded")?,
            integer(report.excluded_captures().len() as u64),
        ),
        (
            InputKey::new("coverage.captures.placed")?,
            integer(report.placed_captures().len() as u64),
        ),
        (
            InputKey::new("coverage.captures.unaccounted")?,
            integer(report.unaccounted_captures().len() as u64),
        ),
        (
            InputKey::new("coverage.config.gap_threshold_nanos")?,
            integer(report.config().gap_threshold_nanos()),
        ),
        (
            InputKey::new("coverage.config.low_confidence_permille")?,
            integer(u64::from(
                report.config().low_confidence_at_or_below_permille(),
            )),
        ),
        (
            InputKey::new("coverage.config.version")?,
            integer(u64::from(report.config().version())),
        ),
        (
            InputKey::new("coverage.gaps.total")?,
            integer(report.gaps().len() as u64),
        ),
        (
            InputKey::new("coverage.gaps.unexplained")?,
            integer(report.unexplained_gaps().len() as u64),
        ),
        (
            InputKey::new("coverage.ordering.exceptions")?,
            integer(report.ordering_exceptions().len() as u64),
        ),
        (
            InputKey::new("coverage.ordering.findings")?,
            integer(report.ordering_findings().len() as u64),
        ),
        (
            InputKey::new("coverage.render.defects")?,
            integer(qa.findings().len() as u64),
        ),
        (
            InputKey::new("coverage.segment_coverage.denominator")?,
            integer(report.segment_coverage().denominator()),
        ),
        (
            InputKey::new("coverage.segment_coverage.numerator")?,
            integer(report.segment_coverage().numerator()),
        ),
        (
            InputKey::new("coverage.segments.eligible")?,
            integer(
                report
                    .accounts()
                    .len()
                    .saturating_add(report.unmapped().len()) as u64,
            ),
        ),
        (
            InputKey::new("coverage.segments.unmapped")?,
            integer(report.unmapped_count() as u64),
        ),
        (
            InputKey::new("coverage.token_coverage.denominator")?,
            integer(report.token_coverage().denominator()),
        ),
        (
            InputKey::new("coverage.token_coverage.numerator")?,
            integer(report.token_coverage().numerator()),
        ),
    ];

    // One block per segment, keyed by ascending index, so the encoding is a
    // function of the report rather than of a walk order.
    let mut rows: Vec<(usize, String, usize, &'static str)> = Vec::new();
    for account in report.accounts() {
        rows.push((
            account.segment_index(),
            account.segment_id().to_owned(),
            account.token_count(),
            account.status().as_str(),
        ));
    }
    for segment in report.unmapped() {
        rows.push((
            segment.segment_index(),
            segment.segment_id().to_owned(),
            segment.token_count(),
            "UNMAPPED",
        ));
    }
    rows.sort_by_key(|row| row.0);
    for (index, id, token_count, status) in rows {
        entries.push((
            InputKey::new(&format!("coverage.segment.{index:04}.id"))?,
            InputValue::Reference(id),
        ));
        entries.push((
            InputKey::new(&format!("coverage.segment.{index:04}.status"))?,
            InputValue::Reference(status.to_owned()),
        ));
        entries.push((
            InputKey::new(&format!("coverage.segment.{index:04}.tokens"))?,
            integer(token_count as u64),
        ));
    }
    FrozenInputs::new(entries)
}

fn integer(value: u64) -> InputValue {
    InputValue::Integer(i64::try_from(value).unwrap_or(i64::MAX))
}

/// The `TRANSCRIPT_COVERAGE` engine.
///
/// It holds no state at all: the rule set is a hash the caller presents and the
/// inputs are frozen, so two evaluations of the same pair are the same bytes by
/// construction rather than by discipline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TranscriptCoverageEngine;

impl TranscriptCoverageEngine {
    /// Evaluates one frozen coverage problem.
    ///
    /// # Errors
    ///
    /// [`EngineError`] when the frozen inputs are missing a key the rules read,
    /// or when the assembled proof tree does not validate against them.
    pub fn evaluate_coverage(
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
    ) -> Result<EngineOutcome, EngineError> {
        // This engine implements exactly one published rule set. A hash that is
        // not its own is refused rather than evaluated under, because an
        // outcome's canonical bytes bind a rule-set hash the evaluation never
        // read -- which would make two different rule sets produce two
        // different byte strings for one unchanged computation.
        if rule_set_hash != ruleset_hash() {
            return Err(EngineError::MalformedInput(
                "the presented rule-set hash is not this engine's published rule set",
            ));
        }
        let eligible = read_integer(inputs, "coverage.segments.eligible")?;
        let unmapped = read_integer(inputs, "coverage.segments.unmapped")?;
        let segment_numerator = read_integer(inputs, "coverage.segment_coverage.numerator")?;
        let segment_denominator = read_integer(inputs, "coverage.segment_coverage.denominator")?;
        let token_numerator = read_integer(inputs, "coverage.token_coverage.numerator")?;
        let token_denominator = read_integer(inputs, "coverage.token_coverage.denominator")?;
        let ordering_findings = read_integer(inputs, "coverage.ordering.findings")?;
        let unaccounted_captures = read_integer(inputs, "coverage.captures.unaccounted")?;
        let unexplained_gaps = read_integer(inputs, "coverage.gaps.unexplained")?;
        let render_defects = read_integer(inputs, "coverage.render.defects")?;

        // The partition. Every declared segment block has to carry one of the
        // four spellings or `UNMAPPED`, and the two counts have to reconcile
        // against the block count. A status the engine does not recognise is
        // `CONFLICT`, not a silent pass: `EngineOutcome::new` then refuses a
        // `SATISFIED` result over it.
        let mut counted = 0_i64;
        let mut unmapped_seen = 0_i64;
        let mut unrecognised = false;
        let mut segment_keys: Vec<InputKey> = Vec::new();
        for index in 0..eligible.max(0) {
            let Ok(index) = usize::try_from(index) else {
                break;
            };
            let key = InputKey::new(&format!("coverage.segment.{index:04}.status"))?;
            let Some(value) = inputs.get(&key) else {
                continue;
            };
            let InputValue::Reference(status) = value else {
                unrecognised = true;
                segment_keys.push(key);
                continue;
            };
            counted = counted.saturating_add(1);
            if status == "UNMAPPED" {
                unmapped_seen = unmapped_seen.saturating_add(1);
            } else if !crate::coverage::SegmentStatus::SPELLINGS.contains(&status.as_str()) {
                unrecognised = true;
            }
            segment_keys.push(key);
        }
        segment_keys.sort();
        segment_keys.dedup();
        let partition_status = if unrecognised {
            ProofStatus::Conflict
        } else if counted != eligible || unmapped_seen != unmapped {
            ProofStatus::NotSatisfied
        } else if unmapped == 0 {
            ProofStatus::Satisfied
        } else {
            ProofStatus::Needs
        };

        let mut children = vec![
            leaf(
                "n.captures",
                RULE_CAPTURES,
                satisfied_when(unaccounted_captures == 0),
                &[
                    "coverage.captures.excluded",
                    "coverage.captures.placed",
                    "coverage.captures.unaccounted",
                ],
            )?,
            leaf(
                "n.gaps",
                RULE_GAPS,
                satisfied_when(unexplained_gaps == 0),
                &[
                    "coverage.config.gap_threshold_nanos",
                    "coverage.gaps.total",
                    "coverage.gaps.unexplained",
                ],
            )?,
            leaf(
                "n.ordering",
                RULE_ORDERING,
                satisfied_when(ordering_findings == 0),
                &["coverage.ordering.exceptions", "coverage.ordering.findings"],
            )?,
            ProofNode {
                node_id: ProofNodeId::new("n.partition")?,
                rule_id: RuleId::new(RULE_PARTITION)?,
                status: partition_status,
                inputs: {
                    let mut keys = vec![
                        InputKey::new("coverage.segments.eligible")?,
                        InputKey::new("coverage.segments.unmapped")?,
                    ];
                    keys.extend(segment_keys);
                    keys.sort();
                    keys.dedup();
                    keys
                },
                source_locators: Vec::new(),
                children: Vec::new(),
            },
            leaf(
                "n.render",
                RULE_RENDER,
                satisfied_when(render_defects == 0),
                &["coverage.render.defects"],
            )?,
            leaf(
                "n.segments",
                RULE_SEGMENT_COVERAGE,
                satisfied_when(segment_denominator > 0 && segment_numerator == segment_denominator),
                &[
                    "coverage.segment_coverage.denominator",
                    "coverage.segment_coverage.numerator",
                ],
            )?,
            leaf(
                "n.tokens",
                RULE_TOKEN_COVERAGE,
                satisfied_when(token_denominator > 0 && token_numerator == token_denominator),
                &[
                    "coverage.token_coverage.denominator",
                    "coverage.token_coverage.numerator",
                ],
            )?,
        ];
        children.sort_by(|left, right| left.node_id.as_str().cmp(right.node_id.as_str()));

        // The root is satisfied only when every child is. It is not a fold this
        // engine invented: section 12.6's five checks and `INV-C-003`'s
        // partition are conjunctive, and `unmapped_forces_incomplete` is the
        // one arm that has a name of its own.
        let root_status = if children
            .iter()
            .any(|child| child.status == ProofStatus::Conflict)
        {
            ProofStatus::Conflict
        } else if children
            .iter()
            .all(|child| child.status == ProofStatus::Satisfied)
        {
            ProofStatus::Satisfied
        } else if children
            .iter()
            .any(|child| child.status == ProofStatus::Needs)
        {
            ProofStatus::Needs
        } else {
            ProofStatus::NotSatisfied
        };

        let mut values: BTreeMap<String, Decimal> = BTreeMap::new();
        values.insert(
            "segment_coverage_denominator".to_owned(),
            decimal(segment_denominator)?,
        );
        values.insert(
            "segment_coverage_numerator".to_owned(),
            decimal(segment_numerator)?,
        );
        values.insert(
            "token_coverage_denominator".to_owned(),
            decimal(token_denominator)?,
        );
        values.insert(
            "token_coverage_numerator".to_owned(),
            decimal(token_numerator)?,
        );
        values.insert("unmapped_segments".to_owned(), decimal(unmapped)?);

        let root = ProofNode {
            node_id: ProofNodeId::new("n.complete")?,
            rule_id: RuleId::new(RULE_COMPLETE)?,
            status: root_status,
            inputs: vec![
                InputKey::new("coverage.config.version")?,
                InputKey::new("coverage.segments.eligible")?,
                InputKey::new("coverage.segments.unmapped")?,
            ],
            source_locators: Vec::new(),
            children,
        };
        let result = EngineResult {
            status: root_status,
            values,
            unevaluated: Vec::new(),
        };
        EngineOutcome::new(result, root, inputs)
    }
}

impl DeterministicEngine for TranscriptCoverageEngine {
    fn engine_id(&self) -> &'static str {
        TRANSCRIPT_COVERAGE_ENGINE_ID
    }

    fn engine_version(&self) -> EngineVersion {
        EngineVersion::new(TRANSCRIPT_COVERAGE_ENGINE_VERSION).unwrap_or(EngineVersion::MIN)
    }

    fn evaluate(
        &self,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
        _engine_version: EngineVersion,
    ) -> Result<EngineOutcome, EngineError> {
        Self::evaluate_coverage(inputs, rule_set_hash)
    }
}

const fn satisfied_when(condition: bool) -> ProofStatus {
    if condition {
        ProofStatus::Satisfied
    } else {
        ProofStatus::NotSatisfied
    }
}

fn leaf(
    node_id: &str,
    rule: &str,
    status: ProofStatus,
    keys: &[&str],
) -> Result<ProofNode, EngineError> {
    let mut inputs = Vec::with_capacity(keys.len());
    for key in keys {
        inputs.push(InputKey::new(key)?);
    }
    inputs.sort();
    inputs.dedup();
    Ok(ProofNode {
        node_id: ProofNodeId::new(node_id)?,
        rule_id: RuleId::new(rule)?,
        status,
        inputs,
        source_locators: Vec::new(),
        children: Vec::new(),
    })
}

fn read_integer(inputs: &FrozenInputs, key: &str) -> Result<i64, EngineError> {
    let key = InputKey::new(key)?;
    match inputs.get(&key) {
        Some(InputValue::Integer(value)) => Ok(*value),
        _ => Err(EngineError::MalformedInput(
            "a coverage input the rules read is missing or is not an integer",
        )),
    }
}

fn decimal(value: i64) -> Result<Decimal, EngineError> {
    Decimal::new(i128::from(value), 0)
        .map_err(|_| EngineError::MalformedInput("a coverage count does not fit an exact decimal"))
}
