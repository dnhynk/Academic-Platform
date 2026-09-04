//! The three-gate `DETERMINATE` rule, and the exact missing checks that are the
//! alternative to it.
//!
//! Section 11.4: *고위험 결과(졸업 가능/불가)는 rule coverage 100%, unresolved
//! conflict 0, source freshness 기준 충족 시에만 `DETERMINATE`가 된다.*
//!
//! # The gate is three values, not three checks
//!
//! [`DeterminateVerdict`] has private fields and one constructor, and that
//! constructor takes [`CoverageWitness`], [`ConflictFreeWitness`] and
//! [`FreshnessWitness`] **by value**. Each of the three has private fields, no
//! public constructor, and one `establish` that returns `Option<Self>` from the
//! evidence the gate is about. A caller holding two of them has no expression
//! that produces a determinate verdict, and no caller can produce a witness at
//! all: `establish` is crate-private and `DegreeAudit` is the only thing that
//! calls it.
//!
//! That is `P2-R4`'s five-stage-by-value chain applied to three stages, and it
//! is why `determinate_three_gate` can vary each condition independently and
//! observe `INDETERMINATE`: the missing witness is not a branch somebody could
//! forget to write, it is an argument that cannot be supplied.
//!
//! # The freshness criterion is supplied, never assumed
//!
//! `t001`'s `REQ-11-029` row records the source-freshness criterion as an open
//! gate candidate, and nothing in the specification states a number for it.
//! [`SourceFreshnessPolicy`] therefore has no `Default`, no constant, and no
//! constructor that omits the bound: an audit with no recorded policy has no
//! [`FreshnessWitness`] and is `INDETERMINATE`, naming
//! [`MissingCheck::SourceFreshnessPolicyAbsent`]. A number invented here would
//! be a graduation verdict resting on a threshold nobody chose.
//!
//! # Indeterminate is never a shrug
//!
//! [`IndeterminateVerdict`] carries a non-empty [`MissingCheck`] list, refused
//! at construction if it is empty. "The audit is indeterminate and we cannot
//! say why" is not a value.

use academic_domain::{AttemptId, TimestampMillis, engines::ProofStatus};
use academic_ingestion::{ConflictCase, RetrievalInstant};
use academic_requirement::{OpenGate as RuleGate, RuleId, RuleSetVersion};

use crate::{
    gate::OpenGate,
    leaf::ProofLeaf,
    profile::{ProfileField, SelectorDimension},
};

