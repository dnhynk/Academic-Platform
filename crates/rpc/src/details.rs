//! Bounded detail workspace DTOs. Requests carry no source paths, actors or arbitrary events.

use crate::RpcError;
use academic_domain::ContentDigest;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const DETAILS_READ_CAPABILITY: &str = "learning-platform.local.details-read.v1";
pub const DETAILS_DECIDE_CAPABILITY: &str = "learning-platform.local.details-decide.v1";
pub const DETAILS_AUDIO_CAPABILITY: &str = "learning-platform.local.details-audio.v1";
pub const DETAILS_CAPABILITIES: &[&str] = &[
    DETAILS_AUDIO_CAPABILITY,
    DETAILS_DECIDE_CAPABILITY,
    DETAILS_READ_CAPABILITY,
];
pub const MAX_DETAIL_BYTES: usize = 1_048_576;
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailSelector {
    pub view: DetailView,
    pub known_at_accept_seq: Option<u64>,
    pub valid_at_ms: Option<u64>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailView {
    DetailWorkspace,
}
impl Default for DetailSelector {
    fn default() -> Self {
        Self {
            view: DetailView::DetailWorkspace,
            known_at_accept_seq: None,
            valid_at_ms: None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailAction {
    Reject,
    Undo,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDecisionRequest {
    pub relation_id: String,
    pub action: DetailAction,
    pub expected_revision: u64,
    pub expected_profile_id: String,
    pub selector: DetailSelector,
    pub request_id: [u8; 16],
    pub client_instance_id: [u8; 16],
    pub idempotency_key: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum DetailRequest {
    DetailsAudio { audio: DetailAudioRequest },
    DetailsRead { selector: DetailSelector },
    DetailsDecide { decision: DetailDecisionRequest },
}
impl DetailRequest {
    pub const fn capability(&self) -> &'static str {
        match self {
            Self::DetailsAudio { .. } => DETAILS_AUDIO_CAPABILITY,
            Self::DetailsRead { .. } => DETAILS_READ_CAPABILITY,
            Self::DetailsDecide { .. } => DETAILS_DECIDE_CAPABILITY,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailReply {
    pub version: u16,
    pub state: DetailReplyState,
    pub message: String,
    pub receipt_id: Option<[u8; 16]>,
    pub decision_sequence: Option<u64>,
    pub receipt_decision: Option<DetailDecisionReceipt>,
    pub reason: Option<String>,
    pub request_id: Option<[u8; 16]>,
    pub client_instance_id: Option<[u8; 16]>,
    pub idempotency_key: Option<[u8; 32]>,
    pub request_digest: Option<[u8; 32]>,
    pub details: Option<DetailState>,
    pub audio: Option<DetailAudio>,
}
/// Original accepted disposition, independent of current source membership.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailDecisionReceipt {
    pub sequence: u64,
    pub relation_id: String,
    pub relation_claim_id: academic_domain::ClaimId,
    pub action: DetailAction,
    pub undoes: Option<u64>,
    pub actor: academic_domain::EntityId,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailAudioRequest {
    pub lecture_id: String,
    pub expected_profile_id: String,
    pub expected_revision: u64,
    pub selector: DetailSelector,
    pub offset: u64,
    pub length: u64,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailAudio {
    pub lecture_id: String,
    pub media_type: String,
    pub content_digest: String,
    pub total_bytes: u64,
    pub offset: u64,
    pub bytes: Vec<u8>,
}
impl std::fmt::Debug for DetailAudio {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DetailAudio")
            .field("byte_len", &self.bytes.len())
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DetailReplyState {
    Ready,
    Accepted,
    Rejected,
    Unavailable,
}
impl DetailReply {
    pub fn validate(&self) -> Result<(), RpcError> {
        if self.version != 1 {
            return Err(invalid("unsupported detail reply version"));
        }
        if let Some(state) = &self.details {
            state.corpus.validate()?;
            let relations = state.corpus.relations()?;
            let mut previous = 0;
            for event in &state.decisions {
                if event.sequence <= previous
                    || event.sequence > state.known_at_accept_seq
                    || !relations.contains_key(event.relation_id.as_str())
                {
                    return Err(invalid("invalid decision history"));
                }
                previous = event.sequence;
            }
        }
        match self.state {
            DetailReplyState::Accepted
                if self.receipt_id.is_none()
                    || self.decision_sequence.is_none()
                    || self.receipt_decision.is_none()
                    || self.request_id.is_none()
                    || self.client_instance_id.is_none()
                    || self.idempotency_key.is_none()
                    || self.request_digest.is_none()
                    || self.details.is_none()
                    || self.audio.is_some() =>
            {
                return Err(invalid("accepted reply lacks durable correlation"));
            }
            DetailReplyState::Ready
                if self.details.is_some() == self.audio.is_some()
                    || self.receipt_id.is_some()
                    || self.receipt_decision.is_some() =>
            {
                return Err(invalid("ready reply must contain one read result"));
            }
            DetailReplyState::Rejected | DetailReplyState::Unavailable
                if self.receipt_id.is_some()
                    || self.decision_sequence.is_some()
                    || self.receipt_decision.is_some()
                    || self.audio.is_some() =>
            {
                return Err(invalid("rejected reply carries accepted state"));
            }
            _ => {}
        }
        if let Some(receipt) = &self.receipt_decision {
            reference(&receipt.relation_id)?;
            if Some(receipt.sequence) != self.decision_sequence
                || receipt.sequence == 0
                || self
                    .details
                    .as_ref()
                    .is_none_or(|state| receipt.sequence > state.known_at_accept_seq)
                || match receipt.action {
                    DetailAction::Reject => receipt.undoes.is_some(),
                    DetailAction::Undo => receipt
                        .undoes
                        .is_none_or(|target| target == 0 || target >= receipt.sequence),
                }
            {
                return Err(invalid("invalid original decision receipt"));
            }
        }
        if let Some(audio) = &self.audio
            && (audio.media_type != "audio/wav"
                || audio.total_bytes > 4_194_304
                || audio.bytes.is_empty()
                || audio.bytes.len() > 4096
                || audio.content_digest.len() != 64
                || !audio
                    .content_digest
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                || audio
                    .offset
                    .checked_add(
                        u64::try_from(audio.bytes.len())
                            .map_err(|_| invalid("audio length overflow"))?,
                    )
                    .is_none_or(|end| end > audio.total_bytes))
        {
            return Err(invalid("invalid audio result"));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailState {
    pub corpus: DetailCorpus,
    pub decisions: Vec<RelationDecision>,
    pub revision: u64,
    pub profile_id: String,
    pub known_at_accept_seq: u64,
    pub valid_at_ms: u64,
    pub projector_version: String,
    pub source_digest: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RelationDecision {
    pub sequence: u64,
    pub relation_id: String,
    pub action: DispositionAction,
    pub undoes: Option<u64>,
    pub actor: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DispositionAction {
    Reject,
    Undo,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailCorpus {
    pub lectures: Vec<Lecture>,
    pub concepts: Vec<Concept>,
    pub questions: Vec<Question>,
    pub projects: Vec<Project>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub title: String,
    pub locator: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relation {
    pub id: String,
    pub label: String,
    pub source: Source,
    pub status: RelationStatus,
    pub confidence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<DetailTarget>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RelationStatus {
    Proposed,
    Confirmed,
    Contested,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailTarget {
    pub route_id: String,
    pub id: String,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DetailSegment {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub raw: String,
    pub corrected: String,
    pub disposition: Option<CoverageDisposition>,
    pub reason: Option<String>,
}
impl std::fmt::Debug for DetailSegment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DetailSegment")
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CoverageDisposition {
    Unmapped,
    ExcludedNonSpeech,
    RedactedWithPolicy,
    UntranscribedFailure,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Paragraph {
    pub id: String,
    pub text: String,
    pub segment_ids: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capture {
    pub id: String,
    pub title: String,
    pub text: String,
    pub at_ms: u64,
    pub segment_id: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewItem {
    pub id: String,
    pub kind: ReviewKind,
    pub segment_id: String,
    pub note: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewKind {
    #[serde(rename = "Mark Moment")]
    MarkMoment,
    #[serde(rename = "Low confidence")]
    LowConfidence,
    Equation,
    Code,
}
pub type RelationGroups = BTreeMap<String, Vec<Relation>>;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lecture {
    pub id: String,
    pub title: String,
    pub segments: Vec<DetailSegment>,
    pub paragraphs: Vec<Paragraph>,
    pub captures: Vec<Capture>,
    pub review: Vec<ReviewItem>,
    pub links: RelationGroups,
    pub explanation: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Concept {
    pub id: String,
    pub title: String,
    pub state: String,
    pub confidence: String,
    pub freshness: String,
    pub last_strong_evidence: String,
    pub relations: RelationGroups,
    pub explanation: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuestionStatus {
    Open,
    Partial,
    Resolved,
    Reframed,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QuestionRevision {
    pub at: String,
    pub text: String,
    pub concepts: Vec<Relation>,
    pub resolution_evidence: Vec<Relation>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Question {
    pub id: String,
    pub origin: String,
    pub is_new: bool,
    pub status: QuestionStatus,
    pub goal_relevance: String,
    pub goal_rank: u64,
    pub next_context: String,
    pub age_days: u64,
    pub revisions: Vec<QuestionRevision>,
    pub evidence: Vec<Relation>,
    pub explanation: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub success_criteria: Vec<String>,
    pub repository: String,
    pub branch: String,
    pub snapshot: String,
    pub current_snapshot: String,
    pub captured_at: String,
    pub current_at: String,
    pub dirty: bool,
    pub relations: RelationGroups,
    pub files: Vec<DetailFile>,
    pub explanation: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailFile {
    pub path: String,
    pub text: String,
}

pub fn invalid(reason: &'static str) -> RpcError {
    RpcError::InvalidFieldValue {
        field: "details",
        reason,
    }
}
/// The same canonical serialization binds native requests and signed dispositions.
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, RpcError> {
    let bytes = serde_json::to_vec(value).map_err(|_| invalid("JSON encoding failed"))?;
    if bytes.len() > MAX_DETAIL_BYTES {
        return Err(invalid("detail byte limit exceeded"));
    }
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| invalid("invalid JSON"))?;
    validate_value(&value, 0)?;
    Ok(bytes)
}
pub fn decode<T: serde::de::DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<T, RpcError> {
    if bytes.len() > MAX_DETAIL_BYTES {
        return Err(invalid("detail byte limit exceeded"));
    }
    let value: T =
        serde_json::from_slice(bytes).map_err(|_| invalid("invalid closed detail DTO"))?;
    // Equality rejects duplicate fields, alternate number spellings and ignored payload bytes.
    if encode(&value)? != bytes {
        return Err(invalid("detail JSON must be canonical"));
    }
    Ok(value)
}
fn validate_value(value: &serde_json::Value, depth: usize) -> Result<(), RpcError> {
    if depth > 24 {
        return Err(invalid("detail nesting limit exceeded"));
    }
    match value {
        serde_json::Value::Number(number) => {
            if number.as_u64().is_none_or(|n| n > MAX_SAFE_INTEGER) {
                return Err(invalid("number is not a safe nonnegative integer"));
            }
        }
        serde_json::Value::String(s) if s.len() > 65_536 => {
            return Err(invalid("detail string limit exceeded"));
        }
        serde_json::Value::Array(items) => {
            if items.len() > 4096 {
                return Err(invalid("detail list limit exceeded"));
            }
            for item in items {
                validate_value(item, depth + 1)?;
            }
        }
        serde_json::Value::Object(fields) => {
            if fields.len() > 128 {
                return Err(invalid("detail object limit exceeded"));
            }
            for (key, item) in fields {
                if key.len() > 128 {
                    return Err(invalid("detail field limit exceeded"));
                }
                validate_value(item, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
pub fn decision_digest(request: &DetailDecisionRequest) -> Result<ContentDigest, RpcError> {
    Ok(ContentDigest::sha256(
        &[
            b"academic.details-decision.v1\0".as_slice(),
            &encode(request)?,
        ]
        .concat(),
    ))
}
impl DetailCorpus {
    /// Entity aliases whose detail surfaces actually contain the given relation.
    pub fn relation_owners(&self, alias: &str) -> BTreeSet<&str> {
        let mut owners = BTreeSet::new();
        for (id, groups) in self
            .lectures
            .iter()
            .map(|v| (v.id.as_str(), &v.links))
            .chain(self.concepts.iter().map(|v| (v.id.as_str(), &v.relations)))
            .chain(self.projects.iter().map(|v| (v.id.as_str(), &v.relations)))
        {
            if groups.values().flatten().any(|r| r.id == alias) {
                owners.insert(id);
            }
        }
        for question in &self.questions {
            if question
                .evidence
                .iter()
                .chain(
                    question
                        .revisions
                        .iter()
                        .flat_map(|v| v.concepts.iter().chain(&v.resolution_evidence)),
                )
                .any(|r| r.id == alias)
            {
                owners.insert(question.id.as_str());
            }
        }
        owners
    }
    /// Every occurrence is retained until consistency is checked; aliases cannot hide duplicates.
    pub fn relations(&self) -> Result<BTreeMap<&str, &Relation>, RpcError> {
        let mut result = BTreeMap::new();
        let groups = self
            .lectures
            .iter()
            .map(|v| &v.links)
            .chain(self.concepts.iter().map(|v| &v.relations))
            .chain(self.projects.iter().map(|v| &v.relations));
        let mut relations: Vec<&Relation> = groups.flat_map(|g| g.values().flatten()).collect();
        for question in &self.questions {
            relations.extend(&question.evidence);
            for revision in &question.revisions {
                relations.extend(&revision.concepts);
                relations.extend(&revision.resolution_evidence);
            }
        }
        for relation in relations {
            reference(&relation.id)?;
            if relation.source.content.is_empty()
                || relation.source.locator.is_empty()
                || relation.source.id.is_empty()
                || relation
                    .source
                    .href
                    .as_ref()
                    .is_some_and(|href| !href.starts_with('#'))
            {
                return Err(invalid("relation source is missing or nonlocal"));
            }
            if result
                .insert(relation.id.as_str(), relation)
                .is_some_and(|old| old != relation)
            {
                return Err(invalid("relation alias has conflicting definitions"));
            }
        }
        Ok(result)
    }
    pub fn validate(&self) -> Result<(), RpcError> {
        let _ = encode(self)?;
        let mut entities = BTreeSet::new();
        for id in self
            .lectures
            .iter()
            .map(|v| &v.id)
            .chain(self.concepts.iter().map(|v| &v.id))
            .chain(self.questions.iter().map(|v| &v.id))
            .chain(self.projects.iter().map(|v| &v.id))
        {
            reference(id)?;
            if !entities.insert(id) {
                return Err(invalid("duplicate entity alias"));
            }
        }
        for lecture in &self.lectures {
            let mut segments = BTreeMap::new();
            for segment in &lecture.segments {
                if segment.end_ms <= segment.start_ms
                    || segments.insert(&segment.id, segment).is_some()
                {
                    return Err(invalid("invalid or duplicate audio interval"));
                }
            }
            let mut mapped = BTreeSet::new();
            let mut paragraph_ids = BTreeSet::new();
            for paragraph in &lecture.paragraphs {
                if paragraph.segment_ids.is_empty() || !paragraph_ids.insert(&paragraph.id) {
                    return Err(invalid("invalid paragraph"));
                }
                for id in &paragraph.segment_ids {
                    if !segments.contains_key(id) {
                        return Err(invalid("paragraph has unknown raw segment"));
                    }
                    mapped.insert(id);
                }
            }
            for segment in &lecture.segments {
                if mapped.contains(&segment.id) && segment.disposition.is_some() {
                    return Err(invalid("coverage is not a partition"));
                }
                if segment
                    .disposition
                    .is_some_and(|d| d != CoverageDisposition::Unmapped)
                    && segment.reason.as_ref().is_none_or(|r| r.is_empty())
                {
                    return Err(invalid("coverage disposition lacks evidence"));
                }
            }
            for capture in &lecture.captures {
                let segment = segments
                    .get(&capture.segment_id)
                    .ok_or_else(|| invalid("capture has unknown segment"))?;
                if capture.at_ms < segment.start_ms || capture.at_ms >= segment.end_ms {
                    return Err(invalid("capture is outside segment"));
                }
            }
            for item in &lecture.review {
                if !segments.contains_key(&item.segment_id) {
                    return Err(invalid("review has unknown segment"));
                }
            }
        }
        for question in &self.questions {
            if question.revisions.is_empty() {
                return Err(invalid("question has no history"));
            }
        }
        let _ = self.relations()?;
        Ok(())
    }
}
pub fn reference(value: &str) -> Result<(), RpcError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:-".contains(&b))
    {
        return Err(invalid("invalid projection reference"));
    }
    Ok(())
}
