//! Requirement classification as a versioned rule-engine output.
//!
//! "`requirementCategory`는 시도에 사용자가 적어 넣은 영구 label이 아니다. 같은
//! 과목이 적용 RequirementSet에 따라 전필·전선·일선으로 다르게 계산될 수 있으므로
//! rule engine이 생성한 versioned classification이다."
//!
//! Two independent things enforce that, and neither is a comment:
//!
//! - **Construction.** [`RequirementClassification`]'s fields are private to
//!   this module and it has no public constructor. The only value of the type
//!   that can exist anywhere is one [`ClassificationRuleSet::classify`]
//!   returned, and that function names the rule set version and the rule that
//!   produced each one. A caller outside this module cannot write the struct
//!   literal, and `classification_by_ruleset` proves the same at run time by
//!   showing every classification carries a rule id the published set holds.
//!
//! - **Assertion.** A classification travels as an ADR-003 claim under
//!   `AuthorityClass::DeterministicEngine`, and `Claim::validate_for_actor`
//!   permits `Actor::User` exactly one authority class — `UserExplicit`. So a
//!   user-authored claim carrying a classification is refused by the matrix
//!   this repository already has, not by a rule this module adds.
//!   [`classification_claim`] builds the claim and the acceptance row executes
//!   the refusal.

use std::collections::BTreeMap;

use academic_domain::{
    Actor, AuthorityClass, Claim, ClaimId, ClaimObject, EntityId, EpistemicStatus, EvidenceId,
    PredicateId, ScopeId, ValidInterval,
};

use crate::{RecordError, attempt::CourseAttempt};

/// Predicate every classification claim is asserted under.
pub const CLASSIFICATION_PREDICATE: &str = "academic.attempt.requirement.classification";

/// Actor name recorded on a classification claim.
pub const CLASSIFIER_NAME: &str = "academic-record-classifier";

/// The categories a rule set may assign.
///
/// 전필 / 전선 / 교양필수 / 교양선택 / 일선, in that order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RequirementCategory {
    /// 전공필수.
    MajorRequired,
    /// 전공선택.
    MajorElective,
    /// 교양필수.
    GeneralRequired,
    /// 교양선택.
    GeneralElective,
    /// 일반선택.
    FreeElective,
}

impl RequirementCategory {
    /// Every category.
    pub const ALL: [Self; 5] = [
        Self::MajorRequired,
        Self::MajorElective,
        Self::GeneralRequired,
        Self::GeneralElective,
        Self::FreeElective,
    ];

    /// Returns the contract spelling, which is also the frozen-input token.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MajorRequired => "MAJOR_REQUIRED",
            Self::MajorElective => "MAJOR_ELECTIVE",
            Self::GeneralRequired => "GENERAL_REQUIRED",
            Self::GeneralElective => "GENERAL_ELECTIVE",
            Self::FreeElective => "FREE_ELECTIVE",
        }
    }

    /// Resolves a category from its spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|category| category.as_str() == text)
    }

    /// Whether the category counts toward a major grade-point average.
    ///
    /// 전필 and 전선 do; the three non-major categories do not. This is what
    /// `major_gpa_classification` reads, and it reads it from the *rule
    /// engine's* category rather than from anything a user typed.
    #[must_use]
    pub const fn is_major(self) -> bool {
        matches!(self, Self::MajorRequired | Self::MajorElective)
    }
}

/// A programme a classification is scoped to.
///
/// Multi-major is why this exists. One attempt can be 전공 under a primary
/// major and 일선 under an additional one, so a classification is meaningless
/// without the programme it was computed for, and `multi_major_gpa` reads a
/// separate average per programme.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProgramId(String);

impl ProgramId {
    /// Builds a programme identity, refusing a non-identifier spelling.
    ///
    /// The value reaches a deterministic engine's frozen inputs as a `ref:`
    /// token, whose grammar is ASCII alphanumerics, `.`, `_`, and `-`.
    /// Refusing here rather than at the engine boundary keeps one spelling
    /// rule instead of two.
    pub fn new(value: impl Into<String>) -> Result<Self, RecordError> {
        let value = value.into();
        if !crate::check_identifier(&value) {
            return Err(RecordError::MalformedProgramId(value));
        }
        Ok(Self(value))
    }

    /// Returns the programme identity.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One published classification rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationRule {
    /// Rule identity, carried into every classification and every proof node.
    pub rule_id: String,
    /// The programme the rule speaks for.
    pub program: ProgramId,
    /// The course code the rule matches.
    pub course_code: String,
    /// The category the rule assigns.
    pub category: RequirementCategory,
}

/// A published, versioned set of classification rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassificationRuleSet {
    id: String,
    rules: Vec<ClassificationRule>,
}

