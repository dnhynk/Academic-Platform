//! The `Proposed<T>` boundary, and the two types a settled proposal becomes.

use core::fmt;

use academic_domain::{ConfidencePermille, EpistemicStatus};

use crate::tier::RiskTier;

/// Identity of one proposal inside a review queue.
///
/// A caller-chosen opaque identifier. It is the queue's key and it appears in
/// every disposition row, so it is deliberately not derived from the payload:
/// two proposals about the same subject are two entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProposalId(u64);

impl ProposalId {
    /// Wraps a caller-chosen identifier.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// The wrapped identifier.
    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ProposalId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "proposal:{}", self.0)
    }
}

/// How much a proposal changes if it is wrong, in permille.
///
/// Section 29.7 batches a review queue on confidence *and* impact, so impact is
/// a value beside confidence rather than something derived from it. Bounded the
/// same way `ConfidencePermille` is, for the same reason: a band cut is a
/// comparison against a number on a known scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImpactPermille(u16);

/// Rejects an impact value outside 0..=1000.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("impact permille {0} is above 1000")]
pub struct ImpactOutOfRange(pub u16);

impl ImpactPermille {
    /// Constructs an impact value from 0 through 1000 inclusive.
    ///
    /// # Errors
    ///
    /// [`ImpactOutOfRange`] when `value` exceeds 1000.
    pub const fn new(value: u16) -> Result<Self, ImpactOutOfRange> {
        if value > 1000 {
            return Err(ImpactOutOfRange(value));
        }
        Ok(Self(value))
    }

    /// The integer permille representation.
    #[must_use]
    pub const fn value(self) -> u16 {
        self.0
    }
}

/// A model-authored candidate that has not been dispositioned.
///
/// Private fields, and no way to read the payload back out. It implements no
/// `Deref`, `DerefMut`, `AsRef`, `AsMut`, `Borrow`, `Display`, `ToString`,
/// `From` or `Into`, and its one accessor for the payload,
/// [`Proposed::release`], is `pub(crate)` and so cannot be named outside this
/// crate.
///
/// What that buys is narrow and exact, on the terms
/// `docs/contracts/untrusted-content.md` states for `Untrusted<T>`: this crate
/// is free to call `release` on a caller's behalf, so the claim is not "no
/// caller can ever obtain the payload" but "every place the payload comes out
/// is inventoried, and each one has already recorded the disposition that let
/// it out". `every_release_site_is_named_and_justified` is what executes that.
///
/// `Debug` is hand-written, has no `T: Debug` bound, and prints the tier,
/// confidence, impact and identity but no part of the payload -- so there is no
/// instantiation whose payload a format string reaches.
pub struct Proposed<T> {
    id: ProposalId,
    tier: RiskTier,
    confidence: ConfidencePermille,
    impact: ImpactPermille,
    value: T,
}

impl<T> Proposed<T> {
    /// Wraps a candidate at the moment its risk tier is decided.
    #[must_use]
    pub const fn new(
        id: ProposalId,
        tier: RiskTier,
        confidence: ConfidencePermille,
        impact: ImpactPermille,
        value: T,
    ) -> Self {
        Self {
            id,
            tier,
            confidence,
            impact,
            value,
        }
    }

    /// Which proposal this is.
    #[must_use]
    pub const fn id(&self) -> ProposalId {
        self.id
    }

    /// The section 27.4 tier this proposal was classified into.
    #[must_use]
    pub const fn tier(&self) -> RiskTier {
        self.tier
    }

    /// The model's calibrated confidence, as a batching input.
    #[must_use]
    pub const fn confidence(&self) -> ConfidencePermille {
        self.confidence
    }

    /// How much this proposal changes if it is wrong, as a batching input.
    #[must_use]
    pub const fn impact(&self) -> ImpactPermille {
        self.impact
    }

    /// Takes the payload out of the boundary.
    ///
    /// Crate-private, and every call site is inventoried by
    /// `every_release_site_is_named_and_justified` in
    /// `tests/proposal_scans.rs` with a written reason. A fourth site fails
    /// that test as an extra key however it is spelled.
    pub(crate) fn release(self) -> T {
        self.value
    }
}

impl<T> fmt::Debug for Proposed<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Proposed")
            .field("id", &self.id)
            .field("tier", &self.tier)
            .field("confidence_permille", &self.confidence.value())
            .field("impact_permille", &self.impact.value())
            .finish_non_exhaustive()
    }
}

/// A low-risk proposal that was saved without a human.
///
/// Private fields and one producer, [`crate::ReviewQueue::autosave`]. Its
/// epistemic status is not a field: [`Autosaved::EPISTEMIC_STATUS`] is a
/// constant and [`Autosaved::epistemic_status`] returns it, so there is no
/// value a caller could pass that would make an autosaved record anything but
/// `AI_INFERRED`. That is section 27.4's low-risk row expressed as a type
/// rather than as a check a later layer performs.
#[derive(Debug)]
pub struct Autosaved<T> {
    id: ProposalId,
    value: T,
}

impl<T> Autosaved<T> {
    /// The only status an autosaved record ever carries.
    pub const EPISTEMIC_STATUS: EpistemicStatus = EpistemicStatus::AiInferred;

    /// Constructs the record. Crate-private: the queue is the one producer.
    pub(crate) const fn new(id: ProposalId, value: T) -> Self {
        Self { id, value }
    }

    /// Which proposal this record came from.
    #[must_use]
    pub const fn id(&self) -> ProposalId {
        self.id
    }

    /// Always `AI_INFERRED`.
    #[must_use]
    pub const fn epistemic_status(&self) -> EpistemicStatus {
        Self::EPISTEMIC_STATUS
    }

    /// The payload, for a writer that takes an autosaved record.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }
}

/// A proposal a user approved.
///
/// Private fields and two producers, both of which have already recorded the
/// user's decision: [`crate::ReviewQueue::approve`] for the high-approval tier
/// and [`crate::ReviewQueue::commit`] for the other two tiers that need a
/// human. `USER_CONFIRMED` is a constant here for the same reason
/// `AI_INFERRED` is one on [`Autosaved`].
#[derive(Debug)]
pub struct Approved<T> {
    id: ProposalId,
    value: T,
}

impl<T> Approved<T> {
    /// The only status an approved record ever carries.
    pub const EPISTEMIC_STATUS: EpistemicStatus = EpistemicStatus::UserConfirmed;

    /// Constructs the record. Crate-private: the queue is the one producer.
    pub(crate) const fn new(id: ProposalId, value: T) -> Self {
        Self { id, value }
    }

    /// Which proposal this record came from.
    #[must_use]
    pub const fn id(&self) -> ProposalId {
        self.id
    }

    /// Always `USER_CONFIRMED`.
    #[must_use]
    pub const fn epistemic_status(&self) -> EpistemicStatus {
        Self::EPISTEMIC_STATUS
    }

    /// The payload, for a writer that takes an approved record.
    #[must_use]
    pub fn into_inner(self) -> T {
        self.value
    }
}
