//! What one published rule concludes from the frozen facts.
//!
//! # The verdict vocabulary is the harness's, not a second one
//!
//! [`academic_domain::engines::ProofStatus`] is the five-value set the
//! deterministic engine harness fixes -- `SATISFIED`, `NEEDS`,
//! `NOT_SATISFIED`, `UNKNOWN`, `CONFLICT`. A rule outcome is a leaf in the
//! proof tree `P2-U3` builds, so it speaks that vocabulary rather than
//! declaring a parallel one that would have to be mapped.
//!
//! # Unknown is never folded
//!
//! A rule whose applicability, recognition ceiling or double-counting policy is
//! unknown returns `UNKNOWN` and names the section 38 cell that made it so. It
//! never returns `SATISFIED` and never returns `NOT_SATISFIED`, because both
//! would be a verdict manufactured out of an absent official fact. The gate is
//! carried on the outcome so the audit above can display the exact missing
//! check rather than a number.
//!
//! # No float, no division, no clock
//!
//! Every comparison is over integers or exact decimals. The grade-point
//! comparison is a cross-multiplication, so nothing divides and no rounding
//! decision is taken here. `as_of` is an argument on the facts.

use academic_domain::{Decimal, EntityId, engines::ProofStatus};

use crate::{
    dsl::{
        AdmissionYear, Applicability, CoRequisiteTiming, CountConstraint, DoubleCountingPolicy,
        RecognitionPolicy, RuleBody, RuleId,
    },
    error::RequirementError,
    facts::{AcademicFacts, AttemptFact, LanguageEvidence},
    gate::OpenGate,
    publish::RuleSet,
    rule_type::RuleType,
};

/// What a rule measured, exactly.
///
/// Section 11.3 requires a user to be able to open a number and see why it is
/// that number, so every rule that has a numerator reports both halves rather
/// than a verdict alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    /// A count of courses against a required count.
    Count {
        /// How many were found.
        attained: u32,
        /// How many are required.
        required: u32,
    },
    /// A credit sum against a required sum.
    Credits {
        /// How many were found.
        attained: u32,
        /// How many are required.
        required: u32,
    },
    /// A grade-point reading against a threshold, all three exact.
    GradePoint {
        /// The sum of grade points times credits.
        weighted_points: Decimal,
        /// The credits in the denominator.
        denominator_credits: u32,
        /// The threshold the rule requires.
        threshold: Decimal,
    },
}

/// One rule's verdict, with everything a proof leaf needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleOutcome {
    /// Which rule this is the verdict of.
    pub rule: RuleId,
    /// Its type.
    pub rule_type: RuleType,
    /// The verdict.
    pub status: ProofStatus,
    /// What was measured, when the rule measures something.
    pub measure: Option<Measure>,
    /// The section 38 cell that made the verdict `UNKNOWN`, when one did.
    ///
    /// `Some` exactly when `status` is `UNKNOWN` and the cause is an
    /// unconfirmed official fact rather than an absent record.
    pub open_gate: Option<OpenGate>,
    /// The attempts this verdict rests on, in the order they were read.
    pub used_attempts: Vec<EntityId>,
    /// The `EQUIVALENCY` rules that were applied to reach it.
    ///
    /// Section 11.3 requires every leaf to carry its equivalency decision. An
    /// empty list means no substitution was used, which is a decision too.
    pub equivalencies_applied: Vec<RuleId>,
}

impl RuleOutcome {
    fn bare(rule: &RuleId, rule_type: RuleType, status: ProofStatus) -> Self {
        Self {
            rule: rule.clone(),
            rule_type,
            status,
            measure: None,
            open_gate: None,
            used_attempts: Vec::new(),
            equivalencies_applied: Vec::new(),
        }
    }

    fn unknown(rule: &RuleId, rule_type: RuleType, gate: OpenGate) -> Self {
        let mut outcome = Self::bare(rule, rule_type, ProofStatus::Unknown);
        outcome.open_gate = Some(gate);
        outcome
    }
}

