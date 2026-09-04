//! The §28 `GRADUATION_AUDIT` engine, and the section 11.3 tree it publishes.
//!
//! `(frozen_inputs, rule_set_hash, engine_version) -> (result, proof_tree,
//! explanation_snapshot)`. No clock, no RNG, no socket, no model, and no
//! mutable state: the published rule set is `&self` and is covered by
//! `rule_set_hash`, and everything else -- the profile, the transcript, the
//! source placements, the open conflict cases, the freshness criterion and the
//! instant -- arrives through the frozen inputs and is covered by their digest.
//!
//! `GRADUATION_AUDIT` is one of §28's four high-impact paths, which is why its
//! harness carries `adverse/unknown`, `adverse/conflict` and
//! `adverse/partial_failure`. Each is a state the *shipped* rules reach under
//! the *baseline* rule set:
//!
//! | path | how the baseline set reaches it |
//! |---|---|
//! | `unknown` | the thesis rule, whose applicability no confirmed source states (`GATE-38-012`) |
//! | `conflict` | a `MUTUALLY_EXCLUSIVE` rule two recognized attempts both fall under |
//! | `partial_failure` | a published rule the source index does not place, which is therefore not evaluated |
//!
//! # Why a rule with no recorded page is not evaluated
//!
//! Section 11.3 requires a source page and paragraph on every leaf.
//! [`crate::leaf::ProofLeaf`] takes one by value and has no arm for its
//! absence, so a rule the source index does not place cannot become a leaf.
//! Rather than invent a citation, the engine leaves the rule **unevaluated** —
//! which is exactly `EngineResult::is_partial_failure` — reports it as
//! [`crate::verdict::MissingCheck::RuleSourceSpanAbsent`], and refuses the
//! coverage witness. A verdict without a citation and a verdict withheld are
//! different things, and only the second is publishable.
//!
//! # What the root status is, and what it is not
//!
//! The root folds the rule nodes: `CONFLICT` if any conflicts, else `UNKNOWN`
//! if any is unknown, else `NOT_SATISFIED`, else `NEEDS`, else `SATISFIED`.
//! That fold is the audit's own — §11.2 has rule types where a parent is
//! legitimately satisfied over unsatisfied children, so the harness imposes
//! none, and [the harness contract](../../../docs/contracts/engine-harness.md)
//! says the "unknown is never forced into a pass or a fail" invariant is this
//! task's to express. The fold expresses it: there is no arm in which a
//! `Unknown` child produces a `Satisfied` or a `NotSatisfied` root.
//!
//! The root status is **not** the verdict. `DETERMINATE` needs all three of
//! section 11.4's gates and is [`crate::verdict`]'s.

use std::collections::BTreeMap;

use academic_domain::{
    AuditId, ContentDigest, Decimal,
    engines::{
        DeterministicEngine, EngineError, EngineOutcome, EngineResult, EngineVersion, FrozenInputs,
        InputKey, NodeId, ProofNode, ProofStatus, RuleId as EngineRuleId, RuleSetHash,
    },
};
use academic_requirement::{Measure, Operand, RuleBody, RuleId, RuleOutcome, RuleSet, RuleType};

use crate::{
    error::AuditError,
    explain::CreditExplanation,
    facts::{AuditFacts, decode, entry_keys},
    gate::OpenGate,
    leaf::{AttemptUsage, EquivalencyDecision, NoAttemptReason, ProofLeaf},
    select::SelectedRuleSet,
    source::RuleSourceSpan,
    transcript::{EntryAdmission, TranscriptSnapshot, as_attempt},
    verdict::{
        ConflictFreeWitness, ConflictReference, CoverageWitness, DegreeVerdict, DeterminateVerdict,
        FreshnessWitness, GraduationOutcome, IndeterminateVerdict, MissingCheck,
        SourceFreshnessPolicy,
    },
};

/// The registry identifier of the graduation-audit engine.
pub const GRADUATION_ENGINE_ID: &str = "engine.graduation.audit";

