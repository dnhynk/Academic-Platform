//! Non-shipping physical/key composition tests; no recognized-corpus claim.
use super::*;
use academic_contracts::{DeviceAuthorization, sign_batch};
use academic_domain::{
    Actor, Confidentiality, ContentDigest, Event, EventPayload, MediaType, RetentionClass,
    ScopeDescriptor, UnsignedBatch,
};
use academic_store::{
    cipher::create_encrypted_profile, fault::NoFault, idempotency::AcceptanceCommand,
};
use academic_vault::ArtifactIngestRequest;
use ed25519_dalek::SigningKey;
use std::{
    error::Error,
    path::PathBuf,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

type TestResult = Result<(), Box<dyn Error>>;
static NEXT: AtomicU64 = AtomicU64::new(0);

fn id<T: FromStr<Err = academic_domain::DomainError>>(
    n: u64,
) -> Result<T, academic_domain::DomainError> {
    format!("01900000-0000-7000-8000-{n:012x}").parse()
}

struct Fixture {
    session: EncryptedProfileSession,
    root: PathBuf,
    signing: SigningKey,
}

impl Fixture {
    fn new() -> Result<Self, Box<dyn Error>> {
        let base = std::env::temp_dir();
        #[cfg(unix)]
        let base = std::fs::canonicalize(base)?;
        let root = base.join(format!(
            "academic-d1-core-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let master = VaultMasterKey::generate()?;
        let profile_id = ProfileId::from_bytes([0x71; 16]);
        create_encrypted_profile(
            &root,
            &NativePathProbe::default(),
            &master.derive_store_key(profile_id)?,
            [0x91; 32],
        )?;
        let signing = SigningKey::from_bytes(&[0x72; 32]);
        let authorization = DeviceAuthorization::new(id(1)?, id(2)?, signing.verifying_key());
        let session = EncryptedProfileSession::open(
            &root,
            profile_id,
            master,
            &[id(3)?],
            vec![authorization],
        )?;
        Ok(Self {
            session,
            root,
            signing,
        })
    }

    fn accept_scope(&mut self) -> TestResult {
        let authorization = self.session.trust[0].clone();
        let batch = UnsignedBatch {
            schema_version: academic_domain::EVENT_SCHEMA_VERSION,
            batch_id: id(5)?,
            device_id: authorization.device_id(),
            origin_seq_start: 1,
            origin_seq_end: 1,
            previous_batch_hash: None,
            origin_created_at: TimestampMillis::new(10),
            events: vec![Event {
                id: id(6)?,
                origin_seq: 1,
                origin_observed_at: TimestampMillis::new(10),
                domain_id: id(3)?,
                actor: Actor::User { user_id: id(2)? },
                payload: EventPayload::ScopeRegistered(ScopeDescriptor {
                    id: id(4)?,
                    domain_id: id(3)?,
                    label: "D1 synthetic scope".into(),
                }),
            }],
        };
        let envelope = sign_batch(&batch, &self.signing)?;
        crate::authenticated_acceptance::accept_signed_command(
            &mut self.session.store,
            VaultAccess::Encrypted(&self.session.vault),
            AcceptanceCommand {
                request_id: [1; 16],
                client_instance_id: [2; 16],
                idempotency_key: [3; 32],
                expected_revision: Some(0),
                envelope_bytes: &envelope,
            },
            &authorization,
            TimestampMillis::new(20),
            &NoFault,
        )?;
        Ok(())
    }

    fn request() -> Result<dto::DomainReadRequest, Box<dyn Error>> {
        Ok(dto::DomainReadRequest::DetailsDomainReadV3 {
            context: dto::Context {
                domain_id: id(3)?,
                scope_id: id(4)?,
            },
            selector: dto::DomainSelector {
                view: dto::DomainView::DomainDetailV3,
                known_at_accept_seq: None,
                valid_at_ms: Some(30),
            },
            query: dto::Query::Index {
                surface: dto::Surface::Concept,
            },
        })
    }

    fn close(self) -> TestResult {
        drop(self.session);
        std::fs::remove_dir_all(self.root)?;
        Ok(())
    }
}

#[test]
fn keyed_history_authenticates_empty_index_and_refuses_untrusted_signatures() -> TestResult {
    let mut fixture = Fixture::new()?;
    fixture.accept_scope()?;
    let request = Fixture::request()?;
    // The existing store schema supports an authenticated empty index. This
    // fixture registers only a scope: no six-arm registration or recognized
    // corpus/service admission is established by an empty library read.
    let reply = fixture
        .session
        .readers()
        .project_domain(&request, TimestampMillis::new(30));
    let dto::DomainReadReply::Ready { projection, .. } = reply else {
        return Err("authenticated scope history did not produce an empty index".into());
    };
    assert_eq!(
        projection.binding.profile_id,
        fixture.session.local_incarnation()
    );
    assert_eq!(projection.binding.domain_id, id(3)?);
    assert_eq!(projection.binding.scope_id, id(4)?);
    assert_eq!(projection.binding.revision, 1);
    assert_eq!(projection.binding.known_at_accept_seq, 1);
    assert_eq!(projection.binding.valid_at_ms, 30);
    assert_eq!(
        projection.result,
        dto::QueryResult::Index {
            surface: dto::Surface::Concept,
            entries: vec![]
        }
    );
    let unrelated = SigningKey::from_bytes(&[0x73; 32]);
    fixture.session.trust = vec![DeviceAuthorization::new(
        id(1)?,
        id(2)?,
        unrelated.verifying_key(),
    )];
    assert_eq!(
        fixture
            .session
            .readers()
            .project_domain(&request, TimestampMillis::new(30)),
        dto::DomainReadReply::unavailable(dto::ReadFailure::SourceVerificationFailed)
    );
    fixture.close()
}

#[test]
fn session_vault_reads_exact_aead_bytes_and_refuses_absent_or_changed_objects() -> TestResult {
    let fixture = Fixture::new()?;
    let bytes = "Synthetic exact text:  alpha,\n beta\t→ gamma.".as_bytes();
    let sealed = fixture.session.vault.ingest(
        &ArtifactIngestRequest::new(
            id(7)?,
            MediaType::parse("text/plain")?,
            id(3)?,
            Confidentiality::Restricted,
            RetentionClass::UserManaged,
            id(8)?,
        ),
        bytes,
    )?;
    let descriptor = sealed.descriptor().clone();
    let path = sealed.object_path().to_owned();
    assert_eq!(descriptor.content_digest, ContentDigest::sha256(bytes));
    drop(sealed);
    let vault = VaultAccess::Encrypted(&fixture.session.vault);
    let mut object = vault.verify(&descriptor)?;
    assert_eq!(object.read_verified_range(0, bytes.len())?, bytes);
    assert!(object.read_verified_range(0, 4097).is_err());
    assert!(
        object
            .read_verified_range(descriptor.byte_length, 1)
            .is_err()
    );
    drop(object);
    let original = std::fs::read(&path)?;
    std::fs::write(&path, vec![b'!'; original.len()])?;
    assert!(vault.verify(&descriptor).is_err());
    std::fs::remove_file(&path)?;
    assert!(vault.verify(&descriptor).is_err());
    fixture.close()
}

#[test]
fn reader_refuses_changed_local_incarnation_and_unselected_domain() -> TestResult {
    let fixture = Fixture::new()?;
    let mut request = Fixture::request()?;
    let dto::DomainReadRequest::DetailsDomainReadV3 { context, .. } = &mut request;
    context.domain_id = id(9)?;
    assert_eq!(
        fixture
            .session
            .readers()
            .project_domain(&request, TimestampMillis::new(30)),
        dto::DomainReadReply::unavailable(dto::ReadFailure::ContextUnavailable)
    );
    std::fs::write(fixture.root.join("detail-incarnation.v1"), [0x92; 32])?;
    assert_eq!(
        fixture
            .session
            .readers()
            .project_domain(&Fixture::request()?, TimestampMillis::new(30)),
        dto::DomainReadReply::unavailable(dto::ReadFailure::ProfileMismatch)
    );
    let marker = fixture.root.join("detail-incarnation.v1");
    std::fs::write(&marker, [0x93; 3])?;
    assert_eq!(
        fixture
            .session
            .readers()
            .project_domain(&Fixture::request()?, TimestampMillis::new(30)),
        dto::DomainReadReply::unavailable(dto::ReadFailure::ProfileUnavailable)
    );
    assert_eq!(std::fs::read(&marker)?, [0x93; 3]);
    std::fs::remove_file(&marker)?;
    assert_eq!(
        fixture
            .session
            .readers()
            .project_domain(&Fixture::request()?, TimestampMillis::new(30)),
        dto::DomainReadReply::unavailable(dto::ReadFailure::ProfileUnavailable)
    );
    assert!(
        !marker.exists(),
        "a read must never recreate missing local metadata"
    );
    fixture.close()
}
