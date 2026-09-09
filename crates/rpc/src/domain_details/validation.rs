//! Structural, coordinate and provenance closure validation for the closed DTO.
use super::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn validate_refs(refs: &[String], required: bool) -> Result<(), RpcError> {
    if (required && refs.is_empty()) || refs.len() > MAX_FIELD_REFS {
        return Err(invalid("invalid domain provenance reference count"));
    }
    let mut unique = BTreeSet::new();
    for reference in refs {
        crate::details::reference(reference)?;
        if !unique.insert(reference) {
            return Err(invalid("duplicate provenance reference"));
        }
    }
    Ok(())
}

impl Subject {
    pub fn context(&self) -> Context {
        let (domain_id, scope_id) = match self {
            Self::Lecture {
                domain_id,
                scope_id,
                ..
            }
            | Self::Concept {
                domain_id,
                scope_id,
                ..
            }
            | Self::Question {
                domain_id,
                scope_id,
                ..
            }
            | Self::Project {
                domain_id,
                scope_id,
                ..
            } => (*domain_id, *scope_id),
        };
        Context {
            domain_id,
            scope_id,
        }
    }
    pub const fn surface(&self) -> Surface {
        match self {
            Self::Lecture { .. } => Surface::Lecture,
            Self::Concept { .. } => Surface::Concept,
            Self::Question { .. } => Surface::Question,
            Self::Project { .. } => Surface::Project,
        }
    }
    pub fn canonical_id(&self) -> String {
        match self {
            Self::Lecture {
                lecture_session_id, ..
            } => lecture_session_id.to_string(),
            Self::Concept { entity_id, .. }
            | Self::Question { entity_id, .. }
            | Self::Project { entity_id, .. } => entity_id.to_string(),
        }
    }
}

impl DomainReadRequest {
    pub fn parts(&self) -> (&Context, &DomainSelector, &Query) {
        let Self::DetailsDomainReadV3 {
            context,
            selector,
            query,
        } = self;
        (context, selector, query)
    }
    pub fn validate(&self) -> Result<(), RpcError> {
        let _ = crate::details::encode(self)?;
        let (context, _, query) = self.parts();
        if let Query::Detail { subject } = query
            && &subject.context() != context
        {
            return Err(invalid("domain subject context mismatch"));
        }
        Ok(())
    }
}

impl DomainReadReply {
    pub fn unavailable(reason: ReadFailure) -> Self {
        Self::Unavailable {
            version: 1,
            schema_version: 3,
            reason,
            projection: (),
        }
    }
    pub fn ready(projection: DomainProjection) -> Result<Self, RpcError> {
        let reply = Self::Ready {
            version: 1,
            schema_version: 3,
            projection: Box::new(projection),
        };
        reply.validate()?;
        Ok(reply)
    }
    pub fn validate(&self) -> Result<(), RpcError> {
        let (version, schema_version) = match self {
            Self::Ready {
                version,
                schema_version,
                ..
            }
            | Self::Unavailable {
                version,
                schema_version,
                ..
            } => (*version, *schema_version),
        };
        if version != 1 || schema_version != 3 {
            return Err(invalid("unsupported domain reply version"));
        }
        let _ = crate::details::encode(self)?;
        if let Self::Ready { projection, .. } = self {
            projection.validate()?;
        }
        Ok(())
    }
    pub fn validate_for(&self, request: &DomainReadRequest) -> Result<(), RpcError> {
        self.validate()?;
        request.validate()?;
        let Self::Ready { projection, .. } = self else {
            return Ok(());
        };
        let (context, selector, query) = request.parts();
        let binding = &projection.binding;
        if binding.domain_id != context.domain_id
            || binding.scope_id != context.scope_id
            || selector
                .known_at_accept_seq
                .is_some_and(|known| known != binding.known_at_accept_seq)
            || selector
                .valid_at_ms
                .is_some_and(|valid| valid != binding.valid_at_ms)
        {
            return Err(invalid("domain reply selector mismatch"));
        }
        match (query, &projection.result) {
            (Query::Index { surface }, QueryResult::Index { surface: found, .. })
                if surface == found =>
            {
                Ok(())
            }
            (Query::Detail { subject }, QueryResult::Detail { detail })
                if subject == detail.subject() =>
            {
                Ok(())
            }
            _ => Err(invalid("domain reply query mismatch")),
        }
    }
}

