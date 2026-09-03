//! The four independent effective-dated course relations.
//!
//! Section 11.4: *동일·대체·폐지·경과조치는 독립 rule이며 양방향 동일성으로
//! 단순화하지 않는다* — sameness, replacement, retirement and transitional
//! measures are independent rules and are not simplified into a bidirectional
//! identity. Section 8.2 says the same of the first two: *동일·대체 관계는 별도
//! effective-dated edge다*.
//!
//! Three of those four are course-level and live here.  The fourth,
//! 경과조치, is a curriculum-version-level arrangement and lives in
//! [`crate::version`]; see that module for why, and
//! `docs/contracts/curriculum-aggregates.md` for the placement decision.
//! Section 11.4's *동일* is two questions this crate keeps apart — whether two
//! course rows are the same course ([`IdentityDecision`]) and whether one
//! course may stand in for another ([`EquivalenceRelation`]) — because the plan
//! names both and the acceptance evidence distinguishes them.
//!
//! # What "independent" is executed as
//!
//! Four types, four constructors, four lookups, and no path from any one to any
//! other:
//!
//! - no `From`, `TryFrom`, `Into`, `AsRef` or `Deref` between them, which
//!   `no_relation_derives_another` compares as a whole `impl` set rather than
//!   as a token list;
//! - no method on one that returns another, which the same test pins as the
//!   whole set of signatures in this module;
//! - no field on one that holds another, which the struct pins fix.
//!
//! [`CourseRelations`] answers each question from its own recorded set. Asking
//! whether two courses are the same reads [`IdentityDecision`]s and nothing
//! else, so recording a replacement moves that answer nowhere — which is
//! `replacement_does_not_imply_identity`.
//!
//! # Absence is `UNKNOWN`, and it is not a default
//!
//! [`CourseCodeReuse::Unknown`] is what [`CourseRelations::same_course`]
//! returns when no decision addresses the pair. There is no heuristic on the
//! code string, no rule that two rows sharing a code are one course, and no
//! rule that a replacement makes them one. Section 8.2's `courseCode` is a
//! label the catalogue prints; whether one code names one course across time is
//! an official fact somebody has to record.

use std::collections::BTreeSet;

use academic_domain::{CourseId, DecisionId, TimestampMillis, ValidInterval};

use crate::error::CurriculumError;

/// The three course-level relations this module owns, enumerated.
///
/// Enumerated rather than counted: `the_course_relations_are_section_11_4s_own`
/// maps each variant onto the specification's own sentence and walks the
/// sentence forwards, so a relation dropped from this list fails against
/// section 11.4 rather than against a number written here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CourseRelationKind {
    /// 동일, as an identity: two course rows are one course.
    Identity,
    /// 동일, as substitutability: one course stands in for another.
    Equivalence,
    /// 대체: a course was replaced by another.
    Replacement,
    /// 폐지: a course was retired.
    Retirement,
}

impl CourseRelationKind {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::Identity,
        Self::Equivalence,
        Self::Replacement,
        Self::Retirement,
    ];

    /// Stable spelling, which is also migration 0014's `CHECK` vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Identity => "IDENTITY",
            Self::Equivalence => "EQUIVALENCE",
            Self::Replacement => "REPLACEMENT",
            Self::Retirement => "RETIREMENT",
        }
    }

    /// The specification word this relation is.
    #[must_use]
    pub const fn specification_word(self) -> &'static str {
        match self {
            Self::Identity | Self::Equivalence => "동일",
            Self::Replacement => "대체",
            Self::Retirement => "폐지",
        }
    }
}

/// What a recorded [`IdentityDecision`] says about two course rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CourseCodeReuse {
    /// No decision addresses this pair. Nothing is inferred from the code.
    Unknown,
    /// A decision records that the two rows are one durable course.
    Same,
    /// A decision records that the code was reused for a different course.
    Distinct,
}

impl CourseCodeReuse {
    /// Exhaustive listing, `Unknown` first.
    pub const ALL: [Self; 3] = [Self::Unknown, Self::Same, Self::Distinct];

    /// Stable spelling, which is also migration 0014's `CHECK` vocabulary.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Same => "SAME",
            Self::Distinct => "DISTINCT",
        }
    }
}

/// 동일, as identity. An explicit decision about whether a reused course code
/// names one durable course.
///
/// The decision identifier is required: section 8.2's contract is that
/// course-code reuse is *recorded as an explicit identity decision rather than
/// inferred*, and a decision with nothing to point at would be an inference
/// wearing a record's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityDecision {
    earlier: CourseId,
    later: CourseId,
    verdict: CourseCodeReuse,
    decision: DecisionId,
    valid_time: ValidInterval,
}

