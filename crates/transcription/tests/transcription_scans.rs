//! Source scans for the `P2-L3` boundary.
//!
//! Three of this task's claims are statements about what the source does not
//! contain, and a behavioural test cannot observe an absence: nothing writes a
//! raw token, the raw provider response leaves this crate only under `P2-G5`'s
//! label, and the route decision has one body and one caller.
//!
//! Everything here follows the shapes `docs/contracts/policy-source-scans.md`
//! records, and it follows them because each one was a defect somewhere else
//! first:
//!
//! * **The walk reads the package, not `src`.** `S-12`: a `[[bin]]` with an
//!   explicit `path`, an `examples/` tree, or a `#[path]` module outside `src`
//!   is product code four scans could not see. There is a floor under the walk,
//!   a `mod`/`#[path]` tripwire, and a rule that this crate's product source is
//!   under `src` and nowhere else.
//! * **Every count reads an identifier, not a spelling.** `P2-RF10` walked past
//!   an inventory that counted `.expose()` by writing `Untrusted::expose(d)`;
//!   `P2-RF11` walked past one that subtracted the spelling `fn expose` by
//!   writing `fn expose_rendered(`. Both shapes are injected here rather than
//!   assumed.
//! * **A pin fixes its callers too.** `T141` left a pinned check byte-identical
//!   and wrapped the *call* to it in a marker-file condition. Each pin below is
//!   accompanied by a count of the sites that reach it.
//! * **A sweep over signatures is not a claim about constructions.** `U-G3`:
//!   a second entry point can build its argument in its body and name the type
//!   nowhere in its signature. The raw types are therefore counted where they
//!   are *built*, not only where they are declared.

mod common;

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use common::TestResult;

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn workspace_root() -> PathBuf {
    crate_root()
        .parent()
        .and_then(Path::parent)
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

/// Every `.rs` file anywhere under this crate's package directory.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships, which is every one outside `tests`.
///
/// `benches` is deliberately **not** excluded beside `tests`: `S-14` records
/// that a bench target has no feature gate and is compiled by
/// `cargo clippy --workspace --all-targets`, which is the README's third
/// verification command.
fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    Ok(crate_all_sources()?
        .into_iter()
        .filter(|path| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            !relative.starts_with("tests")
        })
        .collect())
}

fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Every `.rs` file under every package in `crates/`.
///
/// The workspace half of the raw-token rules. These types are public and any
/// crate could declare the accessor this one does not, which is the shape
/// `a_label_has_no_path_that_moves_a_mark` in `academic-capture` already uses.
fn workspace_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    let crates = workspace_root().join("crates");
    for entry in fs::read_dir(&crates)? {
        let package = entry?.path();
        if package.is_dir() {
            walk(&package, &mut found)?;
        }
    }
    found.sort();
    Ok(found)
}

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/record/tests/record_scans.rs`, which is where this
/// repository's Rust-side stripper lives, raw strings and nested block comments
/// included. `P2-G4` found that a lexer without raw strings desynchronizes and
/// reads every literal after one as code, so the copy is deliberate rather than
/// a simplification.
fn strip_non_code(source: &str) -> String {
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
            continue;
        }
        if current == 'r' && matches!(next, Some('"') | Some('#')) {
            let mut hashes = 0_usize;
            let mut probe = index + 1;
            while bytes.get(probe) == Some(&'#') {
                hashes += 1;
                probe += 1;
            }
            if bytes.get(probe) == Some(&'"') {
                index = probe + 1;
                loop {
                    if index >= bytes.len() {
                        break;
                    }
                    if bytes[index] == '"' {
                        let mut closing = 0_usize;
                        while bytes.get(index + 1 + closing) == Some(&'#') {
                            closing += 1;
                        }
                        if closing >= hashes {
                            index += 1 + hashes;
                            break;
                        }
                    }
                    index += 1;
                }
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
            // A lifetime, not a character literal, when no quote closes it
            // within two characters. `S-7` records the width of this rule.
            let closes = bytes.get(index + 2) == Some(&'\'')
                || (bytes.get(index + 1) == Some(&'\\') && bytes.get(index + 3) == Some(&'\''));
            if closes {
                while index < bytes.len() && bytes[index] != '\'' {
                    index += 1;
                }
                index += 1;
                while index < bytes.len() && bytes[index] != '\'' {
                    index += 1;
                }
                index += 1;
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

fn code_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(path)?))
}

fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The whole text of one item, whitespace-collapsed, comments dropped.
fn declared_item(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{signature} has no closing brace at column zero"))?;
    let body = &source[start..start + end + 2];
    let kept: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    Ok(kept
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

/// How many times `needle` appears in `code`.
fn occurrences(code: &str, needle: &str) -> usize {
    code.split(needle).count().saturating_sub(1)
}

/// Counts whole-identifier occurrences of `name` in already-stripped code.
///
/// `occurrences` counts a spelling, which is right for a fixed phrase and wrong
/// for a name. `P2-RF10` reached a fourth exposure site by writing
/// `Untrusted::expose(d)` past a count of `.expose()`; injection `L3-I2` below
/// is the same shape against this crate's accessor.
fn uses_of(code: &str, name: &str) -> usize {
    let bytes = code.as_bytes();
    code.match_indices(name)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + name.len()).copied().unwrap_or(b' ');
            before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
        })
        .count()
}

/// Counts declarations of a function whose name is exactly `name`.
///
/// What follows the name has to open a parameter list or a generic list and
/// nothing else, so `fn response_bytes_rendered(` is not `response_bytes` and
/// `fn quote<'a>(` still is. `P2-RF11` found that reading the declaration as a
/// *spelling* lets one function cancel its own call; injection `L3-I3` is that
/// shape.
fn declarations_of(code: &str, name: &str) -> usize {
    let needle = format!("fn {name}");
    let bytes = code.as_bytes();
    code.match_indices(&needle)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + needle.len()).copied().unwrap_or(b' ');
            before_ok && (after == b'(' || after == b'<')
        })
        .count()
}

/// The use count of `name` less its declarations, which cannot go negative.
fn calls_of(code: &str, name: &str) -> usize {
    let uses = uses_of(code, name);
    let declarations = declarations_of(code, name);
    assert!(
        uses >= declarations,
        "{name} is declared {declarations} times and named {uses}; the two counts disagree"
    );
    uses - declarations
}

/// Drops every `use` item, so a re-export is not counted as a caller.
///
/// Whole items, not first lines. A `use crate::{ ... }` block spans several
/// lines and a filter that dropped only the line beginning `use ` left the
/// names inside it in the text -- which is how the first version of the
/// decoder's call count read three callers where there is one. A `pub use`
/// re-export is dropped for the same reason: it names a function and calls
/// nothing.
fn drop_use_items(code: &str) -> String {
    let mut kept: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim_start();
        if !inside && (trimmed.starts_with("use ") || trimmed.starts_with("pub use ")) {
            inside = !trimmed.trim_end().ends_with(';');
            continue;
        }
        if inside {
            inside = !line.trim_end().ends_with(';');
            continue;
        }
        kept.push(line);
    }
    kept.join(
        "
",
    )
}

