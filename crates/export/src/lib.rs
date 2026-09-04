//! `P2-P1`: the section 37 graduation export, and the vendor-free restore that
//! reads it.
//!
//! # What `INV-C-015` actually asks for
//!
//! Section 37 ends with the sentence this crate exists for: *학교 계정이나 특정
//! AI vendor가 사라져도 Local Core와 export로 계속 사용할 수 있다.* The claim is
//! not that an export **exists**. It is that the export is still readable when
//! the product and the school account are both gone.
//!
//! So the dependency list is the first half of the evidence. This crate links
//! no store, no vault, no crypto, no keystore, no recovery, no retention, no
//! projection engine, no transport and no model. The writer's input is a
//! [`SourceView`] the caller already holds; the reader's input is a directory
//! of bytes and nothing else. [`read::read_bundle`] takes a path and no key, no
//! token, no host and no account, and there is no argument to pass one as.
//!
//! # The reader re-runs the audit rather than re-reading it
//!
//! A bundle that carried a graduation verdict as text would prove the verdict
//! was *once* computed. [`audit::rerun_audit`] re-performs `P2-U3`'s selection
//! from the recorded catalogue scope and the profile decoded out of the frozen
//! inputs, evaluates the engine, and byte-compares
//! [`academic_domain::engines::EngineOutcome::canonical_bytes`] with what the
//! bundle recorded. [`academic_audit::SelectedRuleSet`] has one producer, so
//! the selection is genuinely re-decided and not restored.
//!
//! The published rule set is supplied **by the caller**, never minted from the
//! bundle. `P2-U2` puts a rule behind a two-attestation review gate, and a
//! bundle that could publish rules would be a way around it. What the bundle
//! carries is the rule set's canonical text, whose SHA-256 *is* the
//! `rule_set_hash` section 37 says a historical audit is replayed against.
//!
//! # Projections are excluded, and there is no path that includes them
//!
//! [`PROJECTIONS_INCLUDED`] is `false`, the manifest records it, the reader
//! refuses a manifest that records anything else, and no function here takes a
//! projection generation. Section 6.2 and the Phase 1 portability contract both
//! already say a projection is a disposable generation rebuilt from the ledger.
//!
//! # Originals are a user choice with no default
//!
//! [`OriginalInclusion`] implements no `Default` and the request takes it by
//! value. When originals are withheld the manifest records each artifact's
//! identity and plaintext digest and **no path**, so nothing in a published
//! bundle points at a file the bundle does not carry. An artifact is addressed
//! by its own identifier everywhere; a vault locator is recorded as an
//! attribute and is never a key, a filename or a path segment, because two
//! artifacts with identical bytes share one.

pub mod audit;
pub mod bundle;
pub mod directory;
pub mod error;
pub mod graph;
pub mod label;
pub mod part;
pub mod read;
pub mod source;
pub mod write;

pub use audit::{AuditRecord, AuditRerun, CatalogScopeRecord, rerun_audit};
pub use bundle::{
    BundleManifest, BundleSemantic, BundleVolatile, FileRecord, ObjectRecord, PartRecord,
    PostureBlock,
};
pub use error::{ExportError, ExportResult};
pub use label::{
    CopyrightNotice, DomainTerms, SensitivityLabel, SharingRestriction, TermsRegister,
};
pub use part::BundlePart;
pub use read::{ClaimedBundle, read_bundle};
pub use source::{
    ArtifactSource, BatchSource, ClaimSource, DeviceHead, DomainRecord, GitRef, OriginalInclusion,
    SourceView, StoreIdentity, Watermark, WithheldReason,
};
pub use write::{BundleReceipt, BundleRequest, RecordedAudit, write_bundle};

/// The frozen format name of export schema v2.
pub const GRADUATION_EXPORT_FORMAT: &str = "academic-graduation-export-v2";

/// The frozen manifest version of export schema v2.
pub const GRADUATION_EXPORT_MANIFEST_VERSION: u32 = 2;

