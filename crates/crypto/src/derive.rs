//! The ADR-005 key schedule.
//!
//! Every subordinate key is HKDF-SHA-512 over the Vault Master Key with the
//! profile identity as salt and a purpose-specific info string. The info
//! strings are the frozen contract of this task: changing one is a key-schedule
//! break, not a refactor, so they are constants asserted by name in the tests.

use hkdf::Hkdf;
use sha2::{Digest as _, Sha256, Sha512};
use zeroize::Zeroizing;

use crate::keys::{
    AuditKey, DomainId, DomainKek, DomainLocatorKey, IDENTIFIER_BYTES, KEY_BYTES, ProfileId,
    RecipientMacKey, RehearsalKey, StoreKey, VaultMasterKey,
};

/// Info prefix for a per-domain key-encryption key; the domain identity is
/// appended to it, exactly as written in ADR-005.
pub const KEK_INFO_PREFIX: &[u8] = b"academic-os/kek/v1";
/// Info string for the SQLCipher store key.
pub const STORE_INFO: &[u8] = b"academic-os/store/v1";
/// Info string for the egress-audit key.
pub const AUDIT_INFO: &[u8] = b"academic-os/audit/v1";
/// Info string for the recipient-record MAC key.
///
/// ADR-005 names three derived keys and separately requires a MAC over each
/// recipient record *under the VMK*. That MAC needs its own key rather than
/// reusing one of the three, so `P2-K1` fixes this fourth info string in the
/// same `academic-os/<purpose>/v1` scheme.
pub const RECIPIENT_MAC_INFO: &[u8] = b"academic-os/recipient-mac/v1";
/// Info string for the restore-rehearsal receipt MAC key.
///
/// `P2-K4`'s rehearsal receipt is a profile-local admission fact: it states
/// that a restore of this profile was actually carried out against the key
/// material in force at the time. It is authenticated under the VMK rather
/// than under the backup key, because the gate it feeds runs at ingest time on
/// an unlocked profile, where the VMK is in hand and the recovery phrase is
/// not. Deriving it here rather than reusing `AUDKEY` keeps one key from
/// authenticating both egress audit rows and admission receipts.
pub const REHEARSAL_INFO: &[u8] = b"academic-os/rehearsal/v1";
/// Info string for the public commitment that names one key generation.
///
/// `P2-K5` rotates the Vault Master Key, and both the rotation journal and the
/// recipient set have to say *which* generation a record belongs to while the
/// profile is still locked. That name must therefore be readable without a key
/// and must reveal nothing about one. It is the SHA-256 of an HKDF output
/// under this info string: HKDF-SHA-512 is one-way, and hashing its output
/// again means the published value is structurally not usable as a key, so a
/// leaked generation name cannot become key material by being pasted into the
/// wrong constructor.
pub const KEY_GENERATION_INFO: &[u8] = b"academic-os/key-generation/v1";
/// Info string for a domain's vault-locator HMAC key.
///
/// t068 section 2.3-7 keeps the physical locator a domain-keyed HMAC, and
/// `P2-K3` needs that key for an encrypted profile. It is a *sub-derivation of
/// `KEK_d`*, not a fifth key off the Vault Master Key: the input keying
/// material is the domain KEK, so a domain's locator namespace cannot be
/// computed by anyone who cannot already open that domain's objects. Deriving
/// it rather than using `KEK_d` directly keeps one key from being both an
/// XChaCha20-Poly1305 key and an HMAC-SHA-256 key.
pub const VAULT_LOCATOR_INFO: &[u8] = b"academic-os/vault-locator/v1";

/// A key-schedule failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum KeyScheduleError {
    /// HKDF refused the requested output length.
    ///
    /// Unreachable for the fixed 32-byte outputs of this schedule; it exists so
    /// derivation is a total function with no panicking branch.
    #[error("the key schedule could not expand the requested output")]
    Derivation,
}

fn expand_from(
    ikm: &[u8; KEY_BYTES],
    profile: ProfileId,
    info: &[u8],
) -> Result<Zeroizing<[u8; KEY_BYTES]>, KeyScheduleError> {
    let extracted = Hkdf::<Sha512>::new(Some(profile.as_bytes()), ikm);
    let mut output = Zeroizing::new([0_u8; KEY_BYTES]);
    extracted
        .expand(info, output.as_mut())
        .map_err(|_| KeyScheduleError::Derivation)?;
    Ok(output)
}