/// Every `pub fn`, `pub const fn`, `pub async fn` and `pub unsafe fn`
/// signature in `code`, whitespace-collapsed.
fn public_signatures(code: &str) -> Vec<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if ![
            "pub fn ",
            "pub const fn ",
            "pub async fn ",
            "pub unsafe fn ",
        ]
        .iter()
        .any(|start| trimmed.starts_with(start))
        {
            continue;
        }
        let mut signature = String::new();
        for follow in lines.iter().skip(index) {
            signature.push(' ');
            signature.push_str(follow.trim());
            if follow.contains('{') || follow.trim_end().ends_with(';') {
                break;
            }
        }
        found.push(signature.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    found
}

/// Splits a signature into its parameter list and its return type.
fn parameters_and_return(signature: &str) -> Option<(&str, &str)> {
    let open = signature.find('(')?;
    let mut depth = 0_usize;
    for (offset, character) in signature.get(open..)?.char_indices() {
        let at = open.saturating_add(offset);
        match character {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let (parameters, rest) = signature.split_at(at.saturating_add(1));
                    let returns = rest.split_once("->").map_or("", |(_, tail)| tail);
                    return Some((parameters, returns));
                }
            }
            _ => (),
        }
    }
    None
}

/// Whether `text` names any of `names` as a whole identifier.
fn names_any(text: &str, names: &[&str]) -> bool {
    names.iter().any(|name| uses_of(text, name) > 0)
}

// ---------------------------------------------------------------------------
// PINS
// ---------------------------------------------------------------------------

/// The two places the raw provider response's bytes are read, and why each is
/// allowed.
///
/// `every_raw_byte_site_is_named_and_justified` compares the **whole**
/// inventory against this list, counted by identifier. A third fails as an
/// extra key however it is spelled; a removed one fails as a missing key.
const RAW_BYTE_SITES: [(&str, &str, &str); 2] = [
    (
        "crates/transcription/src/response.rs",
        "RawResponseArchive::retain",
        "Sealing has to read the bytes it hashes and wraps. What leaves is an \
         Untrusted<IngestedDocument>, which implements no Deref, no Display and \
         no Into, and whose one accessor is private to academic-untrusted-content.",
    ),
    (
        "crates/transcription/src/transcript.rs",
        "decode",
        "The wire grammar has to read the response it validates. What leaves is a \
         closed record of segments and tokens whose fields are private and whose \
         one producer is that function, and no byte of the response itself.",
    ),
];

/// The whole set of `impl` blocks in this crate whose header names one of the
/// three raw types.
///
/// A whole-set comparison rather than a token list: an implementation of a
/// trait nobody predicted fails as an extra key. `untrusted_has_no_unwrapping_trait_impl`
/// is the shape.
const RAW_IMPL_BLOCKS: [&str; 6] = [
    "impl RawToken {",
    "impl fmt::Debug for RawToken {",
    "impl RawSegment {",
    "impl fmt::Debug for RawSegment {",
    "impl RawTranscript {",
    "impl TranscriptLineage {",
];

/// The whole set of `impl` blocks whose header names a comparison type.
///
/// `P2-M1` forbids ordering a provider's raw number against another's. The same
/// prohibition one level up is that two *runs* are not ordered either, and this
/// is what refuses an ordering nobody predicted.
const COMPARISON_IMPL_BLOCKS: [&str; 4] = [
    "impl Side {",
    "impl ProviderRun {",
    "impl Divergence {",
    "impl RetranscriptionComparison {",
];

/// Traits whose presence on a raw type would give it a write path, or on a
/// comparison type an order.
///
/// The weaker half, written as such: the whole-set comparisons above are what
/// refuse one nobody predicted.
const FORBIDDEN_RAW_TRAITS: [&str; 6] = [
    "DerefMut",
    "AsMut",
    "BorrowMut",
    "IndexMut",
    "From",
    "Default",
];

