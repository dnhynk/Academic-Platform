//! The three recovery profiles of t068 section 3.3.
//!
//! A profile is a *selection of recipients*, and the selection is the user's.
//! This module ships the three the plan requires, states each one's loss
//! behaviour in the words the plan fixes, and refuses to pick one.

use academic_crypto::{RecipientKind, RecipientRecord};

/// The exact words `DEVICE_ONLY` must state.
///
/// t068 section 3.3 requires the irrecoverability of `DEVICE_ONLY` to be
/// stated "in those words". Every surface that shows the profile shows this
/// constant; nothing paraphrases it, and
/// `device_only_profile_states_irrecoverability_verbatim` fails if a surface
/// stops carrying it.
pub const DEVICE_ONLY_IRRECOVERABILITY_STATEMENT: &str =
    "OS reimage or device loss is unrecoverable";

/// Which recipients a profile requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecipientRequirement {
    /// The operating-system broker holds the wrapping key.
    DeviceKeystore,
    /// A printed 24-word recovery phrase.
    RecoveryPhrase,
    /// A key file the user stores away from the device.
    OfflineKeyFile,
}

impl RecipientRequirement {
    /// Returns the stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeviceKeystore => "DEVICE_KEYSTORE",
            Self::RecoveryPhrase => "RECOVERY_PHRASE",
            Self::OfflineKeyFile => "OFFLINE_KEY_FILE",
        }
    }

    /// Whether this requirement survives the loss of the device.
    ///
    /// This is the property that decides whether a profile can hold a backup
    /// key at all: a backup sealed only under something the device holds is
    /// not a backup.
    #[must_use]
    pub const fn survives_device_loss(self) -> bool {
        match self {
            Self::DeviceKeystore => false,
            Self::RecoveryPhrase | Self::OfflineKeyFile => true,
        }
    }

    /// The `academic-crypto` recipient kind that satisfies this requirement.
    #[must_use]
    pub const fn recipient_kind(self) -> RecipientKind {
        match self {
            Self::DeviceKeystore => RecipientKind::DeviceKeystore,
            Self::RecoveryPhrase | Self::OfflineKeyFile => RecipientKind::RecoverySecret,
        }
    }
}

/// A recovery profile of t068 section 3.3.
///
/// There is deliberately no `Default`. `GATE-38-031` is a blocking user
/// choice: this build implements and drills all three, states what each one
/// loses, and selects none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RecoveryProfile {
    /// Device keystore only.
    DeviceOnly,
    /// Device keystore plus a printed recovery phrase.
    DevicePlusPhrase,
    /// Device keystore, a printed phrase, and an offline key file.
    DevicePlusPhrasePlusOfflineFile,
}

/// Every profile this build ships, in the order t068 section 3.3 lists them.
pub const RECOVERY_PROFILES: &[RecoveryProfile] = &[
    RecoveryProfile::DeviceOnly,
    RecoveryProfile::DevicePlusPhrase,
    RecoveryProfile::DevicePlusPhrasePlusOfflineFile,
];

