//! Section 25.4's last line: a percentage is secondary and never travels alone.
//!
//! > "졸업 72%"는 보조 시각화일 수 있으나 서로 대체 불가능한 requirement를 한
//! > 막대로 오해시키지 않도록 상세 breakdown이 항상 붙는다.
//!
//! # The rule is a type, not a review note
//!
//! [`SecondaryPercentage::over`] is the **only** producer, and it takes a
//! [`RequirementBreakdown`] **by value**. There is no `Default`, no
//! `From<u8>`, no `new(permille)`, no setter and no `&mut` accessor, so:
//!
//! * a percentage with no breakdown is unrepresentable, not merely discouraged;
//! * the number is *computed from* the breakdown rather than supplied beside
//!   it, so it cannot disagree with the parts it claims to summarise;
//! * [`SecondaryPercentage::breakdown`] is total, which is the *항상 붙는다*
//!   half.
//!
//! `P2-Y3` fixed the same shape with one producer taking four disclosures by
//! value, and `P2-N6` with a result that does not exist without five public
//! groups. `percentage_is_secondary_with_breakdown` is the behavioural half and
//! `tests/compile_fail/a_percentage_is_not_built_from_a_number.rs` is the
//! compiled one; the whole-set half — that no *other* item in this crate
//! produces one — is `every_item_that_reaches_a_closed_type_is_pinned` in
//! `crates/contracts/tests/item_inventory_scans.rs`, which is keyed on the type
//! rather than on a spelling.
//!
//! # What the breakdown refuses
//!
//! **A requirement twice.** Two parts naming one requirement is the merge the
//! sentence warns about, arriving inside the breakdown that was supposed to
//! prevent it.
//!
//! **A part nobody evaluated.** `UNKNOWN` is *필요한 정보가 없음* (section 30)
//! and `CONFLICT` is two admitted sources disagreeing. A ratio over either
//! invents a denominator, which is the fold `academic_record::views::GpaValue`
//! refuses one surface over by answering `Unknown(attempts)` instead of a
//! number. The breakdown is still shown; the bar is not drawn.
//!
//! **A requirement that asks for nothing**, which has no ratio, and a part that
//! counts more than its requirement asked for, which would put a bar past its
//! own end.
//!
//! # Secondary is a position, not an adjective
//!
//! `percentage_is_secondary_with_breakdown` also reads
//! [`crate::AcademicDashboard`]: the percentage never occupies the first
//! section, the section it does occupy carries the breakdown's parts beside it,
//! and no accessor on the screen returns a percentage without one.

use crate::{AuditStateReading, DashboardError};

/// One requirement inside a graduation percentage.
///
/// The identity is the label the requirement is published under, and it is what
/// makes two parts the same requirement. The counted and required amounts are
/// credit quantities in the same unit; nothing here converts between units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakdownPart {
    label: String,
    counted: u32,
    required: u32,
    reading: AuditStateReading,
}

impl BreakdownPart {
    /// Records one requirement's contribution.
    pub fn of(
        label: impl Into<String>,
        counted: u32,
        required: u32,
        reading: AuditStateReading,
    ) -> Result<Self, DashboardError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(DashboardError::EmptyField("requirement label"));
        }
        if required == 0 {
            return Err(DashboardError::BreakdownPartRequiresNothing { label });
        }
        if counted > required {
            return Err(DashboardError::BreakdownPartOverflows { label });
        }
        Ok(Self {
            label,
            counted,
            required,
            reading,
        })
    }

    /// The requirement's published label, which is its identity here.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// How much of the requirement is met.
    #[must_use]
    pub const fn counted(&self) -> u32 {
        self.counted
    }

    /// How much the requirement asks for.
    #[must_use]
    pub const fn required(&self) -> u32 {
        self.required
    }

    /// The audit reading for this requirement, engine status and all.
    #[must_use]
    pub const fn reading(&self) -> AuditStateReading {
        self.reading
    }
}

/// The 상세 breakdown a percentage is never shown without.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementBreakdown {
    parts: Vec<BreakdownPart>,
}

impl RequirementBreakdown {
    /// Assembles a breakdown, refusing an empty one and a repeated requirement.
    pub fn assemble(parts: Vec<BreakdownPart>) -> Result<Self, DashboardError> {
        if parts.is_empty() {
            return Err(DashboardError::PercentageWithoutBreakdown);
        }
        for (index, part) in parts.iter().enumerate() {
            if parts
                .iter()
                .skip(index.saturating_add(1))
                .any(|other| other.label() == part.label())
            {
                return Err(DashboardError::BreakdownRepeatsARequirement {
                    label: part.label().to_owned(),
                });
            }
        }
        Ok(Self { parts })
    }

    /// The requirements, in the order they were assembled in.
    #[must_use]
    pub fn parts(&self) -> &[BreakdownPart] {
        &self.parts
    }

    /// The parts no bar may be drawn over.
    #[must_use]
    pub fn unsettled(&self) -> Vec<&BreakdownPart> {
        self.parts
            .iter()
            .filter(|part| !part.reading().is_evaluated())
            .collect()
    }
}

/// A percentage that exists only alongside the breakdown it summarises.
///
/// Held in permille rather than percent so that the parts the sentence calls
/// *서로 대체 불가능한* do not have to be rounded into each other before the
/// reader ever sees the breakdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryPercentage {
    breakdown: RequirementBreakdown,
    permille: u32,
}

impl SecondaryPercentage {
    /// Draws a bar over a breakdown, taking the breakdown by value.
    ///
    /// The only producer. The number is computed here and never accepted, so
    /// there is no argument a caller could pass that makes the bar disagree
    /// with its own parts.
    pub fn over(breakdown: RequirementBreakdown) -> Result<Self, DashboardError> {
        let unsettled = breakdown.unsettled().len();
        if unsettled > 0 {
            return Err(DashboardError::PercentageOverAnUnsettledPart { count: unsettled });
        }
        let mut counted: u64 = 0;
        let mut required: u64 = 0;
        for part in breakdown.parts() {
            counted = counted.saturating_add(u64::from(part.counted()));
            required = required.saturating_add(u64::from(part.required()));
        }
        // `BreakdownPart::of` refuses a requirement of zero and `assemble`
        // refuses an empty breakdown, so the denominator is positive here. The
        // `unwrap_or` is the unreachable arm written out rather than a panic,
        // because this crate denies panicking paths.
        let permille = counted
            .saturating_mul(1000)
            .checked_div(required)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0);
        Ok(Self {
            breakdown,
            permille,
        })
    }

    /// The breakdown, which is always here.
    #[must_use]
    pub const fn breakdown(&self) -> &RequirementBreakdown {
        &self.breakdown
    }

    /// The bar, in permille.
    #[must_use]
    pub const fn permille(&self) -> u32 {
        self.permille
    }
}
