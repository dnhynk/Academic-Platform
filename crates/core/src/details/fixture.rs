//! Explicit synthetic fixture importer. Compiled only for tests or the named
//! synthetic-detail-fixtures tool; no IPC command accepts a corpus or source path.

use crate::{
    details::{
        CORPUS_PREDICATE, CorpusRecord, DetailContext, DetailError, RELATION_PREDICATE, derived_id,
    },
    service::AcceptanceService,
};
use academic_contracts::sign_batch;
use academic_domain::{
    Actor, ArtifactRepresentation, AuthorityClass, Claim, ClaimObject, Confidentiality,
    ContentDigest, EntityId, EpistemicStatus, Event, EventPayload, EvidenceId, EvidenceItem,
    EvidenceLocator, EvidenceRole, EvidenceStrength, MediaType, PredicateId, RetentionClass,
    ScopeDescriptor, TimestampMillis, UnsignedBatch, ValidInterval,
};
use academic_rpc::details::{self as dto, DetailCorpus};
use academic_store::{
    idempotency::AcceptanceCommand, profile::SyntheticProfile, queries::signed_history_snapshot,
};
use academic_vault::{ArtifactIngestRequest, DomainKeyring};
use std::{collections::BTreeMap, io::Cursor};

/// Imports supplied synthetic data through vault sealing, canonical signing and
/// the ordinary atomic acceptance path. Alternate corpora need no source edit.
pub fn import_synthetic_corpus(
    profile: &SyntheticProfile,
    corpus: DetailCorpus,
) -> Result<(), DetailError> {
    import_synthetic_corpus_with_audio(profile, corpus, BTreeMap::new())
}
pub fn import_synthetic_corpus_with_audio(
    profile: &SyntheticProfile,
    corpus: DetailCorpus,
    audio: BTreeMap<String, Vec<u8>>,
) -> Result<(), DetailError> {
    corpus.validate()?;
    let canonical = dto::encode(&corpus)?;
    let namespace = ContentDigest::sha256(&canonical);
    let domain = derived_id(namespace, "domain")?;
    let scope = derived_id(namespace, "scope")?;
    let authorization = crate::fixture_device_authorization()?;
    let mut keyring = DomainKeyring::new();
    keyring.insert(domain, crate::local_service::FIXTURE_LOCATOR_KEY)?;
    let mut service = AcceptanceService::open(profile, keyring)?;
    let mut reader = profile.open_reader()?;
    let history = signed_history_snapshot(&mut reader)?;
    if history.accept_seq_head != 0 {
        return Err(DetailError::Invalid(
            "fixture import requires an empty disposable profile",
        ));
    }
    let mut entities = BTreeMap::new();
    for id in corpus
        .lectures
        .iter()
        .map(|v| &v.id)
        .chain(corpus.concepts.iter().map(|v| &v.id))
        .chain(corpus.projects.iter().map(|v| &v.id))
        .chain(corpus.questions.iter().map(|v| &v.id))
    {
        entities.insert(id.clone(), derived_id::<EntityId>(namespace, id)?);
    }
    let mut payloads = vec![EventPayload::ScopeRegistered(ScopeDescriptor {
        id: scope,
        domain_id: domain,
        label: "Explicit synthetic detail fixture".to_owned(),
    })];
    let mut relations = BTreeMap::new();
    for (alias, relation) in corpus.relations()? {
        let evidence = seal_source(
            &service,
            namespace,
            &format!("relation-{alias}"),
            relation.source.content.as_bytes(),
            &mut payloads,
        )?;
        let id = derived_id(namespace, &format!("claim-{alias}"))?;
        relations.insert(alias.to_owned(), id);
        let owner = corpus
            .relation_owners(alias)
            .into_iter()
            .next()
            .and_then(|alias| entities.get(alias))
            .copied()
            .ok_or(DetailError::Invalid("relation without owner"))?;
        payloads.push(EventPayload::ClaimAsserted(Claim {
            id,
            subject_entity_id: owner,
            predicate_id: PredicateId::parse(RELATION_PREDICATE)?,
            object: ClaimObject::Text(
                String::from_utf8(dto::encode(relation)?)
                    .map_err(|_| DetailError::Invalid("relation UTF-8"))?,
            ),
            scope_id: scope,
            authority_class: AuthorityClass::DirectObservation,
            epistemic_status: EpistemicStatus::CodeObserved,
            confidence: None,
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
            evidence_ids: vec![evidence],
        }));
    }
    let mut media = BTreeMap::new();
    for (lecture, bytes) in audio {
        if !corpus.lectures.iter().any(|l| l.id == lecture)
            || bytes.len() < 44
            || bytes.len() > 4_194_304
            || &bytes[..4] != b"RIFF"
            || &bytes[8..12] != b"WAVE"
        {
            return Err(DetailError::Invalid(
                "synthetic WAV media is outside bounds or lecture membership",
            ));
        }
        let request = ArtifactIngestRequest::new(
            derived_id(
                namespace,
                &format!(
                    "audio-{}",
                    hex::encode(ContentDigest::sha256(&bytes).as_bytes())
                ),
            )?,
            MediaType::parse("audio/wav")?,
            domain,
            Confidentiality::Restricted,
            RetentionClass::UserManaged,
            derived_id(namespace, "audio-permission")?,
        );
        let receipt = service.vault().ingest(&request, Cursor::new(bytes))?;
        media.insert(lecture, receipt.descriptor().id);
        register_once(&mut payloads, receipt.descriptor().clone())?;
    }
    let record = CorpusRecord {
        version: 1,
        corpus,
        relations,
        entities,
        media,
    };
    let record_bytes = dto::encode(&record)?;
    let evidence = seal_source(&service, namespace, "corpus", &record_bytes, &mut payloads)?;
    payloads.push(EventPayload::ClaimAsserted(Claim {
        id: derived_id(namespace, "corpus-claim")?,
        subject_entity_id: derived_id(namespace, "workspace")?,
        predicate_id: PredicateId::parse(CORPUS_PREDICATE)?,
        object: ClaimObject::Text(
            String::from_utf8(record_bytes).map_err(|_| DetailError::Invalid("corpus UTF-8"))?,
        ),
        scope_id: scope,
        authority_class: AuthorityClass::DirectObservation,
        epistemic_status: EpistemicStatus::CodeObserved,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(0)),
        evidence_ids: vec![evidence],
    }));
    let mut events = Vec::new();
    for (index, payload) in payloads.into_iter().enumerate() {
        let sequence =
            u64::try_from(index).map_err(|_| DetailError::Invalid("fixture event count"))? + 1;
        events.push(Event {
            id: derived_id(namespace, &format!("event-{sequence}"))?,
            origin_seq: sequence,
            origin_observed_at: TimestampMillis::new(0),
            actor: Actor::Importer {
                name: "academic.synthetic-detail-fixture".to_owned(),
                version: "1".to_owned(),
            },
            domain_id: domain,
            payload,
        });
    }
    let batch = UnsignedBatch {
        schema_version: academic_domain::EVENT_SCHEMA_VERSION_V4,
        batch_id: derived_id(namespace, "import-batch")?,
        device_id: authorization.device_id(),
        origin_seq_start: 1,
        origin_seq_end: u64::try_from(events.len())
            .map_err(|_| DetailError::Invalid("fixture event count"))?,
        previous_batch_hash: None,
        origin_created_at: TimestampMillis::new(0),
        events,
    };
    let envelope = sign_batch(&batch, &crate::fixture_signing_key())?;
    let outcome = service.accept_signed_command(
        AcceptanceCommand {
            request_id: *batch.batch_id.as_bytes(),
            client_instance_id: *authorization.device_id().as_bytes(),
            idempotency_key: *namespace.as_bytes(),
            expected_revision: Some(0),
            envelope_bytes: &envelope,
        },
        &authorization,
        TimestampMillis::new(0),
    )?;
    if outcome.replayed_request {
        return Err(DetailError::Invalid(
            "empty fixture import unexpectedly replayed",
        ));
    }
    // Exercise the same verified projection before reporting an import as usable.
    let context = DetailContext::new(
        profile,
        authorization,
        crate::fixture_signing_key(),
        Vec::new(),
    )?;
    let _ = context.handle(
        profile,
        &mut service,
        &dto::DetailRequest::DetailsRead {
            selector: dto::DetailSelector::default(),
        },
        TimestampMillis::new(0),
    )?;
    Ok(())
}