/// The rule identifier the tree's root carries.
///
/// The root is the audit, not a published rule: it has no source page, because
/// no official document states "this audit". Section 11.3's *모든 leaf* is a
/// requirement on the leaves, and every node below the root is one.
pub const RULE_DEGREE_AUDIT: &str = "rule.degree.audit";

/// The harness directory the registry names for this engine.
pub const GRADUATION_HARNESS_DIR: &str = "graduation_audit";

/// One node of the published proof tree, with its complete leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditNode {
    node_id: NodeId,
    leaf: ProofLeaf,
    children: Vec<AuditNode>,
}

impl AuditNode {
    /// This node's identifier inside the tree.
    #[must_use]
    pub const fn node_id(&self) -> &NodeId {
        &self.node_id
    }

    /// The complete leaf.
    #[must_use]
    pub const fn leaf(&self) -> &ProofLeaf {
        &self.leaf
    }

    /// Sub-rules, ordered by identifier.
    #[must_use]
    pub fn children(&self) -> &[AuditNode] {
        &self.children
    }

    /// This node and every node below it.
    #[must_use]
    pub fn walk(&self) -> Vec<&Self> {
        let mut nodes = vec![self];
        for child in &self.children {
            nodes.extend(child.walk());
        }
        nodes
    }
}

/// What section 6's `DegreeAuditAggregate` is bound to.
///
/// Three digests and one hash, one per input the specification names. Two
/// audits agree only when all four agree, so mutating any live source after an
/// audit changes the binding of the *next* audit and leaves the recorded one
/// exactly where it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditInputBinding {
    profile_digest: ContentDigest,
    transcript_digest: ContentDigest,
    rule_set_hash: RuleSetHash,
    source_index_digest: ContentDigest,
    frozen_inputs_digest: ContentDigest,
}

impl AuditInputBinding {
    /// The frozen profile.
    #[must_use]
    pub const fn profile_digest(self) -> ContentDigest {
        self.profile_digest
    }

    /// The frozen transcript.
    #[must_use]
    pub const fn transcript_digest(self) -> ContentDigest {
        self.transcript_digest
    }

    /// The published rule set, by the hash a historical replay walks.
    #[must_use]
    pub const fn rule_set_hash(self) -> RuleSetHash {
        self.rule_set_hash
    }

    /// Where every rule was read from.
    #[must_use]
    pub const fn source_index_digest(self) -> ContentDigest {
        self.source_index_digest
    }

    /// The whole frozen input set.
    #[must_use]
    pub const fn frozen_inputs_digest(self) -> ContentDigest {
        self.frozen_inputs_digest
    }

    /// The audit identity this binding produces.
    ///
    /// Derived from the four bound inputs rather than drawn at random, so an
    /// audit over the same inputs under the same rules is the same audit and a
    /// replay can be compared with what it replays.
    #[must_use]
    pub fn canonical_text(self) -> String {
        format!(
            "profile {}\ntranscript {}\nrule_set {}\nsources {}\nfrozen_inputs {}\n",
            self.profile_digest,
            self.transcript_digest,
            self.rule_set_hash,
            self.source_index_digest,
            self.frozen_inputs_digest
        )
    }

    /// The digest of the binding itself.
    #[must_use]
    pub fn digest(self) -> ContentDigest {
        ContentDigest::sha256(self.canonical_text().as_bytes())
    }
}

/// The §28 `GRADUATION_AUDIT` engine.
#[derive(Debug, Clone)]
pub struct GraduationAuditEngine {
    selected: SelectedRuleSet,
    version: EngineVersion,
}

