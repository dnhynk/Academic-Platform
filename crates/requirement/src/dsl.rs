//! The compiled body of each of section 11.2's fourteen rule types.
//!
//! # Nothing here holds free text
//!
//! Every operand, threshold, category and scope is a typed value or a validated
//! identifier. There is no `String` field a sentence could be parked in, so a
//! production audit has nothing to interpret: it reads integers, exact
//! decimals, and identifiers, and section 11.2's *자유 텍스트를 LLM이 매번
//! 해석해 졸업 여부를 판단하는 구조는 금지한다* is a shape the type does not
//! have rather than a check something performs.
//!
//! The one place a sentence exists is [`crate::candidate::RuleCandidate`],
//! which is what a model extracted and is not executable. The quoted source
//! text lives there, has no accessor that produces a [`RuleBody`], and never
//! crosses the review gate.
//!
//! # Where absence is a value
//!
//! Four section 38 cells are open, and each is represented as a value rather
//! than a default. [`Applicability::Unknown`], [`RecognitionPolicy::Unknown`]
//! and [`DoubleCountingPolicy::Unknown`] are readings a rule returns
//! `ProofStatus::Unknown` from. None of them is a `Default`; see [`crate::gate`].

use academic_domain::{CourseId, Decimal, ValidInterval};

use crate::error::RequirementError;

/// The characters admitted in a rule or category identifier.
///
/// The same narrow set `academic_domain::engines` admits, and for the same
/// reason: the canonical encodings separate fields with `=`, `:` and newline,
/// so an identifier that could contain one would make a byte comparison
/// meaningless. It is also what keeps a sentence out: a value that cannot hold
/// a space cannot hold a clause.
fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

macro_rules! identifier_newtype {
    ($name:ident, $kind:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);

        impl $name {
            /// Validates and constructs the identifier.
            pub fn new(value: &str) -> Result<Self, RequirementError> {
                if is_identifier(value) {
                    Ok(Self(value.to_owned()))
                } else {
                    Err(RequirementError::InvalidIdentifier {
                        kind: $kind,
                        value: value.to_owned(),
                    })
                }
            }

            /// Returns the identifier text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str(&self.0)
            }
        }
    };
}

identifier_newtype!(
    RuleId,
    "rule id",
    "A rule's identifier inside its set -- section 11.2's `id:`."
);
identifier_newtype!(
    CreditCategory,
    "credit category",
    "Section 11.2's `category:`, such as `ALL_RECOGNIZED` or `CSE_MAJOR`."
);
identifier_newtype!(
    AreaId,
    "area id",
    "One general-education area, such as section 8.1's `WRITING_AND_SPEAKING`."
);
identifier_newtype!(
    GpaScope,
    "gpa scope",
    "Section 11.2's `scope:`, such as `ALL_GPA_ELIGIBLE`."
);
identifier_newtype!(
    ProgramId,
    "program id",
    "One non-credit training programme, such as section 8.1's life-respect education."
);
identifier_newtype!(
    ApprovalAuthority,
    "approval authority",
    "The office an exception approval was issued by."
);

/// A credit quantity in a requirement.
///
/// Distinct from `academic_curriculum::Credits`, which is what one catalogue
/// row prints and is capped at thirty. A requirement threshold is a sum over
/// many rows -- section 8.1's 130 and 63 and 49 -- so it is a different
/// quantity with a different bound, and giving it the same type would have
/// invited one to be passed where the other belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CreditAmount(u16);

impl CreditAmount {
    /// Constructs a credit amount. Nothing in a degree exceeds a thousand.
    pub fn new(value: u16) -> Result<Self, RequirementError> {
        if value > 1000 {
            return Err(RequirementError::MalformedRule {
                rule: "credit amount".to_owned(),
                reason: "a requirement does not exceed 1000 credits",
            });
        }
        Ok(Self(value))
    }

    /// Returns the amount.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// An admission year, which is what section 8.1 scopes every transition by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AdmissionYear(u16);

impl AdmissionYear {
    /// Constructs an admission year.
    pub fn new(value: u16) -> Result<Self, RequirementError> {
        if !(1900..=2200).contains(&value) {
            return Err(RequirementError::MalformedRule {
                rule: "admission year".to_owned(),
                reason: "an admission year is between 1900 and 2200",
            });
        }
        Ok(Self(value))
    }