impl IdentityDecision {
    /// Records one decision about one ordered pair of course rows.
    ///
    /// `verdict` may not be [`CourseCodeReuse::Unknown`]: `Unknown` is the
    /// absence of a decision, so a decision recording it would be a record
    /// saying nothing was recorded.
    pub fn record(
        earlier: CourseId,
        later: CourseId,
        verdict: CourseCodeReuse,
        decision: DecisionId,
        valid_time: ValidInterval,
    ) -> Result<Self, CurriculumError> {
        if earlier == later {
            return Err(CurriculumError::Reflexive {
                relation: "identity",
            });
        }
        if matches!(verdict, CourseCodeReuse::Unknown) {
            return Err(CurriculumError::Malformed {
                field: "identity decision",
                reason: "UNKNOWN is the absence of a decision, not a verdict one can record",
            });
        }
        Ok(Self {
            earlier,
            later,
            verdict,
            decision,
            valid_time,
        })
    }

    /// The earlier course row.
    #[must_use]
    pub const fn earlier(self) -> CourseId {
        self.earlier
    }

    /// The later course row.
    #[must_use]
    pub const fn later(self) -> CourseId {
        self.later
    }

    /// What the decision says.
    #[must_use]
    pub const fn verdict(self) -> CourseCodeReuse {
        self.verdict
    }

    /// The user decision this was recorded against.
    #[must_use]
    pub const fn decision(self) -> DecisionId {
        self.decision
    }

    /// When the decision applies.
    #[must_use]
    pub const fn valid_time(self) -> ValidInterval {
        self.valid_time
    }
}

/// 동일, as substitutability: `source` may be presented in place of `target`.
///
/// Directional. Section 8.2 calls it an effective-dated edge and an edge has a
/// direction; recognising a transfer course towards a requirement says nothing
/// about the reverse, and `GATE-38-014` is open precisely because the
/// substitution rules are an official fact nobody has confirmed.
///
/// There is no `reverse`, no `symmetric`, and no constructor that records both
/// directions. The second direction is a second assertion.
///
/// The two ends are `source` and `target` rather than `from` and `to` because
/// an inherent `from` shadows `From::from` at the call site, which would have
/// made `EquivalenceRelation::from(other_relation)` resolve here instead of
/// failing as the missing conversion it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EquivalenceRelation {
    source: CourseId,
    target: CourseId,
    valid_time: ValidInterval,
}

impl EquivalenceRelation {
    /// Records that `source` may stand in for `target` over `valid_time`.
    pub fn record(
        source: CourseId,
        target: CourseId,
        valid_time: ValidInterval,
    ) -> Result<Self, CurriculumError> {
        if source == target {
            return Err(CurriculumError::Reflexive {
                relation: "equivalence",
            });
        }
        Ok(Self {
            source,
            target,
            valid_time,
        })
    }

    /// The course that may be presented.
    #[must_use]
    pub const fn source(self) -> CourseId {
        self.source
    }

    /// The course it may be presented for.
    #[must_use]
    pub const fn target(self) -> CourseId {
        self.target
    }

    /// When the substitution applies.
    #[must_use]
    pub const fn valid_time(self) -> ValidInterval {
        self.valid_time
    }
}

/// 대체: `retired` was replaced by `replacement`.
///
/// A replacement is a catalogue event, not a claim that the two courses are one
/// course and not a claim that either stands in for the other. Section 8.1's
/// own example is *기하모델링 폐지·고급컴퓨터그래픽스 대체*: the first course
/// ends and a different one is named in its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplacementRelation {
    retired: CourseId,
    replacement: CourseId,
    valid_time: ValidInterval,
}

impl ReplacementRelation {
    /// Records that `replacement` was named in place of `retired`.
    pub fn record(
        retired: CourseId,
        replacement: CourseId,
        valid_time: ValidInterval,
    ) -> Result<Self, CurriculumError> {
        if retired == replacement {
            return Err(CurriculumError::Reflexive {
                relation: "replacement",
            });
        }
        Ok(Self {
            retired,
            replacement,
            valid_time,
        })
    }

    /// The course that was replaced.
    #[must_use]
    pub const fn retired(self) -> CourseId {
        self.retired
    }

    /// The course named in its place.
    #[must_use]
    pub const fn replacement(self) -> CourseId {
        self.replacement
    }

    /// When the replacement applies.
    #[must_use]
    pub const fn valid_time(self) -> ValidInterval {
        self.valid_time
    }
}

/// 폐지: a course was retired.
///
/// This type has one course and an interval. It has no replacement field, and
/// there is no constructor that takes one, so a retirement with no replacement
/// is not a special case here — it is the only shape a retirement has. Section
/// 8.1's second example, *IT창업개론 폐지·대체 미지정* (retired, replacement
/// unspecified), is therefore representable by construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetirementRelation {
    course: CourseId,
    valid_time: ValidInterval,
}

impl RetirementRelation {
    /// Records that `course` was retired over `valid_time`.
    #[must_use]
    pub const fn record(course: CourseId, valid_time: ValidInterval) -> Self {
        Self { course, valid_time }
    }

    /// The retired course.
    #[must_use]
    pub const fn course(self) -> CourseId {
        self.course
    }