impl GraduationAuditEngine {
    /// Binds an engine to one selected rule set.
    ///
    /// [`SelectedRuleSet`] is taken **by value** and has one construction site,
    /// inside [`crate::select::select`]. An audit over a set nobody selected is
    /// not a call that can be written.
    ///
    /// The rule set is the only thing the engine holds. Where each rule was
    /// read from, which conflict cases are open, and what freshness criterion
    /// applies are all frozen inputs, because an engine that read a value the
    /// digest does not cover would not be the pure function of
    /// `(frozen_inputs, rule_set_hash, engine_version)` the harness fixes.
    #[must_use]
    pub const fn new(selected: SelectedRuleSet, version: EngineVersion) -> Self {
        Self { selected, version }
    }

    /// The selected rule set.
    #[must_use]
    pub const fn selected(&self) -> &SelectedRuleSet {
        &self.selected
    }

    /// The hash every evaluation must present.
    #[must_use]
    pub fn rule_set_hash(&self) -> RuleSetHash {
        RuleSetHash::new(self.selected.rules().rule_set_hash())
    }

    /// Evaluates one audit, returning this crate's error type.
    pub fn evaluate_audit(
        &self,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
    ) -> Result<DegreeAudit, AuditError> {
        if rule_set_hash != self.rule_set_hash() {
            return Err(AuditError::RuleSetHashMismatch);
        }
        let facts = decode(inputs)?;
        DegreeAudit::assemble(self, facts, inputs, rule_set_hash)
    }
}

impl DeterministicEngine for GraduationAuditEngine {
    fn engine_id(&self) -> &'static str {
        GRADUATION_ENGINE_ID
    }

    fn engine_version(&self) -> EngineVersion {
        self.version
    }

    fn evaluate(
        &self,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
        engine_version: EngineVersion,
    ) -> Result<EngineOutcome, EngineError> {
        let _ = engine_version;
        self.evaluate_audit(inputs, rule_set_hash)
            .map(|audit| audit.outcome)
            .map_err(|error| match error {
                AuditError::Engine(engine) => engine,
                other => EngineError::InvalidIdentifier {
                    kind: "graduation audit input",
                    value: other.to_string(),
                },
            })
    }
}

/// One reproducible degree audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DegreeAudit {
    audit_id: Option<AuditId>,
    binding: AuditInputBinding,
    verdict: DegreeVerdict,
    root_status: ProofStatus,
    nodes: Vec<AuditNode>,
    unevaluated: Vec<RuleId>,
    explanations: Vec<CreditExplanation>,
    transcript: TranscriptSnapshot,
    outcome: EngineOutcome,
}

impl DegreeAudit {
    /// Evaluates one audit end to end.
    ///
    /// **There is no plan parameter.** Section 6 binds an audit to a profile, a
    /// requirement set and a transcript snapshot; a plan is none of those, and
    /// there is no argument here to pass one as. See [`crate::plan`].
    pub fn evaluate(
        engine: &GraduationAuditEngine,
        inputs: &FrozenInputs,
    ) -> Result<Self, AuditError> {
        engine.evaluate_audit(inputs, engine.rule_set_hash())
    }

