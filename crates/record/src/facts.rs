//! The frozen fact set both the product views and the deterministic engines read.
//!
//! A deterministic engine is a pure function of `(frozen_inputs, rule_set_hash,
//! engine_version)`. That leaves two ways to build a GPA engine, and only one
//! of them is honest: encode the attempt set into frozen inputs and compute
//! from *those*, or keep a second in-memory path that the golden fixtures never
//! exercise. This module is the first. [`AttemptFacts`] is what a
//! `CourseAttempt` reduces to, [`encode`] renders a set of them as the
//! canonical `key=value` input text, [`decode`] reads it back, and
//! `crate::views` computes over the fact set either way — so a golden fixture
//! and a product call run the same arithmetic over the same values.
//!
//! The identity carried is the attempt's real `EntityId`. A UUID is
//! identifier-shaped under the engine grammar (ASCII alphanumerics and `-`, 36
//! bytes), so the proof tree names the attempt a reader can look up rather than
//! an index into an encoding.

use std::collections::BTreeMap;

use academic_domain::{
    AttemptId, Decimal,
    engines::{FrozenInputs, InputKey, InputValue},
};

use crate::{
    RecordError,
    attempt::{AttemptStatus, CourseAttempt, RepeatStatus},
    classify::{ClassificationRuleSet, ProgramId, RequirementCategory},
    grade::GradeSymbol,
    policy::{AttemptOrigin, RecognitionDecision},
    term::TermKey,
};

/// Which view an evaluation is for.
///
/// The view is a frozen input rather than an argument, because two views over
/// one attempt set are two evaluations with two proof trees, and an engine
/// whose output depended on something outside its frozen inputs would not be
/// the pure function the harness contract fixes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GpaScope {
    /// Every attempt in the set.
    Cumulative,
    /// One term.
    Term(TermKey),
    /// One programme's 전공 attempts.
    Major(ProgramId),
}

impl GpaScope {
    /// Returns the scope's token.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Cumulative => "CUMULATIVE",
            Self::Term(_) => "TERM",
            Self::Major(_) => "MAJOR",
        }
    }
}

/// One attempt, reduced to what the two engines read.
///
/// Deliberately not a `CourseAttempt`: evidence identifiers, the grading-scheme
/// label the row carried, and the superseded-attempt link are all part of the
/// record and none of them changes an average. Encoding them would put values
/// into a frozen-input digest that no rule reads, so a change to one would
/// invalidate every committed fixture for no reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttemptFacts {
    /// The attempt's identity.
    pub id: AttemptId,
    /// The official course code, identifier-shaped.
    pub course_code: String,
    /// The term the attempt was taken in.
    pub term: TermKey,
    /// The attempt status.
    pub status: AttemptStatus,
    /// Where the credits were earned.
    pub origin: AttemptOrigin,
    /// The recorded grade, if any.
    pub grade: Option<GradeSymbol>,
    /// The credits the attempt was taken for.
    pub credits_attempted: Decimal,
    /// The credits the attempt earned.
    pub credits_earned: Decimal,
    /// The repeat status.
    pub repeat_status: RepeatStatus,
    /// The recognition decision for external credits.
    pub recognition: RecognitionDecision,
    /// The rule engine's category per programme.
    pub categories: BTreeMap<ProgramId, RequirementCategory>,
}

impl AttemptFacts {
    /// Reduces one attempt, classifying it under `classification`.
    #[must_use]
    pub fn from_attempt(attempt: &CourseAttempt, classification: &ClassificationRuleSet) -> Self {
        let categories = classification
            .classify(attempt)
            .into_iter()
            .map(|entry| (entry.program().clone(), entry.category()))
            .collect();
        Self {
            id: attempt.id(),
            course_code: attempt.course_code().to_owned(),
            term: attempt.term(),
            status: attempt.status(),
            origin: attempt.origin(),
            grade: attempt.grade(),
            credits_attempted: attempt.credits_attempted(),
            credits_earned: attempt.credits_earned(),
            repeat_status: attempt.repeat_status(),
            recognition: attempt.recognition(),
            categories,
        }
    }