fn expand(
    master: &VaultMasterKey,
    profile: ProfileId,
    info: &[u8],
) -> Result<Zeroizing<[u8; KEY_BYTES]>, KeyScheduleError> {
    expand_from(master.expose_secret(), profile, info)
}

/// Builds the KEK info string: the fixed prefix followed by the domain identity.
fn kek_info(domain: DomainId) -> [u8; 18 + IDENTIFIER_BYTES] {
    let mut info = [0_u8; 18 + IDENTIFIER_BYTES];
    info[..18].copy_from_slice(KEK_INFO_PREFIX);
    info[18..].copy_from_slice(domain.as_bytes());
    info
}

impl VaultMasterKey {
    /// Derives `KEK_d` for one domain.
    pub fn derive_domain_kek(
        &self,
        profile: ProfileId,
        domain: DomainId,
    ) -> Result<DomainKek, KeyScheduleError> {
        Ok(DomainKek::from_zeroizing(expand(
            self,
            profile,
            &kek_info(domain),
        )?))
    }

    /// Derives `SKEY_p`, the raw SQLCipher store key.
    pub fn derive_store_key(&self, profile: ProfileId) -> Result<StoreKey, KeyScheduleError> {
        Ok(StoreKey::from_zeroizing(expand(self, profile, STORE_INFO)?))
    }

    /// Derives `AUDKEY`, the egress-audit key.
    pub fn derive_audit_key(&self, profile: ProfileId) -> Result<AuditKey, KeyScheduleError> {
        Ok(AuditKey::from_zeroizing(expand(self, profile, AUDIT_INFO)?))
    }

    /// Derives the key a restore-rehearsal receipt is authenticated under.
    pub fn derive_rehearsal_key(
        &self,
        profile: ProfileId,
    ) -> Result<RehearsalKey, KeyScheduleError> {
        Ok(RehearsalKey::from_zeroizing(expand(
            self,
            profile,
            REHEARSAL_INFO,
        )?))
    }

    /// Returns the public, non-secret name of this key generation.
    ///
    /// Two Vault Master Keys have the same generation name only if they are the
    /// same key, and the name discloses no part of it. `P2-K5` writes it into
    /// the rotation journal so a resumed rotation can prove it was handed the
    /// same pair of keys it started with, and so the journal can be read and
    /// audited on a locked profile.
    ///
    /// This is not a key and no constructor in this crate accepts it.
    pub fn generation_id(&self, profile: ProfileId) -> Result<[u8; KEY_BYTES], KeyScheduleError> {
        let expanded = expand(self, profile, KEY_GENERATION_INFO)?;
        Ok(Sha256::digest(expanded.as_ref()).into())
    }

    /// Derives the key the recipient-record MAC is taken under.
    pub fn derive_recipient_mac_key(
        &self,
        profile: ProfileId,
    ) -> Result<RecipientMacKey, KeyScheduleError> {
        Ok(RecipientMacKey::from_zeroizing(expand(
            self,
            profile,
            RECIPIENT_MAC_INFO,
        )?))
    }
}

