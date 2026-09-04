//! The GitHub connector: read-only, repository-scoped, and a second grant for
//! a private blob.
//!
//! `P2-R1` fixed the *credential*: [`TokenScope`] names one repository by exact
//! equality, [`TokenPermission`] has three variants and no write variant, and
//! `FineGrainedToken::authorize` refuses an expired, out-of-scope or
//! under-permissioned token. What this module adds is the *operation* half --
//! the closed set of things the connector can ask GitHub for, and the proof
//! that no member of it writes.
//!
//! ## How "read-only" is established
//!
//! Not by a list of forbidden verbs. Three whole-set properties, each of which
//! a write would have to defeat:
//!
//! * every request this connector can build comes from one
//!   [`GitHubOperation`], and [`GitHubOperation::method`] is a total `match`
//!   returning [`HttpMethod`], whose only variant is `Get`. A `Post` variant
//!   added later has to be *returned by some arm* to matter, and
//!   `github_connector_is_read_only_and_scoped` walks
//!   [`GitHubOperation::ALL`];
//! * [`ReadRequest`] has no body. Its whole field set is compared against a
//!   pinned inventory by `the_read_request_carries_no_body`, so a `body` field
//!   fails as an added key rather than as a spotted name;
//! * every public signature in this crate is inventoried, and the inventory is
//!   compared in both directions. A second request builder appears as an extra
//!   entry even if it spells nothing suspicious.
//!
//! A webhook is the inbound half and produces no request at all: a
//! [`WebhookDelivery`] holds `P2-G5`'s `Untrusted<IngestedDocument>` and hands
//! back no operation.
//!
//! ## Why a private blob needs a second grant
//!
//! Section 33's GitHub row says the connector keeps repository and snapshot
//! metadata and that private blob egress is separate. Separate here means a
//! second `P2-G1` grant, minted from its own [`PermissionRequest`] with its own
//! purpose and consumed through the broker before any byte reaches the
//! transport. [`BlobVisibility::required_grants`] is a total function, and
//! [`PrivateBlobEgress::transmit`] refuses a private blob presented with one
//! grant -- or with the same grant twice -- before it stages anything.
//!
//! [`PermissionRequest`]: academic_policy::PermissionRequest
//! [`TokenScope`]: academic_repository::TokenScope
//! [`TokenPermission`]: academic_repository::TokenPermission

use academic_egress_boundary::{
    EgressProxy, OutboundTransport, StagedGrantJournal, StagedPayload, Transmission,
    TransmissionPlan,
};
use academic_policy::{
    BrokerError, CapabilityToken, PermissionBroker, ReasonCode, RuntimeToolCall,
};
use academic_repository::{FineGrainedToken, GitHubError, GitHubRepository, TokenPermission};
use academic_untrusted_content::{
    IngestError, IngestedDocument, SourceId, SourceKind, Untrusted, ingest,
};

/// Why a connector call was refused.
///
/// `Broker` carries `P2-G1`'s own error, which is neither `Clone` nor `Eq`
/// because a store failure is not a value to be compared, so this enum is not
/// either. What a test compares is [`ConnectorError::reason`], which is the
/// closed code.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConnectorError {
    /// `P2-R1`'s credential check refused.
    #[error("the credential was refused: {0}")]
    Credential(GitHubError),
    /// `P2-G5`'s ingest point refused the delivered bytes.
    #[error("the delivery was refused: {0}")]
    Delivery(IngestError),
    /// A policy decision refused the transfer. No byte left.
    #[error("the transfer was refused: {}", .0.reason().as_str())]
    Denied(BlobDenial),
    /// The broker's own store failed. No capability was consumed.
    #[error("the permission store failed: {0}")]
    Broker(BrokerError),
}

impl ConnectorError {
    /// The closed reason code, when the refusal was a decision.
    #[must_use]
    pub const fn reason(&self) -> Option<ReasonCode> {
        match self {
            Self::Denied(denial) => Some(denial.reason()),
            Self::Credential(_) | Self::Delivery(_) | Self::Broker(_) => None,
        }
    }
}

/// A refusal to move a blob, with the closed reason code and nothing else.
///
/// Three fields and no payload, for the reason `academic-egress-boundary`'s
/// `EgressDenial` has four and no payload: a denial that carried the bytes it
/// refused would be a byte-emitting failure path wearing an error type.
/// `a_blob_denial_has_no_payload_field` reads the whole field set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobDenial {
    reason: ReasonCode,
    detail: String,
    bytes_transmitted: usize,
}

