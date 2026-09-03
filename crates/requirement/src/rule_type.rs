//! Section 11.2's rule types, and where each identifier comes from.
//!
//! # The set is fourteen, and it is enumerated rather than counted
//!
//! `t068` section 5's `P2-U2` entry says *all thirteen section 11.2 rule types*
//! and then lists fourteen in its own parenthesis. Its own acceptance evidence
//! names fourteen `dsl_*` tests. The word is wrong and this module does not
//! follow it; nothing here asserts a count.
//!
//! What the specification itself says is in two places, and neither is a list
//! of fourteen on its own:
//!
//! * **The yaml block** (lines 597-629) writes five `type:` values, and they
//!   are the only rule-type identifiers anywhere in the document:
//!   `CREDIT_MINIMUM`, `ALL_OF`, `AT_LEAST_N_OF`, `COUNT_WITH_CONSTRAINTS`,
//!   `GPA_MINIMUM`. It is an example, so it is not a complete list.
//! * **The prose sentence** (line 632) names twelve categories and opens with
//!   *rule type에는 ... 를 포함한다* -- "includes", so it is not closed either.
//!
//! The two overlap. The prose's *course set* is one category and the yaml
//! spells three distinct types under it, because a set every operand must
//! satisfy, a choice of `n`, and a count under constraints are three different
//! shapes with three different operand lists. Twelve prose categories with
//! *course set* opened into the yaml's three is fourteen.
//!
//! The independent reading agrees. `t001`'s requirement matrix, derived from
//! the specification line by line, gives each rule type its own row --
//! `REQ-11-004` through `REQ-11-017`, fourteen consecutive requirements, each
//! naming one of the fourteen `dsl_*` tests. That derivation was made without
//! reference to `t068`'s count.
//!
//! [`SPEC_YAML_TYPES`] and [`SPEC_PROSE_CATEGORIES`] hold both readings, and
//! `the_rule_types_are_the_specifications_own` parses each back out of
//! `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares in both
//! directions. A type this module declares and the specification does not write
//! fails as an extra; one the specification writes and this module drops fails
//! as a missing key. Neither comparison has a length in it.
//!
//! # Where each spelling comes from
//!
//! Five identifiers are the specification's own yaml. The other nine are
//! written in prose and the document names no identifier for them, so the
//! `SCREAMING_SNAKE_CASE` spelling is this crate's, derived from the prose name
//! by one mechanical rule: upper-case, with each space, hyphen or slash
//! becoming an underscore. [`RuleType::spelling_source`] records which of the
//! two each identifier is, and the scan requires a `SpecYaml` spelling to be a
//! byte match against the document and a `SpecProse` spelling to be exactly
//! what that rule applied to the prose name produces.
//!
//! `academic-proposal` met the same shape and resolved it the same way: section
//! 27.4 states four risk tiers in prose and names no identifier, so the plan
//! supplied the spelling and the section stayed the authority for the meaning.

use crate::error::RequirementError;

/// Which document supplied a rule type's identifier spelling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpellingSource {
    /// The specification's own section 11.2 yaml writes this identifier.
    SpecYaml,
    /// Section 11.2's prose names the category and no identifier; the
    /// `SCREAMING_SNAKE_CASE` spelling is derived from that name.
    SpecProse,
}

/// One of section 11.2's rule types.
///
/// `#[non_exhaustive]` is deliberately absent. A fifteenth rule type is a
/// change to what the specification says, and every `match` in this crate is
/// total on purpose: adding a variant must stop the crate compiling until each
/// of them names its evaluation, its identifier, and its provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuleType {
    /// `CREDIT_MINIMUM` (`REQ-11-004`): a scoped credit floor.
    CreditMinimum,
    /// `ALL_OF` (`REQ-11-005`): every operand must be satisfied.
    AllOf,
    /// `AT_LEAST_N_OF` (`REQ-11-006`): `n` of the operands must be.
    AtLeastNOf,
    /// `COUNT_WITH_CONSTRAINTS` (`REQ-11-007`): a minimum count with subtype
    /// minimums and admission-year exclusions.
    CountWithConstraints,
    /// `GPA_MINIMUM` (`REQ-11-008`): a scoped grade-point floor.
    GpaMinimum,
    /// `AREA_DISTRIBUTION` (`REQ-11-009`): per-area credit conditions that a
    /// total does not imply.
    AreaDistribution,
    /// `CO_REQUISITE` (`REQ-11-010`): two courses whose timing is linked.
    CoRequisite,
    /// `MUTUALLY_EXCLUSIVE` (`REQ-11-011`): a set of which at most a stated
    /// number may be recognized.
    MutuallyExclusive,
    /// `EQUIVALENCY` (`REQ-11-012`): a directional, effective-dated
    /// substitution admitted while evaluating a requirement.
    Equivalency,
    /// `MAXIMUM_RECOGNITION` (`REQ-11-013`): a ceiling on how much of a
    /// category may count.
    MaximumRecognition,
    /// `NON_CREDIT_TRAINING` (`REQ-11-014`): a completion that carries no
    /// credit and still gates graduation.
    NonCreditTraining,
    /// `LANGUAGE_OF_INSTRUCTION` (`REQ-11-015`): a count over courses taught in
    /// a stated language.
    LanguageOfInstruction,
    /// `THESIS_RESEARCH` (`REQ-11-016`): a thesis or research completion.
    ThesisResearch,
    /// `EXCEPTION_APPROVAL` (`REQ-11-017`): an approval that alters exactly one
    /// named rule and nothing else.
    ExceptionApproval,
}