    fn assemble(
        engine: &GraduationAuditEngine,
        facts: AuditFacts,
        inputs: &FrozenInputs,
        rule_set_hash: RuleSetHash,
    ) -> Result<Self, AuditError> {
        let set = engine.selected.rules();
        let academic = academic_facts(&facts)?;

        let source_index_digest = source_index_digest(&facts);

        let mut nodes: Vec<AuditNode> = Vec::new();
        let mut unevaluated: Vec<RuleId> = Vec::new();
        let mut missing: Vec<MissingCheck> = Vec::new();
        let mut explanations: Vec<CreditExplanation> = Vec::new();

        for (rule, body) in set.rules() {
            let Some(span) = facts.sources.span(rule) else {
                unevaluated.push(rule.clone());
                missing.push(MissingCheck::RuleSourceSpanAbsent { rule: rule.clone() });
                continue;
            };
            let outcome = set.evaluate(rule, &academic)?;
            let node = build_node(
                rule,
                body,
                &outcome,
                span,
                &facts.transcript,
                set,
                &academic,
            )?;
            collect_missing(&outcome, &mut missing);
            if let RuleBody::CreditMinimum { category, .. } = body {
                explanations.push(CreditExplanation::build(
                    rule.clone(),
                    category.clone(),
                    span.clone(),
                    &facts.transcript,
                ));
            }
            nodes.push(node);
        }
        nodes.sort_by(|left, right| left.node_id.as_str().cmp(right.node_id.as_str()));

        for entry in facts.transcript.pending() {
            missing.push(MissingCheck::RecognitionUndecided {
                attempt: entry.attempt(),
                reason: crate::transcript::reason_token(entry.admission().reason()),
            });
        }

        let applicable: Vec<&ConflictReference> = facts
            .conflicts
            .iter()
            .filter(|case| set.rules().any(|(rule, _)| rule.as_str() == case.rule()))
            .collect();
        for case in &applicable {
            if !case.is_resolved() {
                missing.push(MissingCheck::UnresolvedSourceConflict {
                    rule: case.rule().to_owned(),
                    left_connector: case.left_connector().to_owned(),
                    right_connector: case.right_connector().to_owned(),
                });
            }
        }

        let leaves: Vec<ProofLeaf> = nodes
            .iter()
            .flat_map(AuditNode::walk)
            .map(|node| node.leaf.clone())
            .collect();
        let root_status = fold(&leaves);

        let coverage = CoverageWitness::establish(&leaves, &unevaluated);
        let conflict_free = ConflictFreeWitness::establish(&leaves, &applicable);
        let freshness = FreshnessWitness::establish(
            facts.freshness,
            engine.selected.rules().source().retrieved_at(),
            facts.as_of,
        );
        if facts.freshness.is_none() {
            missing.push(MissingCheck::SourceFreshnessPolicyAbsent);
        } else if freshness.is_none() {
            missing.push(not_fresh(engine, &facts));
        }

        let verdict = match (coverage, conflict_free, freshness) {
            (Some(coverage), Some(conflict_free), Some(freshness)) => {
                DegreeVerdict::Determinate(DeterminateVerdict::new(
                    outcome_of(root_status),
                    coverage,
                    conflict_free,
                    freshness,
                ))
            }
            _ => DegreeVerdict::Indeterminate(
                IndeterminateVerdict::from_checks(missing).unwrap_or_else(|| {
                    // Reached only when a gate refused and produced no check.
                    // That is a defect in this function rather than a state of
                    // the record, and it is reported as one rather than
                    // panicked on: `clippy::panic` is denied and a panic on the
                    // graduation path is the one failure this crate cannot
                    // have.
                    IndeterminateVerdict::new(MissingCheck::SourceFreshnessPolicyAbsent, Vec::new())
                }),
            ),
        };

        let binding = AuditInputBinding {
            profile_digest: facts.profile.digest(),
            transcript_digest: facts.transcript.digest(),
            rule_set_hash,
            source_index_digest,
            frozen_inputs_digest: inputs.digest(),
        };

        let proof_tree = proof_tree(&nodes, &facts, inputs)?;
        let result = EngineResult {
            status: root_status,
            values: published_values(root_status, &leaves, &verdict),
            unevaluated: unevaluated
                .iter()
                .map(|rule| EngineRuleId::new(rule.as_str()))
                .collect::<Result<Vec<_>, _>>()?,
        };
        let outcome = EngineOutcome::new(result, proof_tree, inputs)?;

        Ok(Self {
            audit_id: None,
            binding,
            verdict,
            root_status,
            nodes,
            unevaluated,
            explanations,
            transcript: facts.transcript,
            outcome,
        })
    }

    /// Names this audit with the identity migration `0004`'s `audit` row keys on.
    ///
    /// The identity is the caller's, because it is the ledger that assigns one.
    /// Nothing here draws one, which is what keeps the engine free of an RNG.
    #[must_use]
    pub const fn with_audit_id(mut self, audit_id: AuditId) -> Self {
        self.audit_id = Some(audit_id);
        self
    }

