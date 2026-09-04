//! What a caller hands the writer, and the one choice the user makes.
//!
//! # The writer takes a value, not a database
//!
//! A [`SourceView`] is the canonical state of one committed watermark, already
//! read. That is what keeps this crate free of a store edge: the code that
//! knows how to open a profile stays in the crate that owns the profile, and
//! the code that writes the artefact a user keeps forever links nothing they
//! would have to still have.
//!
//! Each row carries two things: the few fields the bundle **routes** on — a
//! predicate namespace, a security domain, a confidentiality — and the exact
//! canonical JSON record, which is written out unchanged. A bundle therefore
//! carries the same record bytes the Phase 1 export carries without this crate
//! holding a second transcription of every column, which is the drift this
//! shape exists to avoid.
//!
//! # Every row has a security domain, and that is what makes labelling exact
//!
//! Section 32.10 wants a label, a restriction and a notice **per file**. A file
//! that mixed two security domains would have to carry one notice for two sets
//! of terms, and the only ways out of that are inventing a combined notice or
//! picking one. So content files are written per security domain: a claim is
//! placed by its scope's domain, evidence by its artifact's, an event by its
//! own. There is no row this crate can place in a file whose terms it cannot
//! state.

use std::path::PathBuf;

use academic_domain::Confidentiality;
use serde::{Deserialize, Serialize};

use crate::{ExportError, ExportResult, label::SensitivityLabel};

/// Whether the user asked for the original bytes to travel with the bundle.
///
/// No `Default`, and [`crate::BundleRequest`] takes it by value. Section 37
/// writes the archive as *원본을 포함하거나 제외할 수 있는*, which is a choice
/// and therefore may not have a value someone gets by not deciding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OriginalInclusion {
    /// The bundle carries every registered artifact's exact bytes.
    Included,
    /// The bundle carries each artifact's identity and plaintext digest and no
    /// bytes, and no record in it names a path.
    Withheld,
}

impl OriginalInclusion {
    /// Every choice.
    pub const ALL: [Self; 2] = [Self::Included, Self::Withheld];

    /// Whether original bytes travel.
    #[must_use]
    pub const fn includes_originals(self) -> bool {
        matches!(self, Self::Included)
    }

    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Included => "INCLUDED",
            Self::Withheld => "WITHHELD",
        }
    }
}

/// Why a bundle carries an artifact's identity and not its bytes.
///
/// One arm today, and it is the user's choice. It is an enum rather than a
/// boolean so a second reason — a legal hold, a shredded key slot — is a new
/// arm every reader must handle rather than a `false` that reads the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WithheldReason {
    /// The user chose to export without originals.
    UserExcludedOriginals,
}

impl WithheldReason {
    /// The contract spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserExcludedOriginals => "USER_EXCLUDED_ORIGINALS",
        }
    }
}

/// One registered artifact, and where its exact bytes can be read.
///
/// `vault_locator` is recorded and never used. It is not a key in any map, not
/// a filename, and not a path segment: two artifacts with identical bytes in
/// one security domain share a locator, so a bundle that addressed objects by
/// locator would publish one file for two artifacts and lose one of them. The
/// address is [`Self::artifact_id`] everywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactSource {
    artifact_id: String,
    domain_id: String,
    confidentiality: Confidentiality,
    content_sha256: String,
    byte_length: u64,
    media_type: String,
    vault_locator: String,
    original_path: PathBuf,
    canonical_json: String,
}

impl ArtifactSource {
    /// Records one registered artifact.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        artifact_id: impl Into<String>,
        domain_id: impl Into<String>,
        confidentiality: Confidentiality,
        content_sha256: impl Into<String>,
        byte_length: u64,
        media_type: impl Into<String>,
        vault_locator: impl Into<String>,
        original_path: PathBuf,
        canonical_json: impl Into<String>,
    ) -> ExportResult<Self> {
        Ok(Self {
            artifact_id: identifier("artifact identifier", artifact_id.into())?,
            domain_id: identifier("security domain identifier", domain_id.into())?,
            confidentiality,
            content_sha256: identifier("artifact content digest", content_sha256.into())?,
            byte_length,
            media_type: media_type.into(),
            vault_locator: vault_locator.into(),
            original_path,
            canonical_json: canonical_line("artifact record", canonical_json.into())?,
        })
    }

    /// The artifact's own identifier, which is its only address.
    #[must_use]
    pub fn artifact_id(&self) -> &str {
        &self.artifact_id
    }

    /// The security domain whose terms cover it.
    #[must_use]
    pub fn domain_id(&self) -> &str {
        &self.domain_id
    }

    /// Its recorded confidentiality.
    #[must_use]
    pub const fn confidentiality(&self) -> Confidentiality {
        self.confidentiality
    }

    /// The sensitivity label its confidentiality produces.
    #[must_use]
    pub const fn label(&self) -> SensitivityLabel {
        SensitivityLabel::of(self.confidentiality)
    }

    /// The exact plaintext digest, lowercase hex.
    #[must_use]
    pub fn content_sha256(&self) -> &str {
        &self.content_sha256
    }

    /// The exact plaintext length.
    #[must_use]
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// The registered media type.
    #[must_use]
    pub fn media_type(&self) -> &str {
        &self.media_type
    }

    /// The vault locator, recorded as an attribute and used as nothing.
    #[must_use]
    pub fn vault_locator(&self) -> &str {
        &self.vault_locator
    }

    /// Where the exact bytes are readable while the export runs.
    #[must_use]
    pub fn original_path(&self) -> &PathBuf {
        &self.original_path
    }

    /// The exact canonical record.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }
}

