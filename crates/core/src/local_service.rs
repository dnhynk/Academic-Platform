//! Synthetic-only D1 composition over the stable P1, S2, and V1 boundaries.
//!
//! This module does not expose a raw SQLite connection or fabricate a sealed
//! receipt. It resolves the sole repository-allowlisted fixture, durably seals
//! its exact bytes through V1, and asks S2 to verify the receipt again inside
//! the one-writer acceptance path.

use std::{
    collections::BTreeSet,
    io::Cursor,
    time::{SystemTime, UNIX_EPOCH},
};

use academic_contracts::{DeviceAuthorization, VerifiedBatch, verify_signed_batch};
use academic_domain::{ArtifactDescriptor, ContentDigest, EventPayload, TimestampMillis};
use academic_rpc::{
    RpcError,
    convert::{ValidatedMutableRequest, ValidatedWriteCommand, validate_mutable_request},
    generated::{
        AcceptanceRange, ImmutableReceipt, MutableRequest, MutableResponse, MutationStatus,
    },
};
use academic_store::{
    accept::{AcceptError, AcceptanceOutcome},
    fault::{AcceptanceFaultInjector, NoFault},
    idempotency::{AcceptanceCommand, IdempotencyError},
    profile::SyntheticProfile,
    queries::{QueryError, batch_material, canonical_snapshot},
};
use academic_vault::{
    ArtifactIngestRequest, DomainKeyring, ReconcileOptions, ReconcileReport, ReconcileState,
    VaultError,
};
use thiserror::Error;

use crate::{
    CoreError, FixtureDocument, SYNTHETIC_ARTIFACT_BYTES, fixture_device_authorization,
    immutable_v2_fixture_document,
    service::{AcceptanceService, ServiceError},
    verify_fixture_document,
};

/// The only mutable D1 fixture identifier.
pub const PHASE1_SYNTHETIC_FIXTURE_ID: &str = "phase0-synthetic-bitemporal-ledger-v2";
pub(crate) const FIXTURE_LOCATOR_KEY: &[u8] = b"phase0-synthetic-domain-locator-key";
const REQUEST_DIGEST_DOMAIN: &[u8] = b"academic.local-mutable-request.v1\0";
const RESPONSE_DIGEST_DOMAIN: &[u8] = b"academic.local-mutable-response.v1\0";

/// Startup evidence proving V1 reconciliation completed before a listener may bind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalServiceStartup {
    reconciliation: ReconcileReport,
    profile_revision: u64,
}

impl LocalServiceStartup {
    /// Returns every V1 reconciliation decision.
    #[must_use]
    pub const fn reconciliation(&self) -> &ReconcileReport {
        &self.reconciliation
    }

    /// Returns the canonical profile revision observed before opening the sole writer.
    #[must_use]
    pub const fn profile_revision(&self) -> u64 {
        self.profile_revision
    }
}

