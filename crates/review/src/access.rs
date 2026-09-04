//! How the text was obtained, and what a refused source is offered instead.
//!
//! # The three access modes are section 29.5's own
//!
//! The record writes `sourceAccessMode: PUBLIC | USER_PROVIDED_EXPORT |
//! MANUAL_PASTE`. [`SourceAccessMode`] is that union and
//! `the_access_modes_are_section_29_5s_own` reads the line out of the
//! specification and compares the whole set both ways, so a fourth mode fails
//! against the specification rather than passing quietly.
//!
//! Every one of the three is something a person or a public page already
//! offers. [`SourceAccessMode::presents_a_credential`] is `false` for all
//! three, and it is a `match` over the enum rather than a constant, so a fourth
//! arm has to answer the question.
//!
//! # The ledger's default denies
//!
//! `GATE-38-021` — per-source access, storage, analysis and retention rights —
//! is a user and legal decision. [`SourceTermsLedger`] holds what a person
//! recorded per source **and per access mode**, and a pair nobody recorded
//! reads as `academic_ingestion::TermsStatus::Unreviewed`, which permits
//! nothing. That is what "an unconfigured source keeps its connector disabled"
//! looks like from here: not a switch set to off, but the absence of a record.
//!
//! The pair is the key rather than the source alone, because the four
//! fallbacks exist precisely for a source whose terms refuse the collection a
//! connector wanted. A ledger keyed on the source alone would refuse the manual
//! paste that is the remedy.
//!
//! # The denial is `P2-U6`'s
//!
//! [`academic_ingestion::deny`] is the only constructor of a
//! [`academic_ingestion::Denial`], every denial it makes carries the whole of
//! [`academic_ingestion::Fallback::ALL`], and its route is
//! `DenialRoute::ManualOrStop`. This crate reuses it rather than restating the
//! four, so `denied_source_exposes_only_the_four_fallbacks` is a claim about
//! the shipped constructor and not about a copy.

use academic_ingestion::{ConnectorId, Denial, DenialReason, TermsStatus, terms::deny};

/// Section 29.5's `sourceAccessMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SourceAccessMode {
    /// A page anyone can read without presenting anything.
    Public,
    /// A file the user asked the source for and handed over.
    UserProvidedExport,
    /// Text the user pasted.
    ManualPaste,
}

impl SourceAccessMode {
    /// Section 29.5's order, which is the order the record's union lists them.
    pub const ALL: [Self; 3] = [Self::Public, Self::UserProvidedExport, Self::ManualPaste];

    /// The token section 29.5's record writes this mode as.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::UserProvidedExport => "USER_PROVIDED_EXPORT",
            Self::ManualPaste => "MANUAL_PASTE",
        }
    }

    /// Parses the record's spelling.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == value)
    }

    /// Whether this system presents a credential to obtain the text.
    ///
    /// `false` for all three. It is a `match` and not a constant so a fourth
    /// mode is a decision somebody writes down rather than a value it inherits.
    #[must_use]
    pub const fn presents_a_credential(self) -> bool {
        match self {
            Self::Public | Self::UserProvidedExport | Self::ManualPaste => false,
        }
    }

    /// Whether obtaining the text is an act a person performs.
    ///
    /// [`Self::Public`] is the one that is not: reading a public page needs no
    /// person, which is why it is the mode a terms review has to permit before
    /// anything is collected under it.
    #[must_use]
    pub const fn is_a_person_act(self) -> bool {
        matches!(self, Self::UserProvidedExport | Self::ManualPaste)
    }
}

/// What a person recorded about one source, for one access mode.
///
/// A pair with no entry is [`TermsStatus::Unreviewed`]. There is no `insert`
/// that takes a default and no constructor that pre-fills anything: an empty
/// ledger denies every mode of every source.
#[derive(Debug, Clone, Default)]
pub struct SourceTermsLedger {
    recorded: Vec<(ConnectorId, SourceAccessMode, TermsStatus)>,
}

impl SourceTermsLedger {
    /// An empty ledger. Every source reads as unreviewed in every mode.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            recorded: Vec::new(),
        }
    }

    /// Records what a person decided about one source in one mode.
    #[must_use]
    pub fn recording(
        mut self,
        source: ConnectorId,
        mode: SourceAccessMode,
        status: TermsStatus,
    ) -> Self {
        self.recorded
            .retain(|(id, recorded_mode, _)| id != &source || *recorded_mode != mode);
        self.recorded.push((source, mode, status));
        self
    }

    /// What the ledger says about one source in one mode.
    #[must_use]
    pub fn status_of(&self, source: &ConnectorId, mode: SourceAccessMode) -> TermsStatus {
        self.recorded
            .iter()
            .find(|(id, recorded_mode, _)| id == source && *recorded_mode == mode)
            .map_or(TermsStatus::Unreviewed, |(_, _, status)| *status)
    }
}

/// A source and mode a recorded review permits collecting under.
///
/// Private field, and [`permit`] is the only producer. It is what
/// [`crate::record::ReviewRecordDraft::collected`] takes, so a review whose
/// source nobody reviewed is not a value that exists rather than a value a
/// later check refuses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermittedCollection {
    source: ConnectorId,
    mode: SourceAccessMode,
}

impl PermittedCollection {
    /// Which source.
    #[must_use]
    pub const fn source(&self) -> &ConnectorId {
        &self.source
    }

    /// Which mode the recorded review permits.
    #[must_use]
    pub const fn mode(&self) -> SourceAccessMode {
        self.mode
    }
}

/// The one producer of a [`PermittedCollection`].
///
/// Section 29.1 puts the policy and terms check after acquisition, which is
/// what lets a paste arrive without a terms review of a fetch that never
/// happened — but the paste still passes through *this* check, because whether
/// this system may store and analyse somebody else's writing is the question
/// `GATE-38-021` leaves open and it is open for every mode.
///
/// The `match` is total over [`TermsStatus::ALL`] and has no wildcard, so a
/// fifth status is a compile error here rather than a status that silently
/// reads as one of these four. There is no arm that both permits and names a
/// reason: the permitting status is the only one that builds the value, and it
/// is the only construction site of [`PermittedCollection`] in this crate.
///
/// # Errors
///
/// An `academic_ingestion::Denial` carrying the whole of `Fallback::ALL`, for
/// each of the three statuses that are not
/// [`TermsStatus::PermittedForDeclaredMethod`].
pub fn permit(
    ledger: &SourceTermsLedger,
    source: &ConnectorId,
    mode: SourceAccessMode,
) -> Result<PermittedCollection, Denial> {
    match ledger.status_of(source, mode) {
        TermsStatus::PermittedForDeclaredMethod => Ok(PermittedCollection {
            source: source.clone(),
            mode,
        }),
        TermsStatus::Unreviewed => Err(deny(source.clone(), DenialReason::TermsUnreviewed)),
        TermsStatus::Refused => Err(deny(source.clone(), DenialReason::TermsRefuse)),
        TermsStatus::Revoked => Err(deny(source.clone(), DenialReason::TermsRevoked)),
    }
}