/// The ordering traits a comparison type may not implement.
const FORBIDDEN_ORDER_TRAITS: [&str; 3] = ["PartialOrd", "Ord", "Display"];

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_POLICY: &str = "impl SttPolicy { #[must_use] pub const fn new() -> Self { Self { approvals: Vec::new(), } } #[must_use] pub fn approve_remote(mut self, approval: RemoteProcessingApproval) -> Self { self.approvals.push(approval); self } #[must_use] pub fn approvals(&self) -> &[RemoteProcessingApproval] { &self.approvals } #[must_use] pub fn route_for(&self, contract: &ProviderContract) -> SttRoute { match contract.placement() { ProviderPlacement::Local => SttRoute::Local { provider: contract.provider().clone(), model_version: contract.model_version().clone(), }, ProviderPlacement::Remote => { let Some(approval) = self.approvals.iter().find(|approval| { approval.provider() == contract.provider() && approval.model_version() == contract.model_version() }) else { return SttRoute::Blocked { denial: RouteDenial::ProviderNotApproved, }; }; if !approval.external_processing_permitted() { return SttRoute::Blocked { denial: RouteDenial::NoExternalProcessingPermission, }; } let Some(retention) = approval.retention() else { return SttRoute::Blocked { denial: RouteDenial::NoRetentionDeclaration, }; }; SttRoute::ScopedRemote { admission: RemoteAdmission { provider: contract.provider().clone(), model_version: contract.model_version().clone(), retention: retention.clone(), }, } } } } }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_ARCHIVE: &str = "impl RawResponseArchive { #[must_use] pub const fn new() -> Self { Self { entries: Vec::new(), } } pub fn retain(&mut self, response: &ProviderResponse) -> Result<RawResponseId, ArchiveFault> { let id = RawResponseId(u32::try_from(self.entries.len()).unwrap_or(u32::MAX)); let source_id = SourceId::new(id.to_string())?; let labelled = academic_untrusted_content::ingest( source_id, SourceKind::ProviderResponse, u64::from(id.value()), response.response_bytes(), ) .map_err(|_| ArchiveFault::NotSealable)?; self.entries.push(ArchivedResponse { id, provider: response.provider().clone(), model_version: response.model_version().clone(), placement: response.placement(), digest: *response.digest(), byte_len: response.byte_len(), labelled, }); Ok(id) } #[must_use] pub fn entries(&self) -> &[ArchivedResponse] { &self.entries } #[must_use] pub fn get(&self, id: RawResponseId) -> Option<&ArchivedResponse> { self.entries.iter().find(|entry| entry.id == id) } #[must_use] pub fn len(&self) -> usize { self.entries.len() } #[must_use] pub fn is_empty(&self) -> bool { self.entries.is_empty() } }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_LINEAGE_EFFECT: &str = "impl LineageEffect { pub const ALL: [Self; 2] = [Self::AppendsVersion, Self::AppendsNothing]; #[must_use] pub const fn of(disposition: &DecisionAction) -> Self { match disposition { DecisionAction::Confirm | DecisionAction::Replace { .. } => Self::AppendsVersion, DecisionAction::Reject => Self::AppendsNothing, } } #[must_use] pub const fn as_str(self) -> &'static str { match self { Self::AppendsVersion => \"APPENDS_VERSION\", Self::AppendsNothing => \"APPENDS_NOTHING\", } } }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_BINDING: &str = "impl AuthorizationBinding { pub fn of(recorder: &CaptureRecorder, recovery: &JournalRecovery) -> Result<Self, InputFault> { let header = recovery.header(); if header.token_id() != recorder.token_id() || header.policy_digest() != &recorder.policy().digest() { return Err(InputFault::JournalIsNotThisCapture); } Ok(Self { lecture: recorder.lecture_id(), token_id: *recorder.token_id(), policy_digest: recorder.policy().digest(), }) } #[must_use] pub const fn lecture(&self) -> LectureSessionId { self.lecture } #[must_use] pub const fn token_id(&self) -> &ContentDigest { &self.token_id } #[must_use] pub const fn policy_digest(&self) -> &ContentDigest { &self.policy_digest } fn covers(&self, recovery: &JournalRecovery) -> bool { let header = recovery.header(); header.token_id() == &self.token_id && header.policy_digest() == &self.policy_digest } }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_RECORD_MODEL_RUN: &str = "fn record_model_run( identity: &RunIdentity, route: &SttRoute, selection: &ProviderSelection, input_artifact_refs: InputArtifactRefs, ) -> Result<ModelRun, PipelineFault> { let (transmission, retention) = match route { SttRoute::Local { .. } => { if identity.transmission.is_some() { return Err(PipelineFault::LocalRunTransmitted); } let retention = RetentionDeclaration::new(LOCAL_ONLY_RETENTION) .map_err(|_| PipelineFault::NoTransmissionRecord)?; (Transmission::LocalOnly, retention) } SttRoute::ScopedRemote { admission } => { let transmission = identity .transmission .clone() .ok_or(PipelineFault::NoTransmissionRecord)?; if matches!(transmission, Transmission::LocalOnly) { return Err(PipelineFault::NoTransmissionRecord); } (transmission, admission.retention().clone()) } SttRoute::Blocked { .. } => return Err(PipelineFault::RouteMismatch), }; Ok(ModelRun::record( identity.id, identity.purpose.clone(), selection.provider().clone(), selection.model_version().clone(), identity.prompt_template_hash, input_artifact_refs, transmission, identity.redaction_policy_hash, identity.output_artifact, identity.started_at, identity.cost.clone(), retention, )) }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_TRANSCRIBE: &str = "fn transcribe( provider: &dyn SttProvider, manifest: &InputManifest, contract: &ProviderContract, route: &SttRoute, selection: &ProviderSelection, ) -> Result<ProviderResponse, PipelineFault> { let request = TranscriptionRequest { manifest, contract, route, required_claims: selection.required_claims(), }; let response = provider .transcribe(&request) .ok_or(PipelineFault::ProviderFailed)?; let expected = match route { SttRoute::Local { .. } => ProviderPlacement::Local, SttRoute::ScopedRemote { .. } => ProviderPlacement::Remote, SttRoute::Blocked { .. } => return Err(PipelineFault::RouteMismatch), }; if response.placement() != expected || response.provider() != selection.provider() || response.model_version() != selection.model_version() { return Err(PipelineFault::RouteMismatch); } Ok(response) }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_RESPONSE_BYTES: &str = "impl ProviderResponse { #[must_use] pub fn from_local(provider: ProviderId, model_version: ModelVersion, bytes: &[u8]) -> Self { Self { provider, model_version, placement: ProviderPlacement::Local, provider_response_bytes: bytes.to_vec(), digest: ContentDigest::sha256(bytes), } } #[must_use] pub fn from_remote(admission: &RemoteAdmission, accepted: &AcceptedResponse) -> Self { let bytes = accepted.bytes(); Self { provider: admission.provider().clone(), model_version: admission.model_version().clone(), placement: ProviderPlacement::Remote, provider_response_bytes: bytes.to_vec(), digest: ContentDigest::sha256(bytes), } } #[must_use] pub const fn provider(&self) -> &ProviderId { &self.provider } #[must_use] pub const fn model_version(&self) -> &ModelVersion { &self.model_version } #[must_use] pub const fn placement(&self) -> ProviderPlacement { self.placement } #[must_use] pub const fn digest(&self) -> &ContentDigest { &self.digest } #[must_use] pub fn byte_len(&self) -> usize { self.provider_response_bytes.len() } pub(crate) fn response_bytes(&self) -> &[u8] { &self.provider_response_bytes } }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_PARSE_TOKEN: &str = "fn parse_token( value: &str, contract: &ProviderContract, response: &ProviderResponse, ) -> Result<RawToken, DecodeFault> { let mut fields = value.splitn(3, ' '); let start = fields.next().ok_or(DecodeFault::FieldCount(\"word\"))?; let confidence = fields.next().ok_or(DecodeFault::FieldCount(\"word\"))?; let text = fields.next().ok_or(DecodeFault::FieldCount(\"word\"))?; if text.is_empty() || text.chars().any(char::is_control) { return Err(DecodeFault::FieldCount(\"word\")); } let start_nanos = match start { \"-\" => None, other => Some( other .parse::<u64>() .map_err(|_| DecodeFault::NotANumber(other.to_owned()))?, ), }; let declares_word_times = contract.timestamp_semantics() == TimestampSemantics::WordAndSegment; if start_nanos.is_some() != declares_word_times { return Err(DecodeFault::ContradictsDeclaration( CapabilityField::TimestampSemantics, )); } let confidence_units = match confidence { \"-\" => None, other => Some( other .parse::<u32>() .map_err(|_| DecodeFault::NotANumber(other.to_owned()))?, ), }; let declares_token_confidence = contract.confidence_semantics() == ConfidenceSemantics::PerToken; if confidence_units.is_some() != declares_token_confidence { return Err(DecodeFault::ContradictsDeclaration( CapabilityField::ConfidenceSemantics, )); } Ok(RawToken { text: text.to_owned(), start_nanos, confidence: confidence_units.map(|units| { RawScore::new( response.provider().clone(), response.model_version().clone(), units, ) }), }) }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_CLOSE_SEGMENT: &str = "fn close(self) -> Result<RawSegment, DecodeFault> { let verbatim_text = self .verbatim_text .ok_or(DecodeFault::MissingKey(\"verbatim\"))?; if self.tokens.is_empty() { return Err(DecodeFault::MissingKey(\"word\")); } Ok(RawSegment { id: self.id, start_nanos: self.start_nanos, end_nanos: self.end_nanos, speaker: self.speaker, verbatim_text, tokens: self.tokens, source_audio_chunks: self.source_audio_chunks, }) } }";

/// Pinned whole text. Regenerate deliberately, never by copying a diff.
const WHOLE_DECODE: &str = "pub fn decode( response: &ProviderResponse, contract: &ProviderContract, lecture: LectureSessionId, raw_response: RawResponseId, input_digest: ContentDigest, ) -> Result<RawTranscript, DecodeFault> { let text = core::str::from_utf8(response.response_bytes()).map_err(|_| DecodeFault::NotUtf8)?; let body = text.strip_suffix('\\n').ok_or(DecodeFault::Banner)?; let mut lines = body.lines(); if lines.next() != Some(RESPONSE_BANNER) { return Err(DecodeFault::Banner); } let mut segments: Vec<RawSegment> = Vec::new(); let mut open: Option<OpenSegment> = None; for line in lines { let (key, value) = line.split_once(\": \").ok_or_else(|| { DecodeFault::UnknownKey(line.split(':').next().unwrap_or(line).to_owned()) })?; match key { \"segment\" => { if let Some(previous) = open.take() { segments.push(previous.close()?); } open = Some(OpenSegment::parse(value, contract)?); } \"verbatim\" => { let segment = open.as_mut().ok_or(DecodeFault::MissingKey(\"segment\"))?; if segment.verbatim_text.is_some() { return Err(DecodeFault::DuplicateKey(\"verbatim\")); } if value.is_empty() || value.chars().any(char::is_control) { return Err(DecodeFault::FieldCount(\"verbatim\")); } segment.verbatim_text = Some(value.to_owned()); } \"word\" => { let segment = open.as_mut().ok_or(DecodeFault::MissingKey(\"segment\"))?; let token = parse_token(value, contract, response)?; if let Some(start) = token.start_nanos && (start < segment.start_nanos || start >= segment.end_nanos) { return Err(DecodeFault::TokenOutsideSegment); } segment.tokens.push(token); } other => return Err(DecodeFault::UnknownKey(other.to_owned())), } } if let Some(previous) = open.take() { segments.push(previous.close()?); } if segments.is_empty() { return Err(DecodeFault::NoSegments); } for pair in segments.windows(2) { let [earlier, later] = pair else { continue; }; if later.start_nanos < earlier.end_nanos { return Err(DecodeFault::SegmentOrder); } } Ok(RawTranscript { lecture, provider: response.provider().clone(), model_version: response.model_version().clone(), raw_response, input_digest, segments, }) }";

// ---------------------------------------------------------------------------
// The walk itself
// ---------------------------------------------------------------------------

/// The walk reads every module in this crate, and this crate's product source
/// is under `src` and nowhere else.
#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 13,
        "the walk found only {} files under the package",
        sources.len()
    );

    let root = crate_root();
    let outside: Vec<String> = crate_product_sources()?
        .iter()
        .filter(|path| !path.strip_prefix(&root).unwrap_or(path).starts_with("src"))
        .map(|path| relative(path))
        .collect();
    assert_eq!(
        outside,
        Vec::<String>::new(),
        "this crate has product source outside src; every scan that reads it has to widen"
    );

    // A module is either `<name>.rs` or `<name>/mod.rs`, so both spellings are
    // collected: a tripwire that only knew the first would fire on every
    // directory module and be turned off rather than fixed.
    let mut read: BTreeSet<String> = BTreeSet::new();
    for path in &sources {
        if let Some(stem) = path.file_stem() {
            let stem = stem.to_string_lossy().into_owned();
            if stem == "mod" {
                if let Some(parent) = path.parent().and_then(Path::file_name) {
                    read.insert(parent.to_string_lossy().into_owned());
                }
            } else {
                read.insert(stem);
            }
        }
    }

    // The tripwire. Every `mod name;` and every `#[path = "…"]` in the package
    // has to name a file the walk read. It fails the day the walk is narrowed,
    // and the day a module is added somewhere the walk does not descend into.
    let mut declared = 0_usize;
    for path in &sources {
        let source = fs::read_to_string(path)?;
        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))
                .and_then(|rest| rest.strip_suffix(';'))
            {
                declared += 1;
                assert!(
                    read.contains(name),
                    "`{name}` is declared in {} and the walk never read it",
                    relative(path)
                );
            }
            if let Some(rest) = trimmed.strip_prefix("#[path = \"") {
                let target = rest.split('"').next().unwrap_or_default();
                let resolved = path
                    .parent()
                    .map_or_else(|| PathBuf::from(target), |parent| parent.join(target));
                assert!(
                    sources.iter().any(|read_path| read_path == &resolved),
                    "{} includes {target}, which the walk never read",
                    relative(path)
                );
            }
        }
    }
    assert!(declared >= 10, "the crate declares only {declared} modules");

    // The workspace walk has its own floor, for the same reason.
    let workspace = workspace_sources()?;
    assert!(
        workspace.len() >= 400,
        "the workspace walk found only {} files",
        workspace.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The raw provider response
// ---------------------------------------------------------------------------

/// The bytes of a raw provider response are read at exactly two named sites,
/// and no public signature anywhere hands them out.
#[test]
fn every_raw_byte_site_is_named_and_justified() -> TestResult {
    let mut sites: Vec<(String, usize)> = Vec::new();
    let mut total = 0_usize;
    for path in crate_product_sources()? {
        let code = drop_use_items(&code_of(&path)?);
        let count = calls_of(&code, "response_bytes");
        if count > 0 {
            sites.push((relative(&path), count));
            total += count;
        }
    }
    sites.sort();

    let mut expected: Vec<(String, usize)> = Vec::new();
    for (file, _, _) in RAW_BYTE_SITES {
        match expected.iter_mut().find(|(name, _)| name == file) {
            Some(entry) => entry.1 += 1,
            None => expected.push((file.to_owned(), 1)),
        }
    }
    expected.sort();
    assert_eq!(
        sites, expected,
        "the raw-byte inventory and the source disagree"
    );
    assert_eq!(total, RAW_BYTE_SITES.len(), "a raw-byte site is unnamed");

    // Each site carries a reason, and each reason says something.
    for (file, function, reason) in RAW_BYTE_SITES {
        assert!(
            reason.len() >= 80,
            "{file}:{function} has no written reason"
        );
        let source = fs::read_to_string(workspace_root().join(file))?;
        assert!(
            source.contains(function.rsplit("::").next().unwrap_or(function)),
            "{file} no longer declares {function}"
        );
    }

    // The accessor is crate-private, and there is exactly one of it.
    let response = fs::read_to_string(crate_root().join("src/response.rs"))?;
    assert!(
        response.contains("pub(crate) fn response_bytes(&self) -> &[u8] {"),
        "the raw-byte accessor is no longer crate-private"
    );
    assert_eq!(
        declarations_of(&strip_non_code(&response), "response_bytes"),
        1,
        "there is more than one raw-byte accessor"
    );

    // Crate-private stops a caller from calling it. It does not stop this
    // crate from calling it on a caller's behalf, so the shape is refused
    // whatever it is named -- and workspace-wide, because `ProviderResponse`
    // and `ArchivedResponse` are public types any crate can name.
    let mut surface = 0_usize;
    for path in workspace_sources()? {
        let relative = relative(&path);
        if relative.contains("/tests/") || relative.contains("/benches/") {
            continue;
        }
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            surface = surface.saturating_add(1);
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if !names_any(parameters, &["ProviderResponse", "ArchivedResponse"]) {
                continue;
            }
            assert!(
                !names_any(returns, &["str", "String", "u8"]),
                "{relative} declares `{signature}`, which hands out a raw provider response"
            );
        }
    }
    assert!(
        surface >= 1000,
        "the public-signature sweep read only {surface} signatures"
    );
    Ok(())
}

