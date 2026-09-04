//! Reading one graduation bundle with nothing but the directory.
//!
//! # What this function does not take
//!
//! [`read_bundle`] takes a path. Not a key, not a passphrase, not a device
//! authorization, not a token, not a host, not an account, not a session, not a
//! provider. There is no argument to pass one as, and this crate links no
//! transport and no key hierarchy, so the "no vendor and no school account"
//! half of `INV-C-015` is a property of the signature rather than of a
//! condition somebody could relax.
//!
//! # What it refuses
//!
//! The order is fixed and every step fails closed:
//!
//!  1. the plaintext format marker must be this format at this manifest
//!     version, before anything is parsed;
//!  2. the manifest must parse, recompute its own semantic digest, and carry
//!     this format's frozen fields and a posture that does not admit real data;
//!  3. the set of files on disk must **equal** the recorded inventory plus the
//!     manifest — in both directions, so an unlisted file is a refusal rather
//!     than a file nobody checked;
//!  4. every recorded file must hash and measure exactly as recorded;
//!  5. every path referenced anywhere in the manifest — an object's, the
//!     audit's, a part's — must appear in that inventory. This is the
//!     dangling-locator rule, and it is checked over the manifest's own
//!     references rather than over a list of the places references are known to
//!     appear;
//!  6. the six parts must be exactly section 37's six, each with its own
//!     sentence;
//!  7. every file record's sharing restriction must still follow from its
//!     sensitivity label, and every one must carry a notice;
//!  8. an object record carries a path or a withheld reason and never both or
//!     neither, and `originals_included` must agree with every one of them.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use academic_domain::ContentDigest;

use crate::{
    ExportError, ExportResult, FORMAT_MARKER_BYTES, FORMAT_MARKER_FILE, MANIFEST_FILE,
    bundle::{BundleManifest, FileRecord, encode_hex},
    directory,
    part::BundlePart,
};

/// One bundle that has been read and completely verified.
///
/// Holding a value of this type is the statement that every check in
/// [`read_bundle`] passed. There is no constructor that skips one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimedBundle {
    root: PathBuf,
    manifest: BundleManifest,
}

impl ClaimedBundle {
    /// The directory this bundle was read from.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The verified manifest.
    #[must_use]
    pub const fn manifest(&self) -> &BundleManifest {
        &self.manifest
    }

    /// The record of one listed file.
    pub fn file(&self, relative: &str) -> ExportResult<&FileRecord> {
        self.manifest
            .semantic
            .files
            .iter()
            .find(|file| file.path() == relative)
            .ok_or_else(|| ExportError::DanglingLocator {
                referenced_by: "a bundle reader",
                path: relative.to_owned(),
            })
    }

    /// Reads one listed file's exact bytes.
    pub fn read_bytes(&self, relative: &str) -> ExportResult<Vec<u8>> {
        let record = self.file(relative)?;
        let path = directory::resolve_relative(&self.root, relative)?;
        let bytes = std::fs::read(&path)
            .map_err(|source| ExportError::io("read bundle file", &path, source))?;
        let observed = encode_hex(ContentDigest::sha256(&bytes).as_bytes().as_slice());
        if observed != record.sha256() || bytes.len() as u64 != record.byte_length() {
            return Err(ExportError::mismatch(
                "bundle file digest",
                record.sha256(),
                observed,
            ));
        }
        Ok(bytes)
    }

    /// Reads one listed file as UTF-8 text.
    pub fn read_text(&self, relative: &str) -> ExportResult<String> {
        let bytes = self.read_bytes(relative)?;
        String::from_utf8(bytes).map_err(|_| ExportError::Malformed {
            item: "bundle text file",
            value: relative.to_owned(),
        })
    }

    /// Every part record, in the order the manifest lists them.
    #[must_use]
    pub fn parts(&self) -> &[crate::bundle::PartRecord] {
        &self.manifest.semantic.parts
    }

    /// Whether the original bytes travelled with this bundle.
    #[must_use]
    pub const fn originals_included(&self) -> bool {
        self.manifest.semantic.originals_included
    }
}