    /// Returns the year.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Section 11.2's `COURSE_OR_EQUIVALENT` operand.
///
/// The yaml writes the operand as `COURSE_OR_EQUIVALENT: course_discrete_math`,
/// so an operand names a course and admits whatever the set's own
/// `EQUIVALENCY` rules substitute for it. It does **not** admit whatever
/// `academic-curriculum` recorded as an equivalence: that is a catalogue fact
/// at the course level, and a requirement set that silently inherited it would
/// change its own meaning when the catalogue changed. A substitution counts
/// here only when a rule in this set says so.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Operand {
    /// The course the operand names.
    pub course: CourseId,
    /// Whether an `EQUIVALENCY` rule in the same set may satisfy it.
    pub equivalent_admitted: bool,
}

/// One constraint on a `COUNT_WITH_CONSTRAINTS` rule.
///
/// Section 11.2's example carries `atLeastMajorCourses: 1` and
/// `exclusionsByAdmissionYear`, which is section 8.1's *2008학년도 신입생부터
/// 전공 1과목 이상을 포함한 외국어진행강좌 3과목 이상 ... 2012학번부터
/// 대학영어를 제외*.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountConstraint {
    /// At least this many of the counted courses must be major courses.
    AtLeastMajorCourses(u16),
    /// This course stops counting for entrants from this admission year on.
    ExcludedFromAdmissionYear {
        /// The course that stops counting.
        course: CourseId,
        /// The first admission year it does not count for.
        from: AdmissionYear,
    },
}

/// One area's condition inside an `AREA_DISTRIBUTION` rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaRequirement {
    /// The area.
    pub area: AreaId,
    /// The credits required inside it.
    pub credits: CreditAmount,
}

/// How a `CO_REQUISITE` rule relates the two courses' terms.
///
/// `GATE-38-010`-shaped question: `t001`'s `REQ-11-010` row records *exact
/// temporal semantics by source rule* as an open gate candidate, so the timing
/// is a field the official rule fills rather than a constant this crate picks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoRequisiteTiming {
    /// The companion must be attempted in the same term.
    SameTerm,
    /// The companion must be attempted in the same term or earlier.
    SameTermOrEarlier,
}

/// Whether a rule applies to the cohort being audited.
///
/// `Unknown` is `GATE-38-011` (cohort applicability) and `GATE-38-012`
/// (thesis-rule scope) expressed as a value. A rule whose applicability is
/// unknown evaluates to `ProofStatus::Unknown`, never to satisfied and never to
/// not-satisfied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    /// A confirmed official source scopes the rule from this admission year on.
    FromAdmissionYear(AdmissionYear),
    /// A confirmed official source scopes the rule to entrants before this year.
    BeforeAdmissionYear(AdmissionYear),
    /// No confirmed source says who the rule applies to.
    Unknown,
}

/// How much of an outside category may be recognized.
///
/// `Unknown` is `GATE-38-016` (external-credit recognition) as a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognitionPolicy {
    /// A confirmed source caps recognition at this many credits.
    CappedAt(CreditAmount),
    /// No confirmed source states the cap.
    Unknown,
}

/// Whether one attempt may count toward two requirements at once.
///
/// `Unknown` is `GATE-38-015` (multi-major double counting) as a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleCountingPolicy {
    /// A confirmed source admits at most this many of the members.
    AtMost(u16),
    /// No confirmed source says whether the members may both count.
    Unknown,
}

/// The language a `LANGUAGE_OF_INSTRUCTION` rule counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InstructionLanguage {
    /// Taught in Korean.
    Korean,
    /// Taught in a language other than Korean -- section 8.1's 외국어진행강좌.
    Foreign,
}

impl InstructionLanguage {
    /// The identifier a published rule spells.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Korean => "KOREAN",
            Self::Foreign => "FOREIGN",
        }
    }
}

/// How a thesis or research requirement is graded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThesisGrading {
    /// Graded on the letter scale and counted in the grade-point average.
    Graded,
    /// Satisfactory/unsatisfactory, excluded from the average -- section 8.1's
    /// *3학점, S/U*.
    SatisfactoryUnsatisfactory,
}

/// What an `EXCEPTION_APPROVAL` rule requires of an approval before it counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalRequirement {
    /// The office whose approval is admitted.
    pub authority: ApprovalAuthority,
    /// The interval an approval must be valid across to count.
    pub valid_within: ValidInterval,
}

