//! What a behavioural test cannot observe about this crate: the whole set of
//! things it declares.
//!
//! `P2-A3` measured that a `trait impl` appended to a product file is invisible
//! to a suite built on public signatures, and that six of the eight `P2-U`
//! crates had no inventory that would see one. `academic-review` and
//! `academic-ingestion` were the two that did. This is the same defence, in the
//! same shape, for this crate.
//!
//! `docs/contracts/policy-source-scans.md` enumerates every file in this
//! repository that reads another file's Rust source text, and
//! `tools/policy-source-scan-inventory.test.mjs` executes that sentence, so
//! this file has a row on that page.

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The floor the inventory walk must reach, so an empty walk fails as a walk.
const INVENTORY_FILE_FLOOR: usize = 10;

/// Every function this package declares, as `<file> [vis] <signature>`.
const DECLARATIONS: &[&str] = &[
    "src/admission.rs [pub] fn for_fault_injection_only() -> Self",
    "src/admission.rs [pub] fn open(profile_root: &Path) -> Result<Self, TranscriptError>",
    "src/admission.rs [pub] fn platforms(&self) -> &[String]",
    "src/admission.rs [pub] fn receipt_digest(&self) -> &str",
    "src/claims.rs [priv] fn predicate() -> Result<PredicateId, TranscriptError>",
    "src/claims.rs [priv] fn row_object_text(row: &TranscriptRow) -> String",
    "src/claims.rs [pub] fn actor(&self) -> &Actor",
    "src/claims.rs [pub] fn actor(&self) -> &Actor",
    "src/claims.rs [pub] fn claim(&self) -> &Claim",
    "src/claims.rs [pub] fn claim(&self) -> &Claim",
    "src/claims.rs [pub] fn confirm_reconciled_rows( reconciled: &ReconciledTranscript, format: TranscriptFormat, model_read: Option<ModelRead>, user_id: EntityId, ids: &[RowClaimIds], context: &RowClaimContext, ) -> Result<Vec<LinkedRowClaims>, TranscriptError>",
    "src/claims.rs [pub] fn format(&self) -> TranscriptFormat",
    "src/claims.rs [pub] fn import_claim_id(&self) -> ClaimId",
    "src/claims.rs [pub] fn import_row_claim( row: &TranscriptRow, format: TranscriptFormat, model_read: Option<ModelRead>, ids: RowClaimIds, context: &RowClaimContext, ) -> Result<ImportRowClaim, TranscriptError>",
    "src/claims.rs [pub] fn ordinal(&self) -> u32",
    "src/claims.rs [pub] fn ordinal(&self) -> u32",
    "src/fault.rs [priv] fn as_str(self) -> &'static str",
    "src/fault.rs [priv] fn trip(_point: FaultPoint)",
    "src/fault.rs [priv] fn trip(point: FaultPoint)",
    "src/lib.rs [priv] fn io(path: &Path, source: std::io::Error) -> Self",
    "src/lib.rs [pub] fn code(&self) -> &'static str",
    "src/reconcile.rs [pub] fn as_str(&self) -> &'static str",
    "src/reconcile.rs [pub] fn cause(&self) -> &HaltCause",
    "src/reconcile.rs [pub] fn disagreeing_fields(&self) -> &[TranscriptField]",
    "src/reconcile.rs [pub] fn field_digest(&self, ordinal: u32, field: TranscriptField) -> Option<&[u8; 32]>",
    "src/reconcile.rs [pub] fn field_digest(ordinal: u32, field: TranscriptField, value: &str) -> [u8; 32]",
    "src/reconcile.rs [pub] fn halt(&self) -> Option<&ReconciliationHalt>",
    "src/reconcile.rs [pub] fn identity_digest(&self) -> &[u8; 32]",
    "src/reconcile.rs [pub] fn of(transcript: &NormalizedTranscript) -> Self",
    "src/reconcile.rs [pub] fn ordinal(&self) -> u32",
    "src/reconcile.rs [pub] fn reconcile( candidate: &NormalizedTranscript, reference: &TranscriptChecksums, ) -> ReconciliationOutcome",
    "src/reconcile.rs [pub] fn reconciled(&self) -> Option<&ReconciledTranscript>",
    "src/reconcile.rs [pub] fn reference_identity_digest(&self) -> &[u8; 32]",
    "src/reconcile.rs [pub] fn row_count(&self) -> u32",
    "src/reconcile.rs [pub] fn rows_reconciled_before_halt(&self) -> u32",
    "src/reconcile.rs [pub] fn transcript(&self) -> &NormalizedTranscript",
    "src/record.rs [priv] fn check_field(name: &'static str, value: &str) -> Result<(), TranscriptError>",
    "src/record.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "src/record.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "src/record.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "src/record.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "src/record.rs [priv] fn push_field(out: &mut Vec<u8>, bytes: &[u8])",
    "src/record.rs [priv] fn push_u32(out: &mut Vec<u8>, value: u32)",
    "src/record.rs [pub] fn as_str(self) -> &'static str",
    "src/record.rs [pub] fn as_str(self) -> &'static str",
    "src/record.rs [pub] fn canonical_bytes(&self) -> Vec<u8>",
    "src/record.rs [pub] fn canonical_decimal(value: Decimal) -> String",
    "src/record.rs [pub] fn canonical_digest(&self) -> [u8; 32]",
    "src/record.rs [pub] fn course_code(&self) -> &str",
    "src/record.rs [pub] fn credits(&self) -> Decimal",
    "src/record.rs [pub] fn field(&self, field: IdentityField) -> &str",
    "src/record.rs [pub] fn field(&self, field: TranscriptField) -> String",
    "src/record.rs [pub] fn grade(&self) -> &str",
    "src/record.rs [pub] fn identity(&self) -> &TranscriptIdentity",
    "src/record.rs [pub] fn institution(&self) -> &str",
    "src/record.rs [pub] fn issued_on(&self) -> &str",
    "src/record.rs [pub] fn new( identity: TranscriptIdentity, rows: Vec<TranscriptRow>, ) -> Result<Self, TranscriptError>",
    "src/record.rs [pub] fn new( ordinal: u32, course_code: impl Into<String>, term: impl Into<String>, credits: Decimal, grade: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs [pub] fn new( student_number: impl Into<String>, student_name: impl Into<String>, institution: impl Into<String>, issued_on: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs [pub] fn ordinal(&self) -> u32",
    "src/record.rs [pub] fn parse_decimal(value: &str) -> Result<Decimal, TranscriptError>",
    "src/record.rs [pub] fn rows(&self) -> &[TranscriptRow]",
    "src/record.rs [pub] fn student_name(&self) -> &str",
    "src/record.rs [pub] fn student_number(&self) -> &str",
    "src/record.rs [pub] fn term(&self) -> &str",
    "src/redaction.rs [priv] fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result",
    "src/redaction.rs [priv] fn hex(bytes: &[u8; 32]) -> String",
    "src/redaction.rs [pub] fn all() -> [Self; 4]",
    "src/redaction.rs [pub] fn institution(&self) -> &str",
    "src/redaction.rs [pub] fn issued_on(&self) -> &str",
    "src/redaction.rs [pub] fn profile(&self) -> RedactionProfile",
    "src/redaction.rs [pub] fn project(transcript: &NormalizedTranscript, profile: RedactionProfile) -> RedactedProjection",
    "src/redaction.rs [pub] fn redacted_export(projection: &RedactedProjection) -> Vec<u8>",
    "src/redaction.rs [pub] fn removed_fields(self) -> Vec<IdentityField>",
    "src/redaction.rs [pub] fn removes(self, field: IdentityField) -> bool",
    "src/redaction.rs [pub] fn removing(fields: &[IdentityField]) -> Self",
    "src/redaction.rs [pub] fn retain_all() -> Self",
    "src/redaction.rs [pub] fn retained_values(&self) -> Vec<&str>",
    "src/redaction.rs [pub] fn rows(&self) -> &[[String; 4]]",
    "src/redaction.rs [pub] fn source_digest(&self) -> &[u8; 32]",
    "src/redaction.rs [pub] fn student_name(&self) -> Option<&str>",
    "src/redaction.rs [pub] fn student_number(&self) -> Option<&str>",
    "src/session.rs [priv] fn sync_directory(directory: &Path) -> Result<(), TranscriptError>",
    "src/session.rs [priv] fn write_durable(path: &Path, bytes: &[u8]) -> Result<(), TranscriptError>",
    "src/session.rs [pub] fn begin( _admitted: &AdmittedImport, profile_root: &Path, version_id: TranscriptVersionId, ) -> Result<Self, TranscriptError>",
    "src/session.rs [pub] fn directory(&self) -> &Path",
    "src/session.rs [pub] fn encode_confirmed_set( version_id: TranscriptVersionId, reconciled: &ReconciledTranscript, ) -> Vec<u8>",
    "src/session.rs [pub] fn inspect( profile_root: &Path, version_id: TranscriptVersionId, ) -> Result<SessionState, TranscriptError>",
    "src/session.rs [pub] fn is_published(self) -> bool",
    "src/session.rs [pub] fn lease_held(self) -> bool",
    "src/session.rs [pub] fn publish(self) -> Result<PathBuf, TranscriptError>",
    "src/session.rs [pub] fn release(&self) -> Result<(), TranscriptError>",
    "src/session.rs [pub] fn resume( _admitted: &AdmittedImport, profile_root: &Path, version_id: TranscriptVersionId, ) -> Result<Self, TranscriptError>",
    "src/session.rs [pub] fn session_directory(profile_root: &Path, version_id: TranscriptVersionId) -> PathBuf",
    "src/session.rs [pub] fn stage(&self, reconciled: &ReconciledTranscript) -> Result<(), TranscriptError>",
    "src/session.rs [pub] fn version_id(&self) -> TranscriptVersionId",
    "src/source.rs [priv] fn escape_pdf(value: &str) -> String",
    "src/source.rs [priv] fn extract_pdf_text(bytes: &[u8]) -> Result<String, TranscriptError>",
    "src/source.rs [priv] fn finish( student_number: Option<String>, student_name: Option<String>, institution: Option<String>, issued_on: Option<String>, rows: Vec<TranscriptRow>, ) -> Result<NormalizedTranscript, TranscriptError>",
    "src/source.rs [priv] fn parse_labelled_lines(text: &str) -> Result<NormalizedTranscript, TranscriptError>",
    "src/source.rs [priv] fn push_object(out: &mut Vec<u8>, number: u32, body: &[u8])",
    "src/source.rs [priv] fn push_row( position: usize, course_code: &str, term: &str, credits: &str, grade: &str, ) -> Result<TranscriptRow, TranscriptError>",
    "src/source.rs [priv] fn push_stream_object(out: &mut Vec<u8>, number: u32, dictionary: &[u8], payload: &[u8])",
    "src/source.rs [priv] fn set_once( slot: &mut Option<String>, value: &str, field: &'static str, ) -> Result<(), TranscriptError>",
    "src/source.rs [priv] fn text_line(key: &str, values: &[&str]) -> String",
    "src/source.rs [pub] fn as_str(self) -> &'static str",
    "src/source.rs [pub] fn build_synthetic_transcript_pdf(transcript: &NormalizedTranscript) -> SyntheticTranscriptPdf",
    "src/source.rs [pub] fn is_model_read(self) -> bool",
    "src/source.rs [pub] fn parse_csv(bytes: &[u8]) -> Result<NormalizedTranscript, TranscriptError>",
    "src/source.rs [pub] fn parse_manual_entry( student_number: &str, student_name: &str, institution: &str, issued_on: &str, entries: &[ManualRowEntry], ) -> Result<NormalizedTranscript, TranscriptError>",
    "src/source.rs [pub] fn parse_pdf_text_layer(bytes: &[u8]) -> Result<NormalizedTranscript, TranscriptError>",
    "src/source.rs [pub] fn render_csv(transcript: &NormalizedTranscript) -> String",
    "src/source.rs [pub] fn render_manual_entries(transcript: &NormalizedTranscript) -> Vec<ManualRowEntry>",
    "src/vault.rs [pub] fn store_transcript_original( _admitted: &AdmittedImport, vault: &EncryptedVault, request: &ArtifactIngestRequest, original: &[u8], ) -> Result<SealedEncryptedObject, TranscriptError>",
    "src/vault.rs [pub] fn transcript_ingest_request( artifact_id: ArtifactId, media_type: MediaType, domain_id: DomainId, permission_lineage_id: PermissionLineageId, ) -> ArtifactIngestRequest",
];

