//! Real keyed profile/session composition. All recipient material is test-only.
use academic_contracts::DeviceAuthorization;
use academic_core::encrypted_session::{EncryptedProfileSession, EncryptedSessionError};
use academic_crypto::{
    ProfileId, RECOVERY_ARGON2ID_V1, RecipientRecord, RecoverySecret, UnlockThrottle,
    VaultMasterKey, create_recovery_recipient, unlock_with_recovery,
};
use academic_domain::DomainId;
use academic_encrypted_session_tests::TestResult;
use academic_store::{cipher::create_encrypted_profile, path_policy::NativePathProbe};
use ed25519_dalek::SigningKey;
use std::{
    error::Error,
    path::{Path, PathBuf},
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
const PROFILE: ProfileId = ProfileId::from_bytes([0x74; 16]);

fn id<T: FromStr<Err = academic_domain::DomainError>>(
    n: u64,
) -> Result<T, academic_domain::DomainError> {
    format!("01900000-0000-7000-8000-{n:012x}").parse()
}

fn authorization() -> Result<DeviceAuthorization, Box<dyn Error>> {
    Ok(DeviceAuthorization::new(
        id(1)?,
        id(2)?,
        SigningKey::from_bytes(&[0x75; 32]).verifying_key(),
    ))
}

struct Material {
    root: PathBuf,
    recipient: RecipientRecord,
    secret: RecoverySecret,
}

impl Material {
    fn new() -> Result<Self, Box<dyn Error>> {
        let base = std::env::temp_dir();
        #[cfg(unix)]
        let base = std::fs::canonicalize(base)?;
        let root = base.join(format!(
            "academic-d1-public-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let master = VaultMasterKey::generate()?;
        create_encrypted_profile(
            &root,
            &NativePathProbe::default(),
            &master.derive_store_key(PROFILE)?,
            [0x93; 32],
        )?;
        let secret = RecoverySecret::from_entropy([0x76; 32]);
        let recipient =
            create_recovery_recipient(&master, PROFILE, [0x77; 16], &secret, RECOVERY_ARGON2ID_V1)?;
        Ok(Self {
            root,
            recipient,
            secret,
        })
    }

    fn unlock(&self) -> Result<VaultMasterKey, Box<dyn Error>> {
        Ok(unlock_with_recovery(
            &self.recipient,
            PROFILE,
            &self.secret,
            &mut UnlockThrottle::new(),
            0,
        )?)
    }

    fn open(&self) -> Result<EncryptedProfileSession, Box<dyn Error>> {
        Ok(EncryptedProfileSession::open(
            &self.root,
            PROFILE,
            self.unlock()?,
            &[id(3)?],
            vec![authorization()?],
        )?)
    }

    fn close(self) -> TestResult {
        std::fs::remove_dir_all(self.root)?;
        Ok(())
    }
}

#[test]
fn same_root_reopen_retains_local_incarnation_distinct_from_canonical_profile() -> TestResult {
    let material = Material::new()?;
    let session = material.open()?;
    assert_eq!(session.profile_id(), PROFILE);
    assert_eq!(
        format!("{session:?}"),
        "EncryptedProfileSession { domain_count: 1, .. }"
    );
    assert_eq!(
        format!("{:?}", session.readers()),
        "EncryptedReaderFactory { .. }"
    );
    let settings = session.writer_settings()?;
    assert_eq!(settings.user_version, 2);
    assert_eq!(settings.journal_mode, "wal");
    assert!(settings.foreign_keys);
    assert!(!settings.query_only);
    let incarnation = session.local_incarnation().to_owned();
    let marker = std::fs::read(material.root.join("detail-incarnation.v1"))?;
    assert_eq!(marker.len(), 32);
    drop(session);
    let reopened = material.open()?;
    assert_eq!(reopened.local_incarnation(), incarnation);
    assert_eq!(
        std::fs::read(material.root.join("detail-incarnation.v1"))?,
        marker
    );
    drop(reopened);
    let other = Material::new()?;
    let separate = other.open()?;
    assert_eq!(separate.profile_id(), PROFILE);
    assert_ne!(separate.local_incarnation(), incarnation);
    drop(separate);
    other.close()?;
    material.close()
}

#[test]
fn wrong_key_and_profile_never_mint_or_replace_local_identity() -> TestResult {
    let material = Material::new()?;
    let marker = material.root.join("detail-incarnation.v1");
    assert!(!marker.exists());
    assert!(
        EncryptedProfileSession::open(
            &material.root,
            PROFILE,
            VaultMasterKey::generate()?,
            &[id(3)?],
            vec![authorization()?]
        )
        .is_err()
    );
    assert!(!marker.exists());
    assert!(
        EncryptedProfileSession::open(
            &material.root,
            ProfileId::from_bytes([0x78; 16]),
            material.unlock()?,
            &[id(3)?],
            vec![authorization()?]
        )
        .is_err()
    );
    assert!(!marker.exists());
    drop(material.open()?);
    let original = std::fs::read(&marker)?;
    assert!(
        EncryptedProfileSession::open(
            &material.root,
            PROFILE,
            VaultMasterKey::generate()?,
            &[id(3)?],
            vec![authorization()?]
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&marker)?, original);
    let wrong = RecoverySecret::from_entropy([0x79; 32]);
    assert!(
        unlock_with_recovery(
            &material.recipient,
            PROFILE,
            &wrong,
            &mut UnlockThrottle::new(),
            0
        )
        .is_err()
    );
    material.close()
}

#[test]
fn absent_authority_ambiguous_domains_and_missing_profile_refuse() -> TestResult {
    let material = Material::new()?;
    let domain: DomainId = id(3)?;
    assert!(matches!(
        EncryptedProfileSession::open(
            &material.root,
            PROFILE,
            material.unlock()?,
            &[domain],
            vec![]
        ),
        Err(EncryptedSessionError::MissingOrAmbiguousMaterial)
    ));
    assert!(matches!(
        EncryptedProfileSession::open(
            &material.root,
            PROFILE,
            material.unlock()?,
            &[],
            vec![authorization()?]
        ),
        Err(EncryptedSessionError::MissingOrAmbiguousMaterial)
    ));
    assert!(matches!(
        EncryptedProfileSession::open(
            &material.root,
            PROFILE,
            material.unlock()?,
            &[domain, domain],
            vec![authorization()?]
        ),
        Err(EncryptedSessionError::MissingOrAmbiguousMaterial)
    ));
    let auth = authorization()?;
    assert!(matches!(
        EncryptedProfileSession::open(
            &material.root,
            PROFILE,
            material.unlock()?,
            &[domain],
            vec![auth.clone(), auth]
        ),
        Err(EncryptedSessionError::MissingOrAmbiguousMaterial)
    ));
    let absent = material.root.join("absent-profile");
    assert!(
        EncryptedProfileSession::open(
            &absent,
            PROFILE,
            material.unlock()?,
            &[domain],
            vec![authorization()?]
        )
        .is_err()
    );
    assert!(!absent.exists());
    assert!(!material.root.join("detail-incarnation.v1").exists());
    material.close()
}

#[test]
fn encrypted_startup_refuses_before_any_runtime_or_profile_io() -> TestResult {
    let material = Material::new()?;
    let runtime = material.root.join("unpublished-runtime");
    let before = std::fs::read_dir(&material.root)?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        academic_daemon::encrypted::start(&material.root, &runtime),
        Err(academic_daemon::encrypted::EncryptedSyntheticStartupUnavailable)
    );
    assert_eq!(
        academic_daemon::encrypted::start(Path::new(""), Path::new("")),
        Err(academic_daemon::encrypted::EncryptedSyntheticStartupUnavailable)
    );
    let after = std::fs::read_dir(&material.root)?
        .map(|entry| entry.map(|e| e.file_name()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(before, after);
    assert!(!runtime.exists());
    material.close()
}
