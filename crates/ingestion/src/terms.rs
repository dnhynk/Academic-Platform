//! Stage two: the policy and terms check, and what a denial offers instead.
//!
//! Section 29.5 names the four things this system does when a source may not be
//! collected the way a connector wanted: *manual paste, a user export, saving
//! it from the browser yourself, low-frequency manual sync*. [`Fallback`] is
//! that list, [`Denial`] carries the whole of it, and [`deny`] is the only
//! constructor — its text is pinned whole, and the two fields no other
//! expression sets are counted, so a denial with a shorter list or another route
//! is a change to a pinned constant rather than an oversight. The struct's
//! fields are private to this module, so a second construction site anywhere
//! else is a compile error.
//!
//! None of the four is a module that drives a browser. Three are things the
//! user does and hands over; the fourth is a person running a sync. That is the
//! whole of what Phase 2 ships, and [`crate::gate::OpenGate::SourceTermsAndRateLimits`]
//! and [`crate::gate::OpenGate::ManualExportVersusAssistedCapture`] are the two
//! cells that stay empty until the user decides.

use crate::identifier::ConnectorId;

/// What is known about a source's robots file, terms, and rate limits.
///
/// A connector with no entry in the ledger is [`Self::Unreviewed`], not
/// permitted. `GATE-38-020` is open for exactly the sources whose terms nobody
/// has read yet, and an absent record is what "open" looks like from here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TermsStatus {
    /// A recorded review permits the declared access method at the declared
    /// frequency.
    PermittedForDeclaredMethod,
    /// Nobody has recorded a review. This is the default and it denies.
    Unreviewed,
    /// A recorded review refuses the declared access method.
    Refused,
    /// A review permitted it and the permission was withdrawn.
    Revoked,
}

impl TermsStatus {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::PermittedForDeclaredMethod,
        Self::Unreviewed,
        Self::Refused,
        Self::Revoked,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PermittedForDeclaredMethod => "PERMITTED_FOR_DECLARED_METHOD",
            Self::Unreviewed => "UNREVIEWED",
            Self::Refused => "REFUSED",
            Self::Revoked => "REVOKED",
        }
    }

    /// Whether a fetch may proceed under this status.
    #[must_use]
    pub const fn permits_a_fetch(self) -> bool {
        matches!(self, Self::PermittedForDeclaredMethod)
    }
}

/// What a denied source offers instead, from section 29.5.
///
/// Each is an action a person takes. None of them is automation, and none of
/// them presents a credential this system holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fallback {
    /// The user pastes the text.
    ManualPaste,
    /// The user supplies an export the source gave them.
    UserProvidedExport,
    /// The user saves the document from their own browser and imports the file.
    SaveFromYourOwnBrowser,
    /// A person runs the sync, infrequently, by hand.
    LowFrequencyManualSync,
}

impl Fallback {
    /// Section 29.5's order, which is the order the sentence lists them in.
    pub const ALL: [Self; 4] = [
        Self::ManualPaste,
        Self::UserProvidedExport,
        Self::SaveFromYourOwnBrowser,
        Self::LowFrequencyManualSync,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ManualPaste => "MANUAL_PASTE",
            Self::UserProvidedExport => "USER_PROVIDED_EXPORT",
            Self::SaveFromYourOwnBrowser => "SAVE_FROM_YOUR_OWN_BROWSER",
            Self::LowFrequencyManualSync => "LOW_FREQUENCY_MANUAL_SYNC",
        }
    }
}

/// Where a denied collection goes next.
///
/// One value. `academic-egress-boundary`'s `EgressDenial::route` is the same
/// shape and for the same reason: a route that could name a retry, a different
/// credential, or another way in is the shape a bypass is written as.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DenialRoute {
    /// Offer the person the fallbacks and stop.
    ManualOrStop,
}

/// Why a collection was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DenialReason {
    /// The terms have not been reviewed.
    TermsUnreviewed,
    /// A recorded review refuses this access method.
    TermsRefuse,
    /// A permission was withdrawn, possibly during the run.
    TermsRevoked,
    /// The declared cadence does not permit a fetch yet.
    TooSoon,
    /// The connector's declaration is past its verification date.
    DeclarationOverdue,
    /// The target is not one this connector declares.
    UndeclaredTarget,
}