/// One canonical record that is routed by a domain and written unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainRecord {
    id: String,
    domain_id: String,
    canonical_json: String,
}

impl DomainRecord {
    /// Records one row.
    pub fn new(
        id: impl Into<String>,
        domain_id: impl Into<String>,
        canonical_json: impl Into<String>,
    ) -> ExportResult<Self> {
        Ok(Self {
            id: identifier("canonical record identifier", id.into())?,
            domain_id: identifier("security domain identifier", domain_id.into())?,
            canonical_json: canonical_line("canonical record", canonical_json.into())?,
        })
    }

    /// The record's canonical identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The security domain whose terms cover it.
    #[must_use]
    pub fn domain_id(&self) -> &str {
        &self.domain_id
    }

    /// The exact canonical record.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }
}

/// One canonical claim, routed by a domain and by a predicate namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimSource {
    record: DomainRecord,
    predicate_id: String,
    evidence_ids: Vec<String>,
}

impl ClaimSource {
    /// Records one claim.
    pub fn new(
        claim_id: impl Into<String>,
        domain_id: impl Into<String>,
        predicate_id: impl Into<String>,
        evidence_ids: Vec<String>,
        canonical_json: impl Into<String>,
    ) -> ExportResult<Self> {
        Ok(Self {
            record: DomainRecord::new(claim_id, domain_id, canonical_json)?,
            predicate_id: identifier("predicate identifier", predicate_id.into())?,
            evidence_ids,
        })
    }

    /// The routed record.
    #[must_use]
    pub const fn record(&self) -> &DomainRecord {
        &self.record
    }

    /// The predicate whose first segment names the section 37 topic.
    #[must_use]
    pub fn predicate_id(&self) -> &str {
        &self.predicate_id
    }

    /// The evidence this claim rests on.
    #[must_use]
    pub fn evidence_ids(&self) -> &[String] {
        &self.evidence_ids
    }
}

/// One accepted batch, and its original signed envelope on disk.
///
/// The envelope is copied byte-for-byte and re-hashed against the digest the
/// ledger recorded. Re-serialising it would change signed historical bytes,
/// which the deterministic envelope contract forbids and which would make every
/// signature in the bundle unverifiable by anyone but this build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchSource {
    batch_id: String,
    domain_id: String,
    envelope_sha256: String,
    envelope_byte_length: u64,
    envelope_path: PathBuf,
    canonical_json: String,
}

impl BatchSource {
    /// Records one accepted batch.
    ///
    /// The security domain is taken rather than inferred. A batch is accepted
    /// into one domain and the caller reading the ledger knows which; deriving
    /// it here by matching identifiers inside a canonical record would make the
    /// terms a file carries depend on a substring search.
    pub fn new(
        batch_id: impl Into<String>,
        domain_id: impl Into<String>,
        envelope_sha256: impl Into<String>,
        envelope_byte_length: u64,
        envelope_path: PathBuf,
        canonical_json: impl Into<String>,
    ) -> ExportResult<Self> {
        Ok(Self {
            batch_id: identifier("batch identifier", batch_id.into())?,
            domain_id: identifier("security domain identifier", domain_id.into())?,
            envelope_sha256: identifier("envelope digest", envelope_sha256.into())?,
            envelope_byte_length,
            envelope_path,
            canonical_json: canonical_line("batch record", canonical_json.into())?,
        })
    }

    /// The batch identifier, which is the envelope's filename stem.
    #[must_use]
    pub fn batch_id(&self) -> &str {
        &self.batch_id
    }