impl BlobDenial {
    pub(crate) fn new(reason: ReasonCode, detail: impl Into<String>) -> Self {
        Self {
            reason,
            detail: detail.into(),
            bytes_transmitted: 0,
        }
    }

    /// The closed reason code.
    #[must_use]
    pub const fn reason(&self) -> ReasonCode {
        self.reason
    }

    /// What was refused, in words.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Bytes already handed to a transport. Zero for every refusal this module
    /// makes, because each is made before the transport is reached.
    #[must_use]
    pub const fn bytes_transmitted(&self) -> usize {
        self.bytes_transmitted
    }
}

/// The request method a connector may use.
///
/// One variant. It exists so [`GitHubOperation::method`] returns a *type*
/// rather than a string: an operation that wrote would have to return a variant
/// this enum does not have, and adding one puts it in
/// `github_connector_is_read_only_and_scoped`'s whole-set walk over
/// [`GitHubOperation::ALL`] the moment an arm returns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpMethod {
    /// Read.
    Get,
}

impl HttpMethod {
    /// Exhaustive order.
    pub const ALL: [Self; 1] = [Self::Get];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
        }
    }
}

/// Everything the connector can ask a GitHub repository for.
///
/// Six reads. Each maps onto exactly one of `P2-R1`'s three token permissions,
/// so a request the token does not carry is refused by that crate's check
/// rather than by a second rule here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitHubOperation {
    /// Default branch, visibility and description.
    RepositoryMetadata,
    /// The commit the default branch points at.
    DefaultBranchHead,
    /// The tree at one commit.
    TreeAtCommit,
    /// One blob at one path in one commit.
    BlobAtPath,
    /// Issue bodies, which section 17.1 lists as an input.
    IssueBodies,
    /// Pull-request bodies.
    PullRequestBodies,
}

impl GitHubOperation {
    /// Exhaustive order.
    pub const ALL: [Self; 6] = [
        Self::RepositoryMetadata,
        Self::DefaultBranchHead,
        Self::TreeAtCommit,
        Self::BlobAtPath,
        Self::IssueBodies,
        Self::PullRequestBodies,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RepositoryMetadata => "REPOSITORY_METADATA",
            Self::DefaultBranchHead => "DEFAULT_BRANCH_HEAD",
            Self::TreeAtCommit => "TREE_AT_COMMIT",
            Self::BlobAtPath => "BLOB_AT_PATH",
            Self::IssueBodies => "ISSUE_BODIES",
            Self::PullRequestBodies => "PULL_REQUEST_BODIES",
        }
    }

    /// The `P2-R1` permission this operation needs.
    #[must_use]
    pub const fn permission(self) -> TokenPermission {
        match self {
            Self::RepositoryMetadata | Self::DefaultBranchHead => TokenPermission::MetadataRead,
            Self::TreeAtCommit | Self::BlobAtPath => TokenPermission::ContentsRead,
            Self::IssueBodies | Self::PullRequestBodies => TokenPermission::IssuesRead,
        }
    }

    /// The request method. Exhaustive, and every arm is [`HttpMethod::Get`].
    ///
    /// This is the read-only claim in the same shape `P2-R1` gave
    /// `TokenPermission::access`: a total function over the enum rather than a
    /// search for forbidden spellings, so an operation added without an arm is
    /// a compile error.
    #[must_use]
    pub const fn method(self) -> HttpMethod {
        match self {
            Self::RepositoryMetadata
            | Self::DefaultBranchHead
            | Self::TreeAtCommit
            | Self::BlobAtPath
            | Self::IssueBodies
            | Self::PullRequestBodies => HttpMethod::Get,
        }
    }

    /// The path suffix under the repository, without a leading slash.
    #[must_use]
    pub const fn resource(self) -> &'static str {
        match self {
            Self::RepositoryMetadata => "",
            Self::DefaultBranchHead => "commits/HEAD",
            Self::TreeAtCommit => "git/trees",
            Self::BlobAtPath => "contents",
            Self::IssueBodies => "issues",
            Self::PullRequestBodies => "pulls",
        }
    }
}

/// One authorized read, as a value.
///
/// Three fields and no body. The method is a type rather than a string and the
/// path is built from the token's own repository, so neither can be supplied by
/// a caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadRequest {
    operation: GitHubOperation,
    method: HttpMethod,
    path: String,
}

