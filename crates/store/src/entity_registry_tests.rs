//! Named acceptance evidence for the entity registry and the migration-equivalence contract.
//!
//! # What these tests run against
//!
//! A real store: migration `0001`'s canonical core, migration `0003`'s schema-2
//! identity, and migration `0004`'s aggregate closure tables, written through
//! the same [`ClosureWriter`] the acceptance path uses and sealed against a real
//! [`Vault`]. Nothing here simulates the store. That matters because the whole
//! claim under test is physical: the ontology change appends events and rewrites
//! nothing, and the only convincing way to show that is to hold the pre-change
//! bytes, commit the change, and read the same bytes back.
//!
//! # The golden multi-year history
//!
//! Four synthetic years of mastery observations over four concepts, followed by
//! a major ontology change in a second batch: a duplicate concept merges into
//! its survivor, and an ambiguous concept splits into three senses. The change
//! is a second batch rather than a second transaction so the pre-change bytes
//! belong to a batch that is already closed when the change is accepted.
//!
//! Every byte is synthetic. No file here is derived from a personal record, a
//! lecture, or any external fetch.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use academic_domain::{
    Actor, ArtifactDescriptor, ArtifactId, ArtifactRepresentation, AuthorityClass, BatchId, Claim,
    ClaimId, ClaimObject, Confidentiality, ContentDigest, DomainId, EntityId,
    EntityIdentityChangeId, EntityIdentityChangeRegistration, EpistemicStatus, Event, EventId,
    EventPayload, EvidenceId, EvidenceItem, EvidenceLocator, EvidenceRole, EvidenceStrength,
    MasteryLevel, MediaType, PermissionLineageId, PredicateId, RetentionClass, ScopeDescriptor,
    ScopeId, TimestampMillis, UnsignedBatch, ValidInterval,
    entity_registry::{
        Alias, AliasKind, EntityKind, EntityRegistry, EquivalenceClass, IdentityAnchor,
        ImpactPreview, MentionContext, MentionResolution, ObservedState, OntologyChangeProposal,
        OntologyImpactSnapshot, PREDICATE_MERGED_INTO, RegistryError, RegistryFact,
    },
};
use academic_vault::{ArtifactIngestRequest, DomainKeyring, SealedObjectCapability, Vault};
use rusqlite::{Connection, OpenFlags, TransactionBehavior, params};

use crate::{
    aggregate_closure_tests::{apply_schema_two_canonical_core, typed_id},
    migration::apply_aggregate_migration_pre_listen,
    repository::ClosureWriter,
};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

/// Predicate the golden history records a mastery observation under.
///
/// Knowledge state is not this task's aggregate. The registry needs only a value
/// attached to an identity so an ontology change can be shown not to move it.
const PREDICATE_MASTERY: &str = "academic.mastery";

const DOMAIN: u32 = 0x0001;
const SCOPE: u32 = 0x0002;
const DEVICE: u32 = 0x0003;
const LINEAGE: u32 = 0x0004;
const BATCH_HISTORY: u32 = 0x0010;
const BATCH_CHANGE: u32 = 0x0011;

const CON_MVCC: u32 = 0x1001;
const CON_MVCC_DUP: u32 = 0x1002;
const CON_CACHE: u32 = 0x1003;
const CON_CPU_CACHE: u32 = 0x1004;
const CON_WEB_CACHE: u32 = 0x1005;
const CON_DB_CACHE: u32 = 0x1006;
const CON_BTREE: u32 = 0x1007;
const CON_PAXOS: u32 = 0x1008;

const ALIAS_MVCC_ABBREVIATION: u32 = 0x2001;
const ALIAS_MVCC_PREFERRED: u32 = 0x2002;
const ALIAS_MVCC_KOREAN: u32 = 0x2003;
const ALIAS_DUP_PREFERRED: u32 = 0x2004;
const ALIAS_CACHE: u32 = 0x2005;
const ALIAS_CPU_CACHE: u32 = 0x2006;
const ALIAS_WEB_CACHE: u32 = 0x2007;
const ALIAS_DB_CACHE: u32 = 0x2008;
const ALIAS_BTREE_VERSIONED: u32 = 0x2009;

const YEAR_ONE: i64 = 1_000;
const YEAR_TWO: i64 = 2_000;
const YEAR_THREE: i64 = 3_000;
const YEAR_FOUR: i64 = 4_000;
const YEAR_FIVE: i64 = 5_000;

/// A disposable profile root holding the migrated store and its vault.
struct RegistryDatabase {
    root: PathBuf,
    database_path: PathBuf,
    vault: Vault,
}

impl RegistryDatabase {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
        let root = temporary_base()?.join(format!(
            "academic-store-c3-{label}-{}-{sequence}",
            std::process::id()
        ));
        create_private_root(&root)?;
        let database_path = root.join("registry.sqlite3");
        let mut connection = open_at(&database_path)?;
        apply_schema_two_canonical_core(&connection)?;
        apply_aggregate_migration_pre_listen(&mut connection)?;

        let mut keyring = DomainKeyring::new();
        keyring.insert(
            typed_id::<DomainId>(DOMAIN)?,
            b"academic-c3-test-locator-key",
        )?;
        let vault = Vault::open(&root, keyring)?;
        Ok(Self {
            root,
            database_path,
            vault,
        })
    }

    fn open(&self) -> Result<Connection, Box<dyn Error>> {
        open_at(&self.database_path)
    }
}

impl Drop for RegistryDatabase {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.root) {
            eprintln!("test cleanup failed for {}: {error}", self.root.display());
        }
    }
}

/// macOS exposes `$TMPDIR` beneath the `/var` symlink, so the tests address the
/// real directory the way `tests/acceptance.rs` does.
#[cfg(unix)]
fn temporary_base() -> std::io::Result<PathBuf> {
    fs::canonicalize(std::env::temp_dir())
}

/// Windows must not canonicalize: that yields the verbatim device spelling the
/// path facade rejects.
#[cfg(windows)]
fn temporary_base() -> std::io::Result<PathBuf> {
    Ok(std::env::temp_dir())
}

/// Creates the profile root owner-only.
///
/// The vault refuses any directory whose Unix mode grants group or other access,
/// and `/tmp` is world-writable, so a default-mode directory is rejected on Linux
/// while passing on Windows, where the mode check does not apply.
#[cfg(unix)]
fn create_private_root(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;

    fs::DirBuilder::new().mode(0o700).create(path)
}

/// Creates the profile root. Windows carries no Unix mode to set.
#[cfg(windows)]
fn create_private_root(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

fn open_at(path: &Path) -> Result<Connection, Box<dyn Error>> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.execute_batch("PRAGMA foreign_keys = ON;")?;
    Ok(connection)
}

// ---------------------------------------------------------------------------
// Fixture construction
// ---------------------------------------------------------------------------

/// One artifact's worth of synthetic bytes plus everything derived from them.
///
/// The sealed capability is deliberately absent: `SealedObjectCapability` is not
/// `Clone`, because a mintable capability would be a byte/hash bypass around the
/// store to vault seam. Each batch re-verifies the object instead.
struct SyntheticArtifact {
    descriptor: ArtifactDescriptor,
    evidence: EvidenceItem,
}

/// Ingests bytes into the vault and mints the registered descriptor and evidence.
///
/// The descriptor is registered with one whole-artifact representation, so the
/// excerpt digest the evidence carries is the artifact digest itself, which is
/// what `append_evidence` requires.
fn synthetic_artifact(
    vault: &Vault,
    artifact: u32,
    evidence: u32,
    bytes: &[u8],
) -> Result<SyntheticArtifact, Box<dyn Error>> {
    let artifact_id = typed_id::<ArtifactId>(artifact)?;
    let domain_id = typed_id::<DomainId>(DOMAIN)?;
    let request = ArtifactIngestRequest::new(
        artifact_id,
        MediaType::parse("text/plain")?,
        domain_id,
        Confidentiality::Personal,
        RetentionClass::UserManaged,
        typed_id::<PermissionLineageId>(LINEAGE)?,
    );
    let receipt = vault.ingest(&request, bytes)?;
    let digest = ContentDigest::sha256(bytes);
    let length = u64::try_from(bytes.len())?;
    let locator = EvidenceLocator::TextBytes {
        source_digest: digest,
        start: 0,
        end: length,
    };
    let mut descriptor = receipt.descriptor().clone();
    descriptor.evidence_representations = vec![ArtifactRepresentation {
        locator: locator.clone(),
        content_digest: digest,
        byte_length: length,
    }];
    Ok(SyntheticArtifact {
        descriptor,
        evidence: EvidenceItem {
            id: typed_id::<EvidenceId>(evidence)?,
            artifact_id,
            locator,
            excerpt_digest: digest,
            role: EvidenceRole::Supports,
            strength: EvidenceStrength::Direct,
            extraction_method: "academic.c3.synthetic".to_owned(),
            extractor_version: "1.0.0".to_owned(),
        },
    })
}

