//! The integrations boundary: every place this product touches something
//! outside itself, and the one rule all of them share.
//!
//! Section 33 says this system is a control plane rather than a suite of
//! reimplemented learning tools, and it fixes three things about the edge:
//!
//! * **a disconnected integration never blocks the core.** The graph and the
//!   ledger open when every connector is down;
//! * **an external identifier is a mapping and never a canonical identifier.**
//!   [`ExternalIdentity`] carries both halves and there is no function anywhere
//!   in this crate that turns the first into the second;
//! * **a sync conflict is resolved by source authority and valid time, and
//!   neither side is silently overwritten.** [`SyncConflict`] holds both.
//!
//! ## What ships, and what does not
//!
//! The GitHub connector, the webhook admission point, the IDE adapter, the
//! coding-assistant handoff, the generated-code provenance record, the calendar
//! payload and the identity map ship. **No transport ships**, the way
//! `academic-egress-boundary` ships none and `academic-repository` ships no
//! `GitHubRepositoryReader`: every seam this crate names is a trait the caller
//! supplies, and every fixture in its test tree is synthetic and built
//! in-process. `only_egress_crate_has_a_socket` in
//! `tools/phase1-scaffold-policy.test.mjs` reads that as the absence of a
//! `SOCKET_ALLOWANCE` entry for this package.
//!
//! ## Where the core read path is
//!
//! Not here. [`IntegrationSurface::read_core`] hands back what the caller's
//! [`CoreGraph`] returns and consults no connector, and this crate has no
//! product edge to the crate that owns the ledger at all -- so a connector
//! failure has nothing to fail *through*.
//! `core_graph_opens_with_every_connector_down` observes the byte-identical
//! result with every [`ConnectorKind`] down and up, and observes the fleet's
//! call count at zero for the whole read.

mod assistant;
mod calendar;
mod github;
mod ide;
mod identity;

pub use assistant::{
    AssistantContext, AssistantError, AssistantSelection, AssistantUse, EvidenceEligibility,
    GeneratedCode,
};
pub use calendar::{CalendarError, CalendarEventKind, CalendarPayload};
pub use github::{
    BlobDenial, BlobTransfer, BlobVisibility, ConnectorError, GitHubConnector, GitHubOperation,
    HttpMethod, PrivateBlobEgress, ReadRequest, RepositoryBlob, WebhookDelivery, WebhookEventKind,
};
pub use ide::{
    ChangedScope, DeepLink, IdeAdapter, IdeError, IdeWorkspace, ScopeConfirmation, SnapshotRequest,
    SymbolRef, WatchMode, WorkspacePath,
};
pub use identity::{
    CanonicalKind, CanonicalRef, ConflictBasis, ExternalId, ExternalIdentity, IdentityError,
    IdentityMap, SourceAuthority, SyncConflict,
};

/// Every external tool section 33's table names.
///
/// The rows are that table's first column, in its order.
/// `every_section_33_row_is_a_connector_kind` parses the table back out of
/// `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares it with
/// [`ConnectorKind::ALL`] in both directions, so a row added to the design
/// document without a variant here fails, and a variant with no row fails too.
/// Nothing in this crate asserts how many there are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectorKind {
    /// A note application, reached by deep link and Markdown import/export.
    NoteTool,
    /// A flashcard application, reached by opt-in export/import.
    FlashcardTool,
    /// A document question-answering provider, reached by scoped handoff.
    DocumentQa,
    /// A reference manager, reached by citation and deep link.
    ReferenceManager,
    /// A learning-management system, reached by official API or user export.
    Lms,
    /// A calendar, reached by controlled event sync.
    Calendar,
    /// A cloud drive holding an encrypted backup or user-selected files.
    CloudDrive,
    /// A GitHub repository, read-only and repository-scoped.
    GitHub,
    /// An editor, reached by local symbol context and deep link.
    Ide,
    /// A coding assistant, reached by explicitly selected context.
    CodingAssistant,
}

impl ConnectorKind {
    /// Exhaustive order, matching section 33's table order.
    pub const ALL: [Self; 10] = [
        Self::NoteTool,
        Self::FlashcardTool,
        Self::DocumentQa,
        Self::ReferenceManager,
        Self::Lms,
        Self::Calendar,
        Self::CloudDrive,
        Self::GitHub,
        Self::Ide,
        Self::CodingAssistant,
    ];

    /// The design document's own spelling for this row.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoteTool => "Note tool",
            Self::FlashcardTool => "Flashcard tool",
            Self::DocumentQa => "Document Q&A",
            Self::ReferenceManager => "Reference manager",
            Self::Lms => "LMS",
            Self::Calendar => "Calendar",
            Self::CloudDrive => "Cloud drive",
            Self::GitHub => "GitHub",
            Self::Ide => "IDE",
            Self::CodingAssistant => "Coding assistant",
        }
    }
}