impl DomainDetail {
    pub fn subject(&self) -> &Subject {
        match self {
            Self::Lecture { subject, .. }
            | Self::Concept { subject, .. }
            | Self::Question { subject, .. }
            | Self::Project { subject, .. } => subject,
        }
    }
    pub fn groups(&self) -> &[RelationGroup] {
        match self {
            Self::Lecture {
                relation_groups, ..
            }
            | Self::Concept {
                relation_groups, ..
            }
            | Self::Question {
                relation_groups, ..
            }
            | Self::Project {
                relation_groups, ..
            } => relation_groups,
        }
    }
}

impl DomainProjection {
    pub fn validate(&self) -> Result<(), RpcError> {
        if self.source.projector_version != PROJECTOR_VERSION
            || self.binding.profile_id.is_empty()
            || self.read_sources.is_empty()
            || self.provenance.is_empty()
            || self.provenance.len() > MAX_PROVENANCE_ENTRIES
        {
            return Err(invalid("invalid domain projection identity or bounds"));
        }
        let _ = crate::details::encode(self)?;
        let mut sources = BTreeMap::new();
        for source in &self.read_sources {
            crate::details::reference(&source.r#ref)?;
            if sources.insert(source.r#ref.as_str(), source).is_some() {
                return Err(invalid("duplicate domain read source"));
            }
            let (coordinates, registry) = match &source.authority {
                ReadAuthority::CanonicalSnapshot {
                    coordinates,
                    policy_registry,
                    snapshot_adapter_version,
                    ..
                } => {
                    if snapshot_adapter_version.is_empty() {
                        return Err(invalid("empty snapshot adapter"));
                    }
                    (coordinates, policy_registry)
                }
                ReadAuthority::MaterializedGraph {
                    coordinates,
                    policy_registry,
                    generation_id,
                    algorithm_version,
                    effective_configuration,
                    availability,
                    ..
                } => {
                    generation(generation_id)?;
                    if algorithm_version.is_empty()
                        || effective_configuration.reason != MissingReason::UnsupportedField
                    {
                        return Err(invalid("invalid graph configuration provenance"));
                    }
                    match availability {
                        GraphAvailability::Current {} => {}
                        GraphAvailability::Lagging {
                            latest_known_at_accept_seq,
                            latest_source_outbox_seq,
                        }
                        | GraphAvailability::Historical {
                            latest_known_at_accept_seq,
                            latest_source_outbox_seq,
                            ..
                        } => {
                            if *latest_known_at_accept_seq < coordinates.known_at_accept_seq
                                || *latest_source_outbox_seq < coordinates.source_outbox_seq
                            {
                                return Err(invalid("invalid graph availability coordinates"));
                            }
                        }
                    }
                    if let GraphAvailability::Historical {
                        current_generation_id: Some(id),
                        ..
                    } = availability
                    {
                        generation(id)?;
                    }
                    (coordinates, policy_registry)
                }
            };
            if coordinates.domain_id != self.binding.domain_id
                || coordinates.known_at_accept_seq != self.binding.known_at_accept_seq
                || coordinates.valid_at_ms != self.binding.valid_at_ms
                || coordinates.source_outbox_seq != self.binding.source_outbox_seq
                || coordinates.source_ledger_digest != self.binding.source_ledger_digest
            {
                return Err(invalid("domain read source coordinate mismatch"));
            }
            if registry.resolver_version.is_empty() || registry.policy_registry_version.is_empty() {
                return Err(invalid("empty domain policy registry identity"));
            }
            let mut predicates = BTreeSet::new();
            for entry in &registry.predicate_policies {
                academic_domain::PredicateId::parse(&entry.predicate_id)
                    .map_err(|_| invalid("invalid domain predicate"))?;
                if !predicates.insert(&entry.predicate_id) {
                    return Err(invalid("duplicate domain predicate policy"));
                }
            }
        }
        let mut entries = BTreeMap::new();
        for entry in &self.provenance {
            crate::details::reference(&entry.r#ref)?;
            if sources.contains_key(entry.r#ref.as_str())
                || !sources.contains_key(entry.source_ref.as_str())
                || entries.insert(entry.r#ref.as_str(), entry).is_some()
            {
                return Err(invalid("invalid provenance definition"));
            }
        }
        let mut graph = BTreeMap::new();
        let mut excerpt_bytes = 0_usize;
        for entry in &self.provenance {
            self.validate_origin(entry, &entries, &sources)?;
            let value = serde_json::to_value(&entry.origin)
                .map_err(|_| invalid("invalid provenance encoding"))?;
            let mut refs = BTreeSet::new();
            inspect(&value, &entries, &mut refs, &mut excerpt_bytes)?;
            graph.insert(entry.r#ref.as_str(), refs);
        }
        let result =
            serde_json::to_value(&self.result).map_err(|_| invalid("invalid result encoding"))?;
        inspect(&result, &entries, &mut BTreeSet::new(), &mut excerpt_bytes)?;
        if excerpt_bytes > MAX_EXCERPT_BYTES {
            return Err(invalid("domain excerpt limit exceeded"));
        }
        // Shared ancestors are visited once. Cache their longest remaining path
        // so reusing a shallow validation cannot bypass the depth bound later.
        let mut verified_heights = BTreeMap::new();
        for reference in graph.keys() {
            visit(
                reference,
                &graph,
                &mut BTreeSet::new(),
                &mut verified_heights,
                0,
            )?;
        }
        self.validate_result(&entries)?;
        Ok(())
    }

    fn validate_origin(
        &self,
        entry: &ProvenanceEntry,
        entries: &BTreeMap<&str, &ProvenanceEntry>,
        sources: &BTreeMap<&str, &ReadSource>,
    ) -> Result<(), RpcError> {
        let b = &self.binding;
        let check_context = |domain, scope, accept| {
            if domain != b.domain_id
                || scope != b.scope_id
                || accept == 0
                || accept > b.known_at_accept_seq
            {
                Err(invalid("provenance context or coordinate mismatch"))
            } else {
                Ok(())
            }
        };
        match &entry.origin {
            ProvenanceOrigin::AcceptedClaim {
                domain_id,
                scope_id,
                accept_seq,
                origin_event,
                evidence,
                resolution_policy,
                graph_row,
                ..
            } => {
                check_context(*domain_id, *scope_id, *accept_seq)?;
                origin_field(origin_event, *accept_seq, Some(*scope_id), None, entries)?;
                for item in evidence {
                    validate_evidence(item)?;
                }
                let source = sources
                    .get(entry.source_ref.as_str())
                    .ok_or_else(|| invalid("missing source"))?;
                match (&source.authority, graph_row) {
                    (ReadAuthority::CanonicalSnapshot { .. }, None) => {}
                    (ReadAuthority::MaterializedGraph { generation_id, .. }, Some(row))
                        if generation_id == &row.generation_id => {}
                    _ => return Err(invalid("claim graph source mismatch")),
                }
                let registry = match &source.authority {
                    ReadAuthority::CanonicalSnapshot {
                        policy_registry, ..
                    }
                    | ReadAuthority::MaterializedGraph {
                        policy_registry, ..
                    } => policy_registry,
                };
                if !registry
                    .predicate_policies
                    .iter()
                    .any(|p| p.policy == *resolution_policy)
                {
                    return Err(invalid("claim resolution policy absent from registry"));
                }
            }
            ProvenanceOrigin::RegisteredAggregate {
                domain_id,
                scope_id,
                accept_seq,
                origin_event,
                registered_event_id,
                ..
            } => {
                check_context(*domain_id, *scope_id, *accept_seq)?;
                origin_field(
                    origin_event,
                    *accept_seq,
                    Some(*scope_id),
                    Some(*registered_event_id),
                    entries,
                )?;
            }
            ProvenanceOrigin::AcceptedEvent {
                event,
                accept_seq,
                scope_id,
            } => {
                if event.domain_id != b.domain_id
                    || scope_id.is_some_and(|scope| scope != b.scope_id)
                    || *accept_seq == 0
                    || *accept_seq > b.known_at_accept_seq
                {
                    return Err(invalid("accepted event coordinate mismatch"));
                }
                validate_actor(&event.actor)?;
            }
            ProvenanceOrigin::VerifiedArtifact {
                artifact_id: _,
                domain_id,
                format_version,
                media_type,
                registered_event_ref,
                ..
            } => {
                if *domain_id != b.domain_id
                    || !academic_domain::ARTIFACT_FORMAT_VERSIONS.contains(format_version)
                    || academic_domain::MediaType::parse(media_type).is_err()
                {
                    return Err(invalid("invalid verified artifact identity"));
                }
                if !matches!(entries.get(registered_event_ref.as_str()).map(|e| &e.origin), Some(ProvenanceOrigin::AcceptedEvent { event, scope_id: None, .. }) if event.domain_id == *domain_id)
                {
                    return Err(invalid("verified artifact lacks accepted event"));
                }
            }
            ProvenanceOrigin::VerifiedDomainResult {
                engine_id,
                engine_version,
                registration_ref,
                evidence,
                ..
            } => {
                if engine_id.is_empty()
                    || engine_version.is_empty()
                    || !matches!(
                        entries.get(registration_ref.as_str()).map(|e| &e.origin),
                        Some(ProvenanceOrigin::RegisteredAggregate { .. })
                    )
                {
                    return Err(invalid("domain result lacks registered source"));
                }
                for item in evidence {
                    validate_evidence(item)?;
                }
            }
            ProvenanceOrigin::VerifiedQuery { query } => {
                if query.context.domain_id != b.domain_id
                    || query.context.scope_id != b.scope_id
                    || query.coordinates.known_at_accept_seq != b.known_at_accept_seq
                    || query.coordinates.valid_at_ms != b.valid_at_ms
                {
                    return Err(invalid("query witness context mismatch"));
                }
                let max = match &query.selection {
                    QuerySelection::Index { max_entries, .. } if *max_entries == 256 => {
                        *max_entries
                    }
                    QuerySelection::Relations {
                        subject,
                        predicate_ids,
                        max_entries,
                        ..
                    } if *max_entries == 4096 && subject.context() == query.context => {
                        if predicate_ids.is_empty()
                            || predicate_ids.iter().collect::<BTreeSet<_>>().len()
                                != predicate_ids.len()
                        {
                            return Err(invalid("invalid relation query predicates"));
                        }
                        for predicate in predicate_ids {
                            academic_domain::PredicateId::parse(predicate)
                                .map_err(|_| invalid("invalid relation predicate"))?;
                        }
                        *max_entries
                    }
                    QuerySelection::History {
                        subject,
                        through_known_at_accept_seq,
                        max_entries,
                        ..
                    } if *max_entries == 4096
                        && *through_known_at_accept_seq == b.known_at_accept_seq
                        && subject.context() == query.context =>
                    {
                        *max_entries
                    }
                    _ => return Err(invalid("invalid complete query witness")),
                };
                if query.returned_count > max {
                    return Err(invalid("query witness exceeds complete bound"));
                }
            }
        }
        Ok(())
    }

    fn validate_result(&self, entries: &BTreeMap<&str, &ProvenanceEntry>) -> Result<(), RpcError> {
        let context = Context {
            domain_id: self.binding.domain_id,
            scope_id: self.binding.scope_id,
        };
        match &self.result {
            QueryResult::Index {
                surface,
                entries: index,
            } => {
                if index.len() > MAX_INDEX_ENTRIES {
                    return Err(invalid("domain index limit exceeded"));
                }
                let mut previous = None;
                for item in index {
                    let id = item.subject.canonical_id();
                    if item.subject.context() != context
                        || item.subject.surface() != *surface
                        || item.source_ids.is_empty()
                        || previous.as_ref().is_some_and(|old| old >= &id)
                    {
                        return Err(invalid("invalid domain index identity or order"));
                    }
                    previous = Some(id);
                }
                if !entries.values().any(|entry| matches!(&entry.origin, ProvenanceOrigin::VerifiedQuery { query }
                    if query.returned_count == index.len() as u64 && matches!(query.selection, QuerySelection::Index { surface: s, .. } if s == *surface))) {
                    return Err(invalid("index lacks complete query witness"));
                }
            }
            QueryResult::Detail { detail } => {
                let surface = match detail.as_ref() {
                    DomainDetail::Lecture { .. } => Surface::Lecture,
                    DomainDetail::Concept { .. } => Surface::Concept,
                    DomainDetail::Question { .. } => Surface::Question,
                    DomainDetail::Project { .. } => Surface::Project,
                };
                if detail.subject().context() != context || detail.subject().surface() != surface {
                    return Err(invalid("detail subject kind mismatch"));
                }
                if let DomainDetail::Project {
                    files: Field::Available { value: files, .. },
                    ..
                } = detail.as_ref()
                {
                    for file in files {
                        if let Field::Available { value: excerpt, .. } = &file.source_excerpt {
                            let span = excerpt
                                .end
                                .checked_sub(excerpt.start)
                                .ok_or_else(|| invalid("reversed source excerpt range"))?;
                            let byte_count = u64::try_from(excerpt.bytes.len())
                                .map_err(|_| invalid("source excerpt length overflow"))?;
                            let file_length = file
                                .byte_length
                                .0
                                .parse::<u64>()
                                .map_err(|_| invalid("invalid source file byte length"))?;
                            // ByteExcerpt permits contained empty spans, unlike evidence locators.
                            if span != byte_count || excerpt.end > file_length {
                                return Err(invalid("source excerpt range or length mismatch"));
                            }
                        }
                    }
                }
                let mut groups = BTreeSet::new();
                for group in detail.groups() {
                    if !groups.insert(group.kind) {
                        return Err(invalid("duplicate relation group"));
                    }
                    if let Field::Available {
                        value,
                        provenance_refs,
                    } = &group.relations
                    {
                        let mut previous = None;
                        for relation in value {
                            if relation.scope_id != context.scope_id
                                || relation.subject != *detail.subject()
                                || previous.is_some_and(|old| old >= relation.claim_id)
                            {
                                return Err(invalid("invalid relation order or scope"));
                            }
                            previous = Some(relation.claim_id);
                            if !matches!(entries.get(relation.provenance_ref.as_str()).map(|e| &e.origin), Some(ProvenanceOrigin::AcceptedClaim { claim_id, .. }) if *claim_id == relation.claim_id)
                            {
                                return Err(invalid("relation lacks canonical claim provenance"));
                            }
                        }
                        if !provenance_refs.iter().any(|r| matches!(entries.get(r.as_str()).map(|e| &e.origin), Some(ProvenanceOrigin::VerifiedQuery { query })
                            if query.returned_count == value.len() as u64 && matches!(&query.selection, QuerySelection::Relations { subject, predicate_ids, .. }
                                if subject == detail.subject() && value.iter().all(|relation| predicate_ids.contains(&relation.predicate_id))))) {
                            return Err(invalid("relation group lacks complete query witness"));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn generation(id: &str) -> Result<(), RpcError> {
    if id.len() != 32
        || !id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(invalid("invalid graph generation identity"));
    }
    Ok(())
}

fn origin_field(
    field: &Field<OriginEvent>,
    sequence: u64,
    scope: Option<ScopeId>,
    event_id: Option<EventId>,
    entries: &BTreeMap<&str, &ProvenanceEntry>,
) -> Result<(), RpcError> {
    if let Field::Available {
        value,
        provenance_refs,
    } = field
        && (event_id.is_some_and(|id| id != value.event_id) || !provenance_refs.iter().any(|r| matches!(entries.get(r.as_str()).map(|e| &e.origin), Some(ProvenanceOrigin::AcceptedEvent { event, accept_seq, scope_id }) if event == value && *accept_seq == sequence && *scope_id == scope)))
    {
            return Err(invalid("origin event lacks independent accepted provenance"));
    }
    Ok(())
}

fn validate_actor(actor: &ActorRef) -> Result<(), RpcError> {
    match actor {
        ActorRef::User { .. } | ActorRef::ModelRun { .. } => {}
        ActorRef::Importer { name, version } | ActorRef::DeterministicEngine { name, version } => {
            if name.trim().is_empty() || version.trim().is_empty() {
                return Err(invalid("empty canonical actor label"));
            }
        }
        ActorRef::DeterministicPrediction { name, version, .. } => {
            for value in [name, version] {
                if !value
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"._+/-".contains(&b))
                {
                    return Err(invalid("invalid prediction actor label"));
                }
            }
        }
    }
    Ok(())
}

fn validate_evidence(item: &EvidenceRef) -> Result<(), RpcError> {
    if item.extraction_method.trim().is_empty() || item.extractor_version.trim().is_empty() {
        return Err(invalid("missing evidence extraction provenance"));
    }
    match &item.locator {
        Locator::Page { page_number } if *page_number > 0 => {}
        Locator::TextBytes { start, end, .. } if start < end => {}
        Locator::TranscriptTime { start_ms, end_ms } if start_ms < end_ms => {}
        Locator::RepositoryBytes {
            path, start, end, ..
        } if start < end && academic_domain::LogicalPath::parse(path).is_ok() => {}
        _ => return Err(invalid("invalid typed evidence locator")),
    }
    Ok(())
}

fn inspect(
    value: &Value,
    entries: &BTreeMap<&str, &ProvenanceEntry>,
    refs: &mut BTreeSet<String>,
    excerpt_bytes: &mut usize,
) -> Result<(), RpcError> {
    match value {
        Value::Array(values) => {
            for value in values {
                inspect(value, entries, refs, excerpt_bytes)?;
            }
        }
        Value::Object(object) => {
            if object.get("state").and_then(Value::as_str) == Some("available") {
                let field_refs: Vec<String> = serde_json::from_value(
                    object
                        .get("provenance_refs")
                        .cloned()
                        .ok_or_else(|| invalid("available field lacks provenance"))?,
                )
                .map_err(|_| invalid("invalid field references"))?;
                validate_refs(&field_refs, true)?;
                if object.get("value").and_then(Value::as_array).is_some_and(Vec::is_empty)
                    && !field_refs.iter().any(|r| matches!(entries.get(r.as_str()).map(|e| &e.origin), Some(ProvenanceOrigin::VerifiedQuery { query }) if query.returned_count == 0)) {
                    return Err(invalid("empty field lacks complete absence witness"));
                }
            }
            for (key, value) in object {
                if key.ends_with("_refs") {
                    let values: Vec<String> = serde_json::from_value(value.clone())
                        .map_err(|_| invalid("invalid provenance references"))?;
                    validate_refs(&values, false)?;
                    for reference in values {
                        if !entries.contains_key(reference.as_str()) {
                            return Err(invalid("unresolved provenance reference"));
                        }
                        refs.insert(reference);
                    }
                } else if key.ends_with("_ref") && key != "source_ref" {
                    let reference = value
                        .as_str()
                        .ok_or_else(|| invalid("invalid provenance reference"))?;
                    if !entries.contains_key(reference) {
                        return Err(invalid("unresolved provenance reference"));
                    }
                    refs.insert(reference.to_owned());
                }
                if matches!(
                    key.as_str(),
                    "permille" | "confidence_permille" | "deduction_permille"
                ) && value.as_u64().is_none_or(|n| n > 1000)
                {
                    return Err(invalid("invalid typed confidence range"));
                }
                if key == "excerpt"
                    && let Some(text) = value
                        .get("value")
                        .and_then(|v| v.get("text"))
                        .and_then(Value::as_str)
                {
                    *excerpt_bytes = excerpt_bytes
                        .checked_add(text.len())
                        .ok_or_else(|| invalid("excerpt byte overflow"))?;
                }
                if key == "source_excerpt"
                    && let Some(bytes) = value
                        .get("value")
                        .and_then(|v| v.get("bytes"))
                        .and_then(Value::as_array)
                {
                    *excerpt_bytes = excerpt_bytes
                        .checked_add(bytes.len())
                        .ok_or_else(|| invalid("excerpt byte overflow"))?;
                }
                inspect(value, entries, refs, excerpt_bytes)?;
            }
            if object.get("action").and_then(Value::as_str) == Some("REPLACE")
                && object
                    .get("replacement_claim_id")
                    .is_none_or(Value::is_null)
            {
                return Err(invalid("replacement decision lacks replacement identity"));
            }
        }
        _ => {}
    }
    Ok(())
}

fn visit(
    reference: &str,
    graph: &BTreeMap<&str, BTreeSet<String>>,
    active: &mut BTreeSet<String>,
    verified_heights: &mut BTreeMap<String, usize>,
    depth: usize,
) -> Result<usize, RpcError> {
    if let Some(height) = verified_heights.get(reference) {
        return if depth + height <= 24 {
            Ok(*height)
        } else {
            Err(invalid("excessive provenance depth"))
        };
    }
    if depth > 24 || !active.insert(reference.to_owned()) {
        return Err(invalid("cyclic or excessive provenance depth"));
    }
    let mut height = 0;
    if let Some(refs) = graph.get(reference) {
        for next in refs {
            height = height.max(1 + visit(next, graph, active, verified_heights, depth + 1)?);
        }
    }
    active.remove(reference);
    verified_heights.insert(reference.to_owned(), height);
    Ok(height)
}
