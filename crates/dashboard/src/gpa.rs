//! Section 25.4's first line: three averages, each with its own proof.
//!
//! > 누적·학기·전공 GPA와 각 계산 proof.
//!
//! Three scopes, and section 10's calculation view lists the same three first —
//! *누적 GPA와 계산에 포함된 attempt proof*, *학기별 GPA*, *전공 GPA*. Section
//! 10 goes on to *필요 시 다전공별 GPA*, which is a per-programme repetition of
//! the third rather than a fourth scope, and this crate keeps it that way:
//! [`GpaScope::Major`] is published once per programme, and
//! [`crate::AcademicDashboard`] holds one figure per scope for the programme it
//! was assembled for.
//!
//! # Nothing here computes an average
//!
//! [`GpaFigure::publish`] takes an `academic_record::views::GpaValue` and the
//! attempts behind it. This crate declares no grading scheme, no repeat policy
//! and no arithmetic over grades. What it does is refuse a figure whose proof
//! is not a proof:
//!
//! * a `Known` average with no attempt behind it — an average over nothing;
//! * an `Unknown` average whose proof does not name every attempt the value
//!   itself names, which is the case where the surface would say *unknown* and
//!   be unable to say *because of which attempt*.
//!
//! A `NoGradedAttempts` value is the one case where an empty inclusion list is
//! correct, and it is admitted for exactly that value.

use academic_domain::AttemptId;
use academic_record::views::{DispositionReason, GpaValue, RepeatProof};

use crate::DashboardError;

/// One of section 25.4's three averages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GpaScope {
    /// 누적 — over every attempt the rule book admits.
    Cumulative,
    /// 학기 — over one term.
    Term,
    /// 전공 — over the attempts an applied rule classified as major.
    Major,
}

impl GpaScope {
    /// Every scope, in section 25.4's own order.
    pub const ALL: [Self; 3] = [Self::Cumulative, Self::Term, Self::Major];

    /// The word section 25.4 spells this scope with.
    ///
    /// `dashboard_shows_three_gpas_with_proof` splits section 25.4's own first
    /// line on its own middle dot and compares the pieces with these, so a
    /// fourth scope in the document fails as a missing key rather than being
    /// folded into the nearest one.
    #[must_use]
    pub const fn spec_word(self) -> &'static str {
        match self {
            Self::Cumulative => "누적",
            Self::Term => "학기",
            Self::Major => "전공",
        }
    }
}

impl core::fmt::Display for GpaScope {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.spec_word())
    }
}

/// Why one average is the number it is.
///
/// Section 10 asks a calculation view for the included attempts, the difference
/// between earned credits and the denominator, the repeat groups and which
/// grade was recognized, and the reason every special attempt was treated the
/// way it was. Each is a separate field here and none is derived from another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpaProof {
    included: Vec<AttemptId>,
    reasons: Vec<(AttemptId, DispositionReason)>,
    repeats: Vec<RepeatProof>,
}

impl GpaProof {
    /// Records the proof of one average.
    ///
    /// Every argument is taken by value. There is no `Default`, no setter and
    /// no `&mut` accessor, so a proof cannot be emptied after a figure was
    /// built on it.
    #[must_use]
    pub fn recording(
        included: Vec<AttemptId>,
        reasons: Vec<(AttemptId, DispositionReason)>,
        repeats: Vec<RepeatProof>,
    ) -> Self {
        Self {
            included,
            reasons,
            repeats,
        }
    }

    /// The attempts that reached the denominator.
    #[must_use]
    pub fn included(&self) -> &[AttemptId] {
        &self.included
    }

    /// Why each attempt contributed what it did — section 10's last line.
    #[must_use]
    pub fn reasons(&self) -> &[(AttemptId, DispositionReason)] {
        &self.reasons
    }

    /// 재수강 전후 시도와 어느 성적이 인정되었는지.
    #[must_use]
    pub fn repeats(&self) -> &[RepeatProof] {
        &self.repeats
    }

    /// Whether the proof accounts for `attempt` at all.
    #[must_use]
    pub fn accounts_for(&self, attempt: AttemptId) -> bool {
        self.included.contains(&attempt) || self.reasons.iter().any(|(id, _)| *id == attempt)
    }
}

/// One published average and the proof it rests on.
///
/// The one producer is [`GpaFigure::publish`], which takes the proof **by
/// value**. There is no `Default`, no setter, no `&mut` accessor and no
/// constructor that takes only a value, so there is no state in which a figure
/// exists and its proof does not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GpaFigure {
    scope: GpaScope,
    value: GpaValue,
    proof: GpaProof,
}

impl GpaFigure {
    /// Publishes one average under one scope.
    pub fn publish(
        scope: GpaScope,
        value: GpaValue,
        proof: GpaProof,
    ) -> Result<Self, DashboardError> {
        match &value {
            GpaValue::Known(_) => {
                if proof.included().is_empty() {
                    return Err(DashboardError::AverageWithoutProof { scope });
                }
            }
            GpaValue::Unknown(attempts) => {
                let missing = attempts
                    .iter()
                    .filter(|attempt| !proof.accounts_for(**attempt))
                    .count();
                if missing > 0 {
                    return Err(DashboardError::ProofOmitsUnknownAttempts { scope, missing });
                }
            }
            // The one value an empty inclusion list is the honest answer for.
            GpaValue::NoGradedAttempts => {}
        }
        Ok(Self {
            scope,
            value,
            proof,
        })
    }

    /// Which of section 25.4's three averages this is.
    #[must_use]
    pub const fn scope(&self) -> GpaScope {
        self.scope
    }

    /// The average `P2-U4`'s engine published, or the reason there is not one.
    #[must_use]
    pub const fn value(&self) -> &GpaValue {
        &self.value
    }

    /// The proof, which is always here.
    #[must_use]
    pub const fn proof(&self) -> &GpaProof {
        &self.proof
    }
}