/// One thing that has to be settled before this audit can conclude.
///
/// Every arm names the exact cell, rule, attempt or dimension that is
/// outstanding. There is no arm that says only that something is missing:
/// section 11.1 requires *필요한 확인 항목* and a list with an unspecific entry
/// in it would satisfy the letter and lose the point.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MissingCheck {
    /// A profile field section 11.1's selector reads was never recorded.
    ProfileField {
        /// The field.
        field: ProfileField,
        /// The section 38 cell it leaves open, when it is one.
        gate: Option<OpenGate>,
    },
    /// Every selector input is recorded and no published set covers them.
    NoRuleSetCovers {
        /// The dimensions that discriminate, with the profile's values.
        rendered_profile: String,
    },
    /// Two or more published sets cover this profile.
    ///
    /// Section 11.1: *두 RuleSet이 경쟁하면 임의 선택하지 않고 `INDETERMINATE`*.
    /// The versions are named so the user can say which applies; nothing here
    /// picks one, and nothing reads a position out of the list.
    CompetingRuleSets {
        /// Every competing version, in published order.
        versions: Vec<RuleSetVersion>,
    },
    /// A published rule has no recorded source page and paragraph.
    ///
    /// Section 11.3 requires both on every leaf, so the rule is not evaluated
    /// rather than evaluated into a leaf with no citation.
    RuleSourceSpanAbsent {
        /// The rule.
        rule: RuleId,
    },
    /// A rule concluded `UNKNOWN` because an official fact is unconfirmed.
    OpenOfficialFact {
        /// The rule.
        rule: RuleId,
        /// The cell this crate names for it, when it names one.
        gate: Option<OpenGate>,
        /// The cell `academic-requirement` named.
        rule_gate: RuleGate,
    },
    /// A rule concluded `UNKNOWN` because the record holds no reading it needs.
    RuleInputAbsent {
        /// The rule.
        rule: RuleId,
        /// What the rule needed.
        input: &'static str,
    },
    /// A rule concluded `CONFLICT`: two recorded facts cannot both stand.
    RuleConflict {
        /// The rule.
        rule: RuleId,
    },
    /// Two official sources disagree about a rule and nobody has decided.
    ///
    /// Section 8.4 already says a dangerous determination stays
    /// `INDETERMINATE` while a case is unresolved; this is that refusal, with
    /// the reference a user can act on.
    UnresolvedSourceConflict {
        /// The rule in dispute, as the conflict case spells it.
        rule: String,
        /// The connector that collected the first document.
        left_connector: String,
        /// The connector that collected the second document.
        right_connector: String,
    },
    /// An attempt whose credit contribution the record engine could not settle.
    RecognitionUndecided {
        /// The attempt.
        attempt: AttemptId,
        /// The record engine's own reason.
        reason: &'static str,
    },
    /// No source-freshness criterion has been recorded.
    SourceFreshnessPolicyAbsent,
    /// The open-conflict store was never read for this audit.
    ///
    /// Not the same as reading it and finding nothing. Section 8.4 keeps a
    /// dangerous determination `INDETERMINATE` while a case is unresolved, and
    /// an audit that never asked whether a case is open cannot say that none
    /// is -- so it says this instead of counting zero cases and concluding.
    SourceConflictSurveyAbsent,
    /// The rule set's source is older than the recorded criterion admits.
    SourceNotFresh {
        /// How old the source is, in seconds.
        age_seconds: u64,
        /// The largest age the recorded criterion admits, in seconds.
        limit_seconds: u64,
    },
}

impl MissingCheck {
    /// What the user or the administration has to do to settle it.
    #[must_use]
    pub fn action(&self) -> String {
        match self {
            Self::ProfileField { field, gate } => {
                let cell =
                    gate.map_or_else(String::new, |gate| format!(" ({})", gate.identifier()));
                format!("{}{cell}", field.action())
            }
            Self::NoRuleSetCovers { rendered_profile } => format!(
                "publish a requirement set whose scope covers {rendered_profile}, or correct the \
                 profile"
            ),
            Self::CompetingRuleSets { versions } => {
                let mut names: Vec<String> = versions.iter().map(ToString::to_string).collect();
                names.sort();
                format!(
                    "decide which published requirement set applies: {} both cover this profile",
                    names.join(" and ")
                )
            }
            Self::RuleSourceSpanAbsent { rule } => format!(
                "record the official page and paragraph rule {} was read from",
                rule.as_str()
            ),
            Self::OpenOfficialFact {
                rule,
                gate,
                rule_gate,
            } => {
                let identifier = gate.map_or_else(
                    || rule_gate.identifier().to_owned(),
                    |gate| gate.identifier().to_owned(),
                );
                format!(
                    "confirm the official fact rule {} depends on ({identifier})",
                    rule.as_str()
                )
            }
            Self::RuleInputAbsent { rule, input } => format!(
                "record the {input} rule {} reads; the record holds none",
                rule.as_str()
            ),
            Self::RuleConflict { rule } => format!(
                "correct the record: rule {} met two facts that cannot both stand",
                rule.as_str()
            ),
            Self::UnresolvedSourceConflict {
                rule,
                left_connector,
                right_connector,
            } => format!(
                "resolve the source conflict on rule {rule}: {left_connector} and \
                 {right_connector} disagree"
            ),
            Self::RecognitionUndecided { attempt, reason } => format!(
                "record the recognition decision for attempt {attempt}; the record engine reports \
                 {reason} (GATE-38-006)"
            ),
            Self::SourceFreshnessPolicyAbsent => {
                "record the source-freshness criterion a high-impact graduation result must meet; \
                 no source states one and none is assumed"
                    .to_owned()
            }
            Self::SourceConflictSurveyAbsent => {
                "read the open-conflict store for the sources this rule set was published from; \
                 this audit was given no reading of it, and no reading is not the same as none \
                 open"
                    .to_owned()
            }
            Self::SourceNotFresh {
                age_seconds,
                limit_seconds,
            } => format!(
                "re-retrieve the official source: it is {age_seconds}s old and the recorded \
                 criterion admits {limit_seconds}s"
            ),
        }
    }