impl DomainKek {
    /// Derives `LOC_d`, the HMAC key this domain's vault locators use.
    ///
    /// The input keying material is this domain KEK, so the locator namespace
    /// stays scoped to exactly the domain that can open the objects in it.
    pub fn derive_locator_key(
        &self,
        profile: ProfileId,
    ) -> Result<DomainLocatorKey, KeyScheduleError> {
        Ok(DomainLocatorKey::from_zeroizing(expand_from(
            self.expose_secret(),
            profile,
            VAULT_LOCATOR_INFO,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use hmac::{Hmac, Mac};

    use super::*;
    use crate::keys::KEY_BYTES;

    fn master(byte: u8) -> VaultMasterKey {
        VaultMasterKey::from_bytes(Zeroizing::new([byte; KEY_BYTES]))
    }

    /// RFC 5869 HKDF, written out here so the schedule is checked against the
    /// specification rather than against the same crate that produced it.
    fn rfc5869_hkdf_sha512(salt: &[u8], ikm: &[u8], info: &[u8], length: usize) -> Vec<u8> {
        let Ok(mut extract) = <Hmac<Sha512> as Mac>::new_from_slice(salt) else {
            unreachable!("HMAC accepts any key length");
        };
        extract.update(ikm);
        let pseudorandom_key = extract.finalize().into_bytes();

        let mut output = Vec::new();
        let mut previous: Vec<u8> = Vec::new();
        let mut counter = 1_u8;
        while output.len() < length {
            let Ok(mut round) = <Hmac<Sha512> as Mac>::new_from_slice(&pseudorandom_key) else {
                unreachable!("HMAC accepts any key length");
            };
            round.update(&previous);
            round.update(info);
            round.update(&[counter]);
            previous = round.finalize().into_bytes().to_vec();
            output.extend_from_slice(&previous);
            counter += 1;
        }
        output.truncate(length);
        output
    }

    #[test]
    fn info_strings_are_the_frozen_adr_005_literals() {
        assert_eq!(KEK_INFO_PREFIX, b"academic-os/kek/v1");
        assert_eq!(STORE_INFO, b"academic-os/store/v1");
        assert_eq!(AUDIT_INFO, b"academic-os/audit/v1");
        assert_eq!(RECIPIENT_MAC_INFO, b"academic-os/recipient-mac/v1");
        assert_eq!(VAULT_LOCATOR_INFO, b"academic-os/vault-locator/v1");
        assert_eq!(KEK_INFO_PREFIX.len(), 18);
    }

    #[test]
    fn the_locator_key_is_an_hkdf_sub_derivation_of_the_domain_kek() {
        let key = master(0x44);
        let profile = ProfileId::from_bytes([0x55; IDENTIFIER_BYTES]);
        let domain = DomainId::from_bytes([0x66; IDENTIFIER_BYTES]);
        let Ok(kek) = key.derive_domain_kek(profile, domain) else {
            unreachable!("KEK derivation must succeed");
        };
        let Ok(locator) = kek.derive_locator_key(profile) else {
            unreachable!("locator derivation must succeed");
        };
        assert_eq!(
            locator.expose_secret().as_slice(),
            rfc5869_hkdf_sha512(
                profile.as_bytes(),
                kek.expose_secret(),
                VAULT_LOCATOR_INFO,
                KEY_BYTES
            )
            .as_slice()
        );
        // The locator key is never the KEK it came from.
        assert_ne!(locator.expose_secret(), kek.expose_secret());
    }

    #[test]
    fn locator_keys_are_separated_by_domain_and_by_profile() {
        let key = master(0x77);
        let profile_a = ProfileId::from_bytes([0x01; IDENTIFIER_BYTES]);
        let profile_b = ProfileId::from_bytes([0x02; IDENTIFIER_BYTES]);
        let domain_a = DomainId::from_bytes([0x03; IDENTIFIER_BYTES]);
        let domain_b = DomainId::from_bytes([0x04; IDENTIFIER_BYTES]);
        let derive = |profile, domain| {
            let Ok(kek) = key.derive_domain_kek(profile, domain) else {
                unreachable!("KEK derivation must succeed");
            };
            let Ok(locator) = kek.derive_locator_key(profile) else {
                unreachable!("locator derivation must succeed");
            };
            *locator.expose_secret()
        };
        let base = derive(profile_a, domain_a);
        assert_ne!(base, derive(profile_a, domain_b));
        assert_ne!(base, derive(profile_b, domain_a));
    }

    #[test]
    fn the_schedule_agrees_with_rfc_5869_hkdf_sha_512() {
        let key = master(0x11);
        let profile = ProfileId::from_bytes([0x22; IDENTIFIER_BYTES]);
        let domain = DomainId::from_bytes([0x33; IDENTIFIER_BYTES]);

        let Ok(store) = key.derive_store_key(profile) else {
            unreachable!("store derivation must succeed");
        };
        assert_eq!(
            store.expose_secret().as_slice(),
            rfc5869_hkdf_sha512(
                profile.as_bytes(),
                key.expose_secret(),
                STORE_INFO,
                KEY_BYTES
            )
            .as_slice()
        );

        let Ok(kek) = key.derive_domain_kek(profile, domain) else {
            unreachable!("KEK derivation must succeed");
        };
        let mut expected_info = KEK_INFO_PREFIX.to_vec();
        expected_info.extend_from_slice(domain.as_bytes());
        assert_eq!(
            kek.expose_secret().as_slice(),
            rfc5869_hkdf_sha512(
                profile.as_bytes(),
                key.expose_secret(),
                &expected_info,
                KEY_BYTES
            )
            .as_slice()
        );
    }
}
