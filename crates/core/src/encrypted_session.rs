//! Concrete keyed profile lifetime for the optional D1 library scaffold.
//!
//! This is physical/key composition, not an authenticated synthetic service.
//! There is no public writer, seeder, service-ready token or admission posture.
//! D2/D4 must recognize complete material before D3 may serve a selected profile.

use crate::{authenticated_acceptance::VaultAccess, domain_read};
use academic_contracts::DeviceAuthorization;
use academic_crypto::{ProfileId, StoreKey, VaultMasterKey};
use academic_domain::{DomainId, TimestampMillis};
use academic_rpc::domain_details as dto;
use academic_store::{
    accept::AcceptanceStore,
    cipher::{EncryptedProfile, open_encrypted_profile},
    path_policy::NativePathProbe,
    queries::{DomainHistoryRequest, domain_history_snapshot},
};
use academic_vault::{EncryptedDomainKeyring, EncryptedVault};
use std::{collections::BTreeSet, fmt, path::Path};

/// Ordinary refusal to compose existing encrypted material.
#[derive(Debug, thiserror::Error)]
pub enum EncryptedSessionError {
    #[error(
        "encrypted session requires independently supplied authorizations and distinct domains"
    )]
    MissingOrAmbiguousMaterial,
    #[error("encrypted session key derivation failed")]
    KeySchedule(#[from] academic_crypto::KeyScheduleError),
    #[error("encrypted session profile could not open: {0}")]
    Store(#[from] academic_store::error::StoreError),
    #[error("encrypted session vault could not open: {0}")]
    Vault(#[from] academic_vault::VaultError),
}

/// One concrete writer and its key material; neither can be cloned or extracted.
///
/// Rust ownership excludes another alias to this writer. It does not prevent an
/// independent session open; D3 must acquire the existing process singleton
/// before creating a usable service. No encrypted transport exists in D1.
///
/// The owned writer and key material cannot be cloned:
/// ```compile_fail
/// use academic_core::encrypted_session::EncryptedProfileSession;
/// fn duplicate(session: EncryptedProfileSession) {
///     let _second = session.clone();
/// }
/// ```
/// Nor can a caller extract the acceptance writer:
/// ```compile_fail
/// use academic_core::encrypted_session::EncryptedProfileSession;
/// fn extract(session: EncryptedProfileSession) {
///     let _writer = session.store;
/// }
/// ```
pub struct EncryptedProfileSession {
    // Drop the writer before its material. Readers below borrow this whole value.
    store: AcceptanceStore,
    profile: EncryptedProfile,
    profile_id: ProfileId,
    key: StoreKey,
    vault: EncryptedVault,
    trust: Vec<DeviceAuthorization>,
    domains: BTreeSet<DomainId>,
    incarnation: [u8; 32],
    binding: String,
}

impl fmt::Debug for EncryptedProfileSession {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedProfileSession")
            .field("domain_count", &self.domains.len())
            .finish_non_exhaustive()
    }
}

impl EncryptedProfileSession {
    /// Consumes a VMK unlocked by the trusted host using the existing recipient
    /// APIs. Store and vault keys are derived together, for one exact ProfileId.
    /// This opens an existing profile only and performs no provisioning/seeding.
    pub fn open(
        root: &Path,
        profile_id: ProfileId,
        master: VaultMasterKey,
        domains: &[DomainId],
        authorizations: Vec<DeviceAuthorization>,
    ) -> Result<Self, EncryptedSessionError> {
        let domain_set = domains.iter().copied().collect::<BTreeSet<_>>();
        let devices = authorizations
            .iter()
            .map(DeviceAuthorization::device_id)
            .collect::<BTreeSet<_>>();
        if domain_set.is_empty()
            || domain_set.len() != domains.len()
            || devices.is_empty()
            || devices.len() != authorizations.len()
        {
            return Err(EncryptedSessionError::MissingOrAmbiguousMaterial);
        }
        let key = master.derive_store_key(profile_id)?;
        let mut keyring = EncryptedDomainKeyring::new(profile_id);
        for domain in &domain_set {
            let key_domain = academic_crypto::DomainId::from_bytes(*domain.as_bytes());
            keyring.insert(*domain, master.derive_domain_kek(profile_id, key_domain)?)?;
        }
        drop(master);
        let profile = open_encrypted_profile(root, &NativePathProbe::default(), &key)?;
        let store = profile.open_acceptance_store(&key)?;
        let vault = EncryptedVault::open(profile.root(), keyring)?;
        let incarnation = profile.detail_incarnation(&key)?;
        // Canonical crypto identity, physical host location and local incarnation
        // have distinct roles. This digest is correlation metadata, never trust.
        let binding = hex::encode(
            academic_domain::ContentDigest::sha256(
                &[
                    b"academic.encrypted-detail-profile-incarnation.v1\0".as_slice(),
                    profile_id.as_bytes(),
                    &incarnation,
                    profile.root().as_os_str().as_encoded_bytes(),
                ]
                .concat(),
            )
            .as_bytes(),
        );
        Ok(Self {
            store,
            profile,
            profile_id,
            key,
            vault,
            trust: authorizations,
            domains: domain_set,
            incarnation,
            binding,
        })
    }

    /// Crypto identity remains distinct from the local DTO correlation string.
    #[must_use]
    pub const fn profile_id(&self) -> ProfileId {
        self.profile_id
    }

    /// Local correlation identity, retained on same-root reopen; not an authority.
    #[must_use]
    pub fn local_incarnation(&self) -> &str {
        &self.binding
    }

    /// A factory borrows all session material and cannot outlive the sole owner.
    #[must_use]
    pub const fn readers(&self) -> EncryptedReaderFactory<'_> {
        EncryptedReaderFactory { session: self }
    }

    /// Read-back of the existing writer settings, without a connection escape.
    pub fn writer_settings(
        &self,
    ) -> Result<academic_store::connection::PragmaSnapshot, EncryptedSessionError> {
        Ok(self.store.pragma_snapshot()?)
    }
}