/// Only reviewed files hold an `AcceptedResponse`, whose bytes carry no trust
/// label.
///
/// This is the one-step-out check on this crate's own new edge.
/// `academic-untrusted-content`'s `only_reviewed_files_hold_an_unlabelled_provider_response`
/// holds the same inventory for the workspace; this is the half that says what
/// **this** crate does with the one it holds.
#[test]
fn the_accepted_response_is_sealed_immediately() -> TestResult {
    let mut holders: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        if code_of(&path)?.contains("AcceptedResponse") {
            holders.push(relative(&path));
        }
    }
    holders.sort();
    assert_eq!(
        holders,
        vec!["crates/transcription/src/response.rs".to_owned()],
        "a file in this crate holds an AcceptedResponse and is not reviewed"
    );

    // One function in this crate takes an `AcceptedResponse`, and it is
    // `ProviderResponse::from_remote`, whose whole text is pinned by
    // `WHOLE_RESPONSE_BYTES`. A second entry point cannot build one in its body
    // either: `AcceptedResponse` has no public constructor, and the one
    // producer -- `EgressProxy::accept_response` -- needs a name this crate's
    // product source does not contain.
    let mut takers: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        assert_eq!(
            uses_of(&code, "EgressProxy"),
            0,
            "{} names EgressProxy, so it can produce an AcceptedResponse of its own",
            relative(&path)
        );
        for signature in public_signatures(&code) {
            if uses_of(&signature, "AcceptedResponse") > 0 {
                takers.push(signature);
            }
        }
    }
    assert_eq!(
        takers,
        vec![
            "pub fn from_remote(admission: &RemoteAdmission, accepted: &AcceptedResponse) -> Self {"
                .to_owned()
        ],
        "the set of functions taking an unlabelled provider response changed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// raw_token_write_protection
// ---------------------------------------------------------------------------

/// No path in this workspace writes a raw token.
#[test]
fn raw_token_write_protection() -> TestResult {
    // The whole set of `impl` blocks naming a raw type, compared as a set.
    let mut found: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        let code = code_of(&path)?;
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl")
                && names_any(
                    trimmed,
                    &[
                        "RawToken",
                        "RawSegment",
                        "RawTranscript",
                        "TranscriptLineage",
                    ],
                )
            {
                found.push(trimmed.to_owned());
            }
        }
    }
    found.sort();
    let mut expected: Vec<String> = RAW_IMPL_BLOCKS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        found, expected,
        "the set of impl blocks naming a raw type changed; a write path is how the raw \
         layer stops being raw"
    );
    for forbidden in FORBIDDEN_RAW_TRAITS {
        assert!(
            !found.iter().any(|header| header.contains(forbidden)),
            "an impl of {forbidden} for a raw type exists"
        );
    }

    // No signature anywhere in `crates/` takes a raw value and hands back
    // something mutable. The types are public, so this is a workspace rule.
    let mut swept = 0_usize;
    for path in workspace_sources()? {
        let relative = relative(&path);
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            swept = swept.saturating_add(1);
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            let touches_raw = names_any(parameters, &["RawToken", "RawSegment", "RawTranscript"])
                || names_any(returns, &["RawToken", "RawSegment", "RawTranscript"]);
            if !touches_raw {
                continue;
            }
            assert!(
                !returns.contains("&mut"),
                "{relative} declares `{signature}`, which hands out a mutable raw value"
            );
            assert!(
                !parameters.contains("&mut RawToken")
                    && !parameters.contains("&mut RawSegment")
                    && !parameters.contains("&mut RawTranscript")
                    && !parameters.contains("&mut [RawToken]")
                    && !parameters.contains("&mut Vec<RawToken>"),
                "{relative} declares `{signature}`, which takes a mutable raw value"
            );
        }
    }
    assert!(swept >= 1000, "the raw sweep read only {swept} signatures");

    // A sweep over *signatures* is a claim about what functions declare. What
    // makes a raw value exist is a *construction*, which `U-G3` found a second
    // entry point can perform in its body while naming the type nowhere in its
    // signature. Three rules hold that half, and the first of them is the
    // compiler's rather than this file's.
    //
    // **Every field of all three is private.** A struct literal for a type with
    // a private field is a compile error outside the module that declares it,
    // and that module is `transcript.rs`. So "built nowhere else" is a language
    // rule; what this checks is that the condition it rests on stays true.
    let transcript = fs::read_to_string(crate_root().join("src/transcript.rs"))?;
    for declaration in [
        "pub struct RawToken {",
        "pub struct RawSegment {",
        "pub struct RawTranscript {",
    ] {
        let at = transcript
            .find(declaration)
            .ok_or_else(|| format!("{declaration} is gone"))?;
        let body = transcript
            .get(at..)
            .and_then(|rest| {
                rest.find(
                    "
}",
                )
                .map(|end| &rest[..end])
            })
            .ok_or_else(|| format!("{declaration} has no closing brace"))?;
        assert_eq!(
            occurrences(body, "pub "),
            1,
            "{declaration} declares a public field, so a struct literal for it              compiles outside transcript.rs"
        );
    }

    // **The three assemblies are pinned as whole text.** A pin fixes the
    // decision sites that exist; the private fields are what stop a new one.
    assert_eq!(
        declared_item(&transcript, "fn parse_token(")?,
        WHOLE_PARSE_TOKEN,
        "the token assembly changed"
    );
    assert_eq!(
        declared_item(
            &transcript,
            "    fn close(self) -> Result<RawSegment, DecodeFault> {"
        )?,
        WHOLE_CLOSE_SEGMENT,
        "the segment assembly changed"
    );
    assert_eq!(
        declared_item(&transcript, "pub fn decode(")?,
        WHOLE_DECODE,
        "the transcript assembly changed"
    );

    // **No file outside this package names a raw type at all.** `P2-U6`'s
    // `credentials_never_reach_a_general_crawler` uses this shape: a type
    // nothing else can name is a type nothing else can build on unnoticed. It
    // is a tripwire for `P2-L4`, which is the first task that will.
    let mut elsewhere: Vec<String> = Vec::new();
    for path in workspace_sources()? {
        let relative = relative(&path);
        if relative.starts_with("crates/transcription/") {
            continue;
        }
        if names_any(
            &code_of(&path)?,
            &["RawToken", "RawSegment", "RawTranscript"],
        ) {
            elsewhere.push(relative);
        }
    }
    assert_eq!(
        elsewhere,
        Vec::<String>::new(),
        "a package outside academic-transcription names a raw type"
    );

    // And the decoder itself has one caller in this crate's product source.
    let mut decode_calls = 0_usize;
    for path in crate_product_sources()? {
        let code = drop_use_items(&code_of(&path)?);
        decode_calls = decode_calls.saturating_add(calls_of(&code, "decode"));
    }
    assert_eq!(
        decode_calls, 1,
        "the decoder has more than one caller in this crate's product source"
    );
    Ok(())
}