impl RecoveryProfile {
    /// Returns the stable external spelling written into every receipt.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeviceOnly => "DEVICE_ONLY",
            Self::DevicePlusPhrase => "DEVICE_PLUS_PHRASE",
            Self::DevicePlusPhrasePlusOfflineFile => "DEVICE_PLUS_PHRASE_PLUS_OFFLINE_FILE",
        }
    }

    /// Parses the closed profile vocabulary.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        RECOVERY_PROFILES
            .iter()
            .copied()
            .find(|profile| profile.as_str() == value)
    }

    /// Returns the recipients this profile requires, in a fixed order.
    #[must_use]
    pub const fn recipients(self) -> &'static [RecipientRequirement] {
        match self {
            Self::DeviceOnly => &[RecipientRequirement::DeviceKeystore],
            Self::DevicePlusPhrase => &[
                RecipientRequirement::DeviceKeystore,
                RecipientRequirement::RecoveryPhrase,
            ],
            Self::DevicePlusPhrasePlusOfflineFile => &[
                RecipientRequirement::DeviceKeystore,
                RecipientRequirement::RecoveryPhrase,
                RecipientRequirement::OfflineKeyFile,
            ],
        }
    }

    /// Returns the loss behaviour, in the exact words of t068 section 3.3.
    #[must_use]
    pub const fn loss_statement(self) -> &'static str {
        match self {
            Self::DeviceOnly => DEVICE_ONLY_IRRECOVERABILITY_STATEMENT,
            Self::DevicePlusPhrase => "recoverable on a fresh machine with the phrase",
            Self::DevicePlusPhrasePlusOfflineFile => {
                "recoverable with either secondary recipient; largest exposure surface"
            }
        }
    }

    /// Whether this profile can hold a backup key at all.
    ///
    /// A backup key must be independent of the device wrapper, so it must be
    /// wrapped by at least one recipient that survives the loss of the device.
    /// `DEVICE_ONLY` has none, and this is where that fact becomes mechanical
    /// rather than advisory.
    #[must_use]
    pub fn supports_independent_backup(self) -> bool {
        self.recipients()
            .iter()
            .any(|requirement| requirement.survives_device_loss())
    }

    /// Returns the recipients that may wrap a backup key under this profile.
    #[must_use]
    pub fn backup_capable_recipients(self) -> Vec<RecipientRequirement> {
        self.recipients()
            .iter()
            .copied()
            .filter(|requirement| requirement.survives_device_loss())
            .collect()
    }

    /// Checks a live recipient set against what this profile requires.
    ///
    /// The check is by *kind*, because `academic-crypto` distinguishes a
    /// device recipient from a recovery one and nothing finer. A phrase and an
    /// offline key file are both recovery-secret recipients, so this counts
    /// them rather than naming them.
    pub fn validate_recipients(
        self,
        records: &[RecipientRecord],
    ) -> Result<(), RecoveryProfileError> {
        let required_device = self
            .recipients()
            .iter()
            .filter(|requirement| requirement.recipient_kind() == RecipientKind::DeviceKeystore)
            .count();
        let required_recovery = self
            .recipients()
            .iter()
            .filter(|requirement| requirement.recipient_kind() == RecipientKind::RecoverySecret)
            .count();
        let present_device = records
            .iter()
            .filter(|record| record.kind() == RecipientKind::DeviceKeystore)
            .count();
        let present_recovery = records
            .iter()
            .filter(|record| record.kind() == RecipientKind::RecoverySecret)
            .count();
        if present_device < required_device {
            return Err(RecoveryProfileError::MissingRecipient {
                profile: self.as_str(),
                requirement: RecipientRequirement::DeviceKeystore.as_str(),
            });
        }
        if present_recovery < required_recovery {
            return Err(RecoveryProfileError::MissingRecipient {
                profile: self.as_str(),
                requirement: RecipientRequirement::RecoveryPhrase.as_str(),
            });
        }
        if self == Self::DeviceOnly && present_recovery > 0 {
            return Err(RecoveryProfileError::UnexpectedRecipient {
                profile: self.as_str(),
                requirement: RecipientRequirement::RecoveryPhrase.as_str(),
            });
        }
        Ok(())
    }
}

/// A recipient set that does not match the selected profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RecoveryProfileError {
    /// The profile requires a recipient the set does not hold.
    #[error("recovery profile {profile} requires a {requirement} recipient and the set has none")]
    MissingRecipient {
        /// Selected profile.
        profile: &'static str,
        /// The requirement that is unmet.
        requirement: &'static str,
    },
    /// The set holds a recipient the profile excludes.
    #[error(
        "recovery profile {profile} excludes {requirement} recipients; \
         select a profile that includes one instead of adding it silently"
    )]
    UnexpectedRecipient {
        /// Selected profile.
        profile: &'static str,
        /// The requirement that must not be present.
        requirement: &'static str,
    },
    /// The profile cannot hold a backup key.
    #[error(
        "recovery profile {profile} has no recipient that survives the loss of \
         the device, so it cannot hold a backup key: {statement}"
    )]
    NoIndependentBackupRecipient {
        /// Selected profile.
        profile: &'static str,
        /// The profile's own loss statement, verbatim.
        statement: &'static str,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_profile_has_a_distinct_stable_spelling() {
        let mut seen = Vec::new();
        for profile in RECOVERY_PROFILES {
            let name = profile.as_str();
            assert!(!seen.contains(&name), "{name} is listed twice");
            assert_eq!(RecoveryProfile::parse(name), Some(*profile));
            seen.push(name);
        }
        assert_eq!(seen.len(), 3);
        assert_eq!(RecoveryProfile::parse("DEVICE_PLUS_KEYCARD"), None);
    }

    #[test]
    fn only_device_only_lacks_an_independent_backup_recipient() {
        assert!(!RecoveryProfile::DeviceOnly.supports_independent_backup());
        assert!(RecoveryProfile::DevicePlusPhrase.supports_independent_backup());
        assert!(RecoveryProfile::DevicePlusPhrasePlusOfflineFile.supports_independent_backup());
        assert!(
            RecoveryProfile::DeviceOnly
                .backup_capable_recipients()
                .is_empty()
        );
    }

    #[test]
    fn every_profile_requires_the_device_keystore() {
        for profile in RECOVERY_PROFILES {
            assert!(
                profile
                    .recipients()
                    .contains(&RecipientRequirement::DeviceKeystore),
                "{} dropped the device recipient",
                profile.as_str()
            );
        }
    }
}