    /// The stable token the frozen explanation spells.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::ProfileField { .. } => "PROFILE_FIELD",
            Self::NoRuleSetCovers { .. } => "NO_RULE_SET_COVERS",
            Self::CompetingRuleSets { .. } => "COMPETING_RULE_SETS",
            Self::RuleSourceSpanAbsent { .. } => "RULE_SOURCE_SPAN_ABSENT",
            Self::OpenOfficialFact { .. } => "OPEN_OFFICIAL_FACT",
            Self::RuleInputAbsent { .. } => "RULE_INPUT_ABSENT",
            Self::RuleConflict { .. } => "RULE_CONFLICT",
            Self::UnresolvedSourceConflict { .. } => "UNRESOLVED_SOURCE_CONFLICT",
            Self::RecognitionUndecided { .. } => "RECOGNITION_UNDECIDED",
            Self::SourceFreshnessPolicyAbsent => "SOURCE_FRESHNESS_POLICY_ABSENT",
            Self::SourceConflictSurveyAbsent => "SOURCE_CONFLICT_SURVEY_ABSENT",
            Self::SourceNotFresh { .. } => "SOURCE_NOT_FRESH",
        }
    }

    /// Which of section 11.1's eight selector inputs this check is about, when
    /// it is about one.
    #[must_use]
    pub const fn dimension(&self) -> Option<SelectorDimension> {
        match self {
            Self::ProfileField { field, .. } => Some(field.dimension()),
            _ => None,
        }
    }

    /// The canonical single line the explanation renders.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        format!("{} {}", self.kind(), self.action())
    }
}

/// What an audit reads off one of `P2-U6`'s conflict cases.
///
/// Three facts and a decision, and the only producer is
/// [`ConflictReference::of`], which takes a real [`ConflictCase`]. The five
/// dimensions section 8.4 compares, and whether comparing them found a real
/// disagreement, are `academic-ingestion`'s work and are not redone here: this
/// crate reads the disposition that work produced and refuses to conclude while
/// it is `Indeterminate`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictReference {
    rule: String,
    left_connector: String,
    right_connector: String,
    resolved: bool,
}

impl ConflictReference {
    /// Reads one case.
    #[must_use]
    pub fn of(case: &ConflictCase) -> Self {
        Self {
            rule: case.left().rule().as_str().to_owned(),
            left_connector: case.left().connector().as_str().to_owned(),
            right_connector: case.right().connector().as_str().to_owned(),
            resolved: case.disposition() == academic_ingestion::AuditDisposition::Determinate,
        }
    }

    /// Rebuilds one from frozen inputs.
    pub(crate) const fn decoded(
        rule: String,
        left_connector: String,
        right_connector: String,
        resolved: bool,
    ) -> Self {
        Self {
            rule,
            left_connector,
            right_connector,
            resolved,
        }
    }

    /// The rule in dispute.
    #[must_use]
    pub fn rule(&self) -> &str {
        &self.rule
    }

    /// The connector that collected the first document.
    #[must_use]
    pub fn left_connector(&self) -> &str {
        &self.left_connector
    }