/// The generator identity every bundle records.
pub const GRADUATION_EXPORT_GENERATOR: &str = "academic-os.graduation-export.v2";

/// Plaintext marker file naming the format and manifest version.
///
/// It is readable with no software at all, which is the point: someone holding
/// only the directory can see what it claims to be before anything parses it.
pub const FORMAT_MARKER_FILE: &str = "GRADUATION_EXPORT_V2";

/// Exact bytes of the format marker.
pub const FORMAT_MARKER_BYTES: &str = "academic-graduation-export-v2\nmanifest_version 2\n";

/// Relative path of the provenance manifest.
pub const MANIFEST_FILE: &str = "manifest.json";

/// Relative path of the human-readable inventory.
pub const INVENTORY_FILE: &str = "inventory.md";

/// Relative path of the embedded machine-readable contract.
pub const MANIFEST_SCHEMA_FILE: &str = "schemas/graduation-export-v2.schema.json";

/// Directory every section 37 part hangs under.
pub const PARTS_DIRECTORY: &str = "parts";

/// Whether a bundle ever carries a projection generation.
///
/// A projection is a disposable generation rebuilt from the ledger, so it is
/// never export content. This is a constant rather than a request field
/// because there is no caller who may choose otherwise.
pub const PROJECTIONS_INCLUDED: bool = false;

/// Whether a bundle is encrypted.
///
/// Export schema v2 is the open interchange format. Confidentiality of the
/// bundle at rest belongs to `P2-K4`'s encrypted backup, which is a different
/// artefact with a different manifest; conflating the two would make the open
/// format unreadable exactly when it is needed.
pub const BUNDLE_ENCRYPTED: bool = false;

/// Domain separator of the manifest's semantic digest.
pub const SEMANTIC_DIGEST_DOMAIN: &str = "academic-os.graduation-export.manifest.v2";

/// Longest relative path a bundle may contain, in bytes.
///
/// The same bound the Phase 1 export contract fixes, for the same reason: a
/// directory that unpacks on one host and not another is not portable.
pub const MAX_BUNDLE_RELATIVE_PATH_BYTES: usize = 160;

/// The exact JSON Schema shipped inside every bundle.
pub const BUNDLE_MANIFEST_SCHEMA: &str =
    include_str!("../../../schemas/jsonschema/graduation-export-v2.schema.json");

/// The open formats a bundle carries, in the order the inventory prints them.
///
/// Section 32.10 names *machine-readable JSON/JSON-LD, Markdown/PDF, audio
/// 원본, Git refs와 provenance manifest*. Every item on that list is here as a
/// media type except one, and [`PDF_RENDERING_ABSENCE`] is that one, recorded
/// rather than approximated with an empty file.
pub const OPEN_FORMATS: &[&str] = &[
    "application/cbor",
    "application/json",
    "application/ld+json",
    "application/octet-stream",
    "text/markdown",
];

/// What section 32.10 names that this build cannot write, and why.
///
/// Nothing in this repository produces PDF bytes.
/// `academic_lecture_document::PdfArtifact` is a *record* of a rendering — the
/// document it was taken of, the digest of bytes some renderer produced, and
/// what the coverage measurement says about them — and it holds no page. So a
/// bundle carries `text/markdown` and states this absence. Shipping a file with
/// a `.pdf` extension that no PDF reader opens would be worse than not shipping
/// one, and `no_bundle_file_claims_a_format_this_build_cannot_write` fails if
/// one appears.
pub const PDF_RENDERING_ABSENCE: &str = "Section 32.10 lists Markdown/PDF. This build renders Markdown and no PDF: no component in this repository produces PDF bytes, and `academic_lecture_document::PdfArtifact` records a rendering's digest and completeness rather than holding a page. The Markdown in this bundle is the human-readable rendering; a PDF, when one exists, is a rendering of the same records and is never the record.";
