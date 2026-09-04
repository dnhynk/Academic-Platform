//! The proof leaf, and why an incomplete one is not a value.
//!
//! Section 11.3 names four things every leaf carries: the applied rule ID, the
//! source page/paragraph, the `CourseAttempt` used, and the equivalency
//! decision. **Every one of them is a constructor parameter**, and
//! [`ProofLeaf`] has private fields, no `Default`, no setter, and no builder.
//! There is no expression that produces a leaf with three of the four, so
//! "a leaf is missing its citation" is a state the type does not have.
//!
//! The two that could plausibly be empty are the ones that are not `Option`:
//!
//! - [`AttemptUsage`] is either the attempts a verdict rests on -- non-empty by
//!   construction -- or the reason no attempt was used. "This rule used no
//!   attempt" and "nobody recorded which attempts this rule used" are different
//!   facts, and the second is not representable.
//! - [`EquivalencyDecision`] is either the substitutions applied -- non-empty by
//!   construction -- or [`EquivalencyDecision::NoneApplied`], which is a
//!   decision and says so.
//!
//! `tests/compile_fail/` holds the cases: there is no `ProofLeaf::new` with a
//! shorter argument list, no struct literal, and no way to reach a leaf without
//! a [`crate::source::RuleSourceSpan`].

use academic_domain::{AttemptId, engines::ProofStatus};
use academic_requirement::{Measure, OpenGate as RuleGate, RuleId, RuleType};

use crate::{gate::OpenGate, source::RuleSourceSpan};

/// Which attempts a verdict rests on, or why none did.
///
/// Not a `Vec` that may be empty: an empty list would read as "no attempt" and
/// as "nobody said" at the same time, and only one of those can be published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptUsage {
    /// The verdict rests on these attempts, in the order the rule read them.
    Used(Vec<AttemptId>),
    /// No attempt was used, for this stated reason.
    NoneUsed(NoAttemptReason),
}

impl AttemptUsage {
    /// The attempts a verdict rests on, refusing an empty list.
    ///
    /// An empty list is not an error the caller made -- it is the other arm --
    /// so this returns the other arm rather than a `Result`, with the reason
    /// the rule reached it.
    #[must_use]
    pub fn of(attempts: Vec<AttemptId>, when_empty: NoAttemptReason) -> Self {
        if attempts.is_empty() {
            Self::NoneUsed(when_empty)
        } else {
            Self::Used(attempts)
        }
    }

    /// The attempts, when the verdict rests on any.
    #[must_use]
    pub fn attempts(&self) -> &[AttemptId] {
        match self {
            Self::Used(attempts) => attempts,
            Self::NoneUsed(_) => &[],
        }
    }

    /// The canonical rendering, which always says something.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        match self {
            Self::Used(attempts) => {
                let mut rendered: Vec<String> = attempts.iter().map(ToString::to_string).collect();
                rendered.sort();
                format!("used {}", rendered.join(","))
            }
            Self::NoneUsed(reason) => format!("used none: {}", reason.as_str()),
        }
    }
}

/// Why a leaf used no attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NoAttemptReason {
    /// The transcript holds no attempt this rule counts.
    NoMatchingAttempt,
    /// The rule reads no attempt at all -- a grade-point floor reads the
    /// grade-point reading, and an exception approval reads an approval.
    RuleReadsNoAttempt,
    /// The rule could not be evaluated, so no attempt was reached.
    RuleNotEvaluated,
}

impl NoAttemptReason {
    /// The stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoMatchingAttempt => "NO_MATCHING_ATTEMPT",
            Self::RuleReadsNoAttempt => "RULE_READS_NO_ATTEMPT",
            Self::RuleNotEvaluated => "RULE_NOT_EVALUATED",
        }
    }
}

/// Which equivalencies a verdict applied, or the decision that none did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquivalencyDecision {
    /// These `EQUIVALENCY` rules of the same published set were applied.
    Applied(Vec<RuleId>),
    /// No substitution was used. Section 11.3 requires the decision, and this
    /// is one.
    NoneApplied,
}

impl EquivalencyDecision {
    /// The applied substitutions, refusing an empty list into the other arm.
    #[must_use]
    pub fn of(rules: Vec<RuleId>) -> Self {
        if rules.is_empty() {
            Self::NoneApplied
        } else {
            Self::Applied(rules)
        }
    }

