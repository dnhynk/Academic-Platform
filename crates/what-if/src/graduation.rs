//! Section 22.5's fifth bullet: *졸업 판정은 hypothetical mode와 actual mode를
//! 명확히 분리한다.*
//!
//! # The two modes live in two crates, and only one of them is here
//!
//! `P2-U3` owns the actual mode. Its `DeterminateVerdict` has private fields
//! and one constructor, and that constructor takes a `CoverageWitness`, a
//! `ConflictFreeWitness` and a `FreshnessWitness` **by value**, each with a
//! crate-private `establish`. Its `DegreeAudit::evaluate` has no plan
//! parameter at all.
//!
//! This crate does not weaken any of that, because it cannot reach it. There is
//! no `academic-audit` edge of any kind — not a normal one, not a dev one —
//! anywhere in this crate's declared closure, so `academic_audit::` is not a
//! path that resolves here. `crates/what-if/tests/compile_fail/` holds that as
//! a program that does not compile, and
//! `hypothetical_and_actual_graduation_modes_are_distinct` walks the workspace
//! manifests from this package and requires the audit crate to be unreachable,
//! then reads `P2-U3`'s verdict vocabulary out of its own source and requires
//! every name in it to be absent from this crate — with a control that requires
//! the same reader to find those names where they live.
//!
//! That is why [`HypotheticalGraduation`] is not a verdict and cannot become
//! one. It carries what section 22.4's first row shows — a credit contribution
//! per category, with the allocation proof behind it — under the assumption
//! that made it, and it carries a banner saying so.
//!
//! # A mode is a property of the value, not a flag on it
//!
//! [`HypotheticalGraduation::MODE`] is a constant and [`HypotheticalGraduation::mode`]
//! returns it. There is no setter, no constructor parameter and no other
//! function in this crate that returns a [`GraduationMode`], so no value here
//! can be put in the actual mode by any expression at all.

use crate::{
    assumption::HypotheticalCompletion,
    deterministic::{Allocation, CategoryContribution},
    scenario::PlanScenario,
};

/// The two modes section 22.5 separates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GraduationMode {
    /// A reading of what a plan *would* contribute, under a stated assumption.
    Hypothetical,
    /// A verdict about whether the user can graduate.
    Actual,
}

/// Both, hypothetical first.
pub const GRADUATION_MODES: [GraduationMode; 2] =
    [GraduationMode::Hypothetical, GraduationMode::Actual];

impl GraduationMode {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Hypothetical => "HYPOTHETICAL",
            Self::Actual => "ACTUAL",
        }
    }

    /// The package that owns values in this mode.
    ///
    /// A total `match`, and the two answers are two different packages. That is
    /// the separation section 22.5 asks for stated as a fact about the
    /// repository rather than as a label on a value.
    #[must_use]
    pub const fn owner_package(self) -> &'static str {
        match self {
            Self::Hypothetical => "academic-what-if",
            Self::Actual => "academic-audit",
        }
    }

    /// Whether a value in this mode may conclude that the user can graduate.
    #[must_use]
    pub const fn concludes_graduation(self) -> bool {
        match self {
            Self::Hypothetical => false,
            Self::Actual => true,
        }
    }
}

/// What a plan would contribute towards graduation, if it were completed.
///
/// Borrows the plan. There is no owned copy and no `&mut`, so a graduation
/// reading cannot change the plan it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HypotheticalGraduation<'a> {
    plan: &'a PlanScenario,
}

impl<'a> HypotheticalGraduation<'a> {
    /// The mode every value of this type is in.
    pub const MODE: GraduationMode = GraduationMode::Hypothetical;

    /// The banner section 34.5's recovery column requires beside a projection.
    pub const BANNER: &'static str = "가정 시나리오 · 졸업 판정 아님";

    /// Reads one plan in the hypothetical mode.
    #[must_use]
    pub const fn of(plan: &'a PlanScenario) -> Self {
        Self { plan }
    }

    /// The mode, which is always [`GraduationMode::Hypothetical`].
    #[must_use]
    pub const fn mode(&self) -> GraduationMode {
        Self::MODE
    }

    /// The credit contribution per category, under the plan's assumption.
    #[must_use]
    pub fn contributions(&self) -> &[CategoryContribution] {
        self.plan
            .deterministic()
            .rule_contribution()
            .contributions()
    }

    /// The per-choice lines behind those totals.
    #[must_use]
    pub fn proof(&self) -> &Allocation {
        self.plan.deterministic().allocation()
    }

    /// The assumption the reading rests on.
    #[must_use]
    pub fn completion(&self) -> HypotheticalCompletion {
        self.plan.deterministic().rule_contribution().completion()
    }

    /// The plan this reading is of.
    #[must_use]
    pub const fn plan(&self) -> &'a PlanScenario {
        self.plan
    }
}