    /// The recorded identity, when one was assigned.
    #[must_use]
    pub const fn audit_id(&self) -> Option<AuditId> {
        self.audit_id
    }

    /// What the audit is bound to.
    #[must_use]
    pub const fn binding(&self) -> AuditInputBinding {
        self.binding
    }

    /// `DETERMINATE` with its three witnesses, or `INDETERMINATE` with the
    /// exact missing checks.
    #[must_use]
    pub const fn verdict(&self) -> &DegreeVerdict {
        &self.verdict
    }

    /// The fold over the rule nodes. **Not** the verdict.
    #[must_use]
    pub const fn root_status(&self) -> ProofStatus {
        self.root_status
    }

    /// The tree's top-level rule nodes, ordered by identifier.
    #[must_use]
    pub fn nodes(&self) -> &[AuditNode] {
        &self.nodes
    }

    /// Every node in the tree, root excluded.
    #[must_use]
    pub fn walk(&self) -> Vec<&AuditNode> {
        self.nodes.iter().flat_map(AuditNode::walk).collect()
    }

    /// Rules the engine did not evaluate. Non-empty is a partial failure.
    #[must_use]
    pub fn unevaluated(&self) -> &[RuleId] {
        &self.unevaluated
    }

    /// The credit drilldown for one rule, when that rule has one.
    #[must_use]
    pub fn credit_explanation(&self, rule: &RuleId) -> Option<&CreditExplanation> {
        self.explanations
            .iter()
            .find(|explanation| explanation.rule() == rule)
    }

    /// Every credit drilldown.
    #[must_use]
    pub fn credit_explanations(&self) -> &[CreditExplanation] {
        &self.explanations
    }

    /// The transcript this audit was bound to.
    #[must_use]
    pub const fn transcript(&self) -> &TranscriptSnapshot {
        &self.transcript
    }

    /// The harness outcome: result, proof tree, and normalized explanation.
    #[must_use]
    pub const fn outcome(&self) -> &EngineOutcome {
        &self.outcome
    }
}

/// Turns the decoded facts into what a published rule evaluates against.
fn academic_facts(facts: &AuditFacts) -> Result<academic_requirement::AcademicFacts, AuditError> {
    let mut academic = academic_requirement::AcademicFacts::new(facts.as_of);
    if let Some(year) = facts.profile.admission_year().known() {
        academic = academic.with_admission_year(*year);
    }
    for entry in facts.transcript.entries() {
        academic = academic.with_attempt(entry.as_rule_fact()?);
    }
    if let Some(approvals) = facts.profile.exception_approvals().known() {
        for approval in approvals {
            academic = academic.with_approval(approval.clone());
        }
    }
    for (scope, reading) in facts.transcript.readings() {
        academic = academic.with_gpa(&academic_requirement::GpaScope::new(scope)?, *reading);
    }
    Ok(academic)
}

/// Builds one rule's node, with its operand children where it has them.
fn build_node(
    rule: &RuleId,
    body: &RuleBody,
    outcome: &RuleOutcome,
    span: &RuleSourceSpan,
    transcript: &TranscriptSnapshot,
    set: &RuleSet,
    academic: &academic_requirement::AcademicFacts,
) -> Result<AuditNode, AuditError> {
    let leaf = ProofLeaf::new(
        rule.clone(),
        span.clone(),
        AttemptUsage::of(
            outcome
                .used_attempts
                .iter()
                .filter_map(|entity| as_attempt(*entity))
                .collect(),
            no_attempt_reason(outcome.rule_type),
        ),
        EquivalencyDecision::of(outcome.equivalencies_applied.clone()),
        outcome.rule_type,
        outcome.status,
        outcome.measure,
        outcome.open_gate.and_then(OpenGate::from_rule_gate),
        outcome.open_gate,
    );
    let children = match body {
        RuleBody::AllOf { operands } | RuleBody::AtLeastNOf { operands, .. } => {
            operand_children(rule, operands, span, transcript, set, academic)?
        }
        _ => Vec::new(),
    };
    Ok(AuditNode {
        node_id: NodeId::new(rule.as_str())?,
        leaf,
        children,
    })
}