/// One entity the curated ontology import registers.
#[derive(Debug, Clone, Copy)]
struct EntitySpec<'label> {
    entity: u32,
    kind: EntityKind,
    label: &'label str,
    language: &'label str,
    /// The ambiguous concept this identity disambiguates, for a `CONCEPT_SENSE`.
    sense_of: Option<u32>,
}

/// One alias the curated ontology import registers.
#[derive(Debug, Clone, Copy)]
struct AliasSpec<'text> {
    alias: u32,
    entity: u32,
    text: &'text str,
    language: &'text str,
    kind: AliasKind,
    version: Option<&'text str>,
}

/// Accumulates one batch's events and the sealed receipts they need.
struct BatchBuilder {
    /// Where this batch continues the device's origin chain.
    origin_seq_start: u64,
    events: Vec<Event>,
    receipts: BTreeMap<ArtifactId, SealedObjectCapability>,
    next_event: u32,
    next_claim: u32,
    next_change: u32,
    importer: Actor,
    user: Actor,
    domain_id: DomainId,
    scope_id: ScopeId,
}

impl BatchBuilder {
    fn new(
        origin_seq_start: u64,
        event_base: u32,
        claim_base: u32,
        change_base: u32,
    ) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            origin_seq_start,
            events: Vec::new(),
            receipts: BTreeMap::new(),
            next_event: event_base,
            next_claim: claim_base,
            next_change: change_base,
            importer: Actor::Importer {
                name: "academic.c3.ontology".to_owned(),
                version: "1.0.0".to_owned(),
            },
            user: Actor::User {
                user_id: typed_id::<EntityId>(0x0f01)?,
            },
            domain_id: typed_id::<DomainId>(DOMAIN)?,
            scope_id: typed_id::<ScopeId>(SCOPE)?,
        })
    }

    fn push(&mut self, actor: Actor, payload: EventPayload) -> Result<(), Box<dyn Error>> {
        let origin_seq = self.origin_seq_start + u64::try_from(self.events.len())?;
        let event = Event {
            id: typed_id::<EventId>(self.next_event)?,
            origin_seq,
            origin_observed_at: TimestampMillis::new(100),
            actor,
            domain_id: self.domain_id,
            payload,
        };
        self.next_event += 1;
        event.validate()?;
        self.events.push(event);
        Ok(())
    }

    fn push_scope(&mut self, label: &str) -> Result<(), Box<dyn Error>> {
        let payload = EventPayload::ScopeRegistered(ScopeDescriptor {
            id: self.scope_id,
            domain_id: self.domain_id,
            label: label.to_owned(),
        });
        let actor = self.importer.clone();
        self.push(actor, payload)
    }

    fn push_artifact(
        &mut self,
        artifact: &SyntheticArtifact,
        vault: &Vault,
    ) -> Result<(), Box<dyn Error>> {
        self.receipts.insert(
            artifact.descriptor.id,
            vault.verify_sealed_object(&artifact.descriptor)?,
        );
        let actor = self.importer.clone();
        self.push(
            actor.clone(),
            EventPayload::ArtifactRegistered(artifact.descriptor.clone()),
        )?;
        self.push(
            actor,
            EventPayload::EvidenceRegistered(artifact.evidence.clone()),
        )
    }

    /// Appends the `ENTITY_IDENTITY_CHANGED` anchor a registry fact needs.
    fn push_anchor(&mut self, entity: u32, from: i64) -> Result<(), Box<dyn Error>> {
        let payload = EventPayload::EntityIdentityChanged(EntityIdentityChangeRegistration {
            id: typed_id::<EntityIdentityChangeId>(self.next_change)?,
            entity_id: typed_id::<EntityId>(entity)?,
            domain_id: self.domain_id,
            scope_id: self.scope_id,
            source_digest: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(from)),
        });
        self.next_change += 1;
        let actor = self.importer.clone();
        self.push(actor, payload)
    }

    /// Appends one registry fact as a curated claim asserted by the importer.
    fn push_curated_fact(
        &mut self,
        fact: &RegistryFact,
        evidence: EvidenceId,
        from: i64,
    ) -> Result<(), Box<dyn Error>> {
        let claim = Claim {
            id: typed_id::<ClaimId>(self.next_claim)?,
            subject_entity_id: fact.subject(),
            predicate_id: fact.predicate_id()?,
            object: fact.object(),
            scope_id: self.scope_id,
            authority_class: AuthorityClass::Curated,
            epistemic_status: EpistemicStatus::DeterministicDerived,
            confidence: None,
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(from)),
            evidence_ids: vec![evidence],
        };
        self.next_claim += 1;
        let actor = self.importer.clone();
        self.push(actor, EventPayload::ClaimAsserted(claim))
    }

    /// Appends one registry fact as a user-approved identity decision.
    ///
    /// The canonical writer already refuses `USER_EXPLICIT` authority from any
    /// actor but the user, and `USER_CONFIRMED` pairs with no other authority, so
    /// this is the only shape a merge or a split can reach the store in.
    fn push_user_fact(
        &mut self,
        fact: &RegistryFact,
        evidence: EvidenceId,
        from: i64,
    ) -> Result<(), Box<dyn Error>> {
        let claim = Claim {
            id: typed_id::<ClaimId>(self.next_claim)?,
            subject_entity_id: fact.subject(),
            predicate_id: fact.predicate_id()?,
            object: fact.object(),
            scope_id: self.scope_id,
            authority_class: AuthorityClass::UserExplicit,
            epistemic_status: EpistemicStatus::UserConfirmed,
            confidence: None,
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(from)),
            evidence_ids: vec![evidence],
        };
        self.next_claim += 1;
        let actor = self.user.clone();
        self.push(actor, EventPayload::ClaimAsserted(claim))
    }

    /// Appends one mastery observation of the golden history.
    fn push_mastery(
        &mut self,
        entity: u32,
        mastery: MasteryLevel,
        evidence: EvidenceId,
        at: i64,
    ) -> Result<(), Box<dyn Error>> {
        let claim = Claim {
            id: typed_id::<ClaimId>(self.next_claim)?,
            subject_entity_id: typed_id::<EntityId>(entity)?,
            predicate_id: PredicateId::parse(PREDICATE_MASTERY)?,
            object: ClaimObject::Mastery(mastery),
            scope_id: self.scope_id,
            authority_class: AuthorityClass::DirectObservation,
            epistemic_status: EpistemicStatus::CodeObserved,
            confidence: None,
            prediction_metadata: None,
            valid_time: ValidInterval::open_ended(TimestampMillis::new(at)),
            evidence_ids: vec![evidence],
        };
        self.next_claim += 1;
        let actor = self.importer.clone();
        self.push(actor, EventPayload::ClaimAsserted(claim))
    }

    /// Registers one entity and the curated facts that describe it.
    fn push_entity(
        &mut self,
        spec: &EntitySpec<'_>,
        evidence: EvidenceId,
        from: i64,
    ) -> Result<(), Box<dyn Error>> {
        let EntitySpec {
            entity,
            kind,
            label,
            language,
            sense_of,
        } = *spec;
        let entity_id = typed_id::<EntityId>(entity)?;
        self.push_anchor(entity, from)?;
        self.push_curated_fact(
            &RegistryFact::EntityKindDeclared { entity_id, kind },
            evidence,
            from,
        )?;
        self.push_curated_fact(
            &RegistryFact::LabelDeclared {
                entity_id,
                text: label.to_owned(),
            },
            evidence,
            from,
        )?;
        self.push_curated_fact(
            &RegistryFact::LabelLanguageDeclared {
                entity_id,
                language: language.to_owned(),
            },
            evidence,
            from,
        )?;
        if let Some(concept) = sense_of {
            self.push_curated_fact(
                &RegistryFact::SenseOf {
                    sense_id: entity_id,
                    concept_id: typed_id::<EntityId>(concept)?,
                },
                evidence,
                from,
            )?;
        }
        Ok(())
    }

    /// Registers one alias entity and the four or five facts that describe it.
    fn push_alias(
        &mut self,
        spec: &AliasSpec<'_>,
        evidence: EvidenceId,
        from: i64,
    ) -> Result<(), Box<dyn Error>> {
        let AliasSpec {
            alias,
            entity,
            text,
            language,
            kind,
            version,
        } = *spec;
        let alias_id = typed_id::<EntityId>(alias)?;
        self.push_anchor(alias, from)?;
        self.push_curated_fact(
            &RegistryFact::AliasOf {
                alias_id,
                entity_id: typed_id::<EntityId>(entity)?,
            },
            evidence,
            from,
        )?;
        self.push_curated_fact(
            &RegistryFact::AliasText {
                alias_id,
                text: text.to_owned(),
            },
            evidence,
            from,
        )?;
        self.push_curated_fact(
            &RegistryFact::AliasLanguage {
                alias_id,
                language: language.to_owned(),
            },
            evidence,
            from,
        )?;
        self.push_curated_fact(
            &RegistryFact::AliasKindDeclared { alias_id, kind },
            evidence,
            from,
        )?;
        if let Some(version) = version {
            self.push_curated_fact(
                &RegistryFact::AliasVersion {
                    alias_id,
                    version: version.to_owned(),
                },
                evidence,
                from,
            )?;
        }
        Ok(())
    }

    /// Closes the batch, chaining it to its predecessor when it has one.
    ///
    /// `ledger_batch` requires `previous_batch_hash` exactly when the batch does
    /// not start the device's origin chain, so a follow-on batch carries one.
    fn finish(
        self,
        batch: u32,
        previous_batch_hash: Option<ContentDigest>,
    ) -> Result<(UnsignedBatch, ReceiptMap), Box<dyn Error>> {
        let count = u64::try_from(self.events.len())?;
        let unsigned = UnsignedBatch {
            schema_version: academic_domain::EVENT_SCHEMA_VERSION,
            batch_id: typed_id::<BatchId>(batch)?,
            device_id: typed_id(DEVICE)?,
            origin_seq_start: self.origin_seq_start,
            origin_seq_end: self.origin_seq_start + count - 1,
            previous_batch_hash,
            origin_created_at: TimestampMillis::new(100),
            events: self.events,
        };
        unsigned.validate()?;
        Ok((unsigned, self.receipts))
    }
}