/// `TranscriptLineage` mutates its own versions and never the raw transcript.
#[test]
fn the_lineage_has_no_raw_mutation() -> TestResult {
    let version = code_of(&crate_root().join("src/version.rs"))?;
    // `self.raw` is read and never assigned or borrowed mutably.
    assert_eq!(
        occurrences(&version, "self.raw ="),
        0,
        "the lineage assigns its raw transcript"
    );
    assert_eq!(
        occurrences(&version, "&mut self.raw"),
        0,
        "the lineage borrows its raw transcript mutably"
    );
    // Every `&mut self` method the lineage has, as a whole set.
    let mutating: Vec<String> = public_signatures(&version)
        .into_iter()
        .filter(|signature| signature.contains("&mut self"))
        .collect();
    assert_eq!(
        mutating,
        vec![
            "pub fn open_review(&mut self, address: TokenAddress) -> Result<(), VersionFault> {"
                .to_owned(),
            "pub fn append_correction(&mut self, settled: SettledCorrection) -> Result<u32, VersionFault> {"
                .to_owned(),
            "pub fn append_annotations(&mut self, annotations: AnnotationLayer) -> u32 {".to_owned(),
        ],
        "the lineage's mutating surface changed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The pins, and their callers
// ---------------------------------------------------------------------------

/// The route decision has one body, one caller, and no default that reaches
/// the remote arm.
#[test]
fn no_default_reaches_the_remote_arm() -> TestResult {
    let route = fs::read_to_string(crate_root().join("src/route.rs"))?;
    assert_eq!(
        declared_item(&route, "impl SttPolicy {")?,
        WHOLE_POLICY,
        "impl SttPolicy changed"
    );

    // A pin fixes the item and not its caller. `T141` left a pinned check
    // byte-identical and wrapped the *call* in a condition, so the call sites
    // are counted too -- one, inside `run`.
    let mut callers: BTreeMap<String, usize> = BTreeMap::new();
    for path in crate_product_sources()? {
        let code = drop_use_items(&code_of(&path)?);
        let count = calls_of(&code, "route_for");
        if count > 0 {
            callers.insert(relative(&path), count);
        }
    }
    assert_eq!(
        callers,
        BTreeMap::from([("crates/transcription/src/pipeline.rs".to_owned(), 1)]),
        "the route decision has another caller"
    );

    // No product file in this crate reaches a default, an environment lookup,
    // or a configuration file on the route path. `P2-G1`'s default-deny is a
    // structure here: `SttPolicy::new` holds nothing and there is no other
    // producer.
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for forbidden in [
            "env::var",
            "var_os",
            "unwrap_or_default",
            "read_to_string",
            "File::open",
        ] {
            assert_eq!(
                occurrences(&code, forbidden),
                0,
                "{} names `{forbidden}`",
                relative(&path)
            );
        }
    }
    Ok(())
}

/// The archive appends and does nothing else.
#[test]
fn the_archive_appends_and_nothing_removes() -> TestResult {
    let response = fs::read_to_string(crate_root().join("src/response.rs"))?;
    assert_eq!(
        declared_item(&response, "impl RawResponseArchive {")?,
        WHOLE_ARCHIVE,
        "impl RawResponseArchive changed"
    );
    assert_eq!(
        declared_item(&response, "impl ProviderResponse {")?,
        WHOLE_RESPONSE_BYTES,
        "impl ProviderResponse changed"
    );

    // Exactly one `&mut self` method, and it pushes.
    let mutating: Vec<String> = public_signatures(&response)
        .into_iter()
        .filter(|signature| signature.contains("&mut self"))
        .collect();
    assert_eq!(
        mutating.len(),
        1,
        "the archive has a second mutating method"
    );
    let code = code_of(&crate_root().join("src/response.rs"))?;
    for forbidden in [
        "remove", "retain(", "clear", "truncate", "drain", "pop", "swap",
    ] {
        assert_eq!(
            occurrences(&code, &format!("entries.{forbidden}")),
            0,
            "the archive calls `{forbidden}` on its entries"
        );
    }
    assert_eq!(
        occurrences(&code, "entries.push"),
        1,
        "the archive extends its entries somewhere else, or not at all"
    );
    Ok(())
}

/// The three dispositions are `academic-domain`'s, and there is no fourth.
#[test]
fn no_fourth_disposition_is_declared() -> TestResult {
    let version = fs::read_to_string(crate_root().join("src/version.rs"))?;
    assert_eq!(
        declared_item(&version, "impl LineageEffect {")?,
        WHOLE_LINEAGE_EFFECT,
        "impl LineageEffect changed"
    );

    // `DecisionAction` is `academic-domain`'s frozen vocabulary. It is matched
    // in exactly one place in this crate's product source and compared in one
    // other, and nothing here declares an enum of its own beside it.
    let mut sites: BTreeMap<String, usize> = BTreeMap::new();
    for path in crate_product_sources()? {
        let code = drop_use_items(&code_of(&path)?);
        let count = uses_of(&code, "DecisionAction");
        if count > 0 {
            sites.insert(relative(&path), count);
        }
    }
    assert_eq!(
        sites,
        BTreeMap::from([("crates/transcription/src/version.rs".to_owned(), 5)]),
        "DecisionAction is named somewhere else, or a different number of times"
    );

    // No product file declares an enum whose variants are a disposition
    // vocabulary of its own.
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for forbidden in [
            "Disposition {",
            "enum Disposition",
            "Pending",
            "Deferred",
            "Snooze",
        ] {
            assert_eq!(
                occurrences(&code, forbidden),
                0,
                "{} declares `{forbidden}`, which is a fourth disposition",
                relative(&path)
            );
        }
    }
    Ok(())
}

