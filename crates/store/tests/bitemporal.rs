use std::{
    error::Error,
    fs,
    path::PathBuf,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

use academic_contracts::{DeviceAuthorization, sign_batch, verify_signed_batch};
use academic_domain::{
    Actor, ArtifactId, ArtifactRepresentation, AuthorityClass, BatchId, Claim, ClaimObject,
    ClaimRelation, ClaimRelationKind, Confidentiality, ContentDigest, DeviceId, DomainError,
    DomainId, EpistemicStatus, Event, EventId, EventPayload, EvidenceId, EvidenceItem,
    EvidenceLocator, EvidenceRole, EvidenceStrength, MediaType, PermissionLineageId, PredicateId,
    RetentionClass, ScopeDescriptor, ScopeId, TimestampMillis, UnsignedBatch, ValidInterval,
};
use academic_ledger::{AuthorityPolicy, EVENT_SCHEMA_VERSION, LedgerState, ResolutionQuery};
use academic_store::{
    connection::open_reader,
    idempotency::AcceptanceCommand,
    path_policy::NativePathProbe,
    profile::{SyntheticProfile, create_synthetic_profile},
    queries::resolve,
};
use academic_vault::{ArtifactIngestRequest, DomainKeyring, Vault};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct TemporaryDatabase {
    root: PathBuf,
    profile: SyntheticProfile,
}

impl TemporaryDatabase {
    fn new() -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "academic-s2-bitemporal-{}-{sequence}",
            std::process::id()
        ));
        let profile = create_synthetic_profile(&root, &NativePathProbe::default(), [0x82; 32])?;
        Ok(Self { root, profile })
    }
}