type ReceiptMap = BTreeMap<ArtifactId, SealedObjectCapability>;

/// One `claim_evidence` row: claim, evidence, and the ordinal binding them.
type ClaimEvidenceLink = (Vec<u8>, Vec<u8>, i64);

/// One canonical event exactly as the store holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct StoredEvent {
    event_id: Vec<u8>,
    event_kind: String,
    canonical_payload: Vec<u8>,
    payload_hash: Vec<u8>,
}

/// Inserts the `ledger_batch` row every event in the batch references.
///
/// The envelope material is synthetic filler that satisfies the column CHECKs;
/// these tests exercise the closure writer and the migration guards, not
/// signature verification, so no signing key is involved.
fn seed_batch(
    transaction: &rusqlite::Transaction<'_>,
    batch: &UnsignedBatch,
    accept_seq_start: u64,
    filler: u8,
) -> Result<(), Box<dyn Error>> {
    let count = i64::try_from(batch.events.len())?;
    let start = i64::try_from(accept_seq_start)?;
    transaction.execute(
        concat!(
            "INSERT INTO ledger_batch (batch_id, signed_envelope, envelope_hash, ",
            "deterministic_payload, deterministic_payload_hash, signing_public_key, ",
            "signature, device_id, origin_seq_start, origin_seq_end, previous_batch_hash, ",
            "origin_created_at, event_schema_version, accept_seq_start, accept_seq_end, ",
            "accepted_at) ",
            "VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 100, 3, ?12, ?13, 100)"
        ),
        params![
            batch.batch_id.as_bytes().as_slice(),
            vec![filler; 8],
            vec![filler ^ 0x11; 32],
            vec![filler ^ 0x23; 8],
            vec![filler ^ 0x12; 32],
            vec![0x13_u8; 32],
            vec![0x33_u8; 64],
            batch.device_id.as_bytes().as_slice(),
            i64::try_from(batch.origin_seq_start)?,
            i64::try_from(batch.origin_seq_end)?,
            batch
                .previous_batch_hash
                .map(|digest| digest.as_bytes().to_vec()),
            start,
            start + count - 1,
        ],
    )?;
    Ok(())
}

/// Writes one batch through the real closure writer and commits it.
fn accept_batch(
    connection: &mut Connection,
    batch: &UnsignedBatch,
    receipts: &ReceiptMap,
    accept_seq_start: u64,
    filler: u8,
) -> Result<(), Box<dyn Error>> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Exclusive)?;
    seed_batch(&transaction, batch, accept_seq_start, filler)?;
    {
        let mut writer = ClosureWriter::new(&transaction, batch, receipts);
        for (offset, event) in batch.events.iter().enumerate() {
            writer.append_event(event, accept_seq_start + u64::try_from(offset)?)?;
        }
    }
    transaction.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// The golden multi-year history and the ontology change that follows it
// ---------------------------------------------------------------------------

/// Every artifact the fixture ingests, keyed by the role it plays.
struct FixtureEvidence {
    ontology: SyntheticArtifact,
    years: [SyntheticArtifact; 4],
    after_mvcc: SyntheticArtifact,
    /// The ambiguous concept is still observed after its split; that observation
    /// is exactly the value a naive comparison would report growth from.
    after_cache: SyntheticArtifact,
    after_cpu: SyntheticArtifact,
    after_web: SyntheticArtifact,
    after_db: SyntheticArtifact,
    after_btree: SyntheticArtifact,
    after_paxos: SyntheticArtifact,
    merge_preview: SyntheticArtifact,
    split_preview: SyntheticArtifact,
}

/// The impact snapshot the previews are computed against.
///
/// The registry does not own states, edges, or questions, so the counts are
/// supplied the way the owning projections will supply them.
fn impact_snapshot() -> Result<OntologyImpactSnapshot, Box<dyn Error>> {
    let mut snapshot = OntologyImpactSnapshot::default();
    let mvcc = typed_id::<EntityId>(CON_MVCC)?;
    let dup = typed_id::<EntityId>(CON_MVCC_DUP)?;
    let cache = typed_id::<EntityId>(CON_CACHE)?;
    let cpu = typed_id::<EntityId>(CON_CPU_CACHE)?;
    let web = typed_id::<EntityId>(CON_WEB_CACHE)?;
    let database = typed_id::<EntityId>(CON_DB_CACHE)?;

    snapshot.states.extend([(mvcc, 4), (dup, 3), (cache, 4)]);
    snapshot
        .edges
        .extend([(mvcc, 6), (dup, 2), (cache, 5), (cpu, 1), (web, 1)]);
    snapshot
        .questions
        .extend([(mvcc, 2), (dup, 1), (cache, 3), (database, 1)]);
    snapshot.evidence.insert(
        mvcc,
        [
            typed_id::<EvidenceId>(0x5001)?,
            typed_id::<EvidenceId>(0x5002)?,
        ]
        .into(),
    );
    snapshot
        .evidence
        .insert(dup, [typed_id::<EvidenceId>(0x5002)?].into());
    snapshot.evidence.insert(
        cache,
        [
            typed_id::<EvidenceId>(0x5001)?,
            typed_id::<EvidenceId>(0x5002)?,
            typed_id::<EvidenceId>(0x5003)?,
            typed_id::<EvidenceId>(0x5004)?,
        ]
        .into(),
    );
    Ok(snapshot)
}

fn merge_proposal() -> Result<OntologyChangeProposal, Box<dyn Error>> {
    Ok(OntologyChangeProposal::Merge {
        source: typed_id::<EntityId>(CON_MVCC_DUP)?,
        target: typed_id::<EntityId>(CON_MVCC)?,
    })
}

fn split_proposal() -> Result<OntologyChangeProposal, Box<dyn Error>> {
    Ok(OntologyChangeProposal::Split {
        source: typed_id::<EntityId>(CON_CACHE)?,
        targets: vec![
            typed_id::<EntityId>(CON_CPU_CACHE)?,
            typed_id::<EntityId>(CON_WEB_CACHE)?,
            typed_id::<EntityId>(CON_DB_CACHE)?,
        ],
    })
}