fn seal_source(
    service: &AcceptanceService,
    namespace: ContentDigest,
    label: &str,
    bytes: &[u8],
    payloads: &mut Vec<EventPayload>,
) -> Result<EvidenceId, DetailError> {
    let domain = derived_id(namespace, "domain")?;
    let request = ArtifactIngestRequest::new(
        derived_id(
            namespace,
            &format!(
                "text-{}",
                hex::encode(ContentDigest::sha256(bytes).as_bytes())
            ),
        )?,
        MediaType::parse("text/plain")?,
        domain,
        Confidentiality::Restricted,
        RetentionClass::UserManaged,
        derived_id(namespace, "permission")?,
    );
    let receipt = service.vault().ingest(&request, Cursor::new(bytes))?;
    let mut descriptor = receipt.descriptor().clone();
    let locator = EvidenceLocator::TextBytes {
        source_digest: descriptor.content_digest,
        start: 0,
        end: descriptor.byte_length,
    };
    descriptor
        .evidence_representations
        .push(ArtifactRepresentation {
            locator: locator.clone(),
            content_digest: descriptor.content_digest,
            byte_length: descriptor.byte_length,
        });
    let evidence = EvidenceItem {
        id: derived_id(namespace, &format!("evidence-{label}"))?,
        artifact_id: descriptor.id,
        locator,
        excerpt_digest: descriptor.content_digest,
        role: EvidenceRole::Supports,
        strength: EvidenceStrength::Direct,
        extraction_method: "explicit-synthetic-detail-import".to_owned(),
        extractor_version: "1".to_owned(),
    };
    let id = evidence.id;
    register_once(payloads, descriptor)?;
    payloads.push(EventPayload::EvidenceRegistered(evidence));
    Ok(id)
}

fn register_once(
    payloads: &mut Vec<EventPayload>,
    descriptor: academic_domain::ArtifactDescriptor,
) -> Result<(), DetailError> {
    if let Some(existing) = payloads.iter().find_map(|payload| match payload {
        EventPayload::ArtifactRegistered(existing) if existing.id == descriptor.id => {
            Some(existing)
        }
        _ => None,
    }) {
        if existing != &descriptor {
            return Err(DetailError::Invalid(
                "duplicate synthetic source has inconsistent metadata",
            ));
        }
    } else {
        payloads.push(EventPayload::ArtifactRegistered(descriptor));
    }
    Ok(())
}