    /// Whether this attempt is 전공 for `program` under the rule engine.
    #[must_use]
    pub fn is_major_for(&self, program: &ProgramId) -> bool {
        self.categories
            .get(program)
            .is_some_and(|category| category.is_major())
    }
}

/// Renders a fact set and a scope as canonical frozen-input text.
///
/// Attempts are indexed by their position in term order, zero-padded to three
/// digits so the keys sort the way the attempts do. The count is encoded so a
/// truncated input is a refusal rather than a shorter attempt set.
pub fn encode(facts: &[AttemptFacts], scope: &GpaScope) -> Result<FrozenInputs, RecordError> {
    let mut ordered: Vec<&AttemptFacts> = facts.iter().collect();
    ordered.sort_by(|left, right| {
        left.term
            .cmp(&right.term)
            .then_with(|| left.course_code.cmp(&right.course_code))
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut entries: Vec<(InputKey, InputValue)> = Vec::new();
    let mut push = |key: String, value: InputValue| -> Result<(), RecordError> {
        entries.push((InputKey::new(&key)?, value));
        Ok(())
    };

    push(
        "attempt.count".to_owned(),
        InputValue::Integer(
            i64::try_from(ordered.len()).map_err(|_| RecordError::TooManyAttempts)?,
        ),
    )?;

    for (index, facts) in ordered.iter().enumerate() {
        let prefix = format!("attempt.{index:03}");
        push(
            format!("{prefix}.course"),
            InputValue::Reference(facts.course_code.clone()),
        )?;
        push(
            format!("{prefix}.credits.attempted"),
            InputValue::Decimal(facts.credits_attempted),
        )?;
        push(
            format!("{prefix}.credits.earned"),
            InputValue::Decimal(facts.credits_earned),
        )?;
        push(
            format!("{prefix}.grade"),
            facts.grade.map_or(InputValue::Unknown, |grade| {
                InputValue::Reference(grade.as_token().to_owned())
            }),
        )?;
        push(
            format!("{prefix}.id"),
            InputValue::Reference(facts.id.to_string()),
        )?;
        push(
            format!("{prefix}.origin"),
            InputValue::Reference(facts.origin.as_str().to_owned()),
        )?;
        for (program, category) in &facts.categories {
            push(
                format!("{prefix}.program.{}", program.as_str()),
                InputValue::Reference(category.as_str().to_owned()),
            )?;
        }
        push(
            format!("{prefix}.recognition"),
            InputValue::Reference(facts.recognition.as_str().to_owned()),
        )?;
        push(
            format!("{prefix}.repeat"),
            InputValue::Reference(facts.repeat_status.as_str().to_owned()),
        )?;
        push(
            format!("{prefix}.status"),
            InputValue::Reference(facts.status.as_str().to_owned()),
        )?;
        push(
            format!("{prefix}.term"),
            InputValue::Reference(facts.term.canonical_text()),
        )?;
    }

    push(
        "scope".to_owned(),
        InputValue::Reference(scope.tag().to_owned()),
    )?;
    match scope {
        GpaScope::Cumulative => {}
        GpaScope::Term(term) => push(
            "scope.term".to_owned(),
            InputValue::Reference(term.canonical_text()),
        )?,
        GpaScope::Major(program) => push(
            "scope.program".to_owned(),
            InputValue::Reference(program.as_str().to_owned()),
        )?,
    }

    entries.sort_by(|left, right| left.0.as_str().cmp(right.0.as_str()));
    Ok(FrozenInputs::new(entries)?)
}

/// Reads a fact set and a scope back out of frozen inputs.
///
/// Every malformed shape is a typed error: a missing key, a value of the wrong
/// kind, a count that disagrees with the keys present, an unknown token. The
/// decoder never substitutes a default, because a default here would be an
/// attempt fact nobody recorded.
pub fn decode(inputs: &FrozenInputs) -> Result<(Vec<AttemptFacts>, GpaScope), RecordError> {
    let count = match integer(inputs, "attempt.count")? {
        Some(value) => usize::try_from(value).map_err(|_| RecordError::TooManyAttempts)?,
        None => return Err(RecordError::MissingEngineInput("attempt.count")),
    };

    let mut facts = Vec::with_capacity(count);
    for index in 0..count {
        let prefix = format!("attempt.{index:03}");
        let id: AttemptId = required_reference(inputs, &format!("{prefix}.id"))?
            .parse()
            .map_err(|_| RecordError::MalformedEngineInput("attempt id"))?;
        let course_code = required_reference(inputs, &format!("{prefix}.course"))?;
        let term = TermKey::parse(&required_reference(inputs, &format!("{prefix}.term"))?)?;
        let status =
            AttemptStatus::parse(&required_reference(inputs, &format!("{prefix}.status"))?)
                .ok_or(RecordError::MalformedEngineInput("attempt status"))?;
        let origin =
            AttemptOrigin::parse(&required_reference(inputs, &format!("{prefix}.origin"))?)
                .ok_or(RecordError::MalformedEngineInput("attempt origin"))?;
        let grade = match inputs.get(&InputKey::new(&format!("{prefix}.grade"))?) {
            Some(InputValue::Unknown) => None,
            Some(InputValue::Reference(token)) => Some(
                GradeSymbol::parse_token(token)
                    .ok_or(RecordError::MalformedEngineInput("grade symbol"))?,
            ),
            _ => return Err(RecordError::MissingEngineInput("attempt grade")),
        };
        let credits_attempted = required_decimal(inputs, &format!("{prefix}.credits.attempted"))?;
        let credits_earned = required_decimal(inputs, &format!("{prefix}.credits.earned"))?;
        let repeat_status =
            RepeatStatus::parse(&required_reference(inputs, &format!("{prefix}.repeat"))?)
                .ok_or(RecordError::MalformedEngineInput("repeat status"))?;
        let recognition = RecognitionDecision::parse(&required_reference(
            inputs,
            &format!("{prefix}.recognition"),
        )?)
        .ok_or(RecordError::MalformedEngineInput("recognition decision"))?;

        let program_prefix = format!("{prefix}.program.");
        let mut categories = BTreeMap::new();
        for key in inputs.keys() {
            let Some(program) = key.as_str().strip_prefix(&program_prefix) else {
                continue;
            };
            let Some(InputValue::Reference(category)) = inputs.get(key) else {
                return Err(RecordError::MalformedEngineInput("programme category"));
            };
            categories.insert(
                ProgramId::new(program)?,
                RequirementCategory::parse(category)
                    .ok_or(RecordError::MalformedEngineInput("requirement category"))?,
            );
        }

        facts.push(AttemptFacts {
            id,
            course_code,
            term,
            status,
            origin,
            grade,
            credits_attempted,
            credits_earned,
            repeat_status,
            recognition,
            categories,
        });
    }

    let scope = match required_reference(inputs, "scope")?.as_str() {
        "CUMULATIVE" => GpaScope::Cumulative,
        "TERM" => GpaScope::Term(TermKey::parse(&required_reference(inputs, "scope.term")?)?),
        "MAJOR" => GpaScope::Major(ProgramId::new(required_reference(
            inputs,
            "scope.program",
        )?)?),
        _ => return Err(RecordError::MalformedEngineInput("scope")),
    };
    Ok((facts, scope))
}

fn integer(inputs: &FrozenInputs, key: &str) -> Result<Option<i64>, RecordError> {
    match inputs.get(&InputKey::new(key)?) {
        Some(InputValue::Integer(value)) => Ok(Some(*value)),
        Some(_) => Err(RecordError::MalformedEngineInput("expected an integer")),
        None => Ok(None),
    }
}

fn required_reference(inputs: &FrozenInputs, key: &str) -> Result<String, RecordError> {
    match inputs.get(&InputKey::new(key)?) {
        Some(InputValue::Reference(value)) => Ok(value.clone()),
        Some(_) => Err(RecordError::MalformedEngineInput("expected a reference")),
        None => Err(RecordError::MissingEngineInput("reference")),
    }
}

fn required_decimal(inputs: &FrozenInputs, key: &str) -> Result<Decimal, RecordError> {
    match inputs.get(&InputKey::new(key)?) {
        Some(InputValue::Decimal(value)) => Ok(*value),
        Some(_) => Err(RecordError::MalformedEngineInput("expected a decimal")),
        None => Err(RecordError::MissingEngineInput("decimal")),
    }
}