    /// The security domain whose terms cover its envelope.
    #[must_use]
    pub fn domain_id(&self) -> &str {
        &self.domain_id
    }

    /// The digest the ledger recorded for the original envelope.
    #[must_use]
    pub fn envelope_sha256(&self) -> &str {
        &self.envelope_sha256
    }

    /// The exact envelope length.
    #[must_use]
    pub const fn envelope_byte_length(&self) -> u64 {
        self.envelope_byte_length
    }

    /// Where the original envelope is readable while the export runs.
    #[must_use]
    pub fn envelope_path(&self) -> &PathBuf {
        &self.envelope_path
    }

    /// The exact canonical batch record.
    #[must_use]
    pub fn canonical_json(&self) -> &str {
        &self.canonical_json
    }
}

/// One version-control reference the repository part carries.
///
/// Field for field what `academic_repository::RepositorySnapshot` records about
/// where a snapshot sits in history: the branch it was taken on, the commit it
/// names, the snapshots it follows, and the commits its submodules are pinned
/// to. It is a value here rather than an edge, because the reader must be able
/// to read a repository's history out of a directory without the analyser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitRef {
    /// The repository this snapshot is of.
    pub repository_id: String,
    /// The snapshot's own identifier.
    pub snapshot_id: String,
    /// The security domain whose terms cover it.
    pub domain_id: String,
    /// The branch, when the working-tree facts reported one.
    pub branch: Option<String>,
    /// The commit, when there is one.
    pub commit: Option<String>,
    /// Earlier snapshots this one follows, in recorded order.
    pub parent_snapshots: Vec<String>,
    /// Submodule paths and the commits they are pinned to, sorted by path.
    pub submodules: Vec<(String, String)>,
}

impl GitRef {
    /// Refuses a reference this crate cannot write honestly.
    ///
    /// A commit name is lowercase hexadecimal or it is not an object name.
    /// Recording something else would put a value in an open format that no
    /// other tool can resolve, which is the failure this whole crate is about.
    pub fn validate(&self) -> ExportResult<()> {
        identifier("repository identifier", self.repository_id.clone())?;
        identifier("snapshot identifier", self.snapshot_id.clone())?;
        identifier("security domain identifier", self.domain_id.clone())?;
        if let Some(commit) = &self.commit {
            require_object_name(commit)?;
        }
        for (path, commit) in &self.submodules {
            if path.is_empty() {
                return Err(ExportError::Malformed {
                    item: "submodule path",
                    value: path.clone(),
                });
            }
            require_object_name(commit)?;
        }
        if !self
            .submodules
            .is_sorted_by(|left, right| left.0 <= right.0)
        {
            return Err(ExportError::Malformed {
                item: "submodule order",
                value: self.snapshot_id.clone(),
            });
        }
        Ok(())
    }
}

fn require_object_name(value: &str) -> ExportResult<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ExportError::Malformed {
            item: "commit object name",
            value: value.to_owned(),
        });
    }
    Ok(())
}

/// The physical store identity the exported watermark was read from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreIdentity {
    /// The store format UUID.
    pub format_uuid: String,
    /// The numeric schema version.
    pub schema_version: u32,
    /// The semantic schema version.
    pub schema_semver: String,
}

/// The committed watermark the bundle is a bundle of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Watermark {
    /// The next acceptance sequence.
    pub next_accept_seq: u64,
    /// The canonical profile revision.
    pub profile_revision: u64,
    /// The acceptance head.
    pub accept_seq_head: u64,
    /// The projection outbox head.
    pub outbox_head: u64,
}

/// One device's head of the origin chain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceHead {
    /// The device.
    pub device_id: String,
    /// Its next origin sequence.
    pub next_origin_seq: u64,
    /// The digest of the envelope at its head.
    pub head_envelope_sha256: String,
}

/// Everything the writer needs, already read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceView {
    /// The physical store identity.
    pub store: StoreIdentity,
    /// The committed watermark.
    pub watermark: Watermark,
    /// Every device head, sorted by device identifier.
    pub device_heads: Vec<DeviceHead>,
    /// The canonical semantic digest the source recorded for this watermark.
    pub canonical_semantic_digest: String,
    /// Every accepted batch, sorted by batch identifier.
    pub batches: Vec<BatchSource>,
    /// Every accepted event, sorted by event identifier.
    pub events: Vec<DomainRecord>,
    /// Every registered scope, sorted by scope identifier.
    pub scopes: Vec<DomainRecord>,
    /// Every registered artifact, sorted by artifact identifier.
    pub artifacts: Vec<ArtifactSource>,
    /// Every registered evidence item, sorted by evidence identifier.
    pub evidence: Vec<DomainRecord>,
    /// Every asserted claim, sorted by claim identifier.
    pub claims: Vec<ClaimSource>,
    /// Every claim relation, sorted by relation event identifier.
    pub relations: Vec<DomainRecord>,
    /// Every recorded user decision, sorted by decision identifier.
    pub decisions: Vec<DomainRecord>,
    /// Every version-control reference, sorted by snapshot identifier.
    pub git_refs: Vec<GitRef>,
}