/// Reads and completely verifies one published bundle directory.
pub fn read_bundle(root: &Path) -> ExportResult<ClaimedBundle> {
    let marker_path = directory::resolve_relative(root, FORMAT_MARKER_FILE)?;
    let marker = std::fs::read(&marker_path)
        .map_err(|source| ExportError::io("read bundle format marker", &marker_path, source))?;
    if marker != FORMAT_MARKER_BYTES.as_bytes() {
        return Err(ExportError::mismatch(
            "bundle format marker",
            FORMAT_MARKER_BYTES.replace('\n', "\\n"),
            String::from_utf8_lossy(&marker).replace('\n', "\\n"),
        ));
    }

    let manifest_path = directory::resolve_relative(root, MANIFEST_FILE)?;
    let manifest_bytes = std::fs::read(&manifest_path)
        .map_err(|source| ExportError::io("read bundle manifest", &manifest_path, source))?;
    let manifest = BundleManifest::from_json_bytes(&manifest_bytes)?;
    manifest.require_v2_contract()?;
    manifest.verify_semantic_digest()?;

    let mut expected: Vec<&str> = manifest
        .semantic
        .files
        .iter()
        .map(FileRecord::path)
        .collect();
    expected.push(MANIFEST_FILE);
    expected.sort_unstable();
    let deduplicated: BTreeSet<&&str> = expected.iter().collect();
    if deduplicated.len() != expected.len() {
        return Err(ExportError::Malformed {
            item: "bundle file inventory",
            value: "a path is listed twice".to_owned(),
        });
    }
    let observed = directory::list_files(root)?;
    if observed != expected {
        for path in &observed {
            if !expected.contains(&path.as_str()) {
                return Err(ExportError::UnlistedFile(path.clone()));
            }
        }
        for path in &expected {
            if !observed.iter().any(|listed| listed == path) {
                return Err(ExportError::DanglingLocator {
                    referenced_by: "the bundle file inventory",
                    path: (*path).to_owned(),
                });
            }
        }
        return Err(ExportError::mismatch(
            "bundle directory inventory",
            expected.join(", "),
            observed.join(", "),
        ));
    }

    for record in &manifest.semantic.files {
        directory::check_relative_path(record.path())?;
        let path = directory::resolve_relative(root, record.path())?;
        let (digest, byte_length) = directory::hash_file(&path)?;
        let observed = encode_hex(digest.as_bytes().as_slice());
        if observed != record.sha256() || byte_length != record.byte_length() {
            return Err(ExportError::mismatch(
                "bundle file digest",
                record.sha256(),
                observed,
            ));
        }
        if !record.restriction_follows_label() {
            return Err(ExportError::mismatch(
                "bundle file sharing restriction",
                crate::label::SharingRestriction::of(record.sensitivity()).as_str(),
                record.sharing_restriction().as_str(),
            ));
        }
        if record.copyright_notice().as_str().trim().is_empty() {
            return Err(ExportError::NoticeAbsent {
                domain_id: record.path().to_owned(),
            });
        }
    }
    if !manifest
        .semantic
        .manifest_attributes
        .restriction_follows_label()
    {
        return Err(ExportError::mismatch(
            "bundle manifest sharing restriction",
            crate::label::SharingRestriction::of(manifest.semantic.manifest_attributes.sensitivity)
                .as_str(),
            manifest
                .semantic
                .manifest_attributes
                .sharing_restriction
                .as_str(),
        ));
    }

    let listed: BTreeSet<&str> = manifest
        .semantic
        .files
        .iter()
        .map(FileRecord::path)
        .collect();
    require_listed(
        "the recorded graduation audit",
        manifest.semantic.audit.referenced_paths().into_iter(),
        &listed,
    )?;
    for part in &manifest.semantic.parts {
        require_listed(
            "a section 37 part record",
            part.files.iter().map(String::as_str),
            &listed,
        )?;
    }
    for object in &manifest.semantic.objects {
        object.validate()?;
        if object.path.is_some() != manifest.semantic.originals_included {
            return Err(ExportError::mismatch(
                "bundle originals_included",
                manifest.semantic.originals_included,
                object.path.is_some(),
            ));
        }
        if let Some(path) = &object.path {
            require_listed(
                "an original artifact record",
                std::iter::once(path.as_str()),
                &listed,
            )?;
        }
    }

    let recorded: Vec<&str> = manifest
        .semantic
        .parts
        .iter()
        .map(|part| part.part.as_str())
        .collect();
    let named: Vec<&str> = BundlePart::ALL.iter().map(|part| part.as_str()).collect();
    if recorded != named {
        return Err(ExportError::mismatch(
            "bundle section 37 parts",
            named.join(", "),
            recorded.join(", "),
        ));
    }
    for (part, record) in BundlePart::ALL.into_iter().zip(&manifest.semantic.parts) {
        if record.directory != part.directory() {
            return Err(ExportError::mismatch(
                "section 37 part directory",
                part.directory(),
                &record.directory,
            ));
        }
        if record.specification_sentence != part.specification_sentence() {
            return Err(ExportError::mismatch(
                "section 37 part sentence",
                part.specification_sentence(),
                &record.specification_sentence,
            ));
        }
    }

    Ok(ClaimedBundle {
        root: root.to_path_buf(),
        manifest,
    })
}

fn require_listed<'a>(
    referenced_by: &'static str,
    paths: impl Iterator<Item = &'a str>,
    listed: &BTreeSet<&str>,
) -> ExportResult<()> {
    for path in paths {
        if !listed.contains(path) {
            return Err(ExportError::DanglingLocator {
                referenced_by,
                path: path.to_owned(),
            });
        }
    }
    Ok(())
}