/// One child per operand, each a complete leaf of its own.
///
/// Section 11.3's tree does exactly this: *Major required set* is a node and
/// *Data Structures* and *Algorithms* are its children, each with its own
/// verdict, its own attempt and its own equivalency decision. Each child
/// carries the parent's source span, because an operand is part of one
/// published rule and is printed on the same page.
fn operand_children(
    rule: &RuleId,
    operands: &[Operand],
    span: &RuleSourceSpan,
    transcript: &TranscriptSnapshot,
    set: &RuleSet,
    academic: &academic_requirement::AcademicFacts,
) -> Result<Vec<AuditNode>, AuditError> {
    let mut children = Vec::with_capacity(operands.len());
    for (index, operand) in operands.iter().enumerate() {
        let discharged = transcript.entries().iter().find(|entry| {
            entry.course() == operand.course
                && matches!(entry.admission(), EntryAdmission::Counted { .. })
        });
        let substitute = if discharged.is_some() || !operand.equivalent_admitted {
            None
        } else {
            substitution(set, transcript, academic, operand)
        };
        let (attempts, equivalency, status) = match (discharged, substitute) {
            (Some(entry), _) => (
                AttemptUsage::Used(vec![entry.attempt()]),
                EquivalencyDecision::NoneApplied,
                ProofStatus::Satisfied,
            ),
            (None, Some((entry, equivalency))) => (
                AttemptUsage::Used(vec![entry]),
                EquivalencyDecision::Applied(vec![equivalency]),
                ProofStatus::Satisfied,
            ),
            (None, None) => (
                AttemptUsage::NoneUsed(NoAttemptReason::NoMatchingAttempt),
                EquivalencyDecision::NoneApplied,
                // Section 11.3 spells a named course that was not taken
                // `NOT_SATISFIED`: more of the same does not close it, a
                // different course does.
                ProofStatus::NotSatisfied,
            ),
        };
        children.push(AuditNode {
            node_id: NodeId::new(&format!("{}.op.{index:03}", rule.as_str()))?,
            leaf: ProofLeaf::new(
                rule.clone(),
                span.clone(),
                attempts,
                equivalency,
                RuleType::AllOf,
                status,
                Some(Measure::Count {
                    attained: u32::from(status == ProofStatus::Satisfied),
                    required: 1,
                }),
                None,
                None,
            ),
            children: Vec::new(),
        });
    }
    Ok(children)
}

/// Which `EQUIVALENCY` rule of this set discharges an operand, if one does.
///
/// Resolved against the rules published beside the operand and nothing else --
/// `academic-curriculum`'s catalogue equivalences are a different fact and this
/// crate has no edge to them. The rule's own verdict is what says whether the
/// substitution is live at `as_of`, so this asks the published rule rather than
/// re-deciding it.
fn substitution(
    set: &RuleSet,
    transcript: &TranscriptSnapshot,
    academic: &academic_requirement::AcademicFacts,
    operand: &Operand,
) -> Option<(academic_domain::AttemptId, RuleId)> {
    for (candidate, body) in set.rules() {
        let RuleBody::Equivalency {
            presented,
            counts_for,
            ..
        } = body
        else {
            continue;
        };
        if counts_for != &operand.course {
            continue;
        }
        let verdict = set.evaluate(candidate, academic).ok()?;
        if verdict.status != ProofStatus::Satisfied {
            continue;
        }
        let entry = transcript.entries().iter().find(|entry| {
            entry.course() == *presented
                && matches!(entry.admission(), EntryAdmission::Counted { .. })
        })?;
        return Some((entry.attempt(), candidate.clone()));
    }
    None
}