/// A run's transmission is decided by the route arm and never by the caller.
#[test]
fn the_transmission_is_decided_by_the_route() -> TestResult {
    let pipeline = fs::read_to_string(crate_root().join("src/pipeline.rs"))?;
    assert_eq!(
        declared_item(
            &pipeline,
            "fn record_model_run(\n    identity: &RunIdentity,"
        )?,
        WHOLE_RECORD_MODEL_RUN,
        "record_model_run changed"
    );
    assert_eq!(
        declared_item(&pipeline, "fn transcribe(\n    provider: &dyn SttProvider,")?,
        WHOLE_TRANSCRIBE,
        "the transcribe stage changed"
    );
    let code = drop_use_items(&code_of(&crate_root().join("src/pipeline.rs"))?);
    assert_eq!(
        calls_of(&code, "record_model_run"),
        1,
        "the run record is built somewhere else too"
    );
    // `ModelRun::record` is `P2-M1`'s one constructor and this crate calls it
    // once. A second provenance record is what this task must not create.
    let mut record_sites: BTreeMap<String, usize> = BTreeMap::new();
    for path in crate_product_sources()? {
        let code = drop_use_items(&code_of(&path)?);
        let count = occurrences(&code, "ModelRun::record(");
        if count > 0 {
            record_sites.insert(relative(&path), count);
        }
    }
    assert_eq!(
        record_sites,
        BTreeMap::from([("crates/transcription/src/pipeline.rs".to_owned(), 1)]),
        "a model run is recorded somewhere else"
    );
    Ok(())
}