/// The compiled body of one rule.
///
/// One variant per section 11.2 rule type, and
/// [`RuleBody::rule_type`] is a total `match`, so a variant added without a
/// type -- or a type added without a body -- stops the crate compiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleBody {
    /// `CREDIT_MINIMUM`.
    CreditMinimum {
        /// Section 11.2's `category:`.
        category: CreditCategory,
        /// Section 11.2's `threshold:`.
        threshold: CreditAmount,
    },
    /// `ALL_OF`.
    AllOf {
        /// Section 11.2's `operands:`.
        operands: Vec<Operand>,
    },
    /// `AT_LEAST_N_OF`.
    AtLeastNOf {
        /// Section 11.2's `n:`.
        n: u16,
        /// Section 11.2's `operands:`.
        operands: Vec<Operand>,
    },
    /// `COUNT_WITH_CONSTRAINTS`.
    CountWithConstraints {
        /// Section 11.2's `minimum:`.
        minimum: u16,
        /// Section 11.2's `constraints:`.
        constraints: Vec<CountConstraint>,
        /// Which courses the count ranges over.
        counted: Vec<CourseId>,
    },
    /// `GPA_MINIMUM`.
    GpaMinimum {
        /// Section 11.2's `scope:`.
        scope: GpaScope,
        /// Section 11.2's `threshold:`, exact and never a float.
        threshold: Decimal,
    },
    /// `AREA_DISTRIBUTION`.
    AreaDistribution {
        /// One condition per area; a total does not imply them.
        areas: Vec<AreaRequirement>,
    },
    /// `CO_REQUISITE`.
    CoRequisite {
        /// The course the requirement is about.
        subject: CourseId,
        /// The course whose timing is linked to it.
        companion: CourseId,
        /// How the two terms must relate.
        timing: CoRequisiteTiming,
    },
    /// `MUTUALLY_EXCLUSIVE`.
    MutuallyExclusive {
        /// The courses at most some of which may be recognized.
        members: Vec<CourseId>,
        /// How many may count. `Unknown` is `GATE-38-015`.
        policy: DoubleCountingPolicy,
    },
    /// `EQUIVALENCY`.
    Equivalency {
        /// The course the student took.
        presented: CourseId,
        /// The course it may be presented for.
        counts_for: CourseId,
        /// When the substitution holds. It is directional and dated: the
        /// reverse direction is a second rule, never a property of this one.
        effective: ValidInterval,
    },
    /// `MAXIMUM_RECOGNITION`.
    MaximumRecognition {
        /// The category the ceiling applies to.
        category: CreditCategory,
        /// The ceiling. `Unknown` is `GATE-38-016`.
        policy: RecognitionPolicy,
    },
    /// `NON_CREDIT_TRAINING`.
    NonCreditTraining {
        /// The programme that must be completed.
        program: ProgramId,
        /// Who it applies to. `Unknown` is `GATE-38-011`.
        applicability: Applicability,
    },
    /// `LANGUAGE_OF_INSTRUCTION`.
    LanguageOfInstruction {
        /// How many courses must be taught in the language.
        minimum: u16,
        /// Which language.
        language: InstructionLanguage,
        /// Courses that stop counting from an admission year on.
        exclusions: Vec<CountConstraint>,
    },
    /// `THESIS_RESEARCH`.
    ThesisResearch {
        /// The course that discharges it.
        course: CourseId,
        /// Its credits.
        credits: CreditAmount,
        /// How it is graded.
        grading: ThesisGrading,
        /// Who it applies to. `Unknown` is `GATE-38-012`.
        applicability: Applicability,
    },
    /// `EXCEPTION_APPROVAL`.
    ExceptionApproval {
        /// The one rule an admitted approval alters. An approval that names no
        /// rule alters nothing, and one that names this rule alters only it.
        target: RuleId,
        /// What an approval must satisfy to be admitted.
        approval: ApprovalRequirement,
    },
}