/// Why a rule type would have used no attempt.
const fn no_attempt_reason(rule_type: RuleType) -> NoAttemptReason {
    match rule_type {
        RuleType::GpaMinimum | RuleType::ExceptionApproval | RuleType::NonCreditTraining => {
            NoAttemptReason::RuleReadsNoAttempt
        }
        _ => NoAttemptReason::NoMatchingAttempt,
    }
}

/// Records what one rule verdict leaves outstanding.
fn collect_missing(outcome: &RuleOutcome, missing: &mut Vec<MissingCheck>) {
    match outcome.status {
        ProofStatus::Unknown => missing.push(match outcome.open_gate {
            Some(rule_gate) => MissingCheck::OpenOfficialFact {
                rule: outcome.rule.clone(),
                gate: OpenGate::from_rule_gate(rule_gate),
                rule_gate,
            },
            None => MissingCheck::RuleInputAbsent {
                rule: outcome.rule.clone(),
                input: "reading this rule needs",
            },
        }),
        ProofStatus::Conflict => missing.push(MissingCheck::RuleConflict {
            rule: outcome.rule.clone(),
        }),
        ProofStatus::Satisfied | ProofStatus::Needs | ProofStatus::NotSatisfied => {}
    }
}

/// The digest of every recorded rule placement.
///
/// Part of the binding rather than of the engine: section 11.3 makes the
/// citation part of the proof, so an audit whose citations moved is a different
/// audit and has to say so.
fn source_index_digest(facts: &AuditFacts) -> ContentDigest {
    let mut rendered = String::new();
    for (rule, span) in facts.sources.entries() {
        rendered.push_str(rule.as_str());
        rendered.push(' ');
        rendered.push_str(&span.canonical_text());
        rendered.push('\n');
    }
    ContentDigest::sha256(rendered.as_bytes())
}

fn not_fresh(engine: &GraduationAuditEngine, facts: &AuditFacts) -> MissingCheck {
    let limit_seconds = facts
        .freshness
        .map_or(0, SourceFreshnessPolicy::limit_seconds);
    let retrieved = engine.selected.rules().source().retrieved_at().seconds();
    let as_of_seconds = u64::try_from(facts.as_of.value().div_euclid(1_000)).unwrap_or(0);
    MissingCheck::SourceNotFresh {
        age_seconds: as_of_seconds.saturating_sub(retrieved),
        limit_seconds,
    }
}

/// The root's fold over the rule nodes.
///
/// There is no arm in which an `UNKNOWN` child yields a `SATISFIED` or a
/// `NOT_SATISFIED` root, which is §28's *unknown을 pass/fail로 강제하지 않음*
/// as a total function rather than as a comment.
fn fold(leaves: &[ProofLeaf]) -> ProofStatus {
    if leaves.is_empty() {
        return ProofStatus::Unknown;
    }
    if leaves
        .iter()
        .any(|leaf| leaf.status() == ProofStatus::Conflict)
    {
        return ProofStatus::Conflict;
    }
    if leaves
        .iter()
        .any(|leaf| leaf.status() == ProofStatus::Unknown)
    {
        return ProofStatus::Unknown;
    }
    if leaves
        .iter()
        .any(|leaf| leaf.status() == ProofStatus::NotSatisfied)
    {
        return ProofStatus::NotSatisfied;
    }
    if leaves
        .iter()
        .any(|leaf| leaf.status() == ProofStatus::Needs)
    {
        return ProofStatus::Needs;
    }
    ProofStatus::Satisfied
}

const fn outcome_of(status: ProofStatus) -> GraduationOutcome {
    match status {
        ProofStatus::Satisfied => GraduationOutcome::Possible,
        // Unreachable under a coverage witness, which refuses an `UNKNOWN`
        // leaf, and under a conflict-free witness, which refuses a `CONFLICT`
        // one. Written as the fail-closed arm anyway: if either witness were
        // ever weakened, the outcome that follows is 졸업 불가, never 가능.
        ProofStatus::Needs
        | ProofStatus::NotSatisfied
        | ProofStatus::Unknown
        | ProofStatus::Conflict => GraduationOutcome::NotPossible,
    }
}