    /// The connector that collected the second document.
    #[must_use]
    pub fn right_connector(&self) -> &str {
        &self.right_connector
    }

    /// Whether a person has decided.
    #[must_use]
    pub const fn is_resolved(&self) -> bool {
        self.resolved
    }
}

/// The largest age a high-impact graduation result admits in its source.
///
/// No `Default`, no constant, and no constructor that omits the bound. The
/// specification states no number, so the number is the user's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFreshnessPolicy {
    max_age_seconds: u64,
}

impl SourceFreshnessPolicy {
    /// Records the criterion.
    #[must_use]
    pub const fn max_age_seconds(max_age_seconds: u64) -> Self {
        Self { max_age_seconds }
    }

    /// The recorded bound.
    #[must_use]
    pub const fn limit_seconds(self) -> u64 {
        self.max_age_seconds
    }
}

/// Gate one: every applicable rule was evaluated and none reads `UNKNOWN`.
///
/// Private field, no public constructor, and `establish` is crate-private.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverageWitness {
    rules_covered: usize,
}

impl CoverageWitness {
    /// Establishes coverage, or does not.
    ///
    /// Refuses an empty leaf set: a tree with no leaves covers every rule
    /// vacuously, and a vacuous coverage witness is the empty guard this
    /// repository has found in ten consecutive tasks.
    pub(crate) fn establish(leaves: &[ProofLeaf], unevaluated: &[RuleId]) -> Option<Self> {
        if leaves.is_empty() || !unevaluated.is_empty() {
            return None;
        }
        if leaves
            .iter()
            .any(|leaf| leaf.status() == ProofStatus::Unknown)
        {
            return None;
        }
        Some(Self {
            rules_covered: leaves.len(),
        })
    }

    /// How many rules the coverage was established over.
    #[must_use]
    pub const fn rules_covered(self) -> usize {
        self.rules_covered
    }
}

/// Gate two: no leaf is in conflict and no applicable source conflict is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConflictFreeWitness {
    cases_examined: usize,
}

impl ConflictFreeWitness {
    /// Establishes the absence of conflict, or does not.
    ///
    /// `cases` is `None` when the conflict store was not read for this audit.
    /// That is refused, and it is refused for the reason
    /// [`CoverageWitness::establish`] refuses an empty leaf set eleven lines
    /// above: a witness that examined nothing witnesses nothing. The two
    /// halves used to be asymmetric -- coverage refused the vacuous case in a
    /// comment naming it as the failure mode this repository keeps finding,
    /// and the conflict gate next to it issued a witness over an empty slice
    /// and let the audit answer 졸업 가능 with `cases examined = 0`.
    ///
    /// `Some(&[])` is admitted: a store that was read and held nothing is a
    /// real observation, and `cases_examined` records that it was zero.
    pub(crate) fn establish(
        leaves: &[ProofLeaf],
        cases: Option<&[&ConflictReference]>,
    ) -> Option<Self> {
        if leaves
            .iter()
            .any(|leaf| leaf.status() == ProofStatus::Conflict)
        {
            return None;
        }
        let cases = cases?;
        if cases.iter().any(|case| !case.is_resolved()) {
            return None;
        }
        Some(Self {
            cases_examined: cases.len(),
        })
    }

    /// How many source-conflict cases were examined.
    #[must_use]
    pub const fn cases_examined(self) -> usize {
        self.cases_examined
    }
}

/// Gate three: the official source meets the recorded freshness criterion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FreshnessWitness {
    age_seconds: u64,
}

impl FreshnessWitness {
    /// Establishes freshness against a recorded criterion, or does not.
    ///
    /// `None` when no criterion is recorded, when the source is older than it,
    /// and when the source was retrieved after the instant the audit is
    /// anchored to -- a reading from the future is not a fresh reading, it is a
    /// clock that disagrees with the record.
    pub(crate) fn establish(
        policy: Option<SourceFreshnessPolicy>,
        retrieved_at: RetrievalInstant,
        as_of: TimestampMillis,
    ) -> Option<Self> {
        let policy = policy?;
        let as_of_seconds = u64::try_from(as_of.value().div_euclid(1_000)).ok()?;
        let age_seconds = as_of_seconds.checked_sub(retrieved_at.seconds())?;
        (age_seconds <= policy.limit_seconds()).then_some(Self { age_seconds })
    }