impl RuleBody {
    /// The rule type this body is.
    ///
    /// Total on both sides: a `RuleBody` variant with no `RuleType`, or a
    /// `RuleType` no body produces, fails to compile.
    #[must_use]
    pub const fn rule_type(&self) -> crate::rule_type::RuleType {
        use crate::rule_type::RuleType;
        match self {
            Self::CreditMinimum { .. } => RuleType::CreditMinimum,
            Self::AllOf { .. } => RuleType::AllOf,
            Self::AtLeastNOf { .. } => RuleType::AtLeastNOf,
            Self::CountWithConstraints { .. } => RuleType::CountWithConstraints,
            Self::GpaMinimum { .. } => RuleType::GpaMinimum,
            Self::AreaDistribution { .. } => RuleType::AreaDistribution,
            Self::CoRequisite { .. } => RuleType::CoRequisite,
            Self::MutuallyExclusive { .. } => RuleType::MutuallyExclusive,
            Self::Equivalency { .. } => RuleType::Equivalency,
            Self::MaximumRecognition { .. } => RuleType::MaximumRecognition,
            Self::NonCreditTraining { .. } => RuleType::NonCreditTraining,
            Self::LanguageOfInstruction { .. } => RuleType::LanguageOfInstruction,
            Self::ThesisResearch { .. } => RuleType::ThesisResearch,
            Self::ExceptionApproval { .. } => RuleType::ExceptionApproval,
        }
    }

    /// Refuses a body that cannot be evaluated as written.
    ///
    /// Compilation is where a malformed rule is caught, not evaluation: a rule
    /// that reached a proof tree and then reported that it could not be read
    /// would be an audit that failed halfway.
    pub fn compile(&self, rule: &RuleId) -> Result<(), RequirementError> {
        let malformed = |reason: &'static str| RequirementError::MalformedRule {
            rule: rule.as_str().to_owned(),
            reason,
        };
        match self {
            Self::AllOf { operands } => {
                if operands.is_empty() {
                    return Err(malformed("ALL_OF over no operands is satisfied by nothing"));
                }
            }
            Self::AtLeastNOf { n, operands } => {
                if *n == 0 {
                    return Err(malformed("AT_LEAST_N_OF with n = 0 requires nothing"));
                }
                if usize::from(*n) > operands.len() {
                    return Err(malformed("AT_LEAST_N_OF asks for more than it offers"));
                }
            }
            Self::CountWithConstraints {
                minimum, counted, ..
            } => {
                if *minimum == 0 {
                    return Err(malformed(
                        "COUNT_WITH_CONSTRAINTS with minimum 0 requires nothing",
                    ));
                }
                if counted.is_empty() {
                    return Err(malformed("COUNT_WITH_CONSTRAINTS ranges over no course"));
                }
            }
            Self::AreaDistribution { areas } => {
                if areas.is_empty() {
                    return Err(malformed(
                        "AREA_DISTRIBUTION over no area is a total, not a distribution",
                    ));
                }
            }
            Self::CoRequisite {
                subject, companion, ..
            } => {
                if subject == companion {
                    return Err(malformed("a course is not its own co-requisite"));
                }
            }
            Self::MutuallyExclusive { members, .. } => {
                if members.len() < 2 {
                    return Err(malformed("MUTUALLY_EXCLUSIVE needs two members to exclude"));
                }
            }
            Self::Equivalency {
                presented,
                counts_for,
                ..
            } => {
                if presented == counts_for {
                    return Err(malformed("a course is not an equivalency for itself"));
                }
            }
            Self::LanguageOfInstruction { minimum, .. } => {
                if *minimum == 0 {
                    return Err(malformed(
                        "LANGUAGE_OF_INSTRUCTION with minimum 0 requires nothing",
                    ));
                }
            }
            Self::GpaMinimum { threshold, .. } => {
                if threshold.coefficient() < 0 {
                    return Err(malformed("a grade-point threshold is not negative"));
                }
            }
            Self::CreditMinimum { .. }
            | Self::MaximumRecognition { .. }
            | Self::NonCreditTraining { .. }
            | Self::ThesisResearch { .. }
            | Self::ExceptionApproval { .. } => {}
        }
        Ok(())
    }
}

/// One `key=value` token, appended with a leading space.
fn field(rendered: &mut String, key: &str, value: &str) {
    rendered.push(' ');
    rendered.push_str(key);
    rendered.push('=');
    rendered.push_str(value);
}

/// A [`ValidInterval`] as two tokens under `prefix`.
///
/// The absent upper bound is the token `none` rather than an omitted field: a
/// rendering that dropped the key would spell an open-ended interval and a
/// bounded one the same whenever the bound happened to be the last token.
fn interval(rendered: &mut String, prefix: &str, value: ValidInterval) {
    field(
        rendered,
        &format!("{prefix}.from"),
        &value.from().value().to_string(),
    );
    field(
        rendered,
        &format!("{prefix}.to"),
        &value
            .to()
            .map_or_else(|| "none".to_owned(), |end| end.value().to_string()),
    );
}