/// Whether a count that fell short reports `NEEDS` or `NOT_SATISFIED`.
///
/// Section 11.3's tree carries both: *CSE major total: 51 / 63 NEEDS 12* beside
/// *Algorithms: planned only NOT_SATISFIED*. The distinction is whether more of
/// the same thing would satisfy the rule. A credit floor can still be reached,
/// so it is `NEEDS`; a named course that was not taken is `NOT_SATISFIED` for
/// that leaf.
const fn shortfall(attained: u32, required: u32) -> ProofStatus {
    if attained >= required {
        ProofStatus::Satisfied
    } else {
        ProofStatus::Needs
    }
}

fn admission_year_or_unknown(
    facts: &AcademicFacts,
    rule: &RuleId,
    rule_type: RuleType,
    gate: OpenGate,
) -> Result<AdmissionYear, RuleOutcome> {
    facts
        .admission_year()
        .ok_or_else(|| RuleOutcome::unknown(rule, rule_type, gate))
}

fn applies_to(applicability: Applicability, year: AdmissionYear) -> bool {
    match applicability {
        Applicability::FromAdmissionYear(from) => year >= from,
        Applicability::BeforeAdmissionYear(before) => year < before,
        // Unreachable through `evaluate`, which resolves `Unknown` first. Kept
        // total rather than panicking: §2.3-11 is that no boundary panics.
        Applicability::Unknown => false,
    }
}

/// Whether `weighted_points / denominator >= threshold`, without dividing.
///
/// `wp.coefficient * 10^t.scale >= t.coefficient * denominator * 10^wp.scale`.
/// Every product is checked, so an input wide enough to overflow is a refusal
/// rather than a wrapped comparison that would silently invert the verdict.
fn grade_point_meets(
    weighted_points: Decimal,
    denominator_credits: u32,
    threshold: Decimal,
    rule: &RuleId,
) -> Result<bool, RequirementError> {
    let malformed = |reason: &'static str| RequirementError::MalformedRule {
        rule: rule.as_str().to_owned(),
        reason,
    };
    let scale_factor = |scale: u8| -> Result<i128, RequirementError> {
        10_i128
            .checked_pow(u32::from(scale))
            .ok_or_else(|| malformed("a grade-point scale exceeds exact comparison"))
    };
    let left = weighted_points
        .coefficient()
        .checked_mul(scale_factor(threshold.scale())?)
        .ok_or_else(|| malformed("a grade-point numerator exceeds exact comparison"))?;
    let right = threshold
        .coefficient()
        .checked_mul(i128::from(denominator_credits))
        .and_then(|value| value.checked_mul(scale_factor(weighted_points.scale()).ok()?))
        .ok_or_else(|| malformed("a grade-point threshold exceeds exact comparison"))?;
    Ok(left >= right)
}

/// Whether an operand's course is discharged, and by which equivalency.
///
/// Returns the attempt that discharged it and the `EQUIVALENCY` rule applied,
/// so the leaf can name both. A direct attempt applies no equivalency and says
/// so with an empty second element.
fn discharge(
    set: &RuleSet,
    facts: &AcademicFacts,
    course: &academic_domain::CourseId,
    equivalent_admitted: bool,
) -> Option<(EntityId, Option<RuleId>)> {
    if let Some(attempt) = facts
        .attempts()
        .iter()
        .find(|attempt| &attempt.course == course && attempt.status.is_recognized())
    {
        return Some((attempt.attempt, None));
    }
    if !equivalent_admitted {
        return None;
    }
    // A substitution counts only where a rule in this very set says so, in the
    // direction it says, inside the interval it names. Nothing is derived from a
    // replacement, a retirement or a shared course code -- those are
    // `academic-curriculum`'s catalogue facts and are not read here.
    for (rule, body) in set.rules() {
        let RuleBody::Equivalency {
            presented,
            counts_for,
            effective,
        } = body
        else {
            continue;
        };
        if counts_for != course || !effective.contains(facts.as_of()) {
            continue;
        }
        if let Some(attempt) = facts
            .attempts()
            .iter()
            .find(|attempt| &attempt.course == presented && attempt.status.is_recognized())
        {
            return Some((attempt.attempt, Some(rule.clone())));
        }
    }
    None
}