    /// How old the source was when the audit ran, in seconds.
    #[must_use]
    pub const fn age_seconds(self) -> u64 {
        self.age_seconds
    }
}

/// What a determinate audit concluded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraduationOutcome {
    /// 졸업 가능 -- every applicable rule is satisfied.
    Possible,
    /// 졸업 불가 -- at least one applicable rule is not, and the tree says which.
    NotPossible,
}

impl GraduationOutcome {
    /// The stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Possible => "POSSIBLE",
            Self::NotPossible => "NOT_POSSIBLE",
        }
    }
}

/// A graduation determination, and the three witnesses that made it one.
///
/// Private fields and one constructor, which takes all three witnesses by
/// value. There is no `Default`, no setter, and no route from an
/// [`IndeterminateVerdict`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeterminateVerdict {
    outcome: GraduationOutcome,
    coverage: CoverageWitness,
    conflict_free: ConflictFreeWitness,
    freshness: FreshnessWitness,
}

impl DeterminateVerdict {
    /// Assembles a determination from all three gates.
    pub(crate) const fn new(
        outcome: GraduationOutcome,
        coverage: CoverageWitness,
        conflict_free: ConflictFreeWitness,
        freshness: FreshnessWitness,
    ) -> Self {
        Self {
            outcome,
            coverage,
            conflict_free,
            freshness,
        }
    }

    /// 졸업 가능 or 졸업 불가.
    #[must_use]
    pub const fn outcome(self) -> GraduationOutcome {
        self.outcome
    }

    /// Gate one.
    #[must_use]
    pub const fn coverage(self) -> CoverageWitness {
        self.coverage
    }

    /// Gate two.
    #[must_use]
    pub const fn conflict_free(self) -> ConflictFreeWitness {
        self.conflict_free
    }

    /// Gate three.
    #[must_use]
    pub const fn freshness(self) -> FreshnessWitness {
        self.freshness
    }
}

/// An audit that reached no determination, with what is outstanding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndeterminateVerdict {
    missing: Vec<MissingCheck>,
}

impl IndeterminateVerdict {
    /// Records what is outstanding, non-empty by arity.
    ///
    /// The first check is a **parameter**, so there is no call that records
    /// none. An indeterminate audit that cannot say what it is waiting for is
    /// the vague "정보 부족" section 11.1 forbids, and it is not a value that
    /// can be written rather than one a check refuses.
    pub(crate) fn new(first: MissingCheck, rest: Vec<MissingCheck>) -> Self {
        let mut missing = Vec::with_capacity(rest.len() + 1);
        missing.push(first);
        missing.extend(rest);
        Self { missing }
    }

    /// Records a list that may be empty, and answers `None` when it is.
    ///
    /// The companion to [`IndeterminateVerdict::new`] for the one caller that
    /// accumulates checks in a loop and does not know in advance whether it
    /// found any. It splits the list rather than testing it, so the non-empty
    /// invariant is still the constructor's.
    pub(crate) fn from_checks(missing: Vec<MissingCheck>) -> Option<Self> {
        let mut checks = missing.into_iter();
        let first = checks.next()?;
        Some(Self::new(first, checks.collect()))
    }

    /// Every outstanding check, in the order the audit found them.
    #[must_use]
    pub fn missing(&self) -> &[MissingCheck] {
        &self.missing
    }
}

/// Section 11.3's root reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DegreeVerdict {
    /// Section 11.4's three gates all hold.
    Determinate(DeterminateVerdict),
    /// At least one does not, and these are the exact checks.
    Indeterminate(IndeterminateVerdict),
}

