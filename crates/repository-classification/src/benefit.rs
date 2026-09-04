//! Section 18.3's `WOULD_BENEFIT_FROM`, and the four things it cannot be
//! published without.
//!
//! Section 18.3 writes the classification as a document with four fields
//! beside the concept:
//!
//! ```yaml
//! classification: WOULD_BENEFIT_FROM
//! concept: REPLICATION
//! trigger:
//!   - "single database availability target exceeds current recovery objective"
//!   - "read load exceeds measured primary capacity"
//! currentTriggerState: NOT_MET
//! benefit: availability/read scaling
//! tradeoffs: consistency, failover complexity, cost
//! ```
//!
//! and ends with `trigger와 trade-off 없는 "있으면 좋은 기술" 목록은 만들지
//! 않는다`. Section 38's answer sixteen says the same from the other side:
//! `WOULD_BENEFIT는 미래 trigger와 trade-off가 필요하다`.
//!
//! ## Why a generic list produces nothing
//!
//! [`BenefitContract`] has private fields, no `Default`, and one constructor,
//! and that constructor takes all four: at least one [`Trigger`], the
//! [`TriggerState`] those triggers are currently in, a [`BenefitDimension`],
//! and at least one [`TradeOff`]. A list of concept names has none of them, so
//! it does not become a contract that fails validation — it never becomes a
//! contract at all.
//!
//! [`BenefitDraft`] is the same door [`crate::ChainDraft`] is, for the same
//! reason, and it names the missing part rather than defaulting it.
//! `generic_nice_to_have_list_produces_zero_findings` feeds a list of bare
//! concept names through it and observes one refusal per entry and an empty
//! publication.
//!
//! ## Why the benefit is an enumeration and the trade-off is not
//!
//! Section 18.3 names the four things a benefit may be — `scale, resilience,
//! performance, maintainability` — so [`BenefitDimension`] is that closed list
//! and a fifth dimension is a change to this file. It names no such list for
//! trade-offs: `consistency, failover complexity, cost` are three examples of
//! an open set, so a [`TradeOff`] is a caller-chosen identifier the way a
//! [`SubjectId`] is.

use academic_repository_analysis::SubjectId;

use crate::{ClassificationError, scope::validated};

/// Which part of section 18.3's contract a draft was missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BenefitPart {
    /// The concept the benefit is about.
    Concept,
    /// `trigger`.
    Trigger,
    /// `currentTriggerState`.
    TriggerState,
    /// `benefit`.
    Benefit,
    /// `tradeoffs`.
    TradeOff,
}

impl BenefitPart {
    /// Exhaustive order, in section 18.3's own field order.
    pub const ALL: [Self; 5] = [
        Self::Concept,
        Self::Trigger,
        Self::TriggerState,
        Self::Benefit,
        Self::TradeOff,
    ];

    /// The missing-part code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Concept => "MISSING_CONCEPT",
            Self::Trigger => "MISSING_TRIGGER",
            Self::TriggerState => "MISSING_TRIGGER_STATE",
            Self::Benefit => "MISSING_BENEFIT",
            Self::TradeOff => "MISSING_TRADEOFF",
        }
    }
}

/// One condition that would make the concept worth adopting.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Trigger {
    identifier: String,
}