/// Every `impl` block header this package ships, as `<file>: <header>`.
const IMPL_HEADERS: &[&str] = &[
    "src/admission.rs: impl AdmittedImport",
    "src/claims.rs: impl ConfirmedRowClaim",
    "src/claims.rs: impl ImportRowClaim",
    "src/fault.rs: impl FaultPoint",
    "src/lib.rs: impl TranscriptError",
    "src/reconcile.rs: impl HaltCause",
    "src/reconcile.rs: impl ReconciledTranscript",
    "src/reconcile.rs: impl ReconciliationHalt",
    "src/reconcile.rs: impl ReconciliationOutcome",
    "src/reconcile.rs: impl TranscriptChecksums",
    "src/record.rs: impl IdentityField",
    "src/record.rs: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs: impl Into<String>, credits: Decimal, grade: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs: impl Into<String>, institution: impl Into<String>, issued_on: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs: impl Into<String>, issued_on: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs: impl Into<String>, student_name: impl Into<String>, institution: impl Into<String>, issued_on: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs: impl Into<String>, term: impl Into<String>, credits: Decimal, grade: impl Into<String>, ) -> Result<Self, TranscriptError>",
    "src/record.rs: impl NormalizedTranscript",
    "src/record.rs: impl TranscriptField",
    "src/record.rs: impl TranscriptIdentity",
    "src/record.rs: impl TranscriptRow",
    "src/record.rs: impl fmt::Debug for NormalizedTranscript",
    "src/record.rs: impl fmt::Debug for TranscriptIdentity",
    "src/record.rs: impl fmt::Display for IdentityField",
    "src/record.rs: impl fmt::Display for TranscriptField",
    "src/redaction.rs: impl RedactedProjection",
    "src/redaction.rs: impl RedactionProfile",
    "src/redaction.rs: impl fmt::Debug for RedactedProjection",
    "src/session.rs: impl ImportSession",
    "src/session.rs: impl SessionState",
    "src/source.rs: impl TranscriptFormat",
];