fn counts_after_exclusions(
    attempt: &AttemptFact,
    constraints: &[CountConstraint],
    year: Option<AdmissionYear>,
) -> bool {
    !constraints.iter().any(|constraint| match constraint {
        CountConstraint::AtLeastMajorCourses(_) => false,
        CountConstraint::ExcludedFromAdmissionYear { course, from } => {
            &attempt.course == course && year.is_some_and(|year| year >= *from)
        }
    })
}

/// Evaluates one rule of a published set against the frozen facts.
///
/// `set` is the rule's own published set, because a rule's meaning is fixed by
/// what was published beside it: `ALL_OF`'s `COURSE_OR_EQUIVALENT` operands
/// resolve against that set's `EQUIVALENCY` rules and nothing else, and an
/// `EXCEPTION_APPROVAL` names a rule in that set.
pub fn evaluate(
    set: &RuleSet,
    rule: &RuleId,
    body: &RuleBody,
    facts: &AcademicFacts,
) -> Result<RuleOutcome, RequirementError> {
    let rule_type = body.rule_type();
    let outcome = match body {
        RuleBody::CreditMinimum {
            category,
            threshold,
        } => {
            let mut attained: u32 = 0;
            let mut used = Vec::new();
            for attempt in facts.attempts() {
                if attempt.status.is_recognized() && attempt.categories.contains(category) {
                    attained += u32::from(attempt.credits.get());
                    used.push(attempt.attempt);
                }
            }
            let required = u32::from(threshold.get());
            let mut outcome =
                RuleOutcome::bare(rule, rule_type, shortfall(attained, required));
            outcome.measure = Some(Measure::Credits { attained, required });
            outcome.used_attempts = used;
            outcome
        }

        RuleBody::AllOf { operands } => {
            let mut satisfied: u32 = 0;
            let mut used = Vec::new();
            let mut equivalencies = Vec::new();
            for operand in operands {
                if let Some((attempt, equivalency)) =
                    discharge(set, facts, &operand.course, operand.equivalent_admitted)
                {
                    satisfied += 1;
                    used.push(attempt);
                    if let Some(equivalency) = equivalency {
                        equivalencies.push(equivalency);
                    }
                }
            }
            let required = u32::try_from(operands.len()).unwrap_or(u32::MAX);
            let status = if satisfied >= required {
                ProofStatus::Satisfied
            } else {
                // A named course that was not taken is not a shortfall that
                // more of the same closes; section 11.3 spells this leaf
                // NOT_SATISFIED.
                ProofStatus::NotSatisfied
            };
            let mut outcome = RuleOutcome::bare(rule, rule_type, status);
            outcome.measure = Some(Measure::Count {
                attained: satisfied,
                required,
            });
            outcome.used_attempts = used;
            outcome.equivalencies_applied = equivalencies;
            outcome
        }

        RuleBody::AtLeastNOf { n, operands } => {
            let mut satisfied: u32 = 0;
            let mut used = Vec::new();
            let mut equivalencies = Vec::new();
            for operand in operands {
                if let Some((attempt, equivalency)) =
                    discharge(set, facts, &operand.course, operand.equivalent_admitted)
                {
                    satisfied += 1;
                    used.push(attempt);
                    if let Some(equivalency) = equivalency {
                        equivalencies.push(equivalency);
                    }
                }
            }
            let required = u32::from(*n);
            let mut outcome =
                RuleOutcome::bare(rule, rule_type, shortfall(satisfied, required));
            outcome.measure = Some(Measure::Count {
                attained: satisfied,
                required,
            });
            outcome.used_attempts = used;
            outcome.equivalencies_applied = equivalencies;
            outcome
        }

        RuleBody::CountWithConstraints {
            minimum,
            constraints,
            counted,
        } => {
            // The exclusions are scoped by admission year, so a set with an
            // exclusion cannot be evaluated without one. That is `GATE-38-011`
            // and it is answered UNKNOWN rather than by counting as though the
            // exclusion did not apply.
            let has_year_scoped_exclusion = constraints
                .iter()
                .any(|constraint| matches!(constraint, CountConstraint::ExcludedFromAdmissionYear { .. }));
            let year = if has_year_scoped_exclusion {
                match admission_year_or_unknown(
                    facts,
                    rule,
                    rule_type,
                    OpenGate::CohortApplicability,
                ) {
                    Ok(year) => Some(year),
                    Err(unknown) => return Ok(unknown),
                }
            } else {
                facts.admission_year()
            };
            let mut attained: u32 = 0;
            let mut major: u32 = 0;
            let mut used = Vec::new();
            for attempt in facts.attempts() {
                if !attempt.status.is_recognized()
                    || !counted.contains(&attempt.course)
                    || !counts_after_exclusions(attempt, constraints, year)
                {
                    continue;
                }
                attained += 1;
                if attempt.is_major {
                    major += 1;
                }
                used.push(attempt.attempt);
            }
            let required = u32::from(*minimum);
            let major_required = constraints
                .iter()
                .filter_map(|constraint| match constraint {
                    CountConstraint::AtLeastMajorCourses(count) => Some(u32::from(*count)),
                    CountConstraint::ExcludedFromAdmissionYear { .. } => None,
                })
                .max()
                .unwrap_or(0);
            let status = if attained >= required && major >= major_required {
                ProofStatus::Satisfied
            } else {
                ProofStatus::Needs
            };
            let mut outcome = RuleOutcome::bare(rule, rule_type, status);
            outcome.measure = Some(Measure::Count { attained, required });
            outcome.used_attempts = used;
            outcome
        }

        RuleBody::GpaMinimum { scope, threshold } => {
            let Some(reading) = facts.gpa(scope) else {
                // No reading for the scope is an absent record, not an open
                // official fact: it is UNKNOWN with no gate attached.
                return Ok(RuleOutcome::bare(rule, rule_type, ProofStatus::Unknown));
            };
            let met = grade_point_meets(
                reading.weighted_points,
                reading.denominator_credits,
                *threshold,
                rule,
            )?;
            let status = if reading.denominator_credits == 0 {
                // An average over nothing is not zero and is not a pass.
                ProofStatus::Unknown
            } else if met {
                ProofStatus::Satisfied
            } else {
                ProofStatus::Needs
            };
            let mut outcome = RuleOutcome::bare(rule, rule_type, status);
            outcome.measure = Some(Measure::GradePoint {
                weighted_points: reading.weighted_points,
                denominator_credits: reading.denominator_credits,
                threshold: *threshold,
            });
            outcome
        }

        RuleBody::AreaDistribution { areas } => {
            let mut met: u32 = 0;
            let mut used = Vec::new();
            for requirement in areas {
                let mut credits: u32 = 0;
                for attempt in facts.attempts() {
                    if attempt.status.is_recognized()
                        && attempt.area.as_ref() == Some(&requirement.area)
                    {
                        credits += u32::from(attempt.credits.get());
                        used.push(attempt.attempt);
                    }
                }
                if credits >= u32::from(requirement.credits.get()) {
                    met += 1;
                }
            }
            let required = u32::try_from(areas.len()).unwrap_or(u32::MAX);
            let mut outcome = RuleOutcome::bare(rule, rule_type, shortfall(met, required));
            outcome.measure = Some(Measure::Count {
                attained: met,
                required,
            });
            outcome.used_attempts = used;
            outcome
        }

        RuleBody::CoRequisite {
            subject,
            companion,
            timing,
        } => {
            let taken = |course: &academic_domain::CourseId| {
                facts
                    .attempts()
                    .iter()
                    .find(|attempt| &attempt.course == course && attempt.status.is_recognized())
            };
            let Some(subject_attempt) = taken(subject) else {
                // The rule is about the subject; if it was not taken the
                // co-requisite has nothing to constrain.
                return Ok(RuleOutcome::bare(rule, rule_type, ProofStatus::Satisfied));
            };
            let mut used = vec![subject_attempt.attempt];
            let status = match taken(companion) {
                Some(companion_attempt) => {
                    used.push(companion_attempt.attempt);
                    let ok = match timing {
                        CoRequisiteTiming::SameTerm => {
                            companion_attempt.term == subject_attempt.term
                        }
                        CoRequisiteTiming::SameTermOrEarlier => {
                            companion_attempt.term <= subject_attempt.term
                        }
                    };
                    if ok {
                        ProofStatus::Satisfied
                    } else {
                        ProofStatus::NotSatisfied
                    }
                }
                None => ProofStatus::NotSatisfied,
            };
            let mut outcome = RuleOutcome::bare(rule, rule_type, status);
            outcome.used_attempts = used;
            outcome
        }

        RuleBody::MutuallyExclusive { members, policy } => {
            let DoubleCountingPolicy::AtMost(allowed) = policy else {
                return Ok(RuleOutcome::unknown(
                    rule,
                    rule_type,
                    OpenGate::MultiMajorDoubleCounting,
                ));
            };
            let mut recognized: u32 = 0;
            let mut used = Vec::new();
            for attempt in facts.attempts() {
                if attempt.status.is_recognized() && members.contains(&attempt.course) {
                    recognized += 1;
                    used.push(attempt.attempt);
                }
            }
            let allowed = u32::from(*allowed);
            let status = if recognized <= allowed {
                ProofStatus::Satisfied
            } else {
                // More of the members were recognized than the official rule
                // admits. That is not a shortfall and not a miss: it is two
                // recorded facts that cannot both stand.
                ProofStatus::Conflict
            };
            let mut outcome = RuleOutcome::bare(rule, rule_type, status);
            outcome.measure = Some(Measure::Count {
                attained: recognized,
                required: allowed,
            });
            outcome.used_attempts = used;
            outcome
        }

        RuleBody::Equivalency {
            presented,
            effective,
            ..
        } => {
            // An equivalency rule is a substitution the set admits, not a
            // requirement the student meets. Its own verdict says whether the
            // substitution is live at `as_of` and whether the presented course
            // was actually taken; `ALL_OF` and `AT_LEAST_N_OF` read it through
            // `discharge`, in the asserted direction only.
            let live = effective.contains(facts.as_of());
            let attempt = facts
                .attempts()
                .iter()
                .find(|attempt| &attempt.course == presented && attempt.status.is_recognized());
            let status = match (live, attempt) {
                (true, Some(_)) => ProofStatus::Satisfied,
                _ => ProofStatus::NotSatisfied,
            };
            let mut outcome = RuleOutcome::bare(rule, rule_type, status);
            if let Some(attempt) = attempt {
                outcome.used_attempts.push(attempt.attempt);
            }
            if live {
                outcome.equivalencies_applied.push(rule.clone());
            }
            outcome
        }

        RuleBody::MaximumRecognition { category, policy } => {
            let RecognitionPolicy::CappedAt(cap) = policy else {
                return Ok(RuleOutcome::unknown(
                    rule,
                    rule_type,
                    OpenGate::ExternalCreditRecognition,
                ));
            };
            let mut presented: u32 = 0;
            let mut used = Vec::new();
            for attempt in facts.attempts() {
                if attempt.status.is_recognized() && attempt.categories.contains(category) {
                    presented += u32::from(attempt.credits.get());
                    used.push(attempt.attempt);
                }
            }
            let cap = u32::from(cap.get());
            // A ceiling is satisfied by staying under it. Exceeding it is not a
            // failure of the student's record: the excess simply does not
            // count, and the leaf reports both halves so the audit can show
            // which credits were excluded and why.
            let mut outcome = RuleOutcome::bare(rule, rule_type, ProofStatus::Satisfied);
            outcome.measure = Some(Measure::Credits {
                attained: presented.min(cap),
                required: cap,
            });
            outcome.used_attempts = used;
            outcome
        }

        RuleBody::NonCreditTraining {
            program,
            applicability,
        } => {
            if *applicability == Applicability::Unknown {
                return Ok(RuleOutcome::unknown(
                    rule,
                    rule_type,
                    OpenGate::CohortApplicability,
                ));
            }
            let year = match admission_year_or_unknown(
                facts,
                rule,
                rule_type,
                OpenGate::CohortApplicability,
            ) {
                Ok(year) => year,
                Err(unknown) => return Ok(unknown),
            };
            if !applies_to(*applicability, year) {
                // Not applicable is not satisfied and not failed. Section 11.3's
                // tree carries it as its own reading.
                let mut outcome = RuleOutcome::bare(rule, rule_type, ProofStatus::Satisfied);
                outcome.measure = Some(Measure::Count {
                    attained: 0,
                    required: 0,
                });
                return Ok(outcome);
            }
            let completed = facts
                .trainings()
                .iter()
                .any(|training| &training.program == program);
            let status = if completed {
                ProofStatus::Satisfied
            } else {
                ProofStatus::NotSatisfied
            };
            let mut outcome = RuleOutcome::bare(rule, rule_type, status);
            outcome.measure = Some(Measure::Count {
                attained: u32::from(completed),
                required: 1,
            });
            outcome
        }

        RuleBody::LanguageOfInstruction {
            minimum,
            language,
            exclusions,
        } => {
            let has_year_scoped_exclusion = exclusions.iter().any(|constraint| {
                matches!(constraint, CountConstraint::ExcludedFromAdmissionYear { .. })
            });
            let year = if has_year_scoped_exclusion {
                match admission_year_or_unknown(
                    facts,
                    rule,
                    rule_type,
                    OpenGate::CohortApplicability,
                ) {
                    Ok(year) => Some(year),
                    Err(unknown) => return Ok(unknown),
                }
            } else {
                facts.admission_year()
            };
            let mut attained: u32 = 0;
            let mut used = Vec::new();
            for attempt in facts.attempts() {
                if !attempt.status.is_recognized()
                    || !counts_after_exclusions(attempt, exclusions, year)
                {
                    continue;
                }
                // An unverified language is not a negative reading; it simply
                // does not count. `REQ-11-015`: only verified in-scope attempts
                // count.
                if attempt.language == LanguageEvidence::Verified(*language) {
                    attained += 1;
                    used.push(attempt.attempt);
                }
            }
            let required = u32::from(*minimum);
            let mut outcome =
                RuleOutcome::bare(rule, rule_type, shortfall(attained, required));
            outcome.measure = Some(Measure::Count { attained, required });
            outcome.used_attempts = used;
            outcome
        }

        RuleBody::ThesisResearch {
            course,
            applicability,
            ..
        } => {
            if *applicability == Applicability::Unknown {
                // Section 8.1: the 2027 thesis rule's exact scope and
                // transitional arrangement need a departmental notice and an
                // administrative confirmation. `GATE-38-012` is open, so this
                // is UNKNOWN and never a pass or a fail.
                return Ok(RuleOutcome::unknown(rule, rule_type, OpenGate::ThesisRuleScope));
            }
            let year = match admission_year_or_unknown(
                facts,
                rule,
                rule_type,
                OpenGate::CohortApplicability,
            ) {
                Ok(year) => year,
                Err(unknown) => return Ok(unknown),
            };
            if !applies_to(*applicability, year) {
                let mut outcome = RuleOutcome::bare(rule, rule_type, ProofStatus::Satisfied);
                outcome.measure = Some(Measure::Count {
                    attained: 0,
                    required: 0,
                });
                return Ok(outcome);
            }
            let attempt = facts
                .attempts()
                .iter()
                .find(|attempt| &attempt.course == course && attempt.status.is_recognized());
            let mut outcome = RuleOutcome::bare(
                rule,
                rule_type,
                if attempt.is_some() {
                    ProofStatus::Satisfied
                } else {
                    ProofStatus::NotSatisfied
                },
            );
            outcome.measure = Some(Measure::Count {
                attained: u32::from(attempt.is_some()),
                required: 1,
            });
            if let Some(attempt) = attempt {
                outcome.used_attempts.push(attempt.attempt);
            }
            outcome
        }

        RuleBody::ExceptionApproval { target, approval } => {
            let admitted = facts.approvals().iter().any(|recorded| {
                &recorded.rule == target
                    && recorded.authority == approval.authority
                    && approval.valid_within.contains(recorded.issued_at)
                    && recorded
                        .expires_at
                        .is_none_or(|expiry| facts.as_of() < expiry)
            });
            let status = if admitted {
                ProofStatus::Satisfied
            } else {
                ProofStatus::NotSatisfied
            };
            let mut outcome = RuleOutcome::bare(rule, rule_type, status);
            outcome.measure = Some(Measure::Count {
                attained: u32::from(admitted),
                required: 1,
            });
            outcome
        }
    };
    Ok(outcome)
}
