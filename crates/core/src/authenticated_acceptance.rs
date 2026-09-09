//! Private concrete vault composition shared by the two exclusive core lanes.

#[cfg(any(feature = "plaintext-core", test))]
use academic_contracts::{DeviceAuthorization, verify_signed_batch};
use academic_domain::ArtifactDescriptor;
#[cfg(any(feature = "plaintext-core", test))]
use academic_domain::TimestampMillis;
#[cfg(any(feature = "plaintext-core", test))]
use academic_store::{
    accept::{AcceptanceOutcome, AcceptanceStore},
    fault::{AcceptanceFaultInjector, AcceptanceFaultPoint},
    idempotency::AcceptanceCommand,
};
use academic_vault::{VaultError, VaultResult};

#[cfg(feature = "encrypted-synthetic-core")]
use academic_vault::SealedObjectVerifier;

/// This private sum cannot be extended by a caller-supplied verifier.
#[derive(Clone, Copy)]
pub(crate) enum VaultAccess<'a> {
    #[cfg(feature = "plaintext-core")]
    Plain(&'a academic_vault::Vault),
    #[cfg(feature = "encrypted-synthetic-core")]
    Encrypted(&'a academic_vault::EncryptedVault),
}

pub(crate) enum VerifiedObject<'a> {
    #[cfg(feature = "plaintext-core")]
    Plain(
        academic_vault::SealedObjectCapability,
        std::marker::PhantomData<&'a ()>,
    ),
    #[cfg(feature = "encrypted-synthetic-core")]
    Encrypted {
        vault: &'a academic_vault::EncryptedVault,
        receipt: academic_vault::SealedEncryptedObject,
        reader: academic_vault::EncryptedObjectReader,
    },
}

impl<'a> VaultAccess<'a> {
    pub(crate) fn verify(self, descriptor: &ArtifactDescriptor) -> VaultResult<VerifiedObject<'a>> {
        match self {
            #[cfg(feature = "plaintext-core")]
            Self::Plain(vault) => Ok(VerifiedObject::Plain(
                vault.verify_sealed_object(descriptor)?,
                std::marker::PhantomData,
            )),
            #[cfg(feature = "encrypted-synthetic-core")]
            Self::Encrypted(vault) => {
                // Retain the verified object and shared lease while the separate
                // authenticated reader resolves each chunk. Revalidate the exact
                // canonical object around every range; the projector also checks
                // the complete returned plaintext digest against signed history.
                let receipt = vault.verify_sealed_object(descriptor)?;
                let reader = vault.open_reader(descriptor)?;
                Ok(VerifiedObject::Encrypted {
                    vault,
                    receipt,
                    reader,
                })
            }
        }
    }
}

impl VerifiedObject<'_> {
    pub(crate) fn read_verified_range(
        &mut self,
        offset: u64,
        length: usize,
    ) -> VaultResult<Vec<u8>> {
        match self {
            #[cfg(feature = "plaintext-core")]
            Self::Plain(capability, _) => capability.read_verified_range(offset, length),
            #[cfg(feature = "encrypted-synthetic-core")]
            Self::Encrypted {
                vault,
                receipt,
                reader,
            } => {
                use std::io::{Read, Seek, SeekFrom};
                if length == 0
                    || length > 4096
                    || offset
                        .checked_add(
                            u64::try_from(length).map_err(|_| VaultError::ArtifactTooLarge)?,
                        )
                        .is_none_or(|end| end > receipt.descriptor().byte_length)
                {
                    return Err(VaultError::ArtifactTooLarge);
                }
                vault.revalidate_sealed_object(receipt)?;
                let mut bytes = vec![0; length];
                reader
                    .seek(SeekFrom::Start(offset))
                    .and_then(|_| reader.read_exact(&mut bytes))
                    .map_err(|_| VaultError::IntegrityMismatch(receipt.object_path().to_owned()))?;
                vault.revalidate_sealed_object(receipt)?;
                Ok(bytes)
            }
        }
    }
}

// Encrypted D1 has no public writer command. Its in-crate fixture tests exercise
// this identical authenticated acceptance body; D2/D4 remain separate work.
#[cfg(any(feature = "plaintext-core", test))]
pub(crate) fn accept_signed_command<F: AcceptanceFaultInjector>(
    store: &mut AcceptanceStore,
    vault: VaultAccess<'_>,
    command: AcceptanceCommand<'_>,
    authorization: &DeviceAuthorization,
    accepted_at: TimestampMillis,
    faults: &F,
) -> Result<AcceptanceOutcome, ServiceError> {
    let verified = verify_signed_batch(command.envelope_bytes, authorization)?;
    let outcome = match vault {
        #[cfg(feature = "plaintext-core")]
        VaultAccess::Plain(vault) => store.accept_verified_batch_with_faults(
            &verified,
            command,
            accepted_at,
            vault,
            faults,
        )?,
        #[cfg(feature = "encrypted-synthetic-core")]
        VaultAccess::Encrypted(vault) => store.accept_verified_batch_with_faults(
            &verified,
            command,
            accepted_at,
            vault,
            faults,
        )?,
    };
    faults.hit(AcceptanceFaultPoint::Ipc02)?;
    Ok(outcome)
}

#[cfg(any(feature = "plaintext-core", test))]
use academic_contracts::ContractError;
#[cfg(any(feature = "plaintext-core", test))]
use academic_store::{accept::AcceptError, error::StoreError, fault::InjectedFault};
#[cfg(any(feature = "plaintext-core", test))]
use std::{error::Error, fmt};

/// Authentication or durable-store failure at the local core boundary.
#[derive(Debug)]
#[cfg(any(feature = "plaintext-core", test))]
pub enum ServiceError {
    Contract(ContractError),
    Store(StoreError),
    Vault(VaultError),
    Acceptance(AcceptError),
    Injected(InjectedFault),
}

#[cfg(any(feature = "plaintext-core", test))]
impl fmt::Display for ServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Contract(error) => write!(formatter, "signed acceptance rejected: {error}"),
            Self::Store(error) => write!(formatter, "acceptance store could not open: {error}"),
            Self::Vault(error) => write!(formatter, "acceptance vault could not open: {error}"),
            Self::Acceptance(error) => write!(formatter, "durable acceptance rejected: {error}"),
            Self::Injected(error) => write!(formatter, "{error}"),
        }
    }
}

#[cfg(any(feature = "plaintext-core", test))]
impl Error for ServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Contract(error) => Some(error),
            Self::Store(error) => Some(error),
            Self::Vault(error) => Some(error),
            Self::Acceptance(error) => Some(error),
            Self::Injected(error) => Some(error),
        }
    }
}

#[cfg(any(feature = "plaintext-core", test))]
impl From<ContractError> for ServiceError {
    fn from(error: ContractError) -> Self {
        Self::Contract(error)
    }
}

#[cfg(any(feature = "plaintext-core", test))]
impl From<StoreError> for ServiceError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

#[cfg(any(feature = "plaintext-core", test))]
impl From<VaultError> for ServiceError {
    fn from(error: VaultError) -> Self {
        Self::Vault(error)
    }
}

#[cfg(any(feature = "plaintext-core", test))]
impl From<AcceptError> for ServiceError {
    fn from(error: AcceptError) -> Self {
        Self::Acceptance(error)
    }
}

#[cfg(any(feature = "plaintext-core", test))]
impl From<InjectedFault> for ServiceError {
    fn from(error: InjectedFault) -> Self {
        Self::Injected(error)
    }
}