// ---------------------------------------------------------------------------
// every_declaration_and_impl_in_this_crate_is_pinned
// ---------------------------------------------------------------------------
//
// `P2-A3` measured this crate's blind spot directly: four `impl From<..>` blocks
// appended to a product file gave an external crate a route to a value the
// crate's own doc says has one construction site, and every acceptance test in
// this crate stayed green. A `trait impl` declares no `pub fn`, so a scan built
// on public signatures does not see it, and no scan here counted `impl` blocks
// at all.
//
// `P2-X5` measured the same class as six invisible injections out of nineteen,
// and `P2-Y3` closed it in `crates/cs-map` by pinning the whole set of `impl`
// headers. `academic-review` and `academic-ingestion` were the only two U crates
// carrying that defence. This is it, ported: two whole sets, compared in both
// directions, over every `.rs` file this package ships.
//
// It is deliberately not a list of forbidden spellings. A new function, a new
// method, a new inherent `impl`, a new trait `impl` and a new file all fail as
// an entry nobody wrote down, whatever they are called.

/// Every `.rs` file this package ships: everything outside `tests`.
///
/// The whole package rather than `src`, because `S-12` in
/// `docs/contracts/policy-source-scans.md` is the row about a walk that reads
/// `<crate>/src` and stops seeing product-shaped code beside it --
/// `examples/`, `benches/` and `probes/` are all compiled by
/// `cargo clippy --workspace --all-targets`.
fn inventory_sources() -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let base = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    let mut pending = vec![base.clone()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|name| name == "tests" || name == "target")
                {
                    continue;
                }
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let name = path
                    .strip_prefix(&base)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((name, std::fs::read_to_string(&path)?));
            }
        }
    }
    found.sort();
    Ok(found)
}