impl Drop for TemporaryDatabase {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

#[test]
fn sql_bitemporal_cases_match_oracle() -> Result<(), Box<dyn Error>> {
    let database = TemporaryDatabase::new()?;
    let domain_id = id::<DomainId>(0x101)?;
    let mut keyring = DomainKeyring::new();
    keyring.insert(domain_id, b"academic-s2-bitemporal-key")?;
    let vault = Vault::open(database.profile.root(), keyring)?;
    let (batch, subject_id, scope_id, predicate_id) = bitemporal_batch(&vault)?;
    let seed = [0x59_u8; 32];
    let signing_key = seed.as_slice().try_into()?;
    let envelope = sign_batch(&batch, &signing_key)?;
    let authorization =
        DeviceAuthorization::new(batch.device_id, id(0xff2)?, signing_key.verifying_key());
    let verified = verify_signed_batch(&envelope, &authorization)?;

    let mut oracle = LedgerState::new();
    oracle.accept_verified_batch(&verified)?;
    let mut store = database.profile.open_acceptance_store()?;
    store.accept_verified_batch(
        &verified,
        AcceptanceCommand {
            request_id: [1; 16],
            client_instance_id: [2; 16],
            idempotency_key: [3; 32],
            expected_revision: Some(0),
            envelope_bytes: &envelope,
        },
        TimestampMillis::new(1_000),
        &vault,
    )?;
    drop(store);
    let reader = open_reader(database.profile.database_path())?;

    let cases = [
        (TimestampMillis::new(50), 0),
        (TimestampMillis::new(50), 3),
        (TimestampMillis::new(50), 4),
        (TimestampMillis::new(50), 5),
        (TimestampMillis::new(50), 6),
        (TimestampMillis::new(150), 0),
        (TimestampMillis::new(150), 3),
        (TimestampMillis::new(150), 4),
        (TimestampMillis::new(150), 5),
        (TimestampMillis::new(150), 6),
        (TimestampMillis::new(500), 5),
        (TimestampMillis::new(500), 6),
    ];
    assert_eq!(cases.len(), 12);
    for (valid_at, known_at_accept_seq) in cases {
        let query = ResolutionQuery {
            subject_entity_id: subject_id,
            scope_id,
            predicate_id: predicate_id.clone(),
            valid_at,
            known_at_accept_seq,
            policy: AuthorityPolicy::OfficialFact,
        };
        assert_eq!(
            resolve(&reader, &query)?,
            oracle.resolve(&query),
            "valid_at={valid_at:?}, known_at={known_at_accept_seq}"
        );
    }
    Ok(())
}

fn bitemporal_batch(
    vault: &Vault,
) -> Result<
    (
        UnsignedBatch,
        academic_domain::EntityId,
        ScopeId,
        PredicateId,
    ),
    Box<dyn Error>,
> {
    let namespace = 0x100_u32;
    let domain_id = id::<DomainId>(namespace + 1)?;
    let scope_id = id::<ScopeId>(namespace + 2)?;
    let artifact_id = id::<ArtifactId>(namespace + 3)?;
    let evidence_id = id::<EvidenceId>(namespace + 4)?;
    let old_claim_id = id(namespace + 5)?;
    let new_claim_id = id(namespace + 6)?;
    let subject_id = id(namespace + 7)?;
    let predicate_id = PredicateId::parse("academic.deadline")?;
    let bytes = b"SYNTHETIC BITEMPORAL EVIDENCE";
    let digest = ContentDigest::sha256(bytes);
    let length = u64::try_from(bytes.len()).map_err(|_| DomainError::InvalidRange)?;
    let media_type = MediaType::parse("text/plain")?;
    let locator = EvidenceLocator::TextBytes {
        source_digest: digest,
        start: 0,
        end: length,
    };
    let actor = Actor::Importer {
        name: "academic.s2.bitemporal".to_owned(),
        version: "1.0.0".to_owned(),
    };
    let request = ArtifactIngestRequest::new(
        artifact_id,
        media_type,
        domain_id,
        Confidentiality::Personal,
        RetentionClass::UserManaged,
        id::<PermissionLineageId>(namespace + 8)?,
    );
    let receipt = vault.ingest(&request, bytes.as_slice())?;
    let mut descriptor = receipt.descriptor().clone();
    descriptor.evidence_representations = vec![ArtifactRepresentation {
        locator: locator.clone(),
        content_digest: digest,
        byte_length: length,
    }];
    let claim = |id, value: &str| Claim {
        id,
        subject_entity_id: subject_id,
        predicate_id: predicate_id.clone(),
        object: ClaimObject::Text(value.to_owned()),
        scope_id,
        authority_class: AuthorityClass::Official,
        epistemic_status: EpistemicStatus::OfficialConfirmed,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(100)),
        evidence_ids: vec![evidence_id],
    };
    let payloads = vec![
        EventPayload::ScopeRegistered(ScopeDescriptor {
            id: scope_id,
            domain_id,
            label: "synthetic bitemporal scope".to_owned(),
        }),
        EventPayload::ArtifactRegistered(descriptor),
        EventPayload::EvidenceRegistered(EvidenceItem {
            id: evidence_id,
            artifact_id,
            locator,
            excerpt_digest: digest,
            role: EvidenceRole::Supports,
            strength: EvidenceStrength::Direct,
            extraction_method: "academic.s2.synthetic".to_owned(),
            extractor_version: "1.0.0".to_owned(),
        }),
        EventPayload::ClaimAsserted(claim(old_claim_id, "2027-04-01")),
        EventPayload::ClaimAsserted(claim(new_claim_id, "2027-04-15")),
        EventPayload::ClaimRelated(ClaimRelation {
            source_claim_id: new_claim_id,
            target_claim_id: old_claim_id,
            kind: ClaimRelationKind::Supersedes,
            scope_id,
        }),
    ];
    let mut events = Vec::new();
    for (index, payload) in payloads.into_iter().enumerate() {
        let sequence = u64::try_from(index + 1).map_err(|_| DomainError::InvalidRange)?;
        let event = Event {
            id: id::<EventId>(
                namespace + 20 + u32::try_from(index).map_err(|_| DomainError::InvalidRange)?,
            )?,
            origin_seq: sequence,
            origin_observed_at: TimestampMillis::new(100),
            actor: actor.clone(),
            domain_id,
            payload,
        };
        event.validate()?;
        events.push(event);
    }
    Ok((
        UnsignedBatch {
            schema_version: EVENT_SCHEMA_VERSION,
            batch_id: id::<BatchId>(namespace + 40)?,
            device_id: id::<DeviceId>(namespace + 41)?,
            origin_seq_start: 1,
            origin_seq_end: 6,
            previous_batch_hash: None,
            origin_created_at: TimestampMillis::new(100),
            events,
        },
        subject_id,
        scope_id,
        predicate_id,
    ))
}

fn id<T>(suffix: u32) -> Result<T, DomainError>
where
    T: FromStr<Err = DomainError>,
{
    format!("01900000-0000-7000-8000-{suffix:012x}").parse()
}