    /// The rules applied, when any were.
    #[must_use]
    pub fn rules(&self) -> &[RuleId] {
        match self {
            Self::Applied(rules) => rules,
            Self::NoneApplied => &[],
        }
    }

    /// The canonical rendering, which always says something.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        match self {
            Self::Applied(rules) => {
                let mut rendered: Vec<&str> = rules.iter().map(RuleId::as_str).collect();
                rendered.sort_unstable();
                format!("equivalency {}", rendered.join(","))
            }
            Self::NoneApplied => "equivalency none".to_owned(),
        }
    }
}

/// One node of section 11.3's proof tree, with everything a leaf must carry.
///
/// Private fields and one constructor. Section 11.3's four are the first four
/// parameters, in the order the sentence names them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofLeaf {
    rule: RuleId,
    source: RuleSourceSpan,
    attempts: AttemptUsage,
    equivalency: EquivalencyDecision,
    rule_type: RuleType,
    status: ProofStatus,
    measure: Option<Measure>,
    open_gate: Option<OpenGate>,
    rule_gate: Option<RuleGate>,
}

impl ProofLeaf {
    /// Builds a leaf from all four of section 11.3's parts and the verdict.
    ///
    /// There is no shorter form. `measure` is an `Option` because section 11.2
    /// has rule types that measure nothing -- a co-requisite is met or it is
    /// not -- and `None` there is "this rule has no numerator", which the
    /// explanation renders as such. `open_gate` is `Some` exactly when the
    /// status is `UNKNOWN` for an unconfirmed official fact.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        rule: RuleId,
        source: RuleSourceSpan,
        attempts: AttemptUsage,
        equivalency: EquivalencyDecision,
        rule_type: RuleType,
        status: ProofStatus,
        measure: Option<Measure>,
        open_gate: Option<OpenGate>,
        rule_gate: Option<RuleGate>,
    ) -> Self {
        Self {
            rule,
            source,
            attempts,
            equivalency,
            rule_type,
            status,
            measure,
            open_gate,
            rule_gate,
        }
    }

    /// The applied rule identifier -- section 11.3's first part.
    #[must_use]
    pub const fn rule(&self) -> &RuleId {
        &self.rule
    }

    /// The source page and paragraph -- section 11.3's second part.
    #[must_use]
    pub const fn source(&self) -> &RuleSourceSpan {
        &self.source
    }

    /// The attempts used -- section 11.3's third part.
    #[must_use]
    pub const fn attempts(&self) -> &AttemptUsage {
        &self.attempts
    }

    /// The equivalency decision -- section 11.3's fourth part.
    #[must_use]
    pub const fn equivalency(&self) -> &EquivalencyDecision {
        &self.equivalency
    }

    /// The rule's type.
    #[must_use]
    pub const fn rule_type(&self) -> RuleType {
        self.rule_type
    }

    /// The verdict.
    #[must_use]
    pub const fn status(&self) -> ProofStatus {
        self.status
    }

    /// What was measured, when the rule measures something.
    #[must_use]
    pub const fn measure(&self) -> Option<Measure> {
        self.measure
    }

    /// The section 38 cell this crate names for an `UNKNOWN` leaf.
    #[must_use]
    pub const fn open_gate(&self) -> Option<OpenGate> {
        self.open_gate
    }

    /// The cell `academic-requirement` named, whether or not this crate
    /// restates it.
    ///
    /// `GATE-38-015` and `GATE-38-016` map to no cell here and are still
    /// readable through this accessor, so a leaf never loses the reason its
    /// verdict was `UNKNOWN`.
    #[must_use]
    pub const fn rule_gate(&self) -> Option<RuleGate> {
        self.rule_gate
    }

    /// Whether every one of section 11.3's four parts is present on this leaf.
    ///
    /// It is `true` by construction and there is no input that makes it
    /// `false`; it exists so `proof_leaf_completeness` can walk a whole tree
    /// and say so over every node rather than over the one it happened to
    /// build. What the test cannot do is construct a counter-example, and the
    /// compile-fail cases are where that absence is observed.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        !self.rule.as_str().is_empty()
            && self.source.page() > 0
            && match &self.attempts {
                AttemptUsage::Used(attempts) => !attempts.is_empty(),
                AttemptUsage::NoneUsed(_) => true,
            }
            && match &self.equivalency {
                EquivalencyDecision::Applied(rules) => !rules.is_empty(),
                EquivalencyDecision::NoneApplied => true,
            }
    }
}