impl DegreeVerdict {
    /// The stable spelling section 11.3's root line carries.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Determinate(_) => "DETERMINATE",
            Self::Indeterminate(_) => "INDETERMINATE",
        }
    }

    /// The outstanding checks, which are empty exactly when determinate.
    #[must_use]
    pub fn missing(&self) -> &[MissingCheck] {
        match self {
            Self::Determinate(_) => &[],
            Self::Indeterminate(verdict) => verdict.missing(),
        }
    }

    /// The determination, when there is one.
    #[must_use]
    pub const fn determinate(&self) -> Option<DeterminateVerdict> {
        match self {
            Self::Determinate(verdict) => Some(*verdict),
            Self::Indeterminate(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    //! The three gates' refusals, driven directly.
    //!
    //! `establish` is crate-private on all three, so these live here rather
    //! than in `tests/`. `P2-A3`'s guard-deletion sweep found each of the
    //! refusals below still passing the whole `academic-audit` suite when it
    //! was deleted: nothing drove them.

    use super::{
        ConflictFreeWitness, CoverageWitness, FreshnessWitness, RetrievalInstant,
        SourceFreshnessPolicy, TimestampMillis,
    };

    const AS_OF: TimestampMillis = TimestampMillis::new(1_772_100_000_000);
    const AS_OF_SECONDS: u64 = 1_772_100_000;
    const POLICY: SourceFreshnessPolicy = SourceFreshnessPolicy::max_age_seconds(200_000);

    /// A tree with no leaf covers every rule vacuously.
    ///
    /// `RuleSetDraft::publish` refuses a set with no rule, so this is the
    /// refusal that documents the invariant at the point that depends on it
    /// rather than a reachable state. Nothing else drives it: the sweep deleted
    /// it and all twenty-nine tests passed.
    #[test]
    fn coverage_refuses_an_empty_leaf_set() {
        assert!(CoverageWitness::establish(&[], &[]).is_none());
    }

    /// A conflict survey nobody made is not a survey that found nothing.
    #[test]
    fn the_conflict_gate_separates_an_unread_store_from_an_empty_one() {
        assert!(ConflictFreeWitness::establish(&[], None).is_none());
        assert_eq!(
            ConflictFreeWitness::establish(&[], Some(&[])).map(ConflictFreeWitness::cases_examined),
            Some(0),
            "a store that was read and held nothing is a real observation"
        );
    }

    /// A reading from the future is not a fresh reading.
    ///
    /// Without the `checked_sub`, a source retrieved after the instant the
    /// audit is anchored to reads as age zero -- maximally fresh, from a clock
    /// that disagrees with the record. The three cases below are one second
    /// after the anchor, the anchor itself, and one second before it.
    #[test]
    fn freshness_refuses_a_retrieval_after_the_audit_instant() {
        assert!(
            FreshnessWitness::establish(
                Some(POLICY),
                RetrievalInstant::at(AS_OF_SECONDS + 1),
                AS_OF,
            )
            .is_none(),
            "a source retrieved after the audit instant established freshness"
        );
        assert_eq!(
            FreshnessWitness::establish(Some(POLICY), RetrievalInstant::at(AS_OF_SECONDS), AS_OF)
                .map(FreshnessWitness::age_seconds),
            Some(0)
        );
        assert_eq!(
            FreshnessWitness::establish(
                Some(POLICY),
                RetrievalInstant::at(AS_OF_SECONDS - 1),
                AS_OF,
            )
            .map(FreshnessWitness::age_seconds),
            Some(1)
        );
        // And the bound itself still refuses, so the future case above is not
        // passing because every reading refuses.
        assert!(
            FreshnessWitness::establish(
                Some(POLICY),
                RetrievalInstant::at(AS_OF_SECONDS - POLICY.limit_seconds() - 1),
                AS_OF,
            )
            .is_none()
        );
    }
}