    /// When the retirement applies.
    #[must_use]
    pub const fn valid_time(self) -> ValidInterval {
        self.valid_time
    }
}

/// The four recorded relation sets, and the four lookups over them.
///
/// Each lookup reads exactly one set. That is what makes the four independent:
/// there is no query here whose answer is a function of more than one of them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CourseRelations {
    identities: Vec<IdentityDecision>,
    equivalences: Vec<EquivalenceRelation>,
    replacements: Vec<ReplacementRelation>,
    retirements: Vec<RetirementRelation>,
}

impl CourseRelations {
    /// An empty set of relations.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            identities: Vec::new(),
            equivalences: Vec::new(),
            replacements: Vec::new(),
            retirements: Vec::new(),
        }
    }

    /// Appends one identity decision.
    pub fn record_identity(&mut self, decision: IdentityDecision) {
        self.identities.push(decision);
    }

    /// Appends one equivalence.
    pub fn record_equivalence(&mut self, relation: EquivalenceRelation) {
        self.equivalences.push(relation);
    }

    /// Appends one replacement.
    pub fn record_replacement(&mut self, relation: ReplacementRelation) {
        self.replacements.push(relation);
    }

    /// Appends one retirement.
    pub fn record_retirement(&mut self, relation: RetirementRelation) {
        self.retirements.push(relation);
    }

    /// Rewinds each set to the length it had at a recorded mark.
    ///
    /// `pub(crate)` on purpose: an append-only record has no public truncation.
    /// The one caller is [`crate::publish::CurriculumPublisher::publish`],
    /// which uses it to undo a publication that failed part-way, and
    /// `the_relations_have_no_public_truncation` pins that this stays the only
    /// one and that it is not public.
    pub(crate) fn truncate_to(
        &mut self,
        identities: usize,
        equivalences: usize,
        replacements: usize,
        retirements: usize,
    ) {
        self.identities.truncate(identities);
        self.equivalences.truncate(equivalences);
        self.replacements.truncate(replacements);
        self.retirements.truncate(retirements);
    }

    /// Every recorded identity decision.
    #[must_use]
    pub fn identities(&self) -> &[IdentityDecision] {
        &self.identities
    }

    /// Every recorded equivalence.
    #[must_use]
    pub fn equivalences(&self) -> &[EquivalenceRelation] {
        &self.equivalences
    }

    /// Every recorded replacement.
    #[must_use]
    pub fn replacements(&self) -> &[ReplacementRelation] {
        &self.replacements
    }

    /// Every recorded retirement.
    #[must_use]
    pub fn retirements(&self) -> &[RetirementRelation] {
        &self.retirements
    }

    /// Whether two course rows are one durable course at `instant`.
    ///
    /// Reads [`Self::identities`] and nothing else. With no decision addressing
    /// the ordered pair the answer is [`CourseCodeReuse::Unknown`]; a
    /// replacement, an equivalence, a retirement, and a shared course code all
    /// leave it there.
    #[must_use]
    pub fn same_course(
        &self,
        earlier: CourseId,
        later: CourseId,
        instant: TimestampMillis,
    ) -> CourseCodeReuse {
        self.identities
            .iter()
            .find(|decision| {
                decision.earlier == earlier
                    && decision.later == later
                    && decision.valid_time.contains(instant)
            })
            .map_or(CourseCodeReuse::Unknown, |decision| decision.verdict())
    }

    /// Whether `from` may be presented for `to` at `instant`.
    ///
    /// Reads [`Self::equivalences`] and nothing else, in the asserted direction
    /// only. `equivalent(a, b)` and `equivalent(b, a)` are two questions.
    #[must_use]
    pub fn equivalent(&self, source: CourseId, target: CourseId, instant: TimestampMillis) -> bool {
        self.equivalences.iter().any(|relation| {
            relation.source == source
                && relation.target == target
                && relation.valid_time.contains(instant)
        })
    }

    /// Which courses were named in place of `retired` at `instant`.
    ///
    /// Reads [`Self::replacements`] and nothing else. An empty result is
    /// section 8.1's *대체 미지정*: retired with no replacement named.
    #[must_use]
    pub fn replacements_for(
        &self,
        retired: CourseId,
        instant: TimestampMillis,
    ) -> BTreeSet<CourseId> {
        self.replacements
            .iter()
            .filter(|relation| relation.retired == retired && relation.valid_time.contains(instant))
            .map(|relation| relation.replacement)
            .collect()
    }

    /// Whether `course` is retired at `instant`.
    ///
    /// Reads [`Self::retirements`] and nothing else. A course with a
    /// replacement recorded against it is not retired unless a retirement says
    /// so, and a retired course needs no replacement to be one.
    #[must_use]
    pub fn retired(&self, course: CourseId, instant: TimestampMillis) -> bool {
        self.retirements
            .iter()
            .any(|relation| relation.course == course && relation.valid_time.contains(instant))
    }
}