/// Fail-closed D1 composition error.
#[derive(Debug, Error)]
pub enum LocalServiceError {
    /// The deterministic fixture or its trust anchor drifted.
    #[error(transparent)]
    Core(#[from] CoreError),
    /// P1 request/response validation failed.
    #[error(transparent)]
    Rpc(#[from] RpcError),
    /// The fixture envelope was not valid under its independent trust anchor.
    #[error("fixture verification failed: {0}")]
    Contract(#[from] academic_contracts::ContractError),
    /// The canonical read model could not be reconstructed safely.
    #[error(transparent)]
    Query(#[from] QueryError),
    /// V1 sealing or reconciliation failed.
    #[error(transparent)]
    Vault(#[from] VaultError),
    /// S2 durable acceptance failed.
    #[error(transparent)]
    Service(#[from] ServiceError),
    /// Hex fixture bytes were malformed.
    #[error("fixture envelope hex is invalid: {0}")]
    Hex(#[from] hex::FromHexError),
    /// The profile contains canonical state outside the sole D1 allowlist.
    #[error("profile canonical state is outside the D1 synthetic allowlist: {0}")]
    UnexpectedCanonicalState(&'static str),
    /// Startup reconciliation found a state that must not be served.
    #[error("vault reconciliation requires repair before listening")]
    RepairRequired,
    /// A response reason exceeded the bounded P1 contract.
    #[error("response reason is not a bounded nonempty code")]
    InvalidReason,
    /// The system clock cannot be represented by the signed domain coordinate.
    #[error("system clock is outside the signed millisecond coordinate")]
    ClockOutOfRange,
}

/// One current-profile service. It is designed to be created and retained on
/// the dedicated writer OS thread.
#[derive(Debug)]
pub struct LocalService {
    profile: SyntheticProfile,
    service: AcceptanceService,
    fixture: FixtureContext,
}

impl LocalService {
    /// Reconciles the vault and only then opens the sole guarded writer.
    pub fn open(
        profile: SyntheticProfile,
        now: SystemTime,
    ) -> Result<(Self, LocalServiceStartup), LocalServiceError> {
        let fixture = FixtureContext::load()?;
        let reader = profile.open_reader().map_err(QueryError::from)?;
        let snapshot = canonical_snapshot(&reader)?;

        let referenced = match snapshot.batch_count {
            0 => {
                if snapshot.event_count != 0
                    || snapshot.artifact_count != 0
                    || snapshot.receipt_count != 0
                {
                    return Err(LocalServiceError::UnexpectedCanonicalState(
                        "rows exist without an accepted batch",
                    ));
                }
                Vec::new()
            }
            1 => {
                let stored = batch_material(&reader, fixture.verified.batch().batch_id)?;
                if stored.signed_envelope != fixture.envelope
                    || stored.envelope_hash != fixture.verified.envelope_hash()
                    || stored.payload_hash != fixture.verified.payload_hash()
                {
                    return Err(LocalServiceError::UnexpectedCanonicalState(
                        "accepted batch is not the exact allowlisted fixture",
                    ));
                }
                fixture.descriptors.clone()
            }
            _ => {
                return Err(LocalServiceError::UnexpectedCanonicalState(
                    "more than one canonical batch exists",
                ));
            }
        };

        let mut keyring = DomainKeyring::new();
        let mut domains = BTreeSet::new();
        for descriptor in &fixture.descriptors {
            if domains.insert(descriptor.domain_id) {
                keyring.insert(descriptor.domain_id, FIXTURE_LOCATOR_KEY)?;
            }
        }
        drop(reader);
        let service = AcceptanceService::open(&profile, keyring)?;
        let reconciliation = service.vault().reconcile(
            &ReconcileOptions::new(now)
                .with_referenced(&referenced)
                .with_retry_candidates(&fixture.descriptors),
        )?;
        if reconciliation.repair_required()
            || reconciliation
                .records()
                .iter()
                .any(|record| record.state() == ReconcileState::UnsafeEntry)
        {
            return Err(LocalServiceError::RepairRequired);
        }

        Ok((
            Self {
                profile,
                service,
                fixture,
            },
            LocalServiceStartup {
                reconciliation,
                profile_revision: snapshot.profile_revision,
            },
        ))
    }

    /// Executes one P1 mutable request. Policy denials and optimistic conflicts
    /// are returned as valid, immutable P1 rejection responses.
    pub fn handle_mutable_request(
        &mut self,
        request: &MutableRequest,
        accepted_at: TimestampMillis,
    ) -> Result<MutableResponse, LocalServiceError> {
        self.handle_mutable_request_with(request, accepted_at, &NoFault)
    }

    /// Executes one request using the current UTC system clock.
    pub fn handle_mutable_request_now(
        &mut self,
        request: &MutableRequest,
    ) -> Result<MutableResponse, LocalServiceError> {
        self.handle_mutable_request(request, timestamp_now()?)
    }

    /// X1 process-harness entry point over the identical request body.
    ///
    /// Compiled only by the non-default `phase1-fault-injection` feature. The
    /// production entry points above reach the same body with [`NoFault`], so
    /// the harness kills the real acceptance path rather than a copy of it.
    #[cfg(feature = "phase1-fault-injection")]
    pub fn handle_mutable_request_now_with_faults<F>(
        &mut self,
        request: &MutableRequest,
        faults: &F,
    ) -> Result<MutableResponse, LocalServiceError>
    where
        F: AcceptanceFaultInjector,
    {
        self.handle_mutable_request_with(request, timestamp_now()?, faults)
    }

    /// The single mutable-request body. `NoFault` compiles every checkpoint away.
    fn handle_mutable_request_with<F>(
        &mut self,
        request: &MutableRequest,
        accepted_at: TimestampMillis,
        faults: &F,
    ) -> Result<MutableResponse, LocalServiceError>
    where
        F: AcceptanceFaultInjector,
    {
        let validated = validate_mutable_request(request)?;
        if mutable_request_digest(request)? != validated.request_digest {
            return self.reject(request, "REQUEST_DIGEST_MISMATCH", None);
        }

        match &validated.command {
            ValidatedWriteCommand::SyntheticIngest { fixture_id }
                if fixture_id == PHASE1_SYNTHETIC_FIXTURE_ID => {}
            ValidatedWriteCommand::SyntheticIngest { .. } => {
                return self.reject(request, "FIXTURE_NOT_ALLOWLISTED", None);
            }
            ValidatedWriteCommand::SyntheticBackup => {
                return self.reject(request, "BACKUP_NOT_AVAILABLE_UNTIL_B1", None);
            }
            ValidatedWriteCommand::SyntheticRestore { .. } => {
                return self.reject(request, "RESTORE_NOT_AVAILABLE_UNTIL_B1", None);
            }
        }

        self.seal_fixture_artifacts()?;
        let command = AcceptanceCommand {
            request_id: *validated.request_id.as_bytes(),
            client_instance_id: *validated.client_instance_id.as_bytes(),
            idempotency_key: *validated.idempotency_key.as_bytes(),
            expected_revision: validated.expected_profile_revision,
            envelope_bytes: &self.fixture.envelope,
        };
        let outcome = match self.service.accept_signed_command_with(
            command,
            &self.fixture.authorization,
            accepted_at,
            faults,
        ) {
            Ok(outcome) => outcome,
            Err(ServiceError::Acceptance(AcceptError::ExpectedRevisionConflict {
                actual, ..
            })) => return self.reject(request, "REVISION_CONFLICT", Some(actual)),
            Err(ServiceError::Acceptance(AcceptError::Idempotency(
                IdempotencyError::KeyCollision,
            ))) => return self.reject(request, "IDEMPOTENCY_KEY_COLLISION", None),
            Err(error) => return Err(error.into()),
        };
        accepted_response(request, &validated, &outcome)
    }

    /// Opens a fresh OS-read-only/query-only connection for a caller that does
    /// not own the writer thread.
    pub fn open_reader(
        &self,
    ) -> Result<academic_store::connection::ReaderConnection, LocalServiceError> {
        Ok(self.profile.open_reader().map_err(QueryError::from)?)
    }

    fn seal_fixture_artifacts(&self) -> Result<(), LocalServiceError> {
        for descriptor in &self.fixture.descriptors {
            let request = ArtifactIngestRequest::new(
                descriptor.id,
                descriptor.media_type.clone(),
                descriptor.domain_id,
                descriptor.confidentiality,
                descriptor.retention_class,
                descriptor.permission_lineage_id,
            );
            let receipt = self
                .service
                .vault()
                .ingest(&request, Cursor::new(SYNTHETIC_ARTIFACT_BYTES))?;
            if receipt.descriptor().content_digest != descriptor.content_digest
                || receipt.descriptor().byte_length != descriptor.byte_length
                || receipt.descriptor().vault_locator != descriptor.vault_locator
            {
                return Err(LocalServiceError::UnexpectedCanonicalState(
                    "sealed fixture descriptor does not match the signed descriptor",
                ));
            }
        }
        Ok(())
    }

    fn reject(
        &self,
        request: &MutableRequest,
        reason: &'static str,
        revision: Option<u64>,
    ) -> Result<MutableResponse, LocalServiceError> {
        let revision = match revision {
            Some(value) => value,
            None => {
                let reader = self.profile.open_reader().map_err(QueryError::from)?;
                canonical_snapshot(&reader)?.profile_revision
            }
        };
        rejection_response(request, reason, revision)
    }
}

#[derive(Debug)]
struct FixtureContext {
    envelope: Vec<u8>,
    authorization: DeviceAuthorization,
    verified: VerifiedBatch,
    descriptors: Vec<ArtifactDescriptor>,
}

impl FixtureContext {
    fn load() -> Result<Self, LocalServiceError> {
        let document: FixtureDocument = immutable_v2_fixture_document()?;
        if document.name != PHASE1_SYNTHETIC_FIXTURE_ID {
            return Err(LocalServiceError::UnexpectedCanonicalState(
                "fixture identifier drifted",
            ));
        }
        let _ = verify_fixture_document(&document)?;
        let envelope = hex::decode(&document.signed_batch_cbor_hex)?;
        let authorization = fixture_device_authorization().map_err(CoreError::from)?;
        let verified = verify_signed_batch(&envelope, &authorization)?;
        let descriptors = verified
            .batch()
            .events
            .iter()
            .filter_map(|event| match &event.payload {
                EventPayload::ArtifactRegistered(descriptor) => Some(descriptor.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        if descriptors.is_empty() {
            return Err(LocalServiceError::UnexpectedCanonicalState(
                "fixture has no artifact closure",
            ));
        }
        Ok(Self {
            envelope,
            authorization,
            verified,
            descriptors,
        })
    }
}

/// Computes the stable non-self-referential digest of a P1 mutable request.
pub fn mutable_request_digest(request: &MutableRequest) -> Result<ContentDigest, RpcError> {
    let mut candidate = request.clone();
    candidate.request_digest = vec![0; 32];
    let validated = validate_mutable_request(&candidate)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(REQUEST_DIGEST_DOMAIN);
    bytes.extend_from_slice(validated.request_id.as_bytes());
    bytes.extend_from_slice(validated.client_instance_id.as_bytes());
    bytes.extend_from_slice(validated.idempotency_key.as_bytes());
    append_optional_u64(&mut bytes, validated.expected_profile_revision);
    append_bytes(&mut bytes, validated.capability_id.as_bytes());
    match validated.command {
        ValidatedWriteCommand::SyntheticIngest { fixture_id } => {
            bytes.push(1);
            append_bytes(&mut bytes, fixture_id.as_bytes());
        }
        ValidatedWriteCommand::SyntheticBackup => bytes.push(2),
        ValidatedWriteCommand::SyntheticRestore { backup_receipt_id } => {
            bytes.push(3);
            bytes.extend_from_slice(backup_receipt_id.as_bytes());
        }
    }
    Ok(ContentDigest::sha256(&bytes))
}

/// Builds a valid P1 rejection after queue admission or another non-mutating denial.
pub fn rejection_response(
    request: &MutableRequest,
    reason: &str,
    profile_revision: u64,
) -> Result<MutableResponse, LocalServiceError> {
    if reason.is_empty() || reason.len() > 128 || !reason.is_ascii() {
        return Err(LocalServiceError::InvalidReason);
    }
    let validated = validate_mutable_request(request)?;
    let mut response = MutableResponse {
        request_id: validated.request_id.as_bytes().to_vec(),
        status: MutationStatus::Rejected as i32,
        reason: reason.to_owned(),
        receipt: Some(ImmutableReceipt {
            receipt_id: validated.request_id.as_bytes().to_vec(),
            request_id: validated.request_id.as_bytes().to_vec(),
            client_instance_id: validated.client_instance_id.as_bytes().to_vec(),
            idempotency_key: validated.idempotency_key.as_bytes().to_vec(),
            request_digest: validated.request_digest.as_bytes().to_vec(),
            profile_revision,
            acceptance_range: None,
        }),
        profile_revision,
        acceptance_range: None,
        response_digest: vec![0; 32],
    };
    response.response_digest = mutable_response_digest(&response)?.as_bytes().to_vec();
    let _ = academic_rpc::convert::validate_mutable_response(&response)?;
    Ok(response)
}

fn accepted_response(
    request: &MutableRequest,
    validated: &ValidatedMutableRequest,
    outcome: &AcceptanceOutcome,
) -> Result<MutableResponse, LocalServiceError> {
    let range = AcceptanceRange {
        accept_seq_start: outcome.receipt.accept_seq_start,
        accept_seq_end: outcome.receipt.accept_seq_end,
    };
    let duplicate = outcome.replayed_request || outcome.duplicate_batch;
    let mut response = MutableResponse {
        request_id: validated.request_id.as_bytes().to_vec(),
        status: if duplicate {
            MutationStatus::Duplicate as i32
        } else {
            MutationStatus::Accepted as i32
        },
        reason: if duplicate { "DUPLICATE" } else { "ACCEPTED" }.to_owned(),
        receipt: Some(ImmutableReceipt {
            receipt_id: outcome.receipt.batch_id.as_bytes().to_vec(),
            request_id: validated.request_id.as_bytes().to_vec(),
            client_instance_id: validated.client_instance_id.as_bytes().to_vec(),
            idempotency_key: validated.idempotency_key.as_bytes().to_vec(),
            request_digest: request.request_digest.clone(),
            profile_revision: outcome.receipt.committed_revision,
            acceptance_range: Some(range),
        }),
        profile_revision: outcome.receipt.committed_revision,
        acceptance_range: Some(range),
        response_digest: vec![0; 32],
    };
    response.response_digest = mutable_response_digest(&response)?.as_bytes().to_vec();
    let _ = academic_rpc::convert::validate_mutable_response(&response)?;
    Ok(response)
}

fn mutable_response_digest(response: &MutableResponse) -> Result<ContentDigest, RpcError> {
    let mut candidate = response.clone();
    candidate.response_digest = vec![0; 32];
    let validated = academic_rpc::convert::validate_mutable_response(&candidate)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(RESPONSE_DIGEST_DOMAIN);
    bytes.extend_from_slice(validated.request_id.as_bytes());
    bytes.extend_from_slice(&(validated.status as i32).to_be_bytes());
    append_bytes(&mut bytes, validated.reason.as_bytes());
    bytes.extend_from_slice(validated.receipt.receipt_id.as_bytes());
    bytes.extend_from_slice(validated.receipt.request_id.as_bytes());
    bytes.extend_from_slice(validated.receipt.client_instance_id.as_bytes());
    bytes.extend_from_slice(validated.receipt.idempotency_key.as_bytes());
    bytes.extend_from_slice(validated.receipt.request_digest.as_bytes());
    bytes.extend_from_slice(&validated.profile_revision.to_be_bytes());
    append_range(&mut bytes, validated.acceptance_range);
    Ok(ContentDigest::sha256(&bytes))
}

fn append_range(
    bytes: &mut Vec<u8>,
    range: Option<academic_rpc::convert::ValidatedAcceptanceRange>,
) {
    match range {
        Some(range) => {
            bytes.push(1);
            bytes.extend_from_slice(&range.start.to_be_bytes());
            bytes.extend_from_slice(&range.end.to_be_bytes());
        }
        None => bytes.push(0),
    }
}

fn append_optional_u64(bytes: &mut Vec<u8>, value: Option<u64>) {
    match value {
        Some(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        None => bytes.push(0),
    }
}

fn append_bytes(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&u64::try_from(value.len()).unwrap_or(u64::MAX).to_be_bytes());
    target.extend_from_slice(value);
}

fn timestamp_now() -> Result<TimestampMillis, LocalServiceError> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| LocalServiceError::ClockOutOfRange)?
        .as_millis();
    let millis = i64::try_from(millis).map_err(|_| LocalServiceError::ClockOutOfRange)?;
    Ok(TimestampMillis::new(millis))
}

impl From<academic_store::error::StoreError> for LocalServiceError {
    fn from(error: academic_store::error::StoreError) -> Self {
        Self::Query(QueryError::from(error))
    }
}
