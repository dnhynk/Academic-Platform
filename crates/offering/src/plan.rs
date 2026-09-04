//! The determinate plan, and why `HISTORICALLY_LIKELY` cannot enter one.
//!
//! Section 8.3's Planner 취급 cell for `HISTORICALLY_LIKELY` is
//! *placeholder만, 졸업계획 확정에 사용 금지* -- a placeholder only, forbidden
//! for use in confirming the graduation plan. That prohibition is a type here
//! and not a check:
//!
//! [`DeterminatePlan::commit`] takes a `Vec<ConfirmedSeat>` **by value**, and
//! `academic_offering::standing::ConfirmedSeat` has private fields, no public
//! constructor and one producer -- `ConfirmedStanding::seat`. A likely
//! standing, an uncertain standing and a cancelled standing have no `seat`
//! method that returns one, so there is no expression anywhere that produces
//! the argument. `historically_likely_cannot_enter_determinate_plan` observes
//! that, and `tests/compile_fail/` compiles four attempts to get around it and
//! requires each to fail.
//!
//! # Three layers, and this is the outermost
//!
//! `P2-U4` gave `PlanScenarioChoice` no route to a `CourseAttempt` and
//! `AttemptStatus::Planned` no producer. `P2-U3` gave
//! `DegreeAudit::evaluate` no plan parameter, so a plan cannot move a
//! graduation measure at all. This module adds the layer between them: a plan
//! that says *these seats are real* can only be built out of seats that are.
//!
//! # An indeterminate plan always says what is outstanding
//!
//! [`IndeterminatePlan::new`] takes its first [`PlanRefusal`] as a
//! **parameter**, so an indeterminate plan with an empty list is not a call
//! that can be written -- the shape `P2-U3` used for `IndeterminateVerdict`.
//! Every arm names the exact course and term and states what settles it,
//! because section 8.3's `UNCERTAIN` row requires 경고와 대체 경로 -- a warning
//! *and* an alternative path -- and a refusal that only said "not confirmed"
//! would satisfy the letter and lose the point.

use academic_record::{plan::PlanScenario, term::TermKey};

use crate::standing::ConfirmedSeat;

/// What a determinate plan refused, and what settles it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanRefusal {
    /// The scenario names a course for a term with no confirmed seat.
    ///
    /// There is deliberately no field here saying which of section 8.3's other
    /// three rows the offering is on. This function is not handed the
    /// standings and inventing one -- `UNCERTAIN`, say, for an offering that
    /// is actually `CANCELLED` -- would put a status on the screen that no
    /// source produced. The caller holds the `Resolution` and reads the row
    /// from it.
    NoConfirmedSeat {
        /// The course the scenario named.
        course: String,
        /// The term it was planned for.
        term: TermKey,
    },
    /// A seat was supplied for a term the scenario does not plan that course
    /// for.
    SeatForAnotherTerm {
        /// The course.
        course: String,
        /// The term the seat confirms.
        seat_term: TermKey,
        /// The term the scenario plans it for.
        planned_term: TermKey,
    },
    /// A seat was supplied for a course the scenario does not name.
    ///
    /// Refused rather than ignored: a plan silently wider than the scenario it
    /// was committed from is a plan the user did not make.
    SeatWithNoChoice {
        /// The course the seat confirms.
        course: String,
        /// The term it confirms.
        term: TermKey,
    },
}

impl PlanRefusal {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NoConfirmedSeat { .. } => "NO_CONFIRMED_SEAT",
            Self::SeatForAnotherTerm { .. } => "SEAT_FOR_ANOTHER_TERM",
            Self::SeatWithNoChoice { .. } => "SEAT_WITH_NO_CHOICE",
        }
    }

    /// The course this refusal is about.
    #[must_use]
    pub fn course(&self) -> &str {
        match self {
            Self::NoConfirmedSeat { course, .. }
            | Self::SeatForAnotherTerm { course, .. }
            | Self::SeatWithNoChoice { course, .. } => course,
        }
    }

    /// What settles it.
    #[must_use]
    pub const fn action(&self) -> &'static str {
        match self {
            Self::NoConfirmedSeat { .. } => {
                "re-read the registration system for this term, or plan an \
                 alternative course for it"
            }
            Self::SeatForAnotherTerm { .. } => {
                "move the plan choice to the term the seat confirms, or confirm \
                 the term the choice names"
            }
            Self::SeatWithNoChoice { .. } => {
                "add the course to the scenario, or drop the seat from the \
                 commit"
            }
        }
    }

    /// Whether this refusal is one section 8.3's `UNCERTAIN` row requires an
    /// alternative path for.
    ///
    /// A course the plan wants and no source confirms is exactly that case; a
    /// seat that does not line up with the scenario is a bookkeeping error and
    /// needs a correction rather than another course.
    #[must_use]
    pub const fn requires_alternative_path(&self) -> bool {
        matches!(self, Self::NoConfirmedSeat { .. })
    }
}