impl SourceView {
    /// Refuses a view the writer could not turn into a portable bundle.
    ///
    /// Ordering is checked rather than imposed: a writer that sorted its input
    /// would produce identical bytes from two differently ordered reads and
    /// hide a source that had stopped being deterministic.
    pub fn validate(&self) -> ExportResult<()> {
        identifier(
            "canonical semantic digest",
            self.canonical_semantic_digest.clone(),
        )?;
        require_sorted(
            "device head",
            self.device_heads.iter().map(|head| head.device_id.as_str()),
        )?;
        require_sorted("batch", self.batches.iter().map(BatchSource::batch_id))?;
        require_sorted("event", self.events.iter().map(DomainRecord::id))?;
        require_sorted("scope", self.scopes.iter().map(DomainRecord::id))?;
        require_sorted(
            "artifact",
            self.artifacts.iter().map(ArtifactSource::artifact_id),
        )?;
        require_sorted("evidence", self.evidence.iter().map(DomainRecord::id))?;
        require_sorted("claim", self.claims.iter().map(|claim| claim.record().id()))?;
        require_sorted("relation", self.relations.iter().map(DomainRecord::id))?;
        require_sorted("decision", self.decisions.iter().map(DomainRecord::id))?;
        require_sorted(
            "git ref",
            self.git_refs
                .iter()
                .map(|reference| reference.snapshot_id.as_str()),
        )?;
        for reference in &self.git_refs {
            reference.validate()?;
        }
        Ok(())
    }

    /// Every security domain any row in this view belongs to, sorted and
    /// deduplicated.
    ///
    /// The order does **not** reach a byte of a bundle: every file list is
    /// sorted by path before it is written, and every label is a maximum. So
    /// `P1-I5` replaced this sort with a hash set and no output changed and no
    /// acceptance test could see it. What holds the order is this function's
    /// own contract test, `domains_are_sorted_and_deduplicated`, rather than a
    /// downstream comparison that would only have looked like it did.
    pub fn domains(&self) -> Vec<String> {
        let mut domains: Vec<String> = Vec::new();
        let mut push = |domain: &str| {
            if !domains.iter().any(|known| known == domain) {
                domains.push(domain.to_owned());
            }
        };
        for batch in &self.batches {
            push(batch.domain_id());
        }
        for record in &self.events {
            push(record.domain_id());
        }
        for record in &self.scopes {
            push(record.domain_id());
        }
        for artifact in &self.artifacts {
            push(artifact.domain_id());
        }
        for record in &self.evidence {
            push(record.domain_id());
        }
        for claim in &self.claims {
            push(claim.record().domain_id());
        }
        for record in &self.relations {
            push(record.domain_id());
        }
        for record in &self.decisions {
            push(record.domain_id());
        }
        for reference in &self.git_refs {
            push(reference.domain_id.as_str());
        }
        domains.sort();
        domains
    }

    /// The strongest label any artifact in one security domain carries.
    #[must_use]
    pub fn domain_label(&self, domain_id: &str) -> SensitivityLabel {
        SensitivityLabel::strongest_of(
            self.artifacts
                .iter()
                .filter(|artifact| artifact.domain_id() == domain_id)
                .map(ArtifactSource::label),
        )
    }
}

fn require_sorted<'a>(
    kind: &'static str,
    values: impl Iterator<Item = &'a str>,
) -> ExportResult<()> {
    let mut previous: Option<&str> = None;
    for value in values {
        if previous.is_some_and(|earlier| earlier >= value) {
            return Err(ExportError::Malformed {
                item: "source ordering",
                value: format!("{kind} {value} is not strictly after the one before it"),
            });
        }
        previous = Some(value);
    }
    Ok(())
}

fn identifier(item: &'static str, value: String) -> ExportResult<String> {
    if value.is_empty()
        || value.contains('/')
        || value.contains('\\')
        || value.contains('\n')
        || value.contains(' ')
    {
        return Err(ExportError::Malformed { item, value });
    }
    Ok(value)
}