fn ingest_fixture_evidence(vault: &Vault) -> Result<FixtureEvidence, Box<dyn Error>> {
    let snapshot = impact_snapshot()?;
    let merge_preview = ImpactPreview::compute(&merge_proposal()?, &snapshot)?;
    let split_preview = ImpactPreview::compute(&split_proposal()?, &snapshot)?;
    Ok(FixtureEvidence {
        ontology: synthetic_artifact(vault, 0x4000, 0x5000, b"SYNTHETIC C3 ONTOLOGY IMPORT")?,
        years: [
            synthetic_artifact(vault, 0x4001, 0x5001, b"SYNTHETIC C3 YEAR ONE")?,
            synthetic_artifact(vault, 0x4002, 0x5002, b"SYNTHETIC C3 YEAR TWO")?,
            synthetic_artifact(vault, 0x4003, 0x5003, b"SYNTHETIC C3 YEAR THREE")?,
            synthetic_artifact(vault, 0x4004, 0x5004, b"SYNTHETIC C3 YEAR FOUR")?,
        ],
        after_mvcc: synthetic_artifact(vault, 0x4005, 0x5005, b"SYNTHETIC C3 YEAR FIVE MVCC")?,
        after_cache: synthetic_artifact(vault, 0x400b, 0x500b, b"SYNTHETIC C3 YEAR FIVE CACHE")?,
        after_cpu: synthetic_artifact(vault, 0x4006, 0x5006, b"SYNTHETIC C3 YEAR FIVE CPU CACHE")?,
        after_web: synthetic_artifact(vault, 0x4007, 0x5007, b"SYNTHETIC C3 YEAR FIVE WEB CACHE")?,
        after_db: synthetic_artifact(vault, 0x4008, 0x5008, b"SYNTHETIC C3 YEAR FIVE DB CACHE")?,
        after_btree: synthetic_artifact(vault, 0x4009, 0x5009, b"SYNTHETIC C3 YEAR FIVE BTREE")?,
        after_paxos: synthetic_artifact(vault, 0x400a, 0x500a, b"SYNTHETIC C3 YEAR FIVE PAXOS")?,
        merge_preview: synthetic_artifact(vault, 0x4010, 0x5010, &merge_preview.canonical_bytes())?,
        split_preview: synthetic_artifact(vault, 0x4011, 0x5011, &split_preview.canonical_bytes())?,
    })
}

/// Batch one: the curated ontology plus four years of mastery observations.
fn history_batch(
    evidence: &FixtureEvidence,
    vault: &Vault,
) -> Result<(UnsignedBatch, ReceiptMap), Box<dyn Error>> {
    let mut builder = BatchBuilder::new(1, 0x0100, 0x0800, 0x3000)?;
    builder.push_scope("synthetic c3 knowledge scope")?;
    builder.push_artifact(&evidence.ontology, vault)?;
    for year in &evidence.years {
        builder.push_artifact(year, vault)?;
    }
    let curated = evidence.ontology.evidence.id;

    builder.push_entity(
        &EntitySpec {
            entity: CON_MVCC,
            kind: EntityKind::Concept,
            label: "Multi-Version Concurrency Control",
            language: "en",
            sense_of: None,
        },
        curated,
        YEAR_ONE,
    )?;
    builder.push_entity(
        &EntitySpec {
            entity: CON_MVCC_DUP,
            kind: EntityKind::Concept,
            label: "MVCC (second import)",
            language: "en",
            sense_of: None,
        },
        curated,
        YEAR_TWO,
    )?;
    builder.push_entity(
        &EntitySpec {
            entity: CON_CACHE,
            kind: EntityKind::Concept,
            label: "Cache",
            language: "en",
            sense_of: None,
        },
        curated,
        YEAR_ONE,
    )?;
    builder.push_entity(
        &EntitySpec {
            entity: CON_BTREE,
            kind: EntityKind::Concept,
            label: "B+ Tree",
            language: "en",
            sense_of: None,
        },
        curated,
        YEAR_ONE,
    )?;

    builder.push_alias(
        &AliasSpec {
            alias: ALIAS_MVCC_ABBREVIATION,
            entity: CON_MVCC,
            text: "MVCC",
            language: "en",
            kind: AliasKind::Abbreviation,
            version: None,
        },
        curated,
        YEAR_ONE,
    )?;
    builder.push_alias(
        &AliasSpec {
            alias: ALIAS_MVCC_PREFERRED,
            entity: CON_MVCC,
            text: "Multi-Version Concurrency Control",
            language: "en",
            kind: AliasKind::Preferred,
            version: None,
        },
        curated,
        YEAR_ONE,
    )?;
    builder.push_alias(
        &AliasSpec {
            alias: ALIAS_MVCC_KOREAN,
            entity: CON_MVCC,
            text: "다중 버전 동시성 제어",
            language: "ko",
            kind: AliasKind::Translation,
            version: None,
        },
        curated,
        YEAR_ONE,
    )?;
    builder.push_alias(
        &AliasSpec {
            alias: ALIAS_DUP_PREFERRED,
            entity: CON_MVCC_DUP,
            text: "MVCC (second import)",
            language: "en",
            kind: AliasKind::Preferred,
            version: None,
        },
        curated,
        YEAR_TWO,
    )?;
    builder.push_alias(
        &AliasSpec {
            alias: ALIAS_CACHE,
            entity: CON_CACHE,
            text: "cache",
            language: "en",
            kind: AliasKind::Preferred,
            version: None,
        },
        curated,
        YEAR_ONE,
    )?;
    builder.push_alias(
        &AliasSpec {
            alias: ALIAS_BTREE_VERSIONED,
            entity: CON_BTREE,
            text: "B-Tree",
            language: "en",
            kind: AliasKind::Versioned,
            version: Some("catalog-v2"),
        },
        curated,
        YEAR_ONE,
    )?;

    let years = [
        (YEAR_ONE, evidence.years[0].evidence.id),
        (YEAR_TWO, evidence.years[1].evidence.id),
        (YEAR_THREE, evidence.years[2].evidence.id),
        (YEAR_FOUR, evidence.years[3].evidence.id),
    ];
    let history: [(u32, [Option<MasteryLevel>; 4]); 4] = [
        (
            CON_MVCC,
            [
                Some(MasteryLevel::Exposed),
                Some(MasteryLevel::Understood),
                Some(MasteryLevel::Practiced),
                Some(MasteryLevel::Applied),
            ],
        ),
        (
            CON_MVCC_DUP,
            [
                None,
                Some(MasteryLevel::Exposed),
                Some(MasteryLevel::Understood),
                Some(MasteryLevel::Understood),
            ],
        ),
        (
            CON_CACHE,
            [
                Some(MasteryLevel::Exposed),
                Some(MasteryLevel::Understood),
                Some(MasteryLevel::Practiced),
                Some(MasteryLevel::Practiced),
            ],
        ),
        (
            CON_BTREE,
            [
                Some(MasteryLevel::Understood),
                Some(MasteryLevel::Practiced),
                Some(MasteryLevel::Practiced),
                Some(MasteryLevel::Applied),
            ],
        ),
    ];
    for (entity, levels) in history {
        for (index, level) in levels.into_iter().enumerate() {
            if let Some(level) = level {
                let (at, evidence_id) = years[index];
                builder.push_mastery(entity, level, evidence_id, at)?;
            }
        }
    }
    builder.finish(BATCH_HISTORY, None)
}

/// Batch two: the ontology change, appended over the closed history.
fn change_batch(
    evidence: &FixtureEvidence,
    vault: &Vault,
    origin_seq_start: u64,
) -> Result<(UnsignedBatch, ReceiptMap), Box<dyn Error>> {
    let mut builder = BatchBuilder::new(origin_seq_start, 0x0300, 0x0a00, 0x3100)?;
    builder.push_artifact(&evidence.after_mvcc, vault)?;
    builder.push_artifact(&evidence.after_cache, vault)?;
    builder.push_artifact(&evidence.after_cpu, vault)?;
    builder.push_artifact(&evidence.after_web, vault)?;
    builder.push_artifact(&evidence.after_db, vault)?;
    builder.push_artifact(&evidence.after_btree, vault)?;
    builder.push_artifact(&evidence.after_paxos, vault)?;
    builder.push_artifact(&evidence.merge_preview, vault)?;
    builder.push_artifact(&evidence.split_preview, vault)?;
    let curated = evidence.after_mvcc.evidence.id;

    // The three senses the split produces, and one concept that did not exist
    // before the change at all.
    for (sense, label) in [
        (CON_CPU_CACHE, "CPU cache"),
        (CON_WEB_CACHE, "Web cache"),
        (CON_DB_CACHE, "Database buffer cache"),
    ] {
        builder.push_entity(
            &EntitySpec {
                entity: sense,
                kind: EntityKind::ConceptSense,
                label,
                language: "en",
                sense_of: Some(CON_CACHE),
            },
            curated,
            YEAR_FIVE,
        )?;
    }
    builder.push_entity(
        &EntitySpec {
            entity: CON_PAXOS,
            kind: EntityKind::Concept,
            label: "Paxos",
            language: "en",
            sense_of: None,
        },
        curated,
        YEAR_FIVE,
    )?;
    for (alias, sense, text) in [
        (ALIAS_CPU_CACHE, CON_CPU_CACHE, "cache"),
        (ALIAS_WEB_CACHE, CON_WEB_CACHE, "cache"),
        (ALIAS_DB_CACHE, CON_DB_CACHE, "buffer cache"),
    ] {
        builder.push_alias(
            &AliasSpec {
                alias,
                entity: sense,
                text,
                language: "en",
                kind: AliasKind::Preferred,
                version: None,
            },
            curated,
            YEAR_FIVE,
        )?;
    }

    // The merge: a new anchor for the identity that changes, then the approved
    // redirect citing the preview the user was shown.
    builder.push_anchor(CON_MVCC_DUP, YEAR_FIVE)?;
    builder.push_user_fact(
        &RegistryFact::MergedInto {
            source: typed_id::<EntityId>(CON_MVCC_DUP)?,
            target: typed_id::<EntityId>(CON_MVCC)?,
        },
        evidence.merge_preview.evidence.id,
        YEAR_FIVE,
    )?;

    // The split: successors first, then the reclassification queue. Nothing here
    // moves an evidence link; each queued row names evidence that stays attached
    // to the concept it was attached to.
    builder.push_anchor(CON_CACHE, YEAR_FIVE)?;
    for target in [CON_CPU_CACHE, CON_WEB_CACHE, CON_DB_CACHE] {
        builder.push_user_fact(
            &RegistryFact::SplitInto {
                source: typed_id::<EntityId>(CON_CACHE)?,
                target: typed_id::<EntityId>(target)?,
            },
            evidence.split_preview.evidence.id,
            YEAR_FIVE,
        )?;
    }
    for year in &evidence.years {
        builder.push_user_fact(
            &RegistryFact::ReclassificationPending {
                source: typed_id::<EntityId>(CON_CACHE)?,
                evidence_id: year.evidence.id,
            },
            evidence.split_preview.evidence.id,
            YEAR_FIVE,
        )?;
    }

    for (entity, mastery, item) in [
        (CON_MVCC, MasteryLevel::Fluent, &evidence.after_mvcc),
        (CON_CACHE, MasteryLevel::Applied, &evidence.after_cache),
        (CON_CPU_CACHE, MasteryLevel::Practiced, &evidence.after_cpu),
        (CON_WEB_CACHE, MasteryLevel::Exposed, &evidence.after_web),
        (CON_DB_CACHE, MasteryLevel::Unseen, &evidence.after_db),
        (CON_BTREE, MasteryLevel::Applied, &evidence.after_btree),
        (CON_PAXOS, MasteryLevel::Exposed, &evidence.after_paxos),
    ] {
        builder.push_mastery(entity, mastery, item.evidence.id, YEAR_FIVE)?;
    }
    builder.finish(
        BATCH_CHANGE,
        Some(ContentDigest::sha256(b"SYNTHETIC C3 HISTORY BATCH")),
    )
}