/// One [`CountConstraint`] under `prefix`, tagged by its arm.
///
/// The arm is a field of its own, so two constraints of different shapes cannot
/// render the same tokens: the tag is read before any payload.
fn count_constraint(rendered: &mut String, prefix: &str, value: &CountConstraint) {
    match value {
        CountConstraint::AtLeastMajorCourses(minimum) => {
            field(
                rendered,
                &format!("{prefix}.kind"),
                "AT_LEAST_MAJOR_COURSES",
            );
            field(rendered, &format!("{prefix}.minimum"), &minimum.to_string());
        }
        CountConstraint::ExcludedFromAdmissionYear { course, from } => {
            field(
                rendered,
                &format!("{prefix}.kind"),
                "EXCLUDED_FROM_ADMISSION_YEAR",
            );
            field(rendered, &format!("{prefix}.course"), &course.to_string());
            field(rendered, &format!("{prefix}.from"), &from.get().to_string());
        }
    }
}

/// One [`Applicability`] as a single token.
fn applicability(value: Applicability) -> String {
    match value {
        Applicability::FromAdmissionYear(year) => format!("FROM_ADMISSION_YEAR:{}", year.get()),
        Applicability::BeforeAdmissionYear(year) => format!("BEFORE_ADMISSION_YEAR:{}", year.get()),
        Applicability::Unknown => "UNKNOWN".to_owned(),
    }
}

/// The operand list of `ALL_OF` and `AT_LEAST_N_OF`.
fn operands_text(rendered: &mut String, operands: &[Operand]) {
    field(rendered, "operands", &operands.len().to_string());
    for (index, operand) in operands.iter().enumerate() {
        let Operand {
            course,
            equivalent_admitted,
        } = operand;
        field(
            rendered,
            &format!("operand.{index}.course"),
            &course.to_string(),
        );
        field(
            rendered,
            &format!("operand.{index}.equivalent_admitted"),
            &equivalent_admitted.to_string(),
        );
    }
}