impl DenialReason {
    /// Exhaustive listing.
    pub const ALL: [Self; 6] = [
        Self::TermsUnreviewed,
        Self::TermsRefuse,
        Self::TermsRevoked,
        Self::TooSoon,
        Self::DeclarationOverdue,
        Self::UndeclaredTarget,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TermsUnreviewed => "TERMS_UNREVIEWED",
            Self::TermsRefuse => "TERMS_REFUSE",
            Self::TermsRevoked => "TERMS_REVOKED",
            Self::TooSoon => "TOO_SOON",
            Self::DeclarationOverdue => "DECLARATION_OVERDUE",
            Self::UndeclaredTarget => "UNDECLARED_TARGET",
        }
    }
}

/// A refused collection, with what is offered instead.
///
/// It is an `Error` so a caller can propagate it, and its `Display` names the
/// connector and the reason. It deliberately does not print the fallbacks: they
/// are a value a caller reads and shows, not a sentence in a log line.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{connector} refused: {}", .reason.as_str())]
pub struct Denial {
    connector: ConnectorId,
    reason: DenialReason,
    route: DenialRoute,
    fallbacks: [Fallback; 4],
    connector_disabled: bool,
}

impl Denial {
    /// Which connector was refused.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// Why.
    #[must_use]
    pub const fn reason(&self) -> DenialReason {
        self.reason
    }

    /// Where it goes next. Always [`DenialRoute::ManualOrStop`].
    #[must_use]
    pub const fn route(&self) -> DenialRoute {
        self.route
    }

    /// What is offered instead. Always the whole of [`Fallback::ALL`].
    #[must_use]
    pub const fn fallbacks(&self) -> &[Fallback] {
        &self.fallbacks
    }

    /// Whether the connector is disabled until a person re-reviews it.
    ///
    /// `IN06`: terms withdrawn during a run disable the connector. A cadence
    /// denial does not — the terms still permit the source, the clock does not
    /// permit the fetch yet.
    #[must_use]
    pub const fn connector_disabled(&self) -> bool {
        self.connector_disabled
    }
}

/// The one constructor for a [`Denial`].
///
/// Whole-text pinned in `tests/ingestion_scans.rs`, and the two field
/// initialisers no other expression writes are counted, because the failure this
/// guards is a denial built somewhere else with an empty fallback list. Every
/// denial carries `Fallback::ALL` and routes to `ManualOrStop`; neither is a
/// parameter.
pub fn deny(connector: ConnectorId, reason: DenialReason) -> Denial {
    Denial {
        connector,
        reason,
        route: DenialRoute::ManualOrStop,
        fallbacks: Fallback::ALL,
        connector_disabled: matches!(
            reason,
            DenialReason::TermsRefuse | DenialReason::TermsRevoked | DenialReason::TermsUnreviewed
        ),
    }
}

/// What a person recorded about each connector's terms, robots file and limits.
///
/// A connector with no record is [`TermsStatus::Unreviewed`]. The ledger is
/// consulted at stage two and again immediately before publication, which is
/// what makes a withdrawal during a run (`IN06`) stop the run rather than the
/// next one.
#[derive(Debug, Clone, Default)]
pub struct TermsLedger {
    recorded: Vec<(ConnectorId, TermsStatus)>,
}

impl TermsLedger {
    /// An empty ledger. Every connector reads as unreviewed.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            recorded: Vec::new(),
        }
    }

    /// Records, or replaces, what a person decided about one connector.
    pub fn record(&mut self, connector: ConnectorId, status: TermsStatus) {
        if let Some(entry) = self
            .recorded
            .iter_mut()
            .find(|(existing, _)| existing == &connector)
        {
            entry.1 = status;
            return;
        }
        self.recorded.push((connector, status));
    }

    /// What the ledger says about one connector.
    #[must_use]
    pub fn status(&self, connector: &ConnectorId) -> TermsStatus {
        self.recorded
            .iter()
            .find(|(existing, _)| existing == connector)
            .map_or(TermsStatus::Unreviewed, |(_, status)| *status)
    }
}