/// A migrated store holding the golden history, with the change not yet applied.
struct LoadedFixture {
    database: RegistryDatabase,
    evidence: FixtureEvidence,
    history_event_count: u64,
}

impl LoadedFixture {
    fn new(label: &str) -> Result<Self, Box<dyn Error>> {
        let database = RegistryDatabase::new(label)?;
        let evidence = ingest_fixture_evidence(&database.vault)?;
        let (batch, receipts) = history_batch(&evidence, &database.vault)?;
        let history_event_count = u64::try_from(batch.events.len())?;
        let mut connection = database.open()?;
        accept_batch(&mut connection, &batch, &receipts, 1, 0x21)?;
        Ok(Self {
            database,
            evidence,
            history_event_count,
        })
    }

    /// Appends the ontology change as a second batch.
    fn apply_ontology_change(&self) -> Result<(), Box<dyn Error>> {
        let (batch, receipts) = change_batch(
            &self.evidence,
            &self.database.vault,
            self.history_event_count + 1,
        )?;
        let mut connection = self.database.open()?;
        accept_batch(
            &mut connection,
            &batch,
            &receipts,
            self.history_event_count + 1,
            0x44,
        )?;
        Ok(())
    }

    fn registry(&self) -> Result<EntityRegistry, Box<dyn Error>> {
        let connection = self.database.open()?;
        let anchors = load_anchors(&connection)?;
        let claims = load_claims(&connection)?;
        Ok(EntityRegistry::build(&anchors, &claims)?)
    }
}

// ---------------------------------------------------------------------------
// Reading canonical state back out of the store
// ---------------------------------------------------------------------------

fn load_anchors(connection: &Connection) -> Result<Vec<IdentityAnchor>, Box<dyn Error>> {
    let mut statement = connection.prepare(concat!(
        "SELECT entity_identity_change_id, entity_id, domain_id, scope_id, valid_from, valid_to ",
        "FROM entity_identity_change ORDER BY entity_identity_change_id"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, Vec<u8>>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, Option<i64>>(5)?,
        ))
    })?;
    let mut anchors = Vec::new();
    for row in rows {
        let (change, entity, domain, scope, from, to) = row?;
        anchors.push(IdentityAnchor {
            change_id: id_from_blob::<EntityIdentityChangeId>(&change)?,
            entity_id: id_from_blob::<EntityId>(&entity)?,
            domain_id: id_from_blob::<DomainId>(&domain)?,
            scope_id: id_from_blob::<ScopeId>(&scope)?,
            valid_time: interval(from, to)?,
        });
    }
    Ok(anchors)
}

/// Reads every claim back in local acceptance order.
///
/// Only the object kinds the fixture writes are reconstructed; any other kind is
/// an error rather than a silently dropped row, so a future fixture that writes
/// a new kind cannot quietly stop being read.
fn load_claims(connection: &Connection) -> Result<Vec<Claim>, Box<dyn Error>> {
    let mut statement = connection.prepare(concat!(
        "SELECT c.claim_id, c.subject_entity_id, c.predicate_id, c.scope_id, c.object_kind, ",
        "c.object_entity_id, c.object_text, c.authority_class, c.epistemic_status, ",
        "c.valid_from, c.valid_to ",
        "FROM claim c JOIN ledger_event e ON e.event_id = c.assertion_event_id ",
        "ORDER BY e.accept_seq"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<Vec<u8>>>(5)?,
            row.get::<_, Option<String>>(6)?,
            row.get::<_, String>(7)?,
            row.get::<_, String>(8)?,
            row.get::<_, i64>(9)?,
            row.get::<_, Option<i64>>(10)?,
        ))
    })?;
    let mut claims = Vec::new();
    for row in rows {
        let (claim, subject, predicate, scope, kind, entity, text, authority, status, from, to) =
            row?;
        let object = match (kind.as_str(), entity, text) {
            ("ENTITY", Some(bytes), _) => ClaimObject::Entity(id_from_blob::<EntityId>(&bytes)?),
            ("TEXT", _, Some(value)) => ClaimObject::Text(value),
            ("MASTERY", _, Some(value)) => ClaimObject::Mastery(mastery_from_str(&value)?),
            (other, _, _) => return Err(format!("unexpected stored object kind {other}").into()),
        };
        claims.push(Claim {
            id: id_from_blob::<ClaimId>(&claim)?,
            subject_entity_id: id_from_blob::<EntityId>(&subject)?,
            predicate_id: PredicateId::parse(predicate)?,
            object,
            scope_id: id_from_blob::<ScopeId>(&scope)?,
            authority_class: authority_from_str(&authority)?,
            epistemic_status: status_from_str(&status)?,
            confidence: None,
            prediction_metadata: None,
            valid_time: interval(from, to)?,
            evidence_ids: Vec::new(),
        });
    }
    Ok(claims)
}

/// Every `(claim, evidence)` pair as the store holds it, in a comparable form.
fn load_claim_evidence(
    connection: &Connection,
) -> Result<BTreeSet<ClaimEvidenceLink>, Box<dyn Error>> {
    let mut statement =
        connection.prepare("SELECT claim_id, evidence_id, evidence_ordinal FROM claim_evidence")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut pairs = BTreeSet::new();
    for row in rows {
        pairs.insert(row?);
    }
    Ok(pairs)
}

/// Every canonical event's identity and exact bytes, keyed by accept order.
fn load_event_bytes(connection: &Connection) -> Result<BTreeMap<i64, StoredEvent>, Box<dyn Error>> {
    let mut statement = connection.prepare(concat!(
        "SELECT accept_seq, event_id, event_kind, canonical_payload, payload_hash ",
        "FROM ledger_event ORDER BY accept_seq"
    ))?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, Vec<u8>>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Vec<u8>>(3)?,
            row.get::<_, Vec<u8>>(4)?,
        ))
    })?;
    let mut events = BTreeMap::new();
    for row in rows {
        let (accept_seq, event_id, event_kind, canonical_payload, payload_hash) = row?;
        events.insert(
            accept_seq,
            StoredEvent {
                event_id,
                event_kind,
                canonical_payload,
                payload_hash,
            },
        );
    }
    Ok(events)
}