impl RuleType {
    /// Every rule type, in the order the specification introduces it: the five
    /// the yaml writes, in yaml order, then the prose categories the yaml does
    /// not, in the order the prose sentence names them.
    ///
    /// This is a Rust array and so has a length. Nothing in this crate compares
    /// against that length: `the_rule_types_are_the_specifications_own` walks
    /// this list against the two specification readings in both directions, so
    /// dropping an entry fails against the document rather than against a
    /// number.
    pub const ALL: [Self; 14] = [
        Self::CreditMinimum,
        Self::AllOf,
        Self::AtLeastNOf,
        Self::CountWithConstraints,
        Self::GpaMinimum,
        Self::AreaDistribution,
        Self::CoRequisite,
        Self::MutuallyExclusive,
        Self::Equivalency,
        Self::MaximumRecognition,
        Self::NonCreditTraining,
        Self::LanguageOfInstruction,
        Self::ThesisResearch,
        Self::ExceptionApproval,
    ];

    /// The rule type's identifier, as a published rule spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CreditMinimum => "CREDIT_MINIMUM",
            Self::AllOf => "ALL_OF",
            Self::AtLeastNOf => "AT_LEAST_N_OF",
            Self::CountWithConstraints => "COUNT_WITH_CONSTRAINTS",
            Self::GpaMinimum => "GPA_MINIMUM",
            Self::AreaDistribution => "AREA_DISTRIBUTION",
            Self::CoRequisite => "CO_REQUISITE",
            Self::MutuallyExclusive => "MUTUALLY_EXCLUSIVE",
            Self::Equivalency => "EQUIVALENCY",
            Self::MaximumRecognition => "MAXIMUM_RECOGNITION",
            Self::NonCreditTraining => "NON_CREDIT_TRAINING",
            Self::LanguageOfInstruction => "LANGUAGE_OF_INSTRUCTION",
            Self::ThesisResearch => "THESIS_RESEARCH",
            Self::ExceptionApproval => "EXCEPTION_APPROVAL",
        }
    }

    /// Which document supplied [`RuleType::as_str`]'s spelling.
    #[must_use]
    pub const fn spelling_source(self) -> SpellingSource {
        match self {
            Self::CreditMinimum
            | Self::AllOf
            | Self::AtLeastNOf
            | Self::CountWithConstraints
            | Self::GpaMinimum => SpellingSource::SpecYaml,
            Self::AreaDistribution
            | Self::CoRequisite
            | Self::MutuallyExclusive
            | Self::Equivalency
            | Self::MaximumRecognition
            | Self::NonCreditTraining
            | Self::LanguageOfInstruction
            | Self::ThesisResearch
            | Self::ExceptionApproval => SpellingSource::SpecProse,
        }
    }

    /// The `t001` requirement this rule type closes.
    ///
    /// One requirement per rule type is what makes fourteen an enumeration
    /// rather than a count: `REQ-11-004`...`REQ-11-017` is a range the matrix
    /// wrote, and each row names the `dsl_*` test below.
    #[must_use]
    pub const fn requirement(self) -> &'static str {
        match self {
            Self::CreditMinimum => "REQ-11-004",
            Self::AllOf => "REQ-11-005",
            Self::AtLeastNOf => "REQ-11-006",
            Self::CountWithConstraints => "REQ-11-007",
            Self::GpaMinimum => "REQ-11-008",
            Self::AreaDistribution => "REQ-11-009",
            Self::CoRequisite => "REQ-11-010",
            Self::MutuallyExclusive => "REQ-11-011",
            Self::Equivalency => "REQ-11-012",
            Self::MaximumRecognition => "REQ-11-013",
            Self::NonCreditTraining => "REQ-11-014",
            Self::LanguageOfInstruction => "REQ-11-015",
            Self::ThesisResearch => "REQ-11-016",
            Self::ExceptionApproval => "REQ-11-017",
        }
    }

    /// The acceptance test `t068` names for this rule type.
    #[must_use]
    pub const fn acceptance_test(self) -> &'static str {
        match self {
            Self::CreditMinimum => "dsl_credit_minimum",
            Self::AllOf => "dsl_required_course_set",
            Self::AtLeastNOf => "dsl_at_least_n",
            Self::CountWithConstraints => "dsl_count_constraints",
            Self::GpaMinimum => "dsl_gpa_minimum",
            Self::AreaDistribution => "dsl_area_distribution",
            Self::CoRequisite => "dsl_corequisite",
            Self::MutuallyExclusive => "dsl_mutually_exclusive",
            Self::Equivalency => "dsl_equivalency",
            Self::MaximumRecognition => "dsl_maximum_recognition",
            Self::NonCreditTraining => "dsl_noncredit_training",
            Self::LanguageOfInstruction => "dsl_language_instruction",
            Self::ThesisResearch => "dsl_thesis_research",
            Self::ExceptionApproval => "dsl_exception_approval",
        }
    }

    /// Parses an identifier, returning `None` for anything not a rule type.
    #[must_use]
    pub fn parse(identifier: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|candidate| candidate.as_str() == identifier)
    }

    /// Parses an identifier or reports it as unknown.
    pub fn require(identifier: &str) -> Result<Self, RequirementError> {
        Self::parse(identifier).ok_or_else(|| RequirementError::UnknownRuleType {
            identifier: identifier.to_owned(),
        })
    }
}