impl ClassificationRuleSet {
    /// Publishes a rule set, refusing two rules for one programme and course.
    ///
    /// Two rules on one pair would make the category depend on rule order.
    /// A course that is genuinely two things is two rules under two
    /// *programmes*, which is the multi-major case and is admitted.
    pub fn publish(
        id: impl Into<String>,
        rules: Vec<ClassificationRule>,
    ) -> Result<Self, RecordError> {
        let mut seen = BTreeMap::new();
        for rule in &rules {
            let key = (rule.program.clone(), rule.course_code.clone());
            if seen.insert(key, rule.rule_id.clone()).is_some() {
                return Err(RecordError::DuplicateClassificationRule {
                    program: rule.program.as_str().to_owned(),
                    course_code: rule.course_code.clone(),
                });
            }
        }
        Ok(Self {
            id: id.into(),
            rules,
        })
    }

    /// Returns the rule set's version identity.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns every published rule.
    #[must_use]
    pub fn rules(&self) -> &[ClassificationRule] {
        &self.rules
    }

    /// Classifies one attempt under every programme this set speaks for.
    ///
    /// The **only** way a [`RequirementClassification`] comes into existence.
    /// An attempt no rule matches yields no classification for that programme;
    /// it is not defaulted to 일선, because "the rule set does not mention this
    /// course" and "the rule set says this course is a free elective" are
    /// different facts and only the second is a published one.
    #[must_use]
    pub fn classify(&self, attempt: &CourseAttempt) -> Vec<RequirementClassification> {
        let mut classified: Vec<RequirementClassification> = self
            .rules
            .iter()
            .filter(|rule| rule.course_code == attempt.course_code())
            .map(|rule| RequirementClassification {
                program: rule.program.clone(),
                category: rule.category,
                rule_id: rule.rule_id.clone(),
                ruleset_id: self.id.clone(),
            })
            .collect();
        classified.sort_by(|left, right| left.program.cmp(&right.program));
        classified
    }

    /// Returns every programme this rule set speaks for, in canonical order.
    #[must_use]
    pub fn programs(&self) -> Vec<ProgramId> {
        let mut programs: Vec<ProgramId> =
            self.rules.iter().map(|rule| rule.program.clone()).collect();
        programs.sort();
        programs.dedup();
        programs
    }
}

/// One programme's classification of one attempt.
///
/// Every field is private to this module and the type has no public
/// constructor, so the only values that exist are the ones
/// [`ClassificationRuleSet::classify`] produced. There is no `From<&str>`, no
/// `Default`, no builder, and no setter: a user cannot write one and cannot
/// edit one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementClassification {
    program: ProgramId,
    category: RequirementCategory,
    rule_id: String,
    ruleset_id: String,
}

impl RequirementClassification {
    /// Returns the programme this classification speaks for.
    #[must_use]
    pub const fn program(&self) -> &ProgramId {
        &self.program
    }

    /// Returns the assigned category.
    #[must_use]
    pub const fn category(&self) -> RequirementCategory {
        self.category
    }

    /// Returns the rule that assigned it.
    #[must_use]
    pub fn rule_id(&self) -> &str {
        &self.rule_id
    }

    /// Returns the rule set version the rule was published in.
    #[must_use]
    pub fn ruleset_id(&self) -> &str {
        &self.ruleset_id
    }
}

/// Builds the ADR-003 claim one classification travels as.
///
/// The authority class is `DeterministicEngine` and the actor is the
/// classifier. `Claim::validate_for_actor` permits `Actor::User` only
/// `AuthorityClass::UserExplicit`, so handing this claim a user actor is
/// refused by the existing matrix. `classification_by_ruleset` executes that
/// refusal rather than describing it.
pub fn classification_claim(
    classification: &RequirementClassification,
    claim_id: ClaimId,
    subject_entity_id: EntityId,
    scope_id: ScopeId,
    valid_time: ValidInterval,
    evidence_ids: Vec<EvidenceId>,
) -> Result<(Claim, Actor), RecordError> {
    let claim = Claim {
        id: claim_id,
        subject_entity_id,
        predicate_id: PredicateId::parse(CLASSIFICATION_PREDICATE)?,
        object: ClaimObject::Text(format!(
            "program={};category={};rule={};ruleset={}",
            classification.program().as_str(),
            classification.category().as_str(),
            classification.rule_id(),
            classification.ruleset_id(),
        )),
        scope_id,
        authority_class: AuthorityClass::DeterministicEngine,
        epistemic_status: EpistemicStatus::DeterministicDerived,
        confidence: None,
        prediction_metadata: None,
        valid_time,
        evidence_ids,
    };
    let actor = Actor::DeterministicEngine {
        name: CLASSIFIER_NAME.to_owned(),
        version: crate::ENGINE_VERSION_TEXT.to_owned(),
    };
    claim.validate_for_actor(&actor)?;
    Ok((claim, actor))
}