impl ReadRequest {
    /// Which operation this is.
    #[must_use]
    pub const fn operation(&self) -> GitHubOperation {
        self.operation
    }

    /// The method. Always [`HttpMethod::Get`], because that is the only variant.
    #[must_use]
    pub const fn method(&self) -> HttpMethod {
        self.method
    }

    /// The resource path, always rooted at the one scoped repository.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }
}

/// The read-only connector for one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubConnector {
    repository: GitHubRepository,
}

impl GitHubConnector {
    /// Binds the connector to exactly one repository.
    #[must_use]
    pub const fn new(repository: GitHubRepository) -> Self {
        Self { repository }
    }

    /// The one repository this connector can reach.
    #[must_use]
    pub const fn repository(&self) -> &GitHubRepository {
        &self.repository
    }

    /// Authorizes `token` for `operation` and builds the read.
    ///
    /// `P2-R1`'s `FineGrainedToken::authorize` runs first and checks expiry,
    /// scope and permission in that fixed order, so a refusal says which
    /// property failed. The path is then built from *this connector's*
    /// repository rather than from anything the caller passed, which is the
    /// scoped half: there is no argument through which another repository could
    /// enter.
    ///
    /// # Errors
    ///
    /// [`ConnectorError::Credential`] carrying `P2-R1`'s own error.
    pub fn read(
        &self,
        token: &FineGrainedToken,
        operation: GitHubOperation,
        now: u64,
    ) -> Result<ReadRequest, ConnectorError> {
        token
            .authorize(&self.repository, operation.permission(), now)
            .map_err(ConnectorError::Credential)?;
        let base = format!(
            "/repos/{}/{}",
            self.repository.owner(),
            self.repository.name()
        );
        let resource = operation.resource();
        let path = if resource.is_empty() {
            base
        } else {
            format!("{base}/{resource}")
        };
        Ok(ReadRequest {
            operation,
            method: operation.method(),
            path,
        })
    }
}

/// The webhook events this system accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WebhookEventKind {
    /// A push to a branch.
    Push,
    /// A pull request opened, updated or closed.
    PullRequest,
    /// An issue opened, updated or closed.
    Issues,
}

impl WebhookEventKind {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [Self::Push, Self::PullRequest, Self::Issues];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Push => "PUSH",
            Self::PullRequest => "PULL_REQUEST",
            Self::Issues => "ISSUES",
        }
    }

    /// The `P2-G5` source kind a body of this event is tagged with.
    ///
    /// No seventh `SourceKind` arm is added: a push body carries commit
    /// messages, which are prose lifted out of a repository, and the other two
    /// carry issue and pull-request bodies, which is the arm `P2-G5` fixed for
    /// exactly that.
    #[must_use]
    pub const fn source_kind(self) -> SourceKind {
        match self {
            Self::Push => SourceKind::Readme,
            Self::PullRequest | Self::Issues => SourceKind::Issue,
        }
    }
}

/// One webhook delivery, held as untrusted content.
///
/// The inbound half of the connector. It builds no [`ReadRequest`] and reaches
/// no transport: what it produces is `P2-G5`'s sealed wrapper, whose one
/// accessor is private to that crate.
#[derive(Debug)]
pub struct WebhookDelivery {
    kind: WebhookEventKind,
    repository: GitHubRepository,
    delivery_id: SourceId,
    body: Untrusted<IngestedDocument>,
}

impl WebhookDelivery {
    /// Admits one delivery through `P2-G5`'s parse-time tagging point.
    ///
    /// # Errors
    ///
    /// [`ConnectorError::Delivery`] when the bytes are oversize or not UTF-8.
    pub fn accept(
        kind: WebhookEventKind,
        repository: GitHubRepository,
        delivery_id: SourceId,
        ingest_seq: u64,
        bytes: &[u8],
    ) -> Result<Self, ConnectorError> {
        let body = ingest(delivery_id.clone(), kind.source_kind(), ingest_seq, bytes)
            .map_err(ConnectorError::Delivery)?;
        Ok(Self {
            kind,
            repository,
            delivery_id,
            body,
        })
    }

    /// Which event this was.
    #[must_use]
    pub const fn kind(&self) -> WebhookEventKind {
        self.kind
    }

    /// Which repository sent it.
    #[must_use]
    pub const fn repository(&self) -> &GitHubRepository {
        &self.repository
    }

