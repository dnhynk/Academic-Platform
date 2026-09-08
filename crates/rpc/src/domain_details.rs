//! Closed v3 normal-domain read contract. Imported wire and receipts remain separate.
use crate::{RpcError, details::invalid};
use academic_domain::{
    ArtifactId, AuthorityClass, ClaimId, ContentDigest, DecisionId, DomainId, EntityId,
    EntityIdentityChangeId, EpistemicStatus, EventId, EvidenceId, EvidenceRole, EvidenceStrength,
    FindingId, LectureDocumentId, LectureSessionId, OfferingId, RepositoryId, ScopeId, SnapshotId,
    TranscriptVersionId,
};
use serde::{Deserialize, Serialize};
pub const CAPABILITY: &str = "learning-platform.local.details-domain-read.v3";
pub const PROJECTOR_VERSION: &str = "academic.domain-details.v3";
pub const MAX_INDEX_ENTRIES: usize = 256;
pub const MAX_PROVENANCE_ENTRIES: usize = 512;
pub const MAX_FIELD_REFS: usize = 32;
pub const MAX_EXCERPT_BYTES: usize = 262_144;
mod validation;
pub mod wire;

/// Exact lowercase SHA-256 without a display prefix.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Digest(String);
impl From<ContentDigest> for Digest {
    fn from(value: ContentDigest) -> Self {
        Self(
            value
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect(),
        )
    }
}
impl TryFrom<String> for Digest {
    type Error = RpcError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(invalid("invalid domain digest"));
        }
        Ok(Self(value))
    }
}
impl From<Digest> for String {
    fn from(value: Digest) -> Self {
        value.0
    }
}
impl Digest {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct I64Decimal(String);
impl TryFrom<String> for I64Decimal {
    type Error = RpcError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let number = value
            .parse::<i64>()
            .map_err(|_| invalid("invalid decimal coordinate"))?;
        if number.to_string() != value {
            return Err(invalid("noncanonical decimal coordinate"));
        }
        Ok(Self(value))
    }
}
impl From<i64> for I64Decimal {
    fn from(value: i64) -> Self {
        Self(value.to_string())
    }
}
impl From<I64Decimal> for String {
    fn from(value: I64Decimal) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct U64Decimal(String);
impl TryFrom<String> for U64Decimal {
    type Error = RpcError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        let number = value
            .parse::<u64>()
            .map_err(|_| invalid("invalid decimal coordinate"))?;
        if number.to_string() != value {
            return Err(invalid("noncanonical decimal coordinate"));
        }
        Ok(Self(value))
    }
}
impl From<u64> for U64Decimal {
    fn from(value: u64) -> Self {
        Self(value.to_string())
    }
}
impl From<U64Decimal> for String {
    fn from(value: U64Decimal) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct OpaqueEngineId(String);
impl TryFrom<String> for OpaqueEngineId {
    type Error = RpcError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(invalid("empty engine identifier"));
        }
        Ok(Self(value))
    }
}
impl From<OpaqueEngineId> for String {
    fn from(value: OpaqueEngineId) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Surface {
    Lecture,
    Concept,
    Question,
    Project,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DomainView {
    DomainDetailV3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    DomainProjection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReadFailure {
    CapabilityUnavailable,
    ProfileUnavailable,
    ProfileLocked,
    ProfileMismatch,
    ProjectionUnavailable,
    SourceVerificationFailed,
    SelectorUnavailable,
    SubjectNotFound,
    ResultTooLarge,
    UnsupportedQuery,
    UnsupportedSchemaVersion,
    ContextUnavailable,
    ContextMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MissingReason {
    NoAcceptedSource,
    RegistrationOnly,
    ProducerNotConnected,
    SourceBodyUnavailable,
    UnsupportedSourceVersion,
    InsufficientEvidence,
    UnsupportedPredicate,
    UnsupportedField,
    ResultTooLarge,
    UnrepresentableCoordinate,
    AmbiguousInScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnavailableState {
    Unavailable,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum Field<T> {
    Available {
        value: T,
        provenance_refs: Vec<String>,
    },
    Unavailable {
        reason: MissingReason,
        source_ids: Vec<OriginalId>,
    },
}

impl<T> std::fmt::Debug for Field<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Available {
                provenance_refs, ..
            } => formatter
                .debug_struct("Available")
                .field("provenance_refs", provenance_refs)
                .finish_non_exhaustive(),
            Self::Unavailable { reason, source_ids } => formatter
                .debug_struct("Unavailable")
                .field("reason", reason)
                .field("source_ids", source_ids)
                .finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnavailableField {
    pub state: UnavailableState,
    pub reason: MissingReason,
    pub source_ids: Vec<OriginalId>,
}

impl<T> Field<T> {
    pub fn unavailable(reason: MissingReason, source_ids: Vec<OriginalId>) -> Self {
        Self::Unavailable { reason, source_ids }
    }
    pub fn available(value: T, provenance_refs: Vec<String>) -> Result<Self, RpcError> {
        validation::validate_refs(&provenance_refs, true)?;
        Ok(Self::Available {
            value,
            provenance_refs,
        })
    }
}
impl UnavailableField {
    pub fn new(reason: MissingReason, source_ids: Vec<OriginalId>) -> Self {
        Self {
            state: UnavailableState::Unavailable,
            reason,
            source_ids,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum OriginalId {
    Entity {
        value: EntityId,
    },
    LectureSession {
        value: LectureSessionId,
    },
    TranscriptVersion {
        value: TranscriptVersionId,
    },
    LectureDocument {
        value: LectureDocumentId,
    },
    Snapshot {
        value: SnapshotId,
    },
    Finding {
        value: FindingId,
    },
    Repository {
        value: RepositoryId,
    },
    Claim {
        value: ClaimId,
    },
    Evidence {
        value: EvidenceId,
    },
    Artifact {
        value: ArtifactId,
    },
    Scope {
        value: ScopeId,
    },
    UserDecision {
        value: DecisionId,
    },
    Offering {
        value: OfferingId,
    },
    EntityIdentityChange {
        value: EntityIdentityChangeId,
    },
    EngineDocument {
        value: OpaqueEngineId,
    },
    EngineNode {
        value: OpaqueEngineId,
    },
    EngineRepository {
        value: OpaqueEngineId,
    },
    EngineSnapshot {
        value: OpaqueEngineId,
    },
    EngineFinding {
        value: OpaqueEngineId,
    },
    #[serde(rename = "RAW_SEGMENT")]
    DomainRawSegment {
        value: OpaqueEngineId,
    },
    KnowledgeAssertion {
        value: Digest,
    },
    RawResponse {
        archive_digest: Digest,
        index: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub domain_id: DomainId,
    pub scope_id: ScopeId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainSelector {
    pub view: DomainView,
    pub known_at_accept_seq: Option<u64>,
    pub valid_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Subject {
    Lecture {
        domain_id: DomainId,
        scope_id: ScopeId,
        lecture_session_id: LectureSessionId,
    },
    Concept {
        domain_id: DomainId,
        scope_id: ScopeId,
        entity_id: EntityId,
    },
    Question {
        domain_id: DomainId,
        scope_id: ScopeId,
        entity_id: EntityId,
    },
    Project {
        domain_id: DomainId,
        scope_id: ScopeId,
        entity_id: EntityId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Query {
    Index { surface: Surface },
    Detail { subject: Subject },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainReadRequest {
    DetailsDomainReadV3 {
        context: Context,
        selector: DomainSelector,
        query: Query,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainReadReply {
    Ready {
        version: u16,
        schema_version: u16,
        projection: Box<DomainProjection>,
    },
    Unavailable {
        version: u16,
        schema_version: u16,
        reason: ReadFailure,
        projection: (),
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectionSource {
    pub kind: SourceKind,
    pub projector_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub profile_id: String,
    pub revision: u64,
    pub domain_id: DomainId,
    pub scope_id: ScopeId,
    pub known_at_accept_seq: u64,
    pub valid_at_ms: u64,
    pub source_outbox_seq: u64,
    pub source_ledger_digest: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainProjection {
    pub source: ProjectionSource,
    pub binding: Binding,
    pub read_sources: Vec<ReadSource>,
    pub provenance: Vec<ProvenanceEntry>,
    pub result: QueryResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum QueryResult {
    Index {
        surface: Surface,
        entries: Vec<IndexEntry>,
    },
    Detail {
        detail: Box<DomainDetail>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexEntry {
    pub subject: Subject,
    pub title: Field<String>,
    pub source_ids: Vec<OriginalId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Locator {
    Page {
        page_number: u32,
    },
    TextBytes {
        source_digest: Digest,
        start: u64,
        end: u64,
    },
    TranscriptTime {
        start_ms: u64,
        end_ms: u64,
    },
    RepositoryBytes {
        snapshot_digest: Digest,
        path: String,
        start: u64,
        end: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TextEncoding {
    #[serde(rename = "UTF-8")]
    Utf8,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Excerpt {
    pub text: String,
    pub encoding: TextEncoding,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRef {
    pub evidence_id: EvidenceId,
    pub artifact_id: ArtifactId,
    pub representation_index: u64,
    pub locator: Locator,
    pub excerpt_digest: Digest,
    pub role: EvidenceRole,
    pub strength: EvidenceStrength,
    pub extraction_method: String,
    pub extractor_version: String,
    pub excerpt: Field<Excerpt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum ActorRef {
    User {
        user_id: EntityId,
    },
    Importer {
        name: String,
        version: String,
    },
    DeterministicEngine {
        name: String,
        version: String,
    },
    ModelRun {
        run_id: EntityId,
    },
    DeterministicPrediction {
        name: String,
        version: String,
        frozen_inputs_digest: Digest,
        rule_set_digest: Digest,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginEvent {
    pub event_id: EventId,
    pub origin_seq: U64Decimal,
    pub origin_observed_at_ms: I64Decimal,
    pub actor: ActorRef,
    pub domain_id: DomainId,
    pub accepted_batch_envelope_digest: Digest,
    pub accepted_batch_payload_digest: Digest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthorityPolicy {
    UserOwned,
    OfficialFact,
    ImplementationObservation,
    CuratedRelation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PredicatePolicy {
    pub predicate_id: String,
    pub policy: AuthorityPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyRegistry {
    pub resolver_version: String,
    pub policy_registry_version: String,
    pub policy_registry_hash: Digest,
    pub predicate_policies: Vec<PredicatePolicy>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceCoordinates {
    pub domain_id: DomainId,
    pub known_at_accept_seq: u64,
    pub valid_at_ms: u64,
    pub source_outbox_seq: u64,
    pub source_ledger_digest: Digest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tokenizer {
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VerifiedState {
    Verified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum GraphAvailability {
    Current {},
    Lagging {
        latest_known_at_accept_seq: u64,
        latest_source_outbox_seq: u64,
    },
    Historical {
        current_generation_id: Option<String>,
        latest_known_at_accept_seq: u64,
        latest_source_outbox_seq: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum ReadAuthority {
    CanonicalSnapshot {
        coordinates: SourceCoordinates,
        snapshot_adapter_version: String,
        policy_registry: PolicyRegistry,
        aggregate_source_row_digest: Option<Digest>,
    },
    MaterializedGraph {
        coordinates: SourceCoordinates,
        generation_id: String,
        schema_version: u64,
        builder_binary_digest: Digest,
        algorithm_version: String,
        tokenizer_version: Tokenizer,
        effective_config_hash: Digest,
        effective_configuration: UnavailableField,
        policy_registry: PolicyRegistry,
        built_at_unix_ms: I64Decimal,
        state: VerifiedState,
        record_count: u64,
        canonical_checksum: Digest,
        availability: GraphAvailability,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadSource {
    pub r#ref: String,
    pub authority: ReadAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IndexOrder {
    CanonicalIdAsc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationOrder {
    ClaimIdAsc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistoryOrder {
    AcceptSeqThenId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Direction {
    Incoming,
    Outgoing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistoryTrack {
    Registration,
    Text,
    ConceptLinks,
    Lifecycle,
    Resolution,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Completeness {
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueryCoordinates {
    pub known_at_accept_seq: u64,
    pub valid_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum QuerySelection {
    Index {
        surface: Surface,
        order: IndexOrder,
        max_entries: u64,
    },
    Relations {
        subject: Subject,
        predicate_ids: Vec<String>,
        direction: Direction,
        order: RelationOrder,
        max_entries: u64,
    },
    History {
        subject: Subject,
        track: HistoryTrack,
        through_known_at_accept_seq: u64,
        order: HistoryOrder,
        max_entries: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedQuery {
    pub context: Context,
    pub coordinates: QueryCoordinates,
    pub selection: QuerySelection,
    pub completeness: Completeness,
    pub returned_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphRow {
    pub generation_id: String,
    pub stable_tiebreaker: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum ProvenanceOrigin {
    AcceptedClaim {
        claim_id: ClaimId,
        domain_id: DomainId,
        scope_id: ScopeId,
        authority_class: AuthorityClass,
        epistemic_status: EpistemicStatus,
        origin_event: Field<OriginEvent>,
        accept_seq: u64,
        valid_from_ms: I64Decimal,
        valid_to_ms: Option<I64Decimal>,
        evidence: Vec<EvidenceRef>,
        resolution_policy: AuthorityPolicy,
        graph_row: Option<GraphRow>,
    },
    RegisteredAggregate {
        id: OriginalId,
        domain_id: DomainId,
        scope_id: ScopeId,
        registered_event_id: EventId,
        origin_event: Field<OriginEvent>,
        accept_seq: u64,
        valid_from_ms: I64Decimal,
        valid_to_ms: Option<I64Decimal>,
        source_digest: Option<Digest>,
        parent: Option<OriginalId>,
    },
    VerifiedDomainResult {
        engine_id: String,
        engine_version: String,
        artifact_id: ArtifactId,
        artifact_digest: Digest,
        registration_ref: String,
        evidence: Vec<EvidenceRef>,
        input_refs: Vec<String>,
    },
    VerifiedQuery {
        query: VerifiedQuery,
    },
    AcceptedEvent {
        event: OriginEvent,
        accept_seq: u64,
        scope_id: Option<ScopeId>,
    },
    VerifiedArtifact {
        artifact_id: ArtifactId,
        domain_id: DomainId,
        artifact_digest: Digest,
        format_version: u16,
        media_type: String,
        byte_length: U64Decimal,
        registered_event_ref: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenanceEntry {
    pub r#ref: String,
    pub source_ref: String,
    pub origin: ProvenanceOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RelationTarget {
    Subject(Subject),
    Original(OriginalId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfidenceKind {
    Calibrated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalibratedConfidence {
    pub kind: ConfidenceKind,
    pub permille: u16,
    pub calibration_provenance_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SufficiencyKind {
    EvidenceSufficiency,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SufficiencyGap {
    ConceptLinkUnresolved,
    AuthorshipUnresolved,
    OutcomeUnresolved,
    SourceIntegrityUnresolved,
    SingleSupportingItem,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SufficiencyDeduction {
    pub code: SufficiencyGap,
    pub deduction_permille: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceSufficiency {
    pub kind: SufficiencyKind,
    pub permille: u16,
    pub gaps: Vec<SufficiencyDeduction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserAction {
    Confirm,
    Reject,
    Replace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDecisionRef {
    pub decision_id: DecisionId,
    pub actor: EntityId,
    pub action: UserAction,
    pub decided_at_ms: I64Decimal,
    pub target_claim_id: ClaimId,
    pub replacement_claim_id: Option<ClaimId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationWriteReason {
    CanonicalRelationWriteAdapterMissing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationWrites {
    pub state: UnavailableState,
    pub reason: RelationWriteReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    pub claim_id: ClaimId,
    pub subject: Subject,
    pub predicate_id: String,
    pub target: Field<RelationTarget>,
    pub scope_id: ScopeId,
    pub label: Field<String>,
    pub confidence: Field<CalibratedConfidence>,
    pub provenance_ref: String,
    pub user_decision: Field<UserDecisionRef>,
    pub writes: RelationWrites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GroupKind {
    ConceptCandidates,
    Questions,
    PrerequisiteGaps,
    NextLecturePreparation,
    Assessments,
    EvidenceTimeline,
    Contradictions,
    Prerequisites,
    UsedIn,
    Snu,
    Projects,
    Competencies,
    Roles,
    Architecture,
    Drift,
    StackInventory,
    Observed,
    Required,
    WouldBenefitFrom,
    IssuesIncidents,
    CriticalPath,
    BuildToLearn,
    LearningOptions,
    CompetencyEvidence,
    SnapshotDiff,
    TextHistory,
    ConceptHistory,
    ResolutionHistory,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RelationGroup {
    pub kind: GroupKind,
    pub relations: Field<Vec<Relation>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NodeKind {
    Section,
    Paragraph,
    Equation,
    CodeBlock,
    CapturePlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DocumentAnnotation {
    InstructorEmphasis,
    LowSttConfidence,
    Repetition,
    Example,
    Digression,
    Terminology,
    UnverifiedEquation,
    UnverifiedCode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PreservationTransform {
    OrderPreservation,
    Punctuation,
    SectionHeading,
    Timestamp,
    SpeakerLabel,
    MathAndCodeFormatting,
    TerminologyMarking,
    RepetitionAndEmphasisAnnotation,
    CapturePlacement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CrossReferenceReason {
    InstructorReturnedToEarlierPoint,
    AnswerToEarlierQuestion,
    RecapAtSectionOpen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnseenBasis {
    NoEvidenceRecorded,
    EvidenceRecordedWithoutPromotion,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainTranscriptSegment {
    pub raw_segment_id: OpaqueEngineId,
    pub index: u64,
    pub start_nanos: U64Decimal,
    pub end_nanos: U64Decimal,
    pub audio_frame_seqs: Vec<u64>,
    pub verbatim_text: String,
    pub corrected_text: String,
    pub correction_provenance_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transcript {
    pub transcript_version_id: TranscriptVersionId,
    pub lineage_version: u64,
    pub raw_response_digest: Digest,
    pub input_digest: Digest,
    pub token_digest: Digest,
    pub segments: Vec<DomainTranscriptSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMapping {
    pub raw_segment_id: OpaqueEngineId,
    pub segment_index: u64,
    pub source_char_start: u64,
    pub source_char_end: u64,
    pub covered_tokens: Vec<u64>,
    pub preservation_transform: PreservationTransform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrossReference {
    pub segment_index: u64,
    pub reason: CrossReferenceReason,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentNode {
    pub node_id: OpaqueEngineId,
    pub kind: NodeKind,
    pub rendered_text: String,
    pub annotations: Vec<DocumentAnnotation>,
    pub nearby_capture_frames: Vec<u64>,
    pub mappings: Vec<SourceMapping>,
    pub cross_reference: Field<Option<CrossReference>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub registered_document_id: Field<LectureDocumentId>,
    pub engine_document_id: OpaqueEngineId,
    pub version: u64,
    pub document_digest: Digest,
    pub transcript_token_digest: Digest,
    pub nodes: Vec<DocumentNode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SegmentDisposition {
    Mapped,
    Unmapped,
    ExcludedNonSpeech,
    RedactedWithPolicy,
    UntranscribedFailure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainSegmentAccount {
    pub raw_segment_id: OpaqueEngineId,
    pub token_count: u64,
    pub disposition: SegmentDisposition,
    pub node_ids: Vec<OpaqueEngineId>,
    pub disposition_provenance_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DomainCompletenessWitness {
    pub report_digest: Digest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub report_digest: Digest,
    pub document_digest: Digest,
    pub transcript_token_digest: Digest,
    pub segment_numerator: u64,
    pub segment_denominator: u64,
    pub token_numerator: u64,
    pub token_denominator: u64,
    pub accounts: Vec<DomainSegmentAccount>,
    pub completeness_witness: Field<Option<DomainCompletenessWitness>>,
    pub ordering_findings: UnavailableField,
    pub capture_findings: UnavailableField,
    pub gap_findings: UnavailableField,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capture {
    pub frame_seq: u64,
    pub artifact_id: ArtifactId,
    pub content_digest: Digest,
    pub clock_domain: Digest,
    pub at_nanos: U64Decimal,
    pub mapping_version: u64,
    pub aligned_segment: Field<OpaqueEngineId>,
    pub alignment_error_nanos: Field<U64Decimal>,
    pub text: Field<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReviewKind {
    MarkMoment,
    LowConfidence,
    Equation,
    Code,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewItem {
    pub kind: ReviewKind,
    pub source_id: OriginalId,
    pub raw_segment_id: Field<OpaqueEngineId>,
    pub start_nanos: U64Decimal,
    pub end_nanos: U64Decimal,
    pub audio_frame_seqs: Vec<u64>,
    pub label: Field<String>,
    pub confidence: Field<CalibratedConfidence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Playback {
    UnavailableDomainMediaAdapter,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OriginalMedia {
    pub artifact_id: ArtifactId,
    pub digest: Digest,
    pub media_type: String,
    pub byte_length: U64Decimal,
    pub playback: Playback,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeConfirmation {
    pub user_id: EntityId,
    pub confirmed_at_ms: I64Decimal,
    pub provenance_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fluency {
    pub distinct_contexts: u64,
    pub confirmed_at_ms: I64Decimal,
    pub provenance_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnowledgeState {
    pub assertion_digest: Digest,
    pub version: u64,
    pub supersedes: Option<Digest>,
    pub as_of_ms: I64Decimal,
    pub level: academic_domain::MasteryLevel,
    pub sufficiency: EvidenceSufficiency,
    pub unseen_basis: Field<Option<UnseenBasis>>,
    pub confirmation: Field<Option<KnowledgeConfirmation>>,
    pub fluency: Field<Option<Fluency>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Freshness {
    pub band: academic_domain::FreshnessBand,
    pub confidence_permille: u16,
    pub as_of_ms: I64Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StrongEvidence {
    pub evidence_id: EvidenceId,
    pub at_ms: I64Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionStatus {
    Open,
    PartiallyResolved,
    Resolved,
    Reframed,
    Obsolete,
    Reopened,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum QuestionOrigin {
    Repository {
        entity_id: EntityId,
        snapshot_digest: Digest,
        path: String,
        line: u64,
    },
    Lecture {
        entity_id: EntityId,
        context_locator: String,
    },
    CourseMaterial {
        entity_id: EntityId,
        context_locator: String,
    },
    Assignment {
        entity_id: EntityId,
        context_locator: String,
    },
    PersonalStudy {
        entity_id: EntityId,
        context_locator: String,
    },
    CodeReview {
        entity_id: EntityId,
        context_locator: String,
    },
    ProjectSpec {
        entity_id: EntityId,
        context_locator: String,
    },
    ConceptDetail {
        entity_id: EntityId,
        context_locator: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Inbox {
    New,
    Seen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Importance {
    UserSet,
    ContextDerived,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoalContext {
    pub goal: OriginalId,
    pub relevance: String,
    pub rank: u64,
    pub provenance_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpcomingContext {
    pub source: OriginalId,
    pub at_ms: I64Decimal,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionRevision {
    pub at_ms: I64Decimal,
    pub previous_text: String,
    pub replacement_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConceptHistory {
    pub known_at_accept_seq: u64,
    pub at_ms: I64Decimal,
    pub concept_claim_ids: Vec<ClaimId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionLifecycle {
    pub from: QuestionStatus,
    pub to: QuestionStatus,
    pub at_ms: I64Decimal,
    pub evidence_ids: Vec<EvidenceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuestionResolution {
    pub decision_id: DecisionId,
    pub target_claim_id: ClaimId,
    pub evidence_ids: Vec<EvidenceId>,
    pub authority_provenance_refs: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CommitAlgorithm {
    Sha1,
    Sha256,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commit {
    pub algorithm: CommitAlgorithm,
    pub hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirtySnapshot {
    pub patch_digest: Digest,
    pub tracked_paths: Vec<String>,
    pub untracked_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepositorySnapshot {
    pub canonical_snapshot_id: Field<SnapshotId>,
    pub engine_snapshot_id: OpaqueEngineId,
    pub canonical_repository_id: Field<RepositoryId>,
    pub engine_repository_id: OpaqueEngineId,
    pub branch: Field<Option<String>>,
    pub commit: Field<Option<Commit>>,
    pub captured_at_ms: I64Decimal,
    pub manifest_digest: Digest,
    pub dirty: Field<Option<DirtySnapshot>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectGoal {
    pub id: OriginalId,
    pub text: String,
    pub success_criteria: Vec<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ByteExcerpt {
    pub start: u64,
    pub end: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFile {
    pub path: String,
    pub content_digest: Digest,
    pub byte_length: U64Decimal,
    pub source_excerpt: Field<ByteExcerpt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnalyzeReason {
    AnalysisDispatchNotConnected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Analyze {
    pub state: UnavailableState,
    pub reason: AnalyzeReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProviderPreviewReason {
    PolicyStagingNotConnected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderPreview {
    pub state: UnavailableState,
    pub reason: ProviderPreviewReason,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum DomainDetail {
    Lecture {
        subject: Subject,
        title: Field<String>,
        entity_id: Field<EntityId>,
        original_ids: Vec<OriginalId>,
        transcript: Field<Transcript>,
        document: Field<Document>,
        coverage: Field<Coverage>,
        captures: Field<Vec<Capture>>,
        review: Field<Vec<ReviewItem>>,
        original_media: Field<OriginalMedia>,
        relation_groups: Vec<RelationGroup>,
    },
    Concept {
        subject: Subject,
        title: Field<String>,
        state: Field<KnowledgeState>,
        freshness: Field<Freshness>,
        last_strong_evidence: Field<Option<StrongEvidence>>,
        relation_groups: Vec<RelationGroup>,
    },
    Question {
        subject: Subject,
        text: Field<String>,
        created_at_ms: Field<I64Decimal>,
        origin: Field<QuestionOrigin>,
        status: Field<QuestionStatus>,
        inbox: Field<Inbox>,
        importance: Field<Importance>,
        goal_context: Field<GoalContext>,
        upcoming_context: Field<UpcomingContext>,
        revisions: Field<Vec<QuestionRevision>>,
        concept_history: Field<Vec<ConceptHistory>>,
        lifecycle: Field<Vec<QuestionLifecycle>>,
        resolution: Field<Option<QuestionResolution>>,
        relation_groups: Vec<RelationGroup>,
    },
    Project {
        subject: Subject,
        title: Field<String>,
        goals: Field<Vec<ProjectGoal>>,
        analyzed_snapshot: Field<RepositorySnapshot>,
        current_snapshot: Field<RepositorySnapshot>,
        relation_groups: Vec<RelationGroup>,
        files: Field<Vec<SourceFile>>,
        analyze: Analyze,
        provider_preview: ProviderPreview,
    },
}

impl std::fmt::Debug for Excerpt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Excerpt").finish_non_exhaustive()
    }
}
impl std::fmt::Debug for DomainTranscriptSegment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DomainTranscriptSegment")
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for DocumentNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DocumentNode")
            .finish_non_exhaustive()
    }
}
impl std::fmt::Debug for ByteExcerpt {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ByteExcerpt")
            .finish_non_exhaustive()
    }
}