/// Replays mastery observations at one valid-time instant out of the ledger.
fn observed_states(connection: &Connection, at: i64) -> Result<Vec<ObservedState>, Box<dyn Error>> {
    let mut statement = connection.prepare(concat!(
        "SELECT c.subject_entity_id, c.object_text, c.valid_from ",
        "FROM claim c JOIN ledger_event e ON e.event_id = c.assertion_event_id ",
        "WHERE c.predicate_id = ?1 AND c.valid_from = ?2 ORDER BY e.accept_seq"
    ))?;
    let rows = statement.query_map(params![PREDICATE_MASTERY, at], |row| {
        Ok((
            row.get::<_, Vec<u8>>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })?;
    let mut states = Vec::new();
    for row in rows {
        let (subject, mastery, from) = row?;
        let Some(mastery) = mastery else {
            return Err("mastery claim stored no object text".into());
        };
        states.push(ObservedState {
            entity_id: id_from_blob::<EntityId>(&subject)?,
            mastery: mastery_from_str(&mastery)?,
            observed_at: TimestampMillis::new(from),
        });
    }
    Ok(states)
}

fn id_from_blob<T>(bytes: &[u8]) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    if bytes.len() != 16 {
        return Err(format!("identifier column held {} bytes", bytes.len()).into());
    }
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    let text = format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    );
    text.parse::<T>()
        .map_err(|error| format!("{text} is not a valid identifier: {error}").into())
}

fn interval(from: i64, to: Option<i64>) -> Result<ValidInterval, Box<dyn Error>> {
    Ok(match to {
        Some(to) => ValidInterval::new(TimestampMillis::new(from), Some(TimestampMillis::new(to)))?,
        None => ValidInterval::open_ended(TimestampMillis::new(from)),
    })
}

fn mastery_from_str(value: &str) -> Result<MasteryLevel, Box<dyn Error>> {
    Ok(match value {
        "UNSEEN" => MasteryLevel::Unseen,
        "EXPOSED" => MasteryLevel::Exposed,
        "UNDERSTOOD" => MasteryLevel::Understood,
        "PRACTICED" => MasteryLevel::Practiced,
        "APPLIED" => MasteryLevel::Applied,
        "FLUENT" => MasteryLevel::Fluent,
        other => return Err(format!("unexpected stored mastery level {other}").into()),
    })
}

fn authority_from_str(value: &str) -> Result<AuthorityClass, Box<dyn Error>> {
    Ok(match value {
        "OFFICIAL" => AuthorityClass::Official,
        "USER_EXPLICIT" => AuthorityClass::UserExplicit,
        "DIRECT_OBSERVATION" => AuthorityClass::DirectObservation,
        "DETERMINISTIC_ENGINE" => AuthorityClass::DeterministicEngine,
        "CURATED" => AuthorityClass::Curated,
        "MODEL_INFERENCE" => AuthorityClass::ModelInference,
        "PREDICTION" => AuthorityClass::Prediction,
        "UNKNOWN" => AuthorityClass::Unknown,
        other => return Err(format!("unexpected stored authority class {other}").into()),
    })
}

fn status_from_str(value: &str) -> Result<EpistemicStatus, Box<dyn Error>> {
    Ok(match value {
        "OFFICIAL_CONFIRMED" => EpistemicStatus::OfficialConfirmed,
        "USER_CONFIRMED" => EpistemicStatus::UserConfirmed,
        "CODE_OBSERVED" => EpistemicStatus::CodeObserved,
        "DETERMINISTIC_DERIVED" => EpistemicStatus::DeterministicDerived,
        "AI_INFERRED" => EpistemicStatus::AiInferred,
        "PREDICTION" => EpistemicStatus::Prediction,
        "DISPUTED" => EpistemicStatus::Disputed,
        "SUPERSEDED" => EpistemicStatus::Superseded,
        "UNKNOWN" => EpistemicStatus::Unknown,
        other => return Err(format!("unexpected stored epistemic status {other}").into()),
    })
}

// ---------------------------------------------------------------------------
// Named acceptance evidence
// ---------------------------------------------------------------------------

/// A merge keeps both identifiers resolvable and rewrites no evidence link.
#[test]
fn merge_preserves_ids_and_redirects() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("merge")?;
    let connection = fixture.database.open()?;
    let before_links = load_claim_evidence(&connection)?;
    let before_registry = fixture.registry()?;
    let survivor = typed_id::<EntityId>(CON_MVCC)?;
    let merged = typed_id::<EntityId>(CON_MVCC_DUP)?;
    assert_eq!(before_registry.resolve_identity(merged), merged);

    fixture.apply_ontology_change()?;
    let registry = fixture.registry()?;

    // Both identifiers still resolve, and the merged-away one is not deleted.
    assert!(
        registry.entity(merged).is_some(),
        "merged identity vanished"
    );
    assert!(registry.entity(survivor).is_some());
    assert_eq!(registry.resolve_identity(merged), survivor);
    assert_eq!(registry.resolve_identity(survivor), survivor);
    assert!(registry.is_redirected(merged));
    assert!(!registry.is_redirected(survivor));
    assert_eq!(registry.redirects_into(survivor), vec![merged]);

    // Every alias the merged identity carried still resolves to the survivor.
    let resolved: BTreeSet<EntityId> = registry
        .aliases_of(merged)
        .into_iter()
        .map(|alias| registry.resolve_identity(alias.entity_id))
        .collect();
    assert_eq!(resolved, [survivor].into());

    // Not one evidence link was rewritten by the merge.
    let after_links = load_claim_evidence(&connection)?;
    assert!(
        before_links.is_subset(&after_links),
        "the merge rewrote an existing claim/evidence link"
    );
    Ok(())
}

/// A split enqueues every affected evidence item and reassigns none of them.
#[test]
fn split_creates_reclassification_queue_and_moves_nothing() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("split")?;
    let connection = fixture.database.open()?;
    let before_links = load_claim_evidence(&connection)?;
    fixture.apply_ontology_change()?;
    let registry = fixture.registry()?;
    let source = typed_id::<EntityId>(CON_CACHE)?;
    let successors = vec![
        typed_id::<EntityId>(CON_CPU_CACHE)?,
        typed_id::<EntityId>(CON_WEB_CACHE)?,
        typed_id::<EntityId>(CON_DB_CACHE)?,
    ];

    assert_eq!(registry.split_targets(source), successors);

    // Every year of the source's evidence is queued, with the successors named
    // as candidates and no decision taken.
    let queued: BTreeSet<EvidenceId> = registry
        .reclassification_queue()
        .iter()
        .map(|item| item.evidence_id)
        .collect();
    let expected: BTreeSet<EvidenceId> = fixture
        .evidence
        .years
        .iter()
        .map(|year| year.evidence.id)
        .collect();
    assert_eq!(queued, expected, "the split did not queue its own evidence");
    for item in registry.reclassification_queue() {
        assert_eq!(item.source, source);
        assert_eq!(item.candidates, successors);
    }

    // Nothing moved: every pre-change link is still present, and no successor
    // acquired a link to the queued evidence.
    let after_links = load_claim_evidence(&connection)?;
    assert!(
        before_links.is_subset(&after_links),
        "the split rewrote an existing claim/evidence link"
    );
    let mut statement = connection.prepare(concat!(
        "SELECT count(*) FROM claim c JOIN claim_evidence l ON l.claim_id = c.claim_id ",
        "WHERE c.subject_entity_id IN (?1, ?2, ?3) AND l.evidence_id IN (?4, ?5, ?6, ?7)"
    ))?;
    let moved: i64 = statement.query_row(
        params![
            successors[0].as_bytes().as_slice(),
            successors[1].as_bytes().as_slice(),
            successors[2].as_bytes().as_slice(),
            fixture.evidence.years[0].evidence.id.as_bytes().as_slice(),
            fixture.evidence.years[1].evidence.id.as_bytes().as_slice(),
            fixture.evidence.years[2].evidence.id.as_bytes().as_slice(),
            fixture.evidence.years[3].evidence.id.as_bytes().as_slice(),
        ],
        |row| row.get(0),
    )?;
    assert_eq!(moved, 0, "the split redistributed evidence to a successor");
    Ok(())
}