    /// The delivery identifier.
    #[must_use]
    pub const fn delivery_id(&self) -> &SourceId {
        &self.delivery_id
    }

    /// The body, still sealed.
    #[must_use]
    pub const fn body(&self) -> &Untrusted<IngestedDocument> {
        &self.body
    }
}

/// Whether a repository's bytes are public.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BlobVisibility {
    /// Anyone can already read these bytes.
    Public,
    /// Only the repository's members can. Section 33 makes egress of these
    /// separate.
    Private,
}

impl BlobVisibility {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Public, Self::Private];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Private => "PRIVATE",
        }
    }

    /// How many distinct `P2-G1` grants a transfer of these bytes needs.
    ///
    /// A total function over the enum, so a visibility added later has no arm
    /// and the crate stops compiling rather than defaulting to one.
    #[must_use]
    pub const fn required_grants(self) -> u8 {
        match self {
            Self::Public => 1,
            Self::Private => 2,
        }
    }
}

/// One blob taken out of a repository, and how visible its source is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryBlob {
    repository: GitHubRepository,
    path: String,
    visibility: BlobVisibility,
}

impl RepositoryBlob {
    /// Records where a blob came from and how visible that place is.
    #[must_use]
    pub fn new(
        repository: GitHubRepository,
        path: impl Into<String>,
        visibility: BlobVisibility,
    ) -> Self {
        Self {
            repository,
            path: path.into(),
            visibility,
        }
    }

    /// The repository.
    #[must_use]
    pub const fn repository(&self) -> &GitHubRepository {
        &self.repository
    }

    /// The path inside it.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// How visible the source is.
    #[must_use]
    pub const fn visibility(&self) -> BlobVisibility {
        self.visibility
    }
}

/// What one completed blob transfer spent.
///
/// The disclosure grant is reported rather than assumed, so
/// `private_blob_egress_needs_a_second_grant` reads the identifier of the
/// second grant that was actually consumed and compares it with the one the
/// broker minted -- the shape `P2-G2`'s `eg04` row uses, where a discarded
/// identifier let two records agree only because the fixture put the same
/// value in both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlobTransfer {
    transmission: Transmission,
    disclosure_grant_id: Option<String>,
}

impl BlobTransfer {
    /// What `P2-G2` handed the transport.
    #[must_use]
    pub const fn transmission(&self) -> &Transmission {
        &self.transmission
    }

    /// The second grant this transfer consumed, when the blob required one.
    #[must_use]
    pub fn disclosure_grant_id(&self) -> Option<&str> {
        self.disclosure_grant_id.as_deref()
    }
}

/// The one path by which a repository blob leaves this machine.
///
/// It holds `P2-G1`'s broker and `P2-G2`'s proxy and adds nothing to either:
/// the staging, the scan, the preview and the capability boundary are all that
/// crate's. What it adds is the disclosure decision that has to exist *before*
/// any of them runs for a private blob.
#[derive(Debug)]
pub struct PrivateBlobEgress<'broker> {
    broker: &'broker PermissionBroker,
    proxy: &'broker EgressProxy<'broker>,
}

impl<'broker> PrivateBlobEgress<'broker> {
    /// Binds the broker whose grants are spent and the proxy that stages.
    #[must_use]
    pub const fn new(
        broker: &'broker PermissionBroker,
        proxy: &'broker EgressProxy<'broker>,
    ) -> Self {
        Self { broker, proxy }
    }