fn canonical_line(item: &'static str, value: String) -> ExportResult<String> {
    if value.is_empty() || value.contains('\n') || value.contains('\r') {
        return Err(ExportError::Malformed { item, value });
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactSource, ClaimSource, DeviceHead, DomainRecord, GitRef, OriginalInclusion,
        SourceView, StoreIdentity, Watermark, WithheldReason,
    };
    use crate::{ExportResult, label::SensitivityLabel};

    fn reference() -> GitRef {
        GitRef {
            repository_id: "repo-1".to_owned(),
            snapshot_id: "snap-1".to_owned(),
            domain_id: "domain-1".to_owned(),
            branch: Some("main".to_owned()),
            commit: Some("0123456789abcdef".to_owned()),
            parent_snapshots: Vec::new(),
            submodules: Vec::new(),
        }
    }

    fn view(domains: &[&str]) -> ExportResult<SourceView> {
        let mut scopes: Vec<DomainRecord> = Vec::new();
        for (index, domain) in domains.iter().enumerate() {
            scopes.push(DomainRecord::new(format!("scope-{index}"), *domain, "{}")?);
        }
        scopes.sort_by(|left, right| left.id().cmp(right.id()));
        Ok(SourceView {
            store: StoreIdentity {
                format_uuid: "f".to_owned(),
                schema_version: 1,
                schema_semver: "1.0.0".to_owned(),
            },
            watermark: Watermark {
                next_accept_seq: 1,
                profile_revision: 1,
                accept_seq_head: 0,
                outbox_head: 0,
            },
            device_heads: Vec::<DeviceHead>::new(),
            canonical_semantic_digest: "00".to_owned(),
            batches: Vec::new(),
            events: Vec::new(),
            scopes,
            artifacts: Vec::<ArtifactSource>::new(),
            evidence: Vec::new(),
            claims: Vec::<ClaimSource>::new(),
            relations: Vec::new(),
            decisions: Vec::new(),
            git_refs: Vec::new(),
        })
    }

    /// The domain list is sorted and holds each domain once.
    ///
    /// This is the guard `P1-I5` fails against. Nothing downstream can hold it:
    /// a bundle's bytes are the same whatever order the domains are visited in,
    /// so an acceptance test comparing two bundles proves nothing about it.
    #[test]
    fn domains_are_sorted_and_deduplicated() -> ExportResult<()> {
        let observed = view(&["zeta", "alpha", "zeta", "mu"])?.domains();
        assert_eq!(observed, vec!["alpha", "mu", "zeta"]);
        assert!(view(&[])?.domains().is_empty());
        Ok(())
    }

    /// A domain with no artifact at all reports the weakest label.
    #[test]
    fn a_domain_with_no_artifact_reports_the_weakest_label() -> ExportResult<()> {
        assert_eq!(
            view(&["alpha"])?.domain_label("alpha"),
            SensitivityLabel::Public
        );
        Ok(())
    }

    #[test]
    fn original_inclusion_has_two_values_and_no_default() {
        assert_eq!(OriginalInclusion::ALL.len(), 2);
        assert!(OriginalInclusion::Included.includes_originals());
        assert!(!OriginalInclusion::Withheld.includes_originals());
    }

    #[test]
    fn a_withheld_reason_is_an_arm_rather_than_a_boolean() {
        assert_eq!(
            WithheldReason::UserExcludedOriginals.as_str(),
            "USER_EXCLUDED_ORIGINALS"
        );
    }

    #[test]
    fn a_commit_that_is_not_an_object_name_is_refused() {
        assert!(reference().validate().is_ok());

        let mut uppercase = reference();
        uppercase.commit = Some("0123456789ABCDEF".to_owned());
        assert!(uppercase.validate().is_err());

        let mut branchy = reference();
        branchy.commit = Some("refs/heads/main".to_owned());
        assert!(branchy.validate().is_err());

        let mut empty = reference();
        empty.commit = Some(String::new());
        assert!(empty.validate().is_err());
    }

    #[test]
    fn submodules_must_be_sorted_and_carry_object_names() {
        let mut unsorted = reference();
        unsorted.submodules = vec![
            ("z".to_owned(), "abc123".to_owned()),
            ("a".to_owned(), "def456".to_owned()),
        ];
        assert!(unsorted.validate().is_err());

        let mut bad_commit = reference();
        bad_commit.submodules = vec![("a".to_owned(), "not-a-commit".to_owned())];
        assert!(bad_commit.validate().is_err());
    }
}