/// An ambiguous surface form stays a mention until something actually decides it.
#[test]
fn ambiguous_mention_abstains() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("mention")?;
    fixture.apply_ontology_change()?;
    let registry = fixture.registry()?;
    let cache = typed_id::<EntityId>(CON_CACHE)?;
    let cpu = typed_id::<EntityId>(CON_CPU_CACHE)?;
    let web = typed_id::<EntityId>(CON_WEB_CACHE)?;

    // Three identities carry "cache" in English. Nothing narrows them, so the
    // resolver abstains and returns the candidates rather than picking one.
    let empty = MentionContext::default();
    match registry.resolve_mention("cache", "en", &empty) {
        MentionResolution::Unresolved { candidates } => {
            assert_eq!(candidates, vec![cache, cpu, web]);
        }
        other => return Err(format!("ambiguous mention did not abstain: {other:?}").into()),
    }

    // Context that names exactly one candidate resolves it; context that names
    // two does not, because a tie is still ambiguous.
    let decided = MentionContext {
        established_entities: [cpu].into(),
    };
    assert_eq!(
        registry.resolve_mention("cache", "en", &decided),
        MentionResolution::Resolved { entity_id: cpu }
    );
    let tied = MentionContext {
        established_entities: [cpu, web].into(),
    };
    assert!(matches!(
        registry.resolve_mention("cache", "en", &tied),
        MentionResolution::Unresolved { .. }
    ));

    // An unambiguous multilingual alias still resolves without any context, and
    // an unknown form is reported as unknown rather than guessed at.
    assert_eq!(
        registry.resolve_mention("다중 버전 동시성 제어", "ko", &empty),
        MentionResolution::Resolved {
            entity_id: typed_id::<EntityId>(CON_MVCC)?
        }
    );
    assert_eq!(
        registry.resolve_mention("cache", "ko", &empty),
        MentionResolution::Unknown
    );
    Ok(())
}

/// Separated senses hold disjoint evidence and never inherit the source's.
#[test]
fn homonym_split_keeps_evidence_separate() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("homonym")?;
    fixture.apply_ontology_change()?;
    let registry = fixture.registry()?;
    let connection = fixture.database.open()?;
    let cache = typed_id::<EntityId>(CON_CACHE)?;

    // Each sense is a CONCEPT_SENSE of the ambiguous concept, not a copy of it.
    let mut senses = Vec::new();
    for sense in [CON_CPU_CACHE, CON_WEB_CACHE, CON_DB_CACHE] {
        let sense_id = typed_id::<EntityId>(sense)?;
        let entity = registry
            .entity(sense_id)
            .ok_or("split successor is not registered")?;
        assert_eq!(entity.kind, EntityKind::ConceptSense);
        assert_eq!(entity.sense_of, Some(cache));
        senses.push(sense_id);
    }

    let mut statement = connection.prepare(concat!(
        "SELECT l.evidence_id FROM claim c JOIN claim_evidence l ON l.claim_id = c.claim_id ",
        "WHERE c.subject_entity_id = ?1 AND c.predicate_id = ?2"
    ))?;
    let mut evidence_for = |entity: EntityId| -> Result<BTreeSet<Vec<u8>>, Box<dyn Error>> {
        let rows = statement.query_map(
            params![entity.as_bytes().as_slice(), PREDICATE_MASTERY],
            |row| row.get::<_, Vec<u8>>(0),
        )?;
        let mut set = BTreeSet::new();
        for row in rows {
            set.insert(row?);
        }
        Ok(set)
    };

    let source_evidence = evidence_for(cache)?;
    assert!(
        !source_evidence.is_empty(),
        "the homonym carried no evidence"
    );
    let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
    for sense in senses {
        let sense_evidence = evidence_for(sense)?;
        assert!(
            sense_evidence.is_disjoint(&source_evidence),
            "a sense inherited the ambiguous concept's evidence"
        );
        assert!(
            sense_evidence.is_disjoint(&seen),
            "two senses share an evidence item"
        );
        seen.extend(sense_evidence);
    }
    Ok(())
}

/// The counts a user is shown are computed before approval and bound to it.
#[test]
fn ontology_change_preview_shows_state_edge_question_counts() -> Result<(), Box<dyn Error>> {
    let snapshot = impact_snapshot()?;
    let merge = ImpactPreview::compute(&merge_proposal()?, &snapshot)?;
    // The merge touches both sides: 4 + 3 states, 6 + 2 edges, 2 + 1 questions,
    // and the union of two evidence sets that overlap in one item.
    assert_eq!(merge.state_count, 7);
    assert_eq!(merge.edge_count, 8);
    assert_eq!(merge.question_count, 3);
    assert_eq!(merge.evidence_count, 2);

    let split = ImpactPreview::compute(&split_proposal()?, &snapshot)?;
    assert_eq!(split.state_count, 4);
    assert_eq!(split.edge_count, 7);
    assert_eq!(split.question_count, 4);
    assert_eq!(split.evidence_count, 4);

    // The approval carries the preview digest as its evidence excerpt, so an
    // approval recorded against different counts cannot verify.
    let fixture = LoadedFixture::new("preview")?;
    fixture.apply_ontology_change()?;
    let connection = fixture.database.open()?;
    let mut statement = connection.prepare(concat!(
        "SELECT e.excerpt_digest FROM claim c ",
        "JOIN claim_evidence l ON l.claim_id = c.claim_id ",
        "JOIN evidence_item e ON e.evidence_id = l.evidence_id ",
        "WHERE c.predicate_id = ?1"
    ))?;
    let cited: Vec<Vec<u8>> = statement
        .query_map(params![PREDICATE_MERGED_INTO], |row| {
            row.get::<_, Vec<u8>>(0)
        })?
        .collect::<Result<_, _>>()?;
    assert_eq!(cited.len(), 1, "the merge cited no single preview");
    assert_eq!(cited[0], merge.digest().as_bytes().as_slice());
    merge.verify_cited(ContentDigest::sha256(&merge.canonical_bytes()))?;

    // A preview over a changed snapshot produces a different digest, so the
    // stored approval stops verifying rather than silently covering new counts.
    let mut inflated = snapshot;
    inflated.states.insert(typed_id::<EntityId>(CON_MVCC)?, 40);
    let restated = ImpactPreview::compute(&merge_proposal()?, &inflated)?;
    assert!(matches!(
        restated.verify_cited(ContentDigest::from_sha256_bytes(
            cited[0]
                .as_slice()
                .try_into()
                .map_err(|_| "stored digest is not 32 bytes")?
        )),
        Err(RegistryError::PreviewDigestMismatch { .. })
    ));
    Ok(())
}

/// Replaying the golden history across the change reproduces it byte for byte.
///
/// This is the executable form of the migration-equivalence claim: the ontology
/// change appends, the pre-change ledger bytes are identical afterwards, the
/// pre-change states replay to the same values, and the canonical tables refuse
/// the mutation that would be needed to make it otherwise.
#[test]
fn historical_state_is_not_silently_distorted() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("replay")?;
    let connection = fixture.database.open()?;

    let before_events = load_event_bytes(&connection)?;
    let before_links = load_claim_evidence(&connection)?;
    let before_states: Vec<Vec<ObservedState>> = [YEAR_ONE, YEAR_TWO, YEAR_THREE, YEAR_FOUR]
        .into_iter()
        .map(|year| observed_states(&connection, year))
        .collect::<Result<_, _>>()?;
    assert_eq!(
        u64::try_from(before_events.len())?,
        fixture.history_event_count
    );

    fixture.apply_ontology_change()?;

    let after_events = load_event_bytes(&connection)?;
    assert!(
        after_events.len() > before_events.len(),
        "the ontology change appended nothing"
    );
    for (accept_seq, before) in &before_events {
        let after = after_events
            .get(accept_seq)
            .ok_or("a pre-change event disappeared")?;
        assert_eq!(before, after, "event at accept_seq {accept_seq} changed");
    }

    let after_links = load_claim_evidence(&connection)?;
    assert!(
        before_links.is_subset(&after_links),
        "an existing claim/evidence link was rewritten"
    );

    let replayed: Vec<Vec<ObservedState>> = [YEAR_ONE, YEAR_TWO, YEAR_THREE, YEAR_FOUR]
        .into_iter()
        .map(|year| observed_states(&connection, year))
        .collect::<Result<_, _>>()?;
    assert_eq!(
        before_states, replayed,
        "the ontology change moved a historical state"
    );

    // The append-only guard is what makes the property above structural rather
    // than a coincidence of this fixture.
    for (table, mutation) in [
        (
            "ledger_event",
            "UPDATE ledger_event SET canonical_payload = x'00'",
        ),
        ("claim", "UPDATE claim SET subject_entity_id = x'00'"),
        (
            "claim_evidence",
            "UPDATE claim_evidence SET evidence_ordinal = 99",
        ),
        (
            "evidence_item",
            "UPDATE evidence_item SET representation_index = 99",
        ),
    ] {
        let update = connection.execute(mutation, []);
        let delete = connection.execute(&format!("DELETE FROM {table}"), []);
        for outcome in [&update, &delete] {
            assert!(
                outcome
                    .as_ref()
                    .err()
                    .is_some_and(|error| error.to_string().contains("append-only")),
                "{table} accepted a mutation: {outcome:?}"
            );
        }
    }
    Ok(())
}