/// What the result publishes as a typed value.
///
/// Nothing derived is published under `CONFLICT`. That is the rule
/// `academic-record` settled one engine over: a reader who sees a number beside
/// a `CONFLICT` has been handed a figure computed over a record that disagrees
/// with itself, and the number is what survives into a screenshot. The counts
/// below describe the tree the reader is already looking at; there is no
/// remaining-credit total here, because a total over rules with different
/// categories would be a number no rule states.
fn published_values(
    status: ProofStatus,
    leaves: &[ProofLeaf],
    verdict: &DegreeVerdict,
) -> BTreeMap<String, Decimal> {
    let mut values = BTreeMap::new();
    if status == ProofStatus::Conflict {
        return values;
    }
    let mut insert = |key: &str, count: usize| {
        if let Ok(value) = Decimal::new(i128::try_from(count).unwrap_or(i128::MAX), 0) {
            values.insert(key.to_owned(), value);
        }
    };
    insert("rules.evaluated", leaves.len());
    for (key, wanted) in [
        ("rules.satisfied", ProofStatus::Satisfied),
        ("rules.needs", ProofStatus::Needs),
        ("rules.not_satisfied", ProofStatus::NotSatisfied),
        ("rules.unknown", ProofStatus::Unknown),
    ] {
        insert(
            key,
            leaves.iter().filter(|leaf| leaf.status() == wanted).count(),
        );
    }
    insert("checks.missing", verdict.missing().len());
    values
}

/// Renders the audit's nodes as the harness proof tree.
fn proof_tree(
    nodes: &[AuditNode],
    facts: &AuditFacts,
    inputs: &FrozenInputs,
) -> Result<ProofNode, AuditError> {
    let mut children = Vec::with_capacity(nodes.len());
    for node in nodes {
        children.push(render(node, facts)?);
    }
    let root = ProofNode {
        node_id: NodeId::new("audit")?,
        rule_id: EngineRuleId::new(RULE_DEGREE_AUDIT)?,
        status: fold(
            &nodes
                .iter()
                .flat_map(AuditNode::walk)
                .map(|node| node.leaf.clone())
                .collect::<Vec<_>>(),
        ),
        inputs: root_inputs(inputs)?,
        source_locators: Vec::new(),
        children,
    };
    Ok(root)
}

/// The frozen keys the root reads: the instant and every profile field.
///
/// The selection rests on the profile, so the root names it. The root carries
/// no source locator because the root is the audit and no official document
/// states an audit.
fn root_inputs(inputs: &FrozenInputs) -> Result<Vec<InputKey>, AuditError> {
    let mut keys: Vec<InputKey> = inputs
        .keys()
        .filter(|key| key.as_str() == "audit.as_of" || key.as_str().starts_with("profile."))
        .cloned()
        .collect();
    keys.sort();
    keys.dedup();
    Ok(keys)
}

fn render(node: &AuditNode, facts: &AuditFacts) -> Result<ProofNode, AuditError> {
    let mut children = Vec::with_capacity(node.children.len());
    for child in &node.children {
        children.push(render(child, facts)?);
    }
    let mut inputs: Vec<InputKey> = Vec::new();
    for attempt in node.leaf.attempts().attempts() {
        if let Some(index) = facts
            .transcript
            .entries()
            .iter()
            .position(|entry| entry.attempt() == *attempt)
        {
            for key in entry_keys(index) {
                inputs.push(InputKey::new(&key)?);
            }
        }
    }
    inputs.sort();
    inputs.dedup();
    Ok(ProofNode {
        node_id: node.node_id.clone(),
        rule_id: EngineRuleId::new(node.leaf.rule().as_str())?,
        status: node.leaf.status(),
        inputs,
        source_locators: node.leaf.source().locators(),
        children,
    })
}
