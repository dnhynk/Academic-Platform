//! What a deletion is *about*, and why a locator alone cannot say it.
//!
//! A vault locator is `HMAC(LOC_d, format || media_type || 0 || content_digest)`.
//! It carries no permission lineage and no retention class, so one security
//! domain gives the same bytes the same locator in every lineage: registering
//! one document twice is two artifacts, two paths, and one name.
//!
//! The fifth `P2-A1` audit found what that costs a deletion path that names its
//! subject by locator (`P1-G1`): deleting both registrations wrote a second
//! tombstone over the first, a restore republished the artifact deleted first
//! as readable, and the receipt reported it as a copy the deletion had
//! deliberately spared. `P2-K5` closed the tombstone record and the tombstone
//! file name. It left the rest of the deletion path open and said so — item
//! `P3-G10` of the rotation contract: `PlannedAction.locator`,
//! `RetentionPlanned.subject_locator` and `ArtifactShredded.locator` still name
//! one deleted object by its locator, and "adding the artifact to these records
//! is a journal format change and is left for whoever writes the executor".
//!
//! This crate is whoever writes the executor. [`DeletionTarget`] is the pair,
//! and every derived record here — a dry-run node, a preview line, an
//! unresolved row, a receipt entry — is keyed by it. `academic-retention`'s
//! `PlannedAction` still carries the locator alone, because that is the value
//! its executor seam takes; what this crate never does is *look one up* by it.

use academic_domain::ArtifactId;

/// One artifact, at one locator.
///
/// Both halves, always. There is no constructor that takes a locator without an
/// artifact, and [`DeletionTarget`] is what every map in this crate is keyed by,
/// so two registrations of the same bytes are two entries and never one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeletionTarget {
    artifact: ArtifactId,
    locator: [u8; 32],
}

impl DeletionTarget {
    /// Names one artifact at one locator.
    #[must_use]
    pub const fn new(artifact: ArtifactId, locator: [u8; 32]) -> Self {
        Self { artifact, locator }
    }

    /// The artifact this deletion is about.
    #[must_use]
    pub const fn artifact(&self) -> ArtifactId {
        self.artifact
    }

    /// The locator the artifact is reachable under.
    #[must_use]
    pub const fn locator(&self) -> &[u8; 32] {
        &self.locator
    }

    /// The artifact's hex spelling.
    #[must_use]
    pub fn artifact_hex(&self) -> String {
        hex_of(self.artifact.as_bytes())
    }

    /// The locator's hex spelling.
    #[must_use]
    pub fn locator_hex(&self) -> String {
        hex_of(&self.locator)
    }

    /// The row a report shows: artifact first, then locator.
    ///
    /// The artifact leads because it is the half that differs when two
    /// registrations of one document are both deleted, and a row that led with
    /// the shared half would sort the two together and read as one.
    #[must_use]
    pub fn to_row(&self) -> String {
        format!("{}@{}", self.artifact_hex(), self.locator_hex())
    }
}

/// Lowercase hex, without a dependency this crate has no other use for.
fn hex_of(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        text.push(char::from_digit(u32::from(byte & 0x0F), 16).unwrap_or('0'));
    }
    text
}