/// Whether one connector can be reached right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ConnectorHealth {
    /// Reachable.
    Up,
    /// Not reachable. Section 33's rule is that this changes nothing about the
    /// core.
    Down,
}

impl ConnectorHealth {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Up, Self::Down];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Up => "UP",
            Self::Down => "DOWN",
        }
    }
}

/// What a caller has to supply for this boundary to know a connector's state.
///
/// It is a trait rather than a field so the acceptance test can count the times
/// a read consulted it. `core_graph_opens_with_every_connector_down` observes
/// that count at zero over a whole core read.
pub trait ConnectorFleet {
    /// Whether `kind` is reachable.
    fn health(&self, kind: ConnectorKind) -> ConnectorHealth;
}

/// The shipped fleet: one health value per [`ConnectorKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectorRegistry {
    health: [ConnectorHealth; ConnectorKind::ALL.len()],
}

impl ConnectorRegistry {
    /// A registry in which every connector is reachable.
    #[must_use]
    pub const fn all_up() -> Self {
        Self {
            health: [ConnectorHealth::Up; ConnectorKind::ALL.len()],
        }
    }

    /// A registry in which no connector is reachable.
    #[must_use]
    pub const fn all_down() -> Self {
        Self {
            health: [ConnectorHealth::Down; ConnectorKind::ALL.len()],
        }
    }

    /// The same registry with one connector's health replaced.
    #[must_use]
    pub const fn with(mut self, kind: ConnectorKind, health: ConnectorHealth) -> Self {
        self.health[kind as usize] = health;
        self
    }

    /// Every connector that is not reachable, in [`ConnectorKind::ALL`] order.
    #[must_use]
    pub fn unreachable(&self) -> Vec<ConnectorKind> {
        ConnectorKind::ALL
            .into_iter()
            .filter(|kind| ConnectorFleet::health(self, *kind) == ConnectorHealth::Down)
            .collect()
    }
}

impl ConnectorFleet for ConnectorRegistry {
    fn health(&self, kind: ConnectorKind) -> ConnectorHealth {
        self.health[kind as usize]
    }
}

/// One named read of the core graph or the append-only ledger.
///
/// A closed vocabulary rather than a query language, because what this type is
/// for is naming the reads `core_graph_opens_with_every_connector_down` has to
/// exercise as a whole set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoreView {
    /// The accepted-sequence head of the ledger.
    LedgerHead,
    /// Every accepted event, in acceptance order.
    AcceptedEvents,
    /// Every claim the graph holds.
    Claims,
    /// Every evidence item the graph holds.
    Evidence,
    /// Every artifact descriptor the graph holds.
    Artifacts,
}

impl CoreView {
    /// Exhaustive order.
    pub const ALL: [Self; 5] = [
        Self::LedgerHead,
        Self::AcceptedEvents,
        Self::Claims,
        Self::Evidence,
        Self::Artifacts,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LedgerHead => "LEDGER_HEAD",
            Self::AcceptedEvents => "ACCEPTED_EVENTS",
            Self::Claims => "CLAIMS",
            Self::Evidence => "EVIDENCE",
            Self::Artifacts => "ARTIFACTS",
        }
    }
}

/// The core graph and ledger, as this boundary is allowed to see them.
///
/// Read-only by construction: one method, `&self`, and an owned answer. This
/// crate implements it for nothing and stores nothing behind it -- the caller
/// owns the ledger, and `academic-integrations` has no product edge to the
/// crate that holds one.
pub trait CoreGraph {
    /// Reads one named view.
    fn read_view(&self, view: CoreView) -> Vec<u8>;
}

/// What a surface holds: the core it reads, and the connectors it does not read
/// through.
#[derive(Debug)]
pub struct IntegrationSurface<'core, G: CoreGraph + ?Sized, F: ConnectorFleet + ?Sized> {
    core: &'core G,
    fleet: &'core F,
}

impl<'core, G: CoreGraph + ?Sized, F: ConnectorFleet + ?Sized> IntegrationSurface<'core, G, F> {
    /// Binds a core reader and a connector fleet.
    #[must_use]
    pub const fn new(core: &'core G, fleet: &'core F) -> Self {
        Self { core, fleet }
    }

    /// Reads one core view.
    ///
    /// The body is the whole claim: it forwards to the core and touches
    /// `self.fleet` not at all, so no connector's health can decide whether a
    /// core read succeeds. `the_core_read_consults_no_connector` pins this
    /// function as whole text, and
    /// `core_graph_opens_with_every_connector_down` observes the fleet's call
    /// count at zero across every [`CoreView`].
    pub fn read_core(&self, view: CoreView) -> Vec<u8> {
        self.core.read_view(view)
    }

    /// Whether one connector is reachable. The only reader of the fleet.
    pub fn connector_health(&self, kind: ConnectorKind) -> ConnectorHealth {
        self.fleet.health(kind)
    }
}