impl Trigger {
    /// Validates and takes a trigger identifier.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::InvalidIdentifier`] when it is empty, over 64
    /// bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, ClassificationError> {
        Ok(Self {
            identifier: validated(value.into(), "trigger")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// One cost the concept brings with it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TradeOff {
    identifier: String,
}

impl TradeOff {
    /// Validates and takes a trade-off identifier.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::InvalidIdentifier`] on the same three conditions
    /// [`Trigger::new`] refuses.
    pub fn new(value: impl Into<String>) -> Result<Self, ClassificationError> {
        Ok(Self {
            identifier: validated(value.into(), "tradeoff")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// Section 18.3's `currentTriggerState`.
///
/// `NOT_MET` is the value section 18.3 writes. `MET` exists because a trigger
/// that has fired is a fact a reader has to be able to see — and because
/// seeing it is *not* a reclassification: a met trigger does not make the
/// concept `REQUIRED`, which needs section 18.2's five-step chain and the
/// user's own evidence gap, neither of which a trigger supplies. `UNKNOWN` is
/// for a trigger this snapshot cannot evaluate, which `REQ-34-097` requires a
/// reader to be shown rather than have guessed for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TriggerState {
    /// The condition has not been reached.
    NotMet,
    /// The condition has been reached.
    Met,
    /// This snapshot does not answer whether it has.
    Unknown,
}

impl TriggerState {
    /// Exhaustive order, weakest claim last.
    pub const ALL: [Self; 3] = [Self::NotMet, Self::Met, Self::Unknown];

    /// Stable spelling; the first is section 18.3's own.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotMet => "NOT_MET",
            Self::Met => "MET",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// Which of section 18.3's four improvements the concept would bring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BenefitDimension {
    /// `scale`.
    Scale,
    /// `resilience`.
    Resilience,
    /// `performance`.
    Performance,
    /// `maintainability`.
    Maintainability,
}

impl BenefitDimension {
    /// Exhaustive order, in section 18.3's own order.
    pub const ALL: [Self; 4] = [
        Self::Scale,
        Self::Resilience,
        Self::Performance,
        Self::Maintainability,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scale => "SCALE",
            Self::Resilience => "RESILIENCE",
            Self::Performance => "PERFORMANCE",
            Self::Maintainability => "MAINTAINABILITY",
        }
    }
}

/// Section 18.3's document as a value: concept, triggers, state, benefit, costs.
///
/// Private fields, no `Default`, one constructor, and neither list may be
/// empty. There is no representation of a benefit without a trigger and no
/// representation of one without a trade-off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenefitContract {
    concept: String,
    triggers: Vec<Trigger>,
    state: TriggerState,
    benefit: BenefitDimension,
    tradeoffs: Vec<TradeOff>,
}

impl BenefitContract {
    /// Builds a contract from all four parts.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::BenefitPartMissing`] carrying
    /// [`BenefitPart::Trigger`] for an empty trigger list and
    /// [`BenefitPart::TradeOff`] for an empty trade-off list. An empty list is
    /// the shape a `있으면 좋은 기술` list arrives in.
    pub fn new(
        concept: &SubjectId,
        triggers: Vec<Trigger>,
        state: TriggerState,
        benefit: BenefitDimension,
        tradeoffs: Vec<TradeOff>,
    ) -> Result<Self, ClassificationError> {
        if triggers.is_empty() {
            return Err(ClassificationError::BenefitPartMissing {
                concept: concept.as_str().to_owned(),
                part: BenefitPart::Trigger,
            });
        }
        if tradeoffs.is_empty() {
            return Err(ClassificationError::BenefitPartMissing {
                concept: concept.as_str().to_owned(),
                part: BenefitPart::TradeOff,
            });
        }
        Ok(Self {
            concept: concept.as_str().to_owned(),
            triggers,
            state,
            benefit,
            tradeoffs,
        })
    }

    /// Which concept would help.
    #[must_use]
    pub fn concept(&self) -> &str {
        &self.concept
    }

    /// The conditions, at least one.
    #[must_use]
    pub fn triggers(&self) -> &[Trigger] {
        &self.triggers
    }

    /// Where those conditions currently stand.
    #[must_use]
    pub const fn state(&self) -> TriggerState {
        self.state
    }

    /// What would improve.
    #[must_use]
    pub const fn benefit(&self) -> BenefitDimension {
        self.benefit
    }

    /// What it would cost, at least one.
    #[must_use]
    pub fn tradeoffs(&self) -> &[TradeOff] {
        &self.tradeoffs
    }
}

/// What one model-authored or imported benefit offers, before it is a contract.
///
/// The one door from an untyped proposal into a [`BenefitContract`]. A bare
/// concept name fills exactly one of the five slots, and
/// [`BenefitDraft::seal`] names the first empty one.
#[derive(Debug, Clone, Default)]
pub struct BenefitDraft {
    concept: Option<String>,
    triggers: Vec<Trigger>,
    state: Option<TriggerState>,
    benefit: Option<BenefitDimension>,
    tradeoffs: Vec<TradeOff>,
}

impl BenefitDraft {
    /// An empty draft, which [`BenefitDraft::seal`] refuses at the concept.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Offers the concept, which is all a `있으면 좋은 기술` list carries.
    #[must_use]
    pub fn with_concept(mut self, concept: &SubjectId) -> Self {
        self.concept = Some(concept.as_str().to_owned());
        self
    }

    /// Offers the triggers.
    #[must_use]
    pub fn with_triggers(mut self, triggers: Vec<Trigger>) -> Self {
        self.triggers = triggers;
        self
    }

    /// Offers the current trigger state.
    #[must_use]
    pub const fn with_state(mut self, state: TriggerState) -> Self {
        self.state = Some(state);
        self
    }

    /// Offers the benefit dimension.
    #[must_use]
    pub const fn with_benefit(mut self, benefit: BenefitDimension) -> Self {
        self.benefit = Some(benefit);
        self
    }

    /// Offers the trade-offs.
    #[must_use]
    pub fn with_tradeoffs(mut self, tradeoffs: Vec<TradeOff>) -> Self {
        self.tradeoffs = tradeoffs;
        self
    }

    /// Builds the contract, or names the first part that is not there.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::BenefitPartMissing`] carrying the
    /// [`BenefitPart`] whose code a blocked publish shows.
    pub fn seal(self) -> Result<BenefitContract, ClassificationError> {
        let concept = self
            .concept
            .ok_or(ClassificationError::BenefitPartMissing {
                concept: String::new(),
                part: BenefitPart::Concept,
            })?;
        let named = |part| ClassificationError::BenefitPartMissing {
            concept: concept.clone(),
            part,
        };
        if self.triggers.is_empty() {
            return Err(named(BenefitPart::Trigger));
        }
        let state = self.state.ok_or_else(|| named(BenefitPart::TriggerState))?;
        let benefit = self.benefit.ok_or_else(|| named(BenefitPart::Benefit))?;
        // The trade-off list is not checked here. `BenefitContract::new` below
        // makes the identical check and raises the identical value, and it is
        // the last thing this function does, so a second check would be one
        // that cannot fail — `P2-A5` measured deleting it changing nothing.
        // The trigger check above is different: it precedes the two
        // extractions, so it decides *which* `BenefitPart` a caller is told
        // about, and `beneficial_trigger_contract` pins that order.
        BenefitContract::new(
            &SubjectId::new(concept)?,
            self.triggers,
            state,
            benefit,
            self.tradeoffs,
        )
    }
}