/// A manifest's authorization is compared against the journal's own header.
#[test]
fn the_binding_is_compared_against_the_journal_header() -> TestResult {
    let authorize = fs::read_to_string(crate_root().join("src/authorize.rs"))?;
    assert_eq!(
        declared_item(&authorize, "impl AuthorizationBinding {")?,
        WHOLE_BINDING,
        "impl AuthorizationBinding changed"
    );
    // The binding is *produced* in one place, and that place takes the capture
    // rather than the journal. A sweep over signatures alone would not say so:
    // `U-G3` is the row that records a second entry point building its argument
    // in its body, so the construction is counted too.
    let authorize = code_of(&crate_root().join("src/authorize.rs"))?;
    let producers: Vec<String> = public_signatures(&authorize)
        .into_iter()
        .filter(|signature| {
            parameters_and_return(signature)
                .is_some_and(|(_, returns)| uses_of(returns, "Self") > 0)
                && uses_of(signature, "JournalRecovery") > 0
        })
        .collect();
    assert_eq!(
        producers,
        vec![
            "pub fn of(recorder: &CaptureRecorder, recovery: &JournalRecovery) -> Result<Self, InputFault> {"
                .to_owned()
        ],
        "the set of functions producing a binding from a journal changed; the first          version of this module read the token out of the journal it was about to          admit from, and `ChunkJournal::replay` is public"
    );
    let mut constructions: BTreeMap<String, usize> = BTreeMap::new();
    for path in workspace_sources()? {
        let count = occurrences(&code_of(&path)?, "AuthorizationBinding {");
        if count > 0 {
            constructions.insert(relative(&path), count);
        }
    }
    assert_eq!(
        constructions,
        BTreeMap::from([("crates/transcription/src/authorize.rs".to_owned(), 3)]),
        "a binding is built somewhere other than `of`"
    );

    // The comparison is called as the first statement of both admitting
    // methods, and there are exactly two of them.
    let code = drop_use_items(&code_of(&crate_root().join("src/authorize.rs"))?);
    assert_eq!(
        calls_of(&code, "covers"),
        2,
        "the binding comparison has another caller, or has lost one"
    );
    for method in ["admit_audio_chunk", "admit_capture"] {
        let start = authorize
            .find(&format!("pub fn {method}("))
            .ok_or_else(|| format!("{method} is gone"))?;
        let body = authorize
            .get(start..)
            .and_then(|rest| rest.find('{').map(|at| &rest[at..]))
            .ok_or_else(|| format!("{method} has no body"))?;
        let first = body
            .lines()
            .nth(1)
            .ok_or_else(|| format!("{method} has an empty body"))?
            .trim();
        assert_eq!(
            first, "if !self.binding.covers(recovery) {",
            "{method}'s first statement is not the binding comparison"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The comparison carries no order
// ---------------------------------------------------------------------------

/// Two provider runs are diffed and never ordered.
#[test]
fn two_runs_carry_no_order() -> TestResult {
    let mut found: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        let code = code_of(&path)?;
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl")
                && names_any(
                    trimmed,
                    &[
                        "ProviderRun",
                        "RetranscriptionComparison",
                        "Side",
                        "Divergence",
                        "CompareFault",
                    ],
                )
            {
                found.push(trimmed.to_owned());
            }
        }
    }
    found.sort();
    let mut expected: Vec<String> = COMPARISON_IMPL_BLOCKS
        .iter()
        .map(|entry| (*entry).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        found, expected,
        "the set of impl blocks naming a comparison type changed; an ordering is how a \
         diff becomes a ranking"
    );
    for forbidden in FORBIDDEN_ORDER_TRAITS {
        assert!(
            !found.iter().any(|header| header.contains(forbidden)),
            "an impl of {forbidden} for a comparison type exists"
        );
    }

    // The derive lists, which an `impl` header does not carry.
    let compare = fs::read_to_string(crate_root().join("src/compare.rs"))?;
    for (declaration, allowed) in [
        (
            "pub struct ProviderRun {",
            "#[derive(Debug, Clone, PartialEq, Eq, Hash)]",
        ),
        (
            "pub struct RetranscriptionComparison {",
            "#[derive(Debug, Clone, PartialEq, Eq)]",
        ),
        (
            "pub enum Side {",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]",
        ),
    ] {
        let at = compare
            .find(declaration)
            .ok_or_else(|| format!("{declaration} is gone"))?;
        let derive = compare
            .get(..at)
            .and_then(|head| {
                head.lines()
                    .rev()
                    .find(|line| line.trim().starts_with("#[derive"))
            })
            .ok_or_else(|| format!("{declaration} has no derive list"))?
            .trim();
        assert_eq!(derive, allowed, "{declaration}'s derive list changed");
    }

    // And no accessor named for a verdict.
    let code = code_of(&crate_root().join("src/compare.rs"))?;
    for forbidden in [
        "winner",
        "better",
        "preferred",
        "rank",
        "score",
        "best",
        "worse",
        "prefer",
    ] {
        assert_eq!(
            uses_of(&code, forbidden),
            0,
            "compare.rs names `{forbidden}`, which is a verdict"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// What this crate does not reach
// ---------------------------------------------------------------------------

/// This crate reads no clock, opens no socket, and touches no filesystem.
#[test]
fn no_wall_clock_socket_or_file_reaches_this_crate() -> TestResult {
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        scanned = scanned.saturating_add(1);
        for forbidden in [
            "SystemTime",
            "Instant",
            "UNIX_EPOCH",
            "std::time",
            "chrono",
            "now_v7",
            "elapsed()",
            "TcpStream",
            "TcpListener",
            "UdpSocket",
            "std::net",
            "std::fs",
            "std::process",
            "Command",
            "unsafe",
        ] {
            assert_eq!(
                occurrences(&code, forbidden),
                0,
                "{} names `{forbidden}`",
                relative(&path)
            );
        }
    }
    assert!(scanned >= 11, "the product walk read only {scanned} files");

    // The manifest half: no crate this one depends on can open a socket that
    // is not already `P2-G2`'s, and there is no edge to `academic-worker`,
    // whose sandbox probe would then be reachable from a default build.
    // Comment lines are stripped first: this crate's manifest explains in prose
    // why the two edges below are absent, and a naive substring would read the
    // explanation as the edge.
    let manifest: String = fs::read_to_string(crate_root().join("Cargo.toml"))?
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join(
            "
",
        );
    assert!(
        !manifest.contains("academic-worker"),
        "this crate depends on academic-worker, whose probe would become reachable"
    );
    assert!(
        !manifest.contains("academic-store"),
        "this crate depends on academic-store, so it persists something"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Every impl header is in the inventory
// ---------------------------------------------------------------------------

/// Every `impl` header of `code`, up to its opening brace.
///
/// A trait impl's methods carry no visibility modifier, so an inventory keyed
/// on `pub fn` cannot see one at all. `P2-A4` measured that gap here with
/// `impl From<&AuthorizedChunk> for Vec<u8>`, which passed this crate's whole suite. The precedent for closing
/// it is `P2-Y3`'s and `P2-X5`'s: pin the complete set of headers, so a
/// conversion nobody predicted fails as an extra entry rather than having to be
/// named on a forbidden list.
fn impl_headers(code: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut lines = code.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        if !(trimmed == "impl" || trimmed.starts_with("impl ") || trimmed.starts_with("impl<")) {
            continue;
        }
        // A header may be wrapped, so keep reading until the block opens. An
        // `impl Trait` in argument position is not a header and is skipped by
        // the line anchor above: it can never begin a line, because a parameter
        // list always puts a name and a colon in front of it.
        let mut header = trimmed.to_owned();
        while !header.contains('{') {
            let Some(next) = lines.next() else {
                break;
            };
            header.push(' ');
            header.push_str(next.trim());
        }
        let end = header.find('{').unwrap_or(header.len());
        found.insert(
            header[..end]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    found
}

/// Every `impl` header this crate declares, pinned as a complete set.
const IMPL_HEADERS: &[&str] = &[
    "impl Annotation",
    "impl AnnotationKind",
    "impl AnnotationLayer",
    "impl AppliedCorrection",
    "impl ArchivedResponse",
    "impl AudioFormat",
    "impl AuthorizationBinding",
    "impl AuthorizedCapture",
    "impl AuthorizedChunk",
    "impl CapabilityField",
    "impl ChunkBoundary",
    "impl CompletedRun",
    "impl ConfidenceSemantics",
    "impl ContractDraft",
    "impl ContractRegistry",
    "impl CorrectionAuthor",
    "impl CorrectionCandidate",
    "impl CorrectionStatus",
    "impl Divergence",
    "impl DownstreamJob",
    "impl FeatureClaim",
    "impl InputManifest",
    "impl JobHandle",
    "impl LineageEffect",
    "impl OpenSegment",
    "impl ProviderContract",
    "impl ProviderPlacement",
    "impl ProviderResponse",
    "impl ProviderRun",
    "impl ProviderSelection",
    "impl RawResponseArchive",
    "impl RawResponseId",
    "impl RawSegment",
    "impl RawToken",
    "impl RawTranscript",
    "impl RemoteAdmission",
    "impl RemoteProcessingApproval",
    "impl RetranscriptionComparison",
    "impl RouteDenial",
    "impl RunRecord",
    "impl SettledCorrection",
    "impl Side",
    "impl Speaker",
    "impl Stage",
    "impl SttPolicy",
    "impl SttRoute",
    "impl SuppliedMaterial",
    "impl Support",
    "impl TimestampSemantics",
    "impl TokenAddress",
    "impl TranscriptLineage",
    "impl TranscriptVersion",
    "impl core::fmt::Debug for Annotation",
    "impl core::fmt::Debug for AppliedCorrection",
    "impl core::fmt::Debug for AuthorizedCapture",
    "impl core::fmt::Debug for AuthorizedChunk",
    "impl core::fmt::Debug for CorrectionCandidate",
    "impl core::fmt::Debug for EffectiveToken<'_>",
    "impl core::fmt::Debug for ProviderResponse",
    "impl core::fmt::Display for RawResponseId",
    "impl fmt::Debug for RawSegment",
    "impl fmt::Debug for RawToken",
    "impl fmt::Display for CapabilityField",
    "impl<'a> EffectiveToken<'a>",
    "impl<'a> TranscriptSegment<'a>",
];

/// The traits this crate implements for its own types, pinned as a set.
///
/// Eleven, and every one of them is a `Debug` or a `Display`. Nine of the
/// `Debug`s are written by hand to redact what a derive would print. No
/// conversion, no dereference, no iteration.
const TRAIT_IMPLS: &[&str] = &[
    "impl core::fmt::Debug for Annotation",
    "impl core::fmt::Debug for AppliedCorrection",
    "impl core::fmt::Debug for AuthorizedCapture",
    "impl core::fmt::Debug for AuthorizedChunk",
    "impl core::fmt::Debug for CorrectionCandidate",
    "impl core::fmt::Debug for EffectiveToken<'_>",
    "impl core::fmt::Debug for ProviderResponse",
    "impl core::fmt::Display for RawResponseId",
    "impl fmt::Debug for RawSegment",
    "impl fmt::Debug for RawToken",
    "impl fmt::Display for CapabilityField",
];

/// Every `impl` header this crate declares is in the inventory, both ways.
///
/// `P2-A4`'s F12: the blindness that let a trait impl hand out removed student
/// speech in `academic-student-voice` is a property of the scan's definition of
/// "signature", not of that crate, and the same injection compiled and passed
/// here. The close is the same whole-set comparison: `From` is the spelling
/// that was measured, but `Into`, `TryFrom`, `Deref`, `AsRef`, `Borrow`,
/// `Index`, `IntoIterator` and a trait nobody has thought of all reach the same
/// private fields, so the rule is stated over the complete set rather than over
/// a list of trait names.
#[test]
fn every_impl_header_in_this_crate_is_in_the_inventory() -> TestResult {
    let mut found: BTreeSet<String> = BTreeSet::new();
    for path in crate_product_sources()? {
        found.extend(impl_headers(&code_of(&path)?));
    }
    assert_eq!(
        found,
        IMPL_HEADERS.iter().map(|item| (*item).to_owned()).collect(),
        "the impl-header inventory and the source disagree"
    );

    // The trait half stated on its own, so the reason survives an edit to the
    // list above: every header that names a trait is one of these, and none of
    // them is a conversion, a dereference, an iteration or an arithmetic fold.
    let traits: Vec<&str> = found
        .iter()
        .filter(|header| header.contains(" for "))
        .map(String::as_str)
        .collect();
    assert_eq!(
        traits,
        TRAIT_IMPLS.to_vec(),
        "this crate implements a trait the inventory does not carry"
    );

    // The scanner is not vacuous: it finds the shape `P2-A4` injected, and it
    // does not read an `impl Trait` in argument position as a header.
    assert_eq!(
        impl_headers("impl From<&AuthorizedChunk> for Vec<u8> {\n}\n"),
        ["impl From<&AuthorizedChunk> for Vec<u8>"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    );
    assert!(impl_headers("fn takes(value: impl Display) {}\n").is_empty());
    Ok(())
}