/// Every cross-change comparison carries the class that licenses or refuses it.
#[test]
fn equivalence_class_is_reported_for_every_cross_change_comparison() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("equivalence")?;
    let connection = fixture.database.open()?;
    let before = observed_states(&connection, YEAR_FOUR)?;
    fixture.apply_ontology_change()?;
    let after = observed_states(&connection, YEAR_FIVE)?;
    let registry = fixture.registry()?;
    let comparisons = registry.compare_across_change(&before, &after);

    // Every node on either side appears exactly once, and every row carries a
    // class. There is no row without one, and no node was dropped.
    assert_eq!(comparisons.len(), before.len() + 4);
    for comparison in &comparisons {
        assert!(comparison.before.is_some() || comparison.after.is_some());
        assert_eq!(
            comparison.delta.is_some(),
            comparison.equivalence.permits_comparison(),
            "a delta and its equivalence class disagree: {comparison:?}"
        );
    }

    let class_of = |entity: u32| -> Result<EquivalenceClass, Box<dyn Error>> {
        let entity_id = typed_id::<EntityId>(entity)?;
        comparisons
            .iter()
            .find(|comparison| comparison.before == Some(entity_id))
            .map(|comparison| comparison.equivalence)
            .ok_or_else(|| "comparison is missing a pre-change node".into())
    };
    assert_eq!(class_of(CON_MVCC)?, EquivalenceClass::Identical);
    assert_eq!(class_of(CON_MVCC_DUP)?, EquivalenceClass::Refined);
    // The split source was observed again after the change. The correspondence
    // exists and both values are present, so only the class stops the comparison.
    assert_eq!(class_of(CON_CACHE)?, EquivalenceClass::SplitAmbiguous);
    let cache = typed_id::<EntityId>(CON_CACHE)?;
    assert!(
        after.iter().any(|state| state.entity_id == cache),
        "the split source has no post-change observation to withhold"
    );
    assert!(
        comparisons
            .iter()
            .any(|comparison| comparison.before == Some(cache)
                && comparison.after == Some(cache)
                && comparison.delta.is_none()),
        "a SPLIT_AMBIGUOUS node carried a delta"
    );
    assert_eq!(class_of(CON_BTREE)?, EquivalenceClass::Identical);

    // The three senses and the concept that did not exist before are reported as
    // having no predecessor rather than being matched to the split source.
    for entity in [CON_CPU_CACHE, CON_WEB_CACHE, CON_DB_CACHE, CON_PAXOS] {
        let entity_id = typed_id::<EntityId>(entity)?;
        let row = comparisons
            .iter()
            .find(|comparison| comparison.before.is_none() && comparison.after == Some(entity_id))
            .ok_or("post-change node is missing from the comparison")?;
        assert_eq!(row.equivalence, EquivalenceClass::Incomparable);
        assert!(row.delta.is_none());
    }
    Ok(())
}

/// A growth narrative counts only what its class licenses and says what it left out.
#[test]
fn incomparable_nodes_are_excluded_from_growth_narratives() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("growth")?;
    let connection = fixture.database.open()?;
    let before = observed_states(&connection, YEAR_FOUR)?;
    fixture.apply_ontology_change()?;
    let after = observed_states(&connection, YEAR_FIVE)?;
    let registry = fixture.registry()?;
    let comparisons = registry.compare_across_change(&before, &after);
    let narrative = EntityRegistry::growth_narrative(&comparisons);

    // Counted: the untouched concept and the two sides of the merge. Growth: the
    // two that deepened; the untouched concept did not move.
    assert_eq!(narrative.counted.len(), 3);
    assert_eq!(narrative.growth_count(), 2);

    // The split source is withheld, not counted, and the four nodes with no
    // predecessor are excluded. Both lists are reported.
    assert_eq!(narrative.withheld_split_ambiguous.len(), 1);
    assert_eq!(
        narrative.withheld_split_ambiguous[0].before,
        Some(typed_id::<EntityId>(CON_CACHE)?)
    );
    assert_eq!(narrative.excluded_incomparable.len(), 4);

    // Nothing that was excluded is also counted, and no counted row carries an
    // equivalence class that forbids comparison.
    let counted: BTreeSet<Option<EntityId>> = narrative
        .counted
        .iter()
        .map(|comparison| comparison.before)
        .collect();
    for exclusion in narrative
        .excluded_incomparable
        .iter()
        .chain(&narrative.withheld_split_ambiguous)
    {
        assert!(
            !counted.contains(&exclusion.before) || exclusion.before.is_none(),
            "an excluded node was also counted"
        );
        assert!(!exclusion.equivalence.permits_comparison());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Guards the named tests depend on
// ---------------------------------------------------------------------------

/// A registry fact whose subject has no 0004 anchor is refused, not ignored.
#[test]
fn registry_fact_without_an_identity_anchor_is_refused() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("unanchored")?;
    let connection = fixture.database.open()?;
    let anchors = load_anchors(&connection)?;
    let claims = load_claims(&connection)?;
    EntityRegistry::build(&anchors, &claims)?;

    let orphaned = typed_id::<EntityId>(CON_MVCC)?;
    let stripped: Vec<IdentityAnchor> = anchors
        .into_iter()
        .filter(|anchor| anchor.entity_id != orphaned)
        .collect();
    assert!(matches!(
        EntityRegistry::build(&stripped, &claims),
        Err(RegistryError::UnanchoredSubject { .. })
    ));
    Ok(())
}

/// A merge asserted below `USER_CONFIRMED` is refused at decode time.
#[test]
fn identity_change_below_user_authority_is_refused() -> Result<(), Box<dyn Error>> {
    let mut claim = Claim {
        id: typed_id::<ClaimId>(0x0b01)?,
        subject_entity_id: typed_id::<EntityId>(CON_MVCC_DUP)?,
        predicate_id: PredicateId::parse(PREDICATE_MERGED_INTO)?,
        object: ClaimObject::Entity(typed_id::<EntityId>(CON_MVCC)?),
        scope_id: typed_id::<ScopeId>(SCOPE)?,
        authority_class: AuthorityClass::ModelInference,
        epistemic_status: EpistemicStatus::AiInferred,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(YEAR_FIVE)),
        evidence_ids: vec![typed_id::<EvidenceId>(0x5010)?],
    };
    assert!(matches!(
        RegistryFact::decode(&claim),
        Err(RegistryError::UnapprovedIdentityChange { .. })
    ));

    claim.authority_class = AuthorityClass::UserExplicit;
    claim.epistemic_status = EpistemicStatus::UserConfirmed;
    assert!(matches!(
        RegistryFact::decode(&claim)?,
        Some(RegistryFact::MergedInto { .. })
    ));
    Ok(())
}

/// The twelve registry predicates all parse, and a foreign predicate is skipped.
#[test]
fn registry_predicates_are_parseable_and_scoped() -> Result<(), Box<dyn Error>> {
    for predicate in academic_domain::entity_registry::REGISTRY_PREDICATES {
        PredicateId::parse(predicate)?;
    }
    let foreign = Claim {
        id: typed_id::<ClaimId>(0x0b02)?,
        subject_entity_id: typed_id::<EntityId>(CON_MVCC)?,
        predicate_id: PredicateId::parse(PREDICATE_MASTERY)?,
        object: ClaimObject::Mastery(MasteryLevel::Applied),
        scope_id: typed_id::<ScopeId>(SCOPE)?,
        authority_class: AuthorityClass::DirectObservation,
        epistemic_status: EpistemicStatus::CodeObserved,
        confidence: None,
        prediction_metadata: None,
        valid_time: ValidInterval::open_ended(TimestampMillis::new(YEAR_FOUR)),
        evidence_ids: vec![typed_id::<EvidenceId>(0x5001)?],
    };
    assert_eq!(RegistryFact::decode(&foreign)?, None);
    Ok(())
}

/// Alias metadata survives the round trip through canonical claims.
#[test]
fn alias_metadata_round_trips_through_canonical_claims() -> Result<(), Box<dyn Error>> {
    let fixture = LoadedFixture::new("alias")?;
    let registry = fixture.registry()?;
    let expected = [
        (
            ALIAS_MVCC_ABBREVIATION,
            "MVCC",
            "en",
            AliasKind::Abbreviation,
            None,
        ),
        (
            ALIAS_MVCC_KOREAN,
            "다중 버전 동시성 제어",
            "ko",
            AliasKind::Translation,
            None,
        ),
        (
            ALIAS_BTREE_VERSIONED,
            "B-Tree",
            "en",
            AliasKind::Versioned,
            Some("catalog-v2"),
        ),
    ];
    for (alias, text, language, kind, version) in expected {
        let alias_id = typed_id::<EntityId>(alias)?;
        let stored = registry
            .aliases()
            .find(|candidate| candidate.alias_id == alias_id)
            .ok_or("alias is missing from the registry")?;
        assert_eq!(
            stored,
            &Alias {
                alias_id,
                entity_id: stored.entity_id,
                text: text.to_owned(),
                language: language.to_owned(),
                kind,
                version: version.map(ToOwned::to_owned),
            }
        );
    }
    Ok(())
}