/// The five `type:` values section 11.2's yaml block writes, in yaml order.
///
/// `the_rule_types_are_the_specifications_own` reads the `type:` lines out of
/// the fenced block between the `### 11.2` heading and the prose sentence and
/// requires this list, deduplicated in first-appearance order, to be exactly
/// what it found.
pub const SPEC_YAML_TYPES: [(&str, RuleType); 5] = [
    ("CREDIT_MINIMUM", RuleType::CreditMinimum),
    ("ALL_OF", RuleType::AllOf),
    ("AT_LEAST_N_OF", RuleType::AtLeastNOf),
    ("COUNT_WITH_CONSTRAINTS", RuleType::CountWithConstraints),
    ("GPA_MINIMUM", RuleType::GpaMinimum),
];

/// The twelve categories section 11.2's prose sentence names, in its order,
/// each with the rule types it opens into.
///
/// Eleven map to one type. *course set* maps to three, because the yaml above
/// spells three distinct `type:` values under that one prose word and each has
/// its own operand shape: `ALL_OF` takes a list every member of which must
/// hold, `AT_LEAST_N_OF` takes a list and an `n`, and
/// `COUNT_WITH_CONSTRAINTS` takes a minimum and a constraint list. Collapsing
/// them would have made the choice requirement and the constrained count
/// unrepresentable, and `REQ-11-005`, `REQ-11-006` and `REQ-11-007` are three
/// requirements.
///
/// The scan requires each key to appear in the prose sentence in this order,
/// requires the sentence to contain no comma-separated name this list omits,
/// and requires the union of the values to be exactly [`RuleType::ALL`].
pub const SPEC_PROSE_CATEGORIES: [(&str, &[RuleType]); 12] = [
    ("credit minimum", &[RuleType::CreditMinimum]),
    (
        "course set",
        &[
            RuleType::AllOf,
            RuleType::AtLeastNOf,
            RuleType::CountWithConstraints,
        ],
    ),
    ("area distribution", &[RuleType::AreaDistribution]),
    ("co-requisite", &[RuleType::CoRequisite]),
    ("mutually exclusive", &[RuleType::MutuallyExclusive]),
    ("equivalency", &[RuleType::Equivalency]),
    ("maximum recognition", &[RuleType::MaximumRecognition]),
    ("GPA", &[RuleType::GpaMinimum]),
    ("non-credit training", &[RuleType::NonCreditTraining]),
    (
        "language-of-instruction",
        &[RuleType::LanguageOfInstruction],
    ),
    ("thesis/research", &[RuleType::ThesisResearch]),
    ("exception approval", &[RuleType::ExceptionApproval]),
];