/// Keyed bounded reader factory tied to its session's lifetime.
///
/// ```compile_fail
/// use academic_core::encrypted_session::{EncryptedProfileSession, EncryptedReaderFactory};
/// fn escape(session: EncryptedProfileSession) -> EncryptedReaderFactory<'static> {
///     session.readers()
/// }
/// ```
pub struct EncryptedReaderFactory<'a> {
    session: &'a EncryptedProfileSession,
}

impl fmt::Debug for EncryptedReaderFactory<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EncryptedReaderFactory")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;

impl EncryptedReaderFactory<'_> {
    /// Projects independently authenticated canonical history in the library.
    /// This authenticates read provenance, not synthetic-corpus membership or
    /// service admission. No transport may advertise this before D2/D3/D4.
    pub fn project_domain(
        &self,
        request: &dto::DomainReadRequest,
        now: TimestampMillis,
    ) -> dto::DomainReadReply {
        match self.projection(request, now) {
            Ok(value) => dto::DomainReadReply::ready(value).unwrap_or_else(|_| {
                dto::DomainReadReply::unavailable(dto::ReadFailure::ResultTooLarge)
            }),
            Err(reason) => dto::DomainReadReply::unavailable(reason),
        }
    }

    fn projection(
        &self,
        request: &dto::DomainReadRequest,
        now: TimestampMillis,
    ) -> Result<dto::DomainProjection, dto::ReadFailure> {
        let session = self.session;
        let (context, selector, query) = request.parts();
        if let dto::Query::Detail { subject } = query
            && subject.context() != *context
        {
            return Err(dto::ReadFailure::ContextMismatch);
        }
        request
            .validate()
            .map_err(|_| dto::ReadFailure::SelectorUnavailable)?;
        if !session.domains.contains(&context.domain_id) {
            return Err(dto::ReadFailure::ContextUnavailable);
        }
        self.verify_incarnation()?;
        let valid = match selector.valid_at_ms {
            Some(value) => value,
            None => {
                u64::try_from(now.value()).map_err(|_| dto::ReadFailure::SelectorUnavailable)?
            }
        };
        if valid > academic_rpc::details::MAX_SAFE_INTEGER {
            return Err(dto::ReadFailure::SelectorUnavailable);
        }
        let source = domain_history_snapshot(
            &mut session
                .profile
                .open_reader(&session.key)
                .map_err(|_| dto::ReadFailure::ProfileUnavailable)?,
            &DomainHistoryRequest {
                domain_id: context.domain_id,
                scope_id: context.scope_id,
                known_at_accept_seq: selector.known_at_accept_seq,
                valid_at: TimestampMillis::new(
                    i64::try_from(valid).map_err(|_| dto::ReadFailure::SelectorUnavailable)?,
                ),
            },
        )
        .map_err(domain_read::query_failure)?;
        let snapshot = domain_read::authenticate(source, &session.trust)?;
        self.verify_incarnation()?;
        let projection = domain_read::project(
            &snapshot,
            VaultAccess::Encrypted(&session.vault),
            &session.binding,
            query,
        )?;
        self.verify_incarnation()?;
        Ok(projection)
    }

    fn verify_incarnation(&self) -> Result<(), dto::ReadFailure> {
        if self
            .session
            .profile
            .read_detail_incarnation(&self.session.key)
            .map_err(|_| dto::ReadFailure::ProfileUnavailable)?
            != self.session.incarnation
        {
            return Err(dto::ReadFailure::ProfileMismatch);
        }
        Ok(())
    }
}