    /// Consumes the disclosure grant a private blob requires.
    ///
    /// The first statement of [`PrivateBlobEgress::transmit`], for the reason
    /// `bind_grant` is the first statement of both of `P2-G2`'s transmit paths:
    /// the refusals below have to be made before a byte can reach a transport,
    /// and a second path that skipped this would be a private blob leaving
    /// under one grant. `the_disclosure_is_bound_once` counts this function's
    /// call sites over the whole package.
    ///
    /// Three refusals, in this order:
    ///
    /// 1. a private blob with no disclosure token at all is `NO_GRANT`;
    /// 2. a disclosure token naming the same grant as the egress token is
    ///    `SCOPE_MISMATCH` -- one decision presented twice is one decision;
    /// 3. whatever the broker itself refuses when the token is spent.
    ///
    /// A public blob needs no disclosure and consumes nothing here, which is
    /// what makes the refusals above attributable to visibility rather than to
    /// a gate that refuses everything.
    #[expect(
        clippy::too_many_arguments,
        reason = "the arguments are the two capabilities, the staged bytes, the plan and the disclosure purpose; folding them into a struct would put the refusals below behind a constructor this function is the only caller of"
    )]
    fn bind_disclosure(
        &self,
        blob: &RepositoryBlob,
        staged: &StagedPayload,
        egress: &CapabilityToken,
        disclosure: Option<&CapabilityToken>,
        plan: &TransmissionPlan<'_>,
        disclosure_purpose_id: &str,
        now: u64,
    ) -> Result<Option<String>, ConnectorError> {
        if blob.visibility.required_grants() < 2 {
            return Ok(None);
        }
        let Some(disclosure) = disclosure else {
            return Err(ConnectorError::Denied(BlobDenial::new(
                ReasonCode::NoGrant,
                format!(
                    "a {} blob needs {} grants and one was supplied",
                    blob.visibility.as_str(),
                    blob.visibility.required_grants()
                ),
            )));
        };
        if disclosure.grant_id() == egress.grant_id() {
            return Err(ConnectorError::Denied(BlobDenial::new(
                ReasonCode::ScopeMismatch,
                format!(
                    "the disclosure and the transfer both name grant {}",
                    egress.grant_id()
                ),
            )));
        }
        let grant_id = disclosure.grant_id().to_owned();
        let range = staged.object_range().map_err(|error| {
            broker_refusal(error, "the staged payload has no valid grant range")
        })?;
        let call = RuntimeToolCall::new(
            plan.actor_id,
            plan.process_class,
            plan.operation,
            disclosure_purpose_id,
            plan.destination_id,
            vec![range],
            staged.preview().bytes(),
        )
        .map_err(|error| {
            broker_refusal(
                error,
                "the disclosure call is outside the process class the capability allows",
            )
        })?;
        self.broker
            .execute(disclosure, call, now, |_authorized| ())
            .map_err(|error| {
                broker_refusal(error, "the capability boundary refused the disclosure")
            })?;
        Ok(Some(grant_id))
    }

    /// Transmits a repository blob, refusing a private one that holds one grant.
    ///
    /// # Errors
    ///
    /// [`ConnectorError::Denied`] with `NO_GRANT` when a private blob has no
    /// disclosure token, with `SCOPE_MISMATCH` when the disclosure names the
    /// transfer's own grant, and with whatever `P2-G1` and `P2-G2` refuse
    /// otherwise. Zero bytes are written on every one of them.
    #[expect(
        clippy::too_many_arguments,
        reason = "the arguments are the two capabilities and P2-G2's own transmit signature; folding them into a struct would hide which token is which"
    )]
    pub fn transmit<T: OutboundTransport>(
        &self,
        blob: &RepositoryBlob,
        staged: &StagedPayload,
        egress: &CapabilityToken,
        disclosure: Option<&CapabilityToken>,
        disclosure_purpose_id: &str,
        plan: &TransmissionPlan<'_>,
        journal: &mut StagedGrantJournal,
        transport: &mut T,
        now: &dyn Fn() -> u64,
    ) -> Result<BlobTransfer, ConnectorError> {
        let disclosure_grant_id = self.bind_disclosure(
            blob,
            staged,
            egress,
            disclosure,
            plan,
            disclosure_purpose_id,
            now(),
        )?;
        let transmission = self
            .proxy
            .transmit(egress, staged, plan, journal, transport, now)
            .map_err(|error| match error {
                academic_egress_boundary::EgressError::Denied(denial) => {
                    ConnectorError::Denied(BlobDenial {
                        reason: denial.reason(),
                        detail: denial.detail().to_owned(),
                        bytes_transmitted: denial.bytes_transmitted(),
                    })
                }
                academic_egress_boundary::EgressError::Broker(broker) => {
                    ConnectorError::Broker(broker)
                }
            })?;
        Ok(BlobTransfer {
            transmission,
            disclosure_grant_id,
        })
    }
}

fn broker_refusal(error: BrokerError, context: &str) -> ConnectorError {
    match error {
        BrokerError::Denied(reason) => {
            ConnectorError::Denied(BlobDenial::new(reason, context.to_owned()))
        }
        BrokerError::InvalidRuntimeCall => ConnectorError::Denied(BlobDenial::new(
            ReasonCode::ScopeMismatch,
            context.to_owned(),
        )),
        other => ConnectorError::Broker(other),
    }
}