/// A plan every one of whose seats is a confirmed offering.
///
/// Private fields, no public constructor, and [`Self::commit`] is the one site
/// that builds one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterminatePlan {
    seats: Vec<ConfirmedSeat>,
}

impl DeterminatePlan {
    /// Commits one scenario against the seats its choices confirm.
    ///
    /// Every choice needs a seat and every seat needs a choice, matched on
    /// course and term. A scenario with no choices commits to an empty plan,
    /// which is a plan that claims nothing.
    #[must_use]
    pub fn commit(scenario: &PlanScenario, seats: Vec<ConfirmedSeat>) -> PlanOutcome {
        let mut refusals = Vec::new();
        let mut matched: Vec<ConfirmedSeat> = Vec::new();

        for choice in scenario.choices() {
            let planned_term = choice.intended_term();
            let code = choice.course_code();
            let seat = seats
                .iter()
                .find(|seat| seat.course().as_str() == code && seat.term() == planned_term);
            match seat {
                Some(seat) => matched.push(seat.clone()),
                None => {
                    let wrong_term = seats.iter().find(|seat| seat.course().as_str() == code);
                    match wrong_term {
                        Some(seat) => refusals.push(PlanRefusal::SeatForAnotherTerm {
                            course: code.to_owned(),
                            seat_term: seat.term(),
                            planned_term,
                        }),
                        None => refusals.push(PlanRefusal::NoConfirmedSeat {
                            course: code.to_owned(),
                            term: planned_term,
                        }),
                    }
                }
            }
        }

        for seat in &seats {
            let named = scenario.choices().iter().any(|choice| {
                choice.course_code() == seat.course().as_str()
                    && choice.intended_term() == seat.term()
            });
            let course_named = scenario
                .choices()
                .iter()
                .any(|choice| choice.course_code() == seat.course().as_str());
            if !named && !course_named {
                refusals.push(PlanRefusal::SeatWithNoChoice {
                    course: seat.course().as_str().to_owned(),
                    term: seat.term(),
                });
            }
        }

        match refusals.split_first() {
            None => PlanOutcome::Determinate(Self { seats: matched }),
            Some((first, rest)) => {
                PlanOutcome::Indeterminate(IndeterminatePlan::new(first.clone(), rest.to_vec()))
            }
        }
    }

    /// The committed seats, in scenario order.
    #[must_use]
    pub fn seats(&self) -> &[ConfirmedSeat] {
        &self.seats
    }

    /// How many seats the plan holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.seats.len()
    }

    /// Whether the plan holds no seats.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.seats.is_empty()
    }
}

/// A plan that could not be committed, and everything outstanding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndeterminatePlan {
    refusals: Vec<PlanRefusal>,
}

impl IndeterminatePlan {
    /// Records at least one refusal.
    ///
    /// The first is a parameter, so an indeterminate plan with an empty list is
    /// not a call that can be written.
    #[must_use]
    pub fn new(first: PlanRefusal, rest: Vec<PlanRefusal>) -> Self {
        let mut refusals = vec![first];
        refusals.extend(rest);
        Self { refusals }
    }

    /// Every refusal, in scenario order.
    #[must_use]
    pub fn refusals(&self) -> &[PlanRefusal] {
        &self.refusals
    }

    /// The courses section 8.3's `UNCERTAIN` row requires an alternative path
    /// for.
    #[must_use]
    pub fn alternative_paths_required(&self) -> Vec<&str> {
        self.refusals
            .iter()
            .filter(|refusal| refusal.requires_alternative_path())
            .map(PlanRefusal::course)
            .collect()
    }
}

/// What committing a scenario produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanOutcome {
    /// Every choice is a confirmed seat.
    Determinate(DeterminatePlan),
    /// Something is outstanding, and the plan says exactly what.
    Indeterminate(IndeterminatePlan),
}