/// Removes comments, string literals and character literals.
///
/// The raw-string-aware reader from `crates/record/tests/record_scans.rs`,
/// copied deliberately: `P2-G4` found that a lexer without raw strings
/// desynchronizes and reads every literal after one as code.
fn inventory_strip(source: &str) -> String {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();

        if current == '/' && next == Some('/') {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            out.push('\n');
            continue;
        }
        if current == '/' && next == Some('*') {
            let mut depth = 1_usize;
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index] == '/' && bytes.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if bytes[index] == '*' && bytes.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            out.push(' ');
            continue;
        }
        if current == 'r' && matches!(next, Some('"') | Some('#')) {
            let mut probe = index + 1;
            let mut hashes = 0_usize;
            while bytes.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if bytes.get(probe) == Some(&'"') {
                let terminator: String = core::iter::once('"')
                    .chain(core::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = bytes[probe + 1..].iter().collect();
                let end = rest.find(&terminator).map_or(bytes.len(), |offset| {
                    probe + 1 + rest[..offset].chars().count() + terminator.chars().count()
                });
                index = end;
                out.push(' ');
                continue;
            }
        }
        if current == '"' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == '\\' {
                    index += 2;
                    continue;
                }
                if bytes[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            out.push(' ');
            continue;
        }
        if current == '\'' {
            let closes = if next == Some('\\') {
                bytes
                    .iter()
                    .skip(index + 2)
                    .position(|character| *character == '\'')
                    .map(|offset| index + 2 + offset)
            } else {
                (bytes.get(index + 2) == Some(&'\'')).then_some(index + 2)
            };
            if let Some(end) = closes {
                index = end + 1;
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

/// Collapses whitespace runs to single spaces.
fn inventory_collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Every function declaration in `code`, as a public flag and a signature.
///
/// Visibility is read off the text before `fn` on the same line: `pub(` is
/// crate-private however it continues, a bare `pub` is public, anything else is
/// private. Reading **signatures** rather than names is what makes the pin a
/// statement about what a function takes and returns, so a widened parameter
/// fails as loudly as a new function.
///
/// The `>` of a `->` is skipped: `crates/review`'s copy of this reader records
/// that treating it as a closing bracket truncated `fn counts(self) -> [u32; 5]`
/// to `fn counts(self) -> [u32`, and a pin on a truncated signature is a pin two
/// different signatures satisfy.
fn inventory_declarations(code: &str) -> Vec<(bool, String)> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("fn ") {
        if !(at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_')) {
            continue;
        }
        let line_start = code[..at].rfind('\n').map_or(0, |index| index + 1);
        let prefix = &code[line_start..at];
        let public = prefix.contains("pub") && !prefix.contains("pub(");
        let mut depth = 0_i32;
        let mut end = None;
        let region = &code[at..];
        let region_bytes = region.as_bytes();
        for (offset, character) in region.char_indices() {
            match character {
                '(' | '<' | '[' => depth += 1,
                '>' if offset > 0 && region_bytes[offset - 1] == b'-' => {}
                ')' | '>' | ']' => depth -= 1,
                '{' | ';' if depth <= 0 => {
                    end = Some(at + offset);
                    break;
                }
                _ => {}
            }
        }
        if let Some(end) = end {
            found.push((public, inventory_collapse(&code[at..end])));
        }
    }
    found
}

/// Every `impl` block header in `code`, whole.
///
/// The header is everything from `impl` to the opening brace, so
/// `impl From<usize> for CoverageWitness` and `impl CoverageWitness` are
/// different entries and a trait implementation cannot arrive as an edit to an
/// inherent one.
fn inventory_impl_headers(code: &str) -> Vec<String> {
    let bytes = code.as_bytes();
    let mut found = Vec::new();
    for (at, _) in code.match_indices("impl") {
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        if code[at + 4..]
            .starts_with(|character: char| character.is_alphanumeric() || character == '_')
        {
            continue;
        }
        let Some(end) = code[at..].find(['{', ';']) else {
            continue;
        };
        found.push(inventory_collapse(&code[at..at + end]));
    }
    found
}

/// Nothing this crate declares is outside the two pinned sets.
///
/// Two whole sets, each compared in both directions:
///
/// 1. every function declaration this package ships, as a file, a visibility
///    and a full signature;
/// 2. every `impl` block header this package ships, as a file and a header.
///
/// The second is the one `P2-A3` walked through. Its injection was four
/// `impl From<..>` blocks in a product file -- no `pub fn`, no new name on any
/// forbidden list, no change to any other file -- and it handed an external
/// crate a value the crate's own documentation says it cannot construct. There
/// is no spelling of that injection that this test does not see, because it does
/// not look for spellings: it compares the set.
#[test]
fn every_declaration_and_impl_in_this_crate_is_pinned() -> TestResult {
    let sources = inventory_sources()?;
    assert!(
        sources.len() >= INVENTORY_FILE_FLOOR,
        "the inventory walk read only {} files",
        sources.len()
    );

    let mut declared = Vec::new();
    let mut headers = Vec::new();
    for (name, text) in &sources {
        let code = inventory_strip(text);
        for (public, signature) in inventory_declarations(&code) {
            let visibility = if public { "pub" } else { "priv" };
            declared.push(format!("{name} [{visibility}] {signature}"));
        }
        for header in inventory_impl_headers(&code) {
            headers.push(format!("{name}: {header}"));
        }
    }
    declared.sort();
    headers.sort();

    assert_eq!(
        declared,
        DECLARATIONS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's declaration set changed"
    );
    assert_eq!(
        headers,
        IMPL_HEADERS
            .iter()
            .map(|entry| (*entry).to_owned())
            .collect::<Vec<_>>(),
        "this crate's impl inventory changed"
    );
    Ok(())
}