impl RuleBody {
    /// The canonical rendering of everything this body says.
    ///
    /// # Why every field is here
    ///
    /// [`crate::publish::RuleSet::rule_set_hash`] is what a historical audit
    /// replays against, so it has to separate any two rule sets that can reach
    /// different verdicts. A rendering carrying a rule's identifier and type
    /// but not its **parameters** does not: two sets differing only in a credit
    /// threshold hashed the same, produced a byte-identical
    /// `AuditInputBinding`, and answered 졸업 불가 and 졸업 가능 respectively --
    /// and the stricter set's recorded hash replayed against the laxer set's
    /// bodies and was accepted.
    ///
    /// The match below is therefore total in both directions: every arm is
    /// listed, and inside every arm every field is bound **by name** with no
    /// `..` anywhere, so a variant added without a rendering and a field added
    /// to an existing variant both stop this crate compiling. Binding a field
    /// and then not writing it is the one mistake the compiler cannot see, and
    /// `every_rule_body_field_moves_the_hash` moves each of them in turn.
    ///
    /// # The grammar
    ///
    /// The rule type token, then space-separated `key=value` tokens. A list is
    /// a `key=<count>` token followed by one index-qualified group per element,
    /// so no separator is ambiguous and no two different structures render the
    /// same bytes. Every value is a validated identifier, a UUID, a decimal
    /// integer or a fixed token, and [`is_identifier`] admits neither a space,
    /// an `=` nor a `:` -- which is what `academic_domain::engines` relies on
    /// for the same reason.
    #[must_use]
    pub fn canonical_text(&self) -> String {
        let mut rendered = String::new();
        rendered.push_str(self.rule_type().as_str());
        match self {
            Self::CreditMinimum {
                category,
                threshold,
            } => {
                field(&mut rendered, "category", category.as_str());
                field(&mut rendered, "threshold", &threshold.get().to_string());
            }
            Self::AllOf { operands } => operands_text(&mut rendered, operands),
            Self::AtLeastNOf { n, operands } => {
                field(&mut rendered, "n", &n.to_string());
                operands_text(&mut rendered, operands);
            }
            Self::CountWithConstraints {
                minimum,
                constraints,
                counted,
            } => {
                field(&mut rendered, "minimum", &minimum.to_string());
                field(&mut rendered, "constraints", &constraints.len().to_string());
                for (index, constraint) in constraints.iter().enumerate() {
                    count_constraint(&mut rendered, &format!("constraint.{index}"), constraint);
                }
                field(&mut rendered, "counted", &counted.len().to_string());
                for (index, course) in counted.iter().enumerate() {
                    field(
                        &mut rendered,
                        &format!("counted.{index}"),
                        &course.to_string(),
                    );
                }
            }
            Self::GpaMinimum { scope, threshold } => {
                field(&mut rendered, "scope", scope.as_str());
                field(
                    &mut rendered,
                    "threshold.coefficient",
                    &threshold.coefficient().to_string(),
                );
                field(
                    &mut rendered,
                    "threshold.scale",
                    &threshold.scale().to_string(),
                );
            }
            Self::AreaDistribution { areas } => {
                field(&mut rendered, "areas", &areas.len().to_string());
                for (index, requirement) in areas.iter().enumerate() {
                    let AreaRequirement { area, credits } = requirement;
                    field(&mut rendered, &format!("area.{index}.area"), area.as_str());
                    field(
                        &mut rendered,
                        &format!("area.{index}.credits"),
                        &credits.get().to_string(),
                    );
                }
            }
            Self::CoRequisite {
                subject,
                companion,
                timing,
            } => {
                field(&mut rendered, "subject", &subject.to_string());
                field(&mut rendered, "companion", &companion.to_string());
                field(
                    &mut rendered,
                    "timing",
                    match timing {
                        CoRequisiteTiming::SameTerm => "SAME_TERM",
                        CoRequisiteTiming::SameTermOrEarlier => "SAME_TERM_OR_EARLIER",
                    },
                );
            }
            Self::MutuallyExclusive { members, policy } => {
                field(&mut rendered, "members", &members.len().to_string());
                for (index, course) in members.iter().enumerate() {
                    field(
                        &mut rendered,
                        &format!("member.{index}"),
                        &course.to_string(),
                    );
                }
                field(
                    &mut rendered,
                    "policy",
                    &match policy {
                        DoubleCountingPolicy::AtMost(most) => format!("AT_MOST:{most}"),
                        DoubleCountingPolicy::Unknown => "UNKNOWN".to_owned(),
                    },
                );
            }
            Self::Equivalency {
                presented,
                counts_for,
                effective,
            } => {
                field(&mut rendered, "presented", &presented.to_string());
                field(&mut rendered, "counts_for", &counts_for.to_string());
                interval(&mut rendered, "effective", *effective);
            }
            Self::MaximumRecognition { category, policy } => {
                field(&mut rendered, "category", category.as_str());
                field(
                    &mut rendered,
                    "policy",
                    &match policy {
                        RecognitionPolicy::CappedAt(cap) => format!("CAPPED_AT:{}", cap.get()),
                        RecognitionPolicy::Unknown => "UNKNOWN".to_owned(),
                    },
                );
            }
            Self::NonCreditTraining {
                program,
                applicability: scope,
            } => {
                field(&mut rendered, "program", program.as_str());
                field(&mut rendered, "applicability", &applicability(*scope));
            }
            Self::LanguageOfInstruction {
                minimum,
                language,
                exclusions,
            } => {
                field(&mut rendered, "minimum", &minimum.to_string());
                field(&mut rendered, "language", language.as_str());
                field(&mut rendered, "exclusions", &exclusions.len().to_string());
                for (index, exclusion) in exclusions.iter().enumerate() {
                    count_constraint(&mut rendered, &format!("exclusion.{index}"), exclusion);
                }
            }
            Self::ThesisResearch {
                course,
                credits,
                grading,
                applicability: scope,
            } => {
                field(&mut rendered, "course", &course.to_string());
                field(&mut rendered, "credits", &credits.get().to_string());
                field(
                    &mut rendered,
                    "grading",
                    match grading {
                        ThesisGrading::Graded => "GRADED",
                        ThesisGrading::SatisfactoryUnsatisfactory => "SATISFACTORY_UNSATISFACTORY",
                    },
                );
                field(&mut rendered, "applicability", &applicability(*scope));
            }
            Self::ExceptionApproval { target, approval } => {
                field(&mut rendered, "target", target.as_str());
                let ApprovalRequirement {
                    authority,
                    valid_within,
                } = approval;
                field(&mut rendered, "approval.authority", authority.as_str());
                interval(&mut rendered, "approval.valid_within", *valid_within);
            }
        }
        rendered
    }
}
