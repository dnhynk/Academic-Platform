//! Source scans for the `P2-G6` consent boundary.
//!
//! Three of this crate's claims are shapes of the source rather than
//! behaviours, so nothing at run time would notice the day they stopped being
//! true. `docs/contracts/policy-source-scans.md` is the page those scans are
//! enumerated on, and this file is written against all five of the empty-scan
//! shapes it names.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends the whole
//! package, not `src` by name, with a floor, a `mod`/`#[path]` tripwire, and a
//! rule that this crate's product source is under `src` and nowhere else. `S-12`
//! on that page is the row about a walk rooted at `<crate>/src`.
//!
//! **The checks are not token lists.** The three that could have been are whole
//! sets: the `impl` blocks naming [`AuthorityGrant`], the `impl` blocks naming
//! [`AttestationRecord`], and the inventory of the files that construct a
//! capability. A conversion nobody predicted fails as an extra key.
//!
//! **The pins fix their callers.** `WHOLE_BIND_PERMISSION` is accompanied by
//! `WHOLE_MINT` and `WHOLE_CONTINUE` and by a call-site count of two, because
//! `T141` found a pinned check skipped by a condition wrapped around it and
//! `P2-RF10` found a second public path that never called one.
//!
//! **Every inventory counts a name, not a spelling.** `P2-RF10` reached a
//! fourth exposure site in another crate by writing `Untrusted::expose(d)`
//! instead of `d.expose()`, which contains no `.expose()`. The counts here are
//! whole-identifier counts of the function's own name, with declarations
//! subtracted, so a call written through the type path counts the same.
//!
//! **The floors bound the coverage.** A walk that returned nothing would pass
//! every loop below it, so each loop has a floor and each whole-set comparison
//! fails on a missing key as well as an extra one.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

use academic_consent::{
    CHECKLIST_DIMENSIONS, CaptureMedium, CaptureProcessing, CaptureStatus, ConsentEventKind,
    DERIVATIVE_CLASSES, GrantAuthority, NotApplicableReason, WrittenEvidenceKind,
};

type TestResult = Result<(), Box<dyn Error>>;

/// The crate root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The workspace root.
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

/// Every `.rs` file that ships: everything outside `tests` and `benches`.
fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    Ok(crate_all_sources()?
        .into_iter()
        .filter(|path| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            !relative.starts_with("tests") && !relative.starts_with("benches")
        })
        .collect())
}

/// Every `.rs` file under every workspace package, less each package's `tests`
/// and `benches`.
///
/// The package rather than its `src`, for `S-12`'s reason: `crates/record`
/// ships an `examples/` tree and `crates/worker` a `probes/` tree, and both are
/// product-shaped code a walk rooted at `src` never reads.
fn workspace_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let crates = workspace_root().join("crates");
    let mut found = Vec::new();
    for entry in fs::read_dir(&crates)? {
        let package = entry?.path();
        if !package.is_dir() {
            continue;
        }
        let mut inside = Vec::new();
        walk(&package, &mut inside)?;
        for path in inside {
            let relative = path.strip_prefix(&package).unwrap_or(&path).to_path_buf();
            if relative.starts_with("tests") || relative.starts_with("benches") {
                continue;
            }
            found.push(path);
        }
    }
    found.sort();
    Ok(found)
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

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/record/tests/record_scans.rs` by way of
/// `crates/untrusted-content/tests/trust_scans.rs`, raw strings and nested
/// block comments included. `P2-G4` found that a lexer without raw strings
/// desynchronizes and reads every literal after one as code.
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
                let terminator: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = bytes[probe + 1..].iter().collect();
                let end = rest.find(&terminator).map_or(bytes.len(), |at| {
                    probe + 1 + rest[..at].chars().count() + terminator.chars().count()
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
            let closes_two_on = bytes.get(index + 2) == Some(&'\'');
            let closes_three_on = bytes.get(index + 3) == Some(&'\'');
            if closes_two_on || (bytes.get(index + 1) == Some(&'\\') && closes_three_on) {
                index += if closes_two_on { 3 } else { 4 };
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

/// Extracts one item's text, comment lines dropped and whitespace collapsed.
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

/// Drops every `use` item, so a re-export is not counted as a caller.
fn without_use_items(code: &str) -> String {
    let mut kept = String::with_capacity(code.len());
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim_start();
        let opens = trimmed.starts_with("use ")
            || (trimmed.starts_with("pub") && trimmed.contains(" use "));
        if inside || opens {
            inside = !line.trim_end().ends_with(';');
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    kept
}

/// Every `pub` function signature in `code`, whitespace-collapsed.
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

/// How many times `name` is called in `code`, declarations subtracted.
///
/// The uses are counted by whole identifier, so a call written through the
/// module path counts the same as a bare one -- `P2-RF10` reached a fourth site
/// in another crate by changing only the spelling of a call. The declarations
/// are subtracted by the `fn name(` prefix rather than by `fn name`, because
/// `fn name` is a substring of `fn named` and this crate has both an `inherit`
/// and an `inherited`.
fn calls_of(code: &str, name: &str) -> usize {
    uses_of(code, name).saturating_sub(occurrences(code, &format!("fn {name}(")))
}

/// How many struct literals of `name` appear in `code`.
///
/// Three other things spell `Name {`: a declaration, an `impl` header, and a
/// function whose return type is `Name` or `&Name`, whose own opening brace
/// follows it. All three are lines this drops, and what is left is a value
/// being built. The direction of the error matters: a dropped line is a
/// construction this would not see, so the drops are by exact line shape rather
/// than by anything a construction could also match.
fn struct_literals(code: &str, name: &str) -> usize {
    let needle = format!("{name} {{");
    code.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("impl")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub enum ")
                || line.contains("fn "))
        })
        .map(|line| occurrences(line, &needle))
        .sum()
}

/// The variant names an enum declares, in declaration order.
///
/// Read out of the source rather than derived from the type, so a variant added
/// to the enum without being added to whatever else names it fails here.
fn enum_variants(source: &str, header: &str) -> Vec<String> {
    source
        .lines()
        .skip_while(|line| !line.contains(header))
        .skip(1)
        .take_while(|line| !line.starts_with('}'))
        .map(str::trim)
        .filter(|line| {
            !line.is_empty()
                && !line.starts_with("///")
                && !line.starts_with("//")
                && !line.starts_with('#')
        })
        .map(|line| line.trim_end_matches(',').to_owned())
        .collect()
}

/// The quoted spellings inside the first `<column> IN ( … )` list in `sql`.
///
/// Returns them sorted, so the comparison is against a set rather than against
/// the order somebody happened to write the `CHECK` in.
fn sql_check_list(sql: &str, column: &str) -> Vec<String> {
    let Some(start) = sql.find(&format!("{column} IN (")) else {
        return Vec::new();
    };
    let rest = &sql[start..];
    let Some(end) = rest.find(')') else {
        return Vec::new();
    };
    let mut found: Vec<String> = rest[..end]
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect();
    found.sort();
    found
}

/// One file, as code with comments and literals removed.
fn code_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(path)?))
}

/// The relative path of `path` under the workspace, with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// Every `impl` block header in `code` that names `type_name` as a whole
/// identifier, whitespace-collapsed.
///
/// The whole set is compared against a pinned list, so an implementation of a
/// trait nobody predicted appears as an extra key. An `impl` in another crate
/// is refused by the orphan rule instead: both the trait and the type would be
/// foreign there.
fn impl_headers_naming(code: &str, type_name: &str) -> Vec<String> {
    let mut found = Vec::new();
    let lines: Vec<&str> = code.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with("impl") {
            continue;
        }
        let mut header = String::new();
        for follow in lines.iter().skip(index) {
            header.push(' ');
            header.push_str(follow.trim());
            if follow.contains('{') {
                break;
            }
        }
        let header = header.split_whitespace().collect::<Vec<_>>().join(" ");
        if uses_of(&header, type_name) > 0 {
            found.push(header);
        }
    }
    found
}

// ---------------------------------------------------------------------------
// Whole-text pins. Each is compared against the item as the source declares it,
// comment lines dropped and whitespace collapsed, so `cargo fmt` decides layout
// and the pin decides content. What editing one costs is in
// `docs/contracts/policy-source-scans.md`.
// ---------------------------------------------------------------------------

/// The whole status derivation. What decides `PERMITTED` and what refuses.
const WHOLE_STATUS_OF: &str = "pub fn status_of(record: &PermissionRecord, at: u64) -> CaptureStatus { let grant: &AuthorityGrant = match record.disposition() { Disposition::Prohibited(_) => return CaptureStatus::Prohibited, Disposition::Granted(grant) => grant, }; if at >= grant.not_after() || !record.scope().contains(at) { return CaptureStatus::Expired; } if record.verified_at() < record.scope().valid_from() { return CaptureStatus::Expired; } if grant.conditions().is_empty() && record.checklist().is_complete() { return CaptureStatus::Permitted; } CaptureStatus::PermittedWithConditions }";

/// The whole status surface, including which statuses permit at all.
const WHOLE_CAPTURE_STATUS: &str = "impl CaptureStatus { #[must_use] pub const fn as_str(self) -> &'static str { match self { Self::Unknown => \"UNKNOWN\", Self::Prohibited => \"PROHIBITED\", Self::Permitted => \"PERMITTED\", Self::PermittedWithConditions => \"PERMITTED_WITH_CONDITIONS\", Self::Expired => \"EXPIRED\", } } #[must_use] pub const fn is_permitting(self) -> bool { matches!(self, Self::Permitted | Self::PermittedWithConditions) } }";

/// The whole binding. Every comparison section 3.7 asks for is in this text.
const WHOLE_BIND_PERMISSION: &str = "pub fn bind_permission( ledger: &ConsentLedger, request: &CaptureRequest, now: u64, ) -> Result<BoundPermission, CaptureDenial> { let deny = |reason: CaptureDenialReason, status: CaptureStatus| CaptureDenial { reason, status }; let resolved = match ResolvedRequest::resolve(request) { Ok(resolved) => resolved, Err(reason) => return Err(deny(reason, CaptureStatus::Unknown)), }; let Some(record) = ledger.permission_for(resolved.offering_id, resolved.term, resolved.lecture_id) else { return Err(deny( CaptureDenialReason::PermissionUnknown, CaptureStatus::Unknown, )); }; let status = status_of(record, now); match status { CaptureStatus::Unknown => { return Err(deny(CaptureDenialReason::PermissionUnknown, status)); } CaptureStatus::Prohibited => { return Err(deny(CaptureDenialReason::PermissionProhibited, status)); } CaptureStatus::Expired => { return Err(deny(CaptureDenialReason::PermissionExpired, status)); } CaptureStatus::Permitted | CaptureStatus::PermittedWithConditions => (), } if !record.scope().contains(now) || !record .scope() .answers(resolved.offering_id, resolved.term, resolved.lecture_id) { return Err(deny(CaptureDenialReason::ScopeMismatch, status)); } let Some(grant) = record.grant() else { return Err(deny(CaptureDenialReason::PermissionUnknown, status)); }; if resolved.media.is_empty() || resolved .media .iter() .any(|medium| !grant.allowed_media().contains(medium)) { return Err(deny(CaptureDenialReason::MediumNotGranted, status)); } if resolved .processing .iter() .any(|step| !grant.allowed_processing().contains(step)) { return Err(deny(CaptureDenialReason::ProcessingNotGranted, status)); } if !grant.external_processing_allowed() && resolved .processing .iter() .any(|step| step.leaves_the_device()) { return Err(deny( CaptureDenialReason::ExternalProcessingNotGranted, status, )); } if resolved.not_after > grant.not_after() || resolved.not_after > record.scope().valid_to() { return Err(deny(CaptureDenialReason::LifetimeExceedsGrant, status)); } Ok(BoundPermission { permission_id: record.permission_id(), permission_seq: record.permission_seq(), offering_id: resolved.offering_id, lecture_id: resolved.lecture_id, status, media: resolved.media.to_vec(), processing: resolved.processing.to_vec(), not_after: resolved.not_after, conditions: grant.conditions().to_vec(), unanswered: record.checklist().unanswered(), retention: *grant.retention(), }) }";

/// The first of the two callers, pinned for the `T141` reason.
const WHOLE_MINT: &str = "pub fn mint_capture_capability( ledger: &mut ConsentLedger, request: &CaptureRequest, now: u64, ) -> Result<CaptureCapabilityToken, CaptureDenial> { let bound = match bind_permission(ledger, request, now) { Ok(bound) => bound, Err(denial) => return Err(ledger.record_capture_denial(request, denial, now)), }; let token_id = token_id(&bound, now); ledger.record_capture_mint(&bound, &token_id, now); Ok(CaptureCapabilityToken { token_id, request: request.clone(), bound, }) }";

/// The second caller. `P2-RF10`'s second path is why this one exists at all.
const WHOLE_CONTINUE: &str = "pub fn continue_capture( ledger: &mut ConsentLedger, token: &CaptureCapabilityToken, now: u64, ) -> Result<(), CaptureDenial> { let bound = match bind_permission(ledger, token.request(), now) { Ok(bound) => bound, Err(denial) => return Err(ledger.record_capture_denial(token.request(), denial, now)), }; if now >= token.not_after() || bound.permission_id() != token.bound().permission_id() { let denial = CaptureDenial { reason: CaptureDenialReason::LifetimeExceedsGrant, status: bound.status(), }; return Err(ledger.record_capture_denial(token.request(), denial, now)); } Ok(()) }";

/// The missing-field resolver, which is `P2-G1`'s default-deny mechanism.
const WHOLE_RESOLVE_REQUEST: &str = "impl<'a> ResolvedRequest<'a> { fn resolve(request: &'a CaptureRequest) -> Result<Self, CaptureDenialReason> { let (Some(offering_id), Some(lecture_id), Some(term)) = (request.offering_id, request.lecture_id, request.term) else { return Err(CaptureDenialReason::IncompleteRequest); }; let (Some(media), Some(processing)) = (request.media.as_deref(), request.processing.as_deref()) else { return Err(CaptureDenialReason::IncompleteRequest); }; let (Some(_requested_at), Some(not_after)) = (request.requested_at, request.not_after) else { return Err(CaptureDenialReason::IncompleteRequest); }; Ok(Self { offering_id, lecture_id, term, media, processing, not_after, }) } }";

/// The two fields, as the struct declares them.
const WHOLE_RETENTION_TERMS_STRUCT: &str =
    "pub struct RetentionTerms { audio: RetentionBound, transcript: RetentionBound, }";

/// The two accessors and the inheritance rule, in one constant.
const WHOLE_RETENTION_TERMS: &str = "impl RetentionTerms { #[must_use] pub const fn new(audio: RetentionBound, transcript: RetentionBound) -> Self { Self { audio, transcript } } #[must_use] pub const fn audio(self) -> RetentionBound { self.audio } #[must_use] pub const fn transcript(self) -> RetentionBound { self.transcript } #[must_use] pub fn inherit(self, requested: Self) -> Self { Self { audio: self.audio.stricter(requested.audio), transcript: self.transcript.stricter(requested.transcript), } } #[must_use] pub fn is_no_wider_than(self, parent: Self) -> bool { self.audio <= parent.audio && self.transcript <= parent.transcript } }";

/// The bound comparison. One character reverses `stricter`.
const WHOLE_RETENTION_BOUND: &str = "impl RetentionBound { #[must_use] pub fn stricter(self, other: Self) -> Self { self.min(other) } #[must_use] pub const fn is_expired_at(self, at: u64) -> bool { match self { Self::Prohibited => true, Self::Until(until) => at >= until, } } #[must_use] pub const fn kind_str(self) -> &'static str { match self { Self::Prohibited => \"PROHIBITED\", Self::Until(_) => \"UNTIL\", } } }";

/// The whole grant surface, whose constructor takes a written authority.
const WHOLE_AUTHORITY_GRANT: &str = "impl AuthorityGrant { #[must_use] pub fn record( authority: WrittenAuthority, permitted_use: PermittedUse, retention: RetentionTerms, conditions: Vec<Condition>, not_after: u64, ) -> Self { let mut listed = conditions; listed.sort_unstable(); listed.dedup(); let conditions_digest = conditions_digest(&listed); Self { authority, permitted_use, retention, conditions: listed, conditions_digest, not_after, } } #[must_use] pub const fn authority(&self) -> &WrittenAuthority { &self.authority } #[must_use] pub const fn permitted_use(&self) -> &PermittedUse { &self.permitted_use } #[must_use] pub fn allowed_media(&self) -> &[CaptureMedium] { self.permitted_use.allowed_media() } #[must_use] pub fn allowed_processing(&self) -> &[CaptureProcessing] { self.permitted_use.allowed_processing() } #[must_use] pub const fn external_processing_allowed(&self) -> bool { self.permitted_use.external_processing_allowed() } #[must_use] pub const fn sharing_allowed(&self) -> bool { self.permitted_use.sharing_allowed() } #[must_use] pub const fn retention(&self) -> &RetentionTerms { &self.retention } #[must_use] pub fn conditions(&self) -> &[Condition] { &self.conditions } #[must_use] pub const fn conditions_digest(&self) -> &ContentDigest { &self.conditions_digest } #[must_use] pub const fn not_after(&self) -> u64 { self.not_after } pub(crate) const fn check_against( &self, valid_from: u64, valid_to: u64, ) -> Result<(), ConsentError> { if self.not_after > valid_to || self.not_after <= valid_from { return Err(ConsentError::GrantOutlivesScope); } Ok(()) } }";

/// The whole attestation surface, which hands back no authority.
const WHOLE_ATTESTATION: &str = "impl AttestationRecord { #[must_use] pub const fn file( kind: AttestationKind, heard_at: u64, conditions_digest: ContentDigest, ) -> Self { Self { kind, heard_at, conditions_digest, } } #[must_use] pub const fn kind(&self) -> AttestationKind { self.kind } #[must_use] pub const fn heard_at(&self) -> u64 { self.heard_at } #[must_use] pub const fn conditions_digest(&self) -> &ContentDigest { &self.conditions_digest } }";

/// The preview, which is also the one caller of the inheritance rule.
const WHOLE_PREVIEW: &str = "pub fn preview_expiry( ledger: &mut ConsentLedger, subject: &SubjectInventory, at: u64, ) -> DeletionImpact { let parent = subject.parent_terms; let audio = MediumImpact { bound: parent.audio(), object_count: subject.audio_objects, expires_now: parent.audio().is_expired_at(at), }; let transcript = MediumImpact { bound: parent.transcript(), object_count: subject.transcript_objects, expires_now: parent.transcript().is_expired_at(at), }; let derivatives = DERIVATIVE_CLASSES .iter() .map(|class| { let reported = subject .derivatives .iter() .find(|(named, _, _)| named == class); let (object_count, requested) = reported.map_or((0, parent), |(_, count, requested)| (*count, *requested)); let inherited = parent.inherit(requested); DerivativeImpact { class: *class, inherited, object_count, audio_expires_now: inherited.audio().is_expired_at(at), transcript_expires_now: inherited.transcript().is_expired_at(at), } }) .collect::<Vec<_>>(); let digest = impact_digest(subject, &audio, &transcript, &derivatives, at); ledger.record_expiry( ConsentEventKind::ExpiryPreviewed, subject.offering_id, subject.term, digest, at, ); DeletionImpact { offering_id: subject.offering_id, term: subject.term, permission_id: subject.permission_id, previewed_at: at, audio, transcript, derivatives, digest, } }";

/// The expiry action, which compares the previewed instant rather than trusting it.
const WHOLE_APPLY: &str = "pub fn apply_expiry( ledger: &mut ConsentLedger, plan: &ExpiryPlan, at: u64, ) -> Result<u64, ExpiryRefusal> { if plan.impact.previewed_at != at { return Err(ExpiryRefusal::PreviewIsForAnotherInstant); } let reached = plan.impact.objects_reached(); if reached == 0 { return Err(ExpiryRefusal::NothingHasExpired); } ledger.record_expiry( ConsentEventKind::ExpiryApplied, plan.impact.offering_id, plan.impact.term, plan.impact.digest, at, ); Ok(reached) }";

/// The plan's whole surface: one constructor, taking a preview.
const WHOLE_EXPIRY_PLAN: &str = "impl ExpiryPlan { #[must_use] pub fn from_preview(impact: DeletionImpact) -> Self { Self { impact } } #[must_use] pub const fn impact(&self) -> &DeletionImpact { &self.impact } }";

/// Every `impl` block in this crate whose header names `AuthorityGrant`.
///
/// One entry. A `From<AttestationRecord>`, a `TryFrom`, an `AsRef`, or a trait
/// nobody has thought of appears here as an extra key and fails. This is the
/// half the compiler does not do for a *later* author: the compiler refuses a
/// caller who passes an attestation today, and this refuses the commit that
/// would have made that call legal.
const AUTHORITY_GRANT_IMPL_BLOCKS: [&str; 1] = ["impl AuthorityGrant {"];

/// Every `impl` block in this crate whose header names `AttestationRecord`.
///
/// One entry, for the same reason from the other side: an attestation gains no
/// method and no trait that hands back an authority, a status, or a capability.
const ATTESTATION_IMPL_BLOCKS: [&str; 1] = ["impl AttestationRecord {"];

/// Every file that names `CaptureCapabilityToken` in product source, and why.
///
/// Compared as a whole inventory rather than searched for, so a second file
/// that reaches the token fails as an extra key and a removed one fails as a
/// missing key. The token is the microphone, so the question this answers is
/// "how many places in this crate can produce or read one".
const CAPABILITY_SITES: [(&str, &str); 2] = [
    (
        "crates/consent/src/capability.rs",
        "The type is declared here, its only struct literal is inside \
         `mint_capture_capability`, and `continue_capture` reads the request \
         back off it to re-bind. Nothing else in this crate builds one, which \
         is what the construction count beside this inventory holds.",
    ),
    (
        "crates/consent/src/lib.rs",
        "The crate root re-exports the type name so a caller can hold one. A \
         re-export is not a construction: `without_use_items` drops it before \
         the construction count runs.",
    ),
];

/// The seven dimensions, as the contract names them.
///
/// Read out of the enum rather than asserted about it, so a dimension added to
/// the source without being added here fails, and so does one dropped from
/// either side. `growth_descriptors_contain_no_scalar_score` in
/// `crates/domain/tests/question_graph.rs` reads its enum the same way.
const CHECKLIST_VARIANTS: [&str; 7] = [
    "SyllabusOrLmsPolicy",
    "StudentSpeech",
    "FilmingScope",
    "AccessibilityProcedure",
    "Copyright",
    "Privacy",
    "InstitutionalRules",
];

/// The five section 3.7 statuses.
const STATUS_VARIANTS: [&str; 5] = [
    "Unknown",
    "Prohibited",
    "Permitted",
    "PermittedWithConditions",
    "Expired",
];

/// Types whose presence in a return position means a permission was produced.
///
/// A signature that takes an attestation, or a legal question, and returns one
/// of these is the defect both of those rules exist to refuse.
const PERMITTING_RETURNS: [&str; 5] = [
    "AuthorityGrant",
    "WrittenAuthority",
    "BoundPermission",
    "CaptureCapabilityToken",
    "CaptureStatus",
];

#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let all = crate_all_sources()?;
    let product = crate_product_sources()?;
    assert!(
        all.len() >= 12,
        "the walk found only {} files, so it proved nothing",
        all.len()
    );
    assert!(
        product.len() >= 10,
        "the product walk found only {} files",
        product.len()
    );

    // Every module this crate declares is a file the walk read. A `mod name;`
    // without its own `#[path]` resolves beside its declarer, so this is a
    // tripwire on the walk: it fails the day the walk is narrowed, not today.
    // A `#[path]` target is the case that is reachable today, and it is the
    // shape `G-I13` used to put an exposure site outside a `src`-rooted walk.
    let read: BTreeSet<String> = all.iter().map(|path| relative(path)).collect();
    let mut declared = 0_usize;
    for path in &all {
        let code = code_of(path)?;
        let directory = path.parent().unwrap_or(Path::new("."));
        for line in code.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))
                && let Some(name) = rest.strip_suffix(';')
            {
                declared += 1;
                let beside = directory.join(format!("{name}.rs"));
                let nested = directory.join(name).join("mod.rs");
                assert!(
                    read.contains(&relative(&beside)) || read.contains(&relative(&nested)),
                    "{}: module {name} is not a file the walk read",
                    relative(path)
                );
            }
        }
        // `#[path]` is refused outright in this crate rather than resolved and
        // checked, which is the stronger of the two rules the repository uses:
        // `crates/untrusted-content` allows one whose target the walk read, and
        // this allows none. The check runs over stripped code so the attribute
        // survives and this file's own prose and diagnostic text -- both of
        // which spell it -- do not.
        assert_eq!(
            occurrences(&code, "#[path"),
            0,
            "{}: a #[path] module bypasses the walk",
            relative(path)
        );
    }
    assert!(
        declared >= 10,
        "only {declared} module declarations were seen"
    );

    // This crate's product source is under `src` and nowhere else, so a
    // `[[bin]]` or an `examples/` tree cannot become unread product code the
    // way `crates/record/examples/` did before `P2-RF10`.
    for path in &product {
        let relative_path = path.strip_prefix(crate_root()).unwrap_or(path);
        assert!(
            relative_path.starts_with("src"),
            "{}: product source outside src",
            relative(path)
        );
    }
    Ok(())
}

#[test]
fn the_capture_decision_is_one_binding_that_every_path_runs() -> TestResult {
    let capability = fs::read_to_string(crate_root().join("src/capability.rs"))?;
    assert_eq!(
        declared_item(&capability, "pub fn bind_permission(")?,
        WHOLE_BIND_PERMISSION,
        "bind_permission changed"
    );
    assert_eq!(
        declared_item(&capability, "impl<'a> ResolvedRequest<'a> {")?,
        WHOLE_RESOLVE_REQUEST,
        "the missing-field resolver changed"
    );
    // A pin on a decision says nothing about whether the decision runs. `T141`
    // wrapped a pinned call in a marker-file condition and every guard passed,
    // so both callers are pinned whole beside it.
    assert_eq!(
        declared_item(&capability, "pub fn mint_capture_capability(")?,
        WHOLE_MINT,
        "the minting path changed"
    );
    assert_eq!(
        declared_item(&capability, "pub fn continue_capture(")?,
        WHOLE_CONTINUE,
        "the continuation path changed"
    );

    // And a pin on the two callers says nothing about a third. The count is by
    // identifier with declarations subtracted, so `capability::bind_permission(..)`
    // written through the module path counts the same as a bare call --
    // `P2-RF10` reached a fourth site in another crate precisely by changing
    // the spelling of a call it did not change the meaning of.
    let mut binds = 0_usize;
    let mut denials = 0_usize;
    let mut mints = 0_usize;
    let mut tokens_built = 0_usize;
    let mut bindings_built = 0_usize;
    for path in crate_product_sources()? {
        let code = without_use_items(&code_of(&path)?);
        binds += calls_of(&code, "bind_permission");
        denials += calls_of(&code, "record_capture_denial");
        mints += calls_of(&code, "record_capture_mint");
        tokens_built += struct_literals(&code, "CaptureCapabilityToken");
        bindings_built += struct_literals(&code, "BoundPermission");
    }
    assert_eq!(binds, 2, "bind_permission has a caller that is not pinned");
    // Three, not two: `mint_capture_capability` has one refusing path and
    // `continue_capture` has two -- the binding failing, and the token's own
    // bound having moved. Every one of the three appends its row, which is what
    // this counts; a fourth refusal that returned without one would fail here.
    assert_eq!(denials, 3, "a refusing path does not append its audit row");
    assert_eq!(mints, 1, "a capability is minted somewhere unaudited");
    assert_eq!(
        tokens_built, 1,
        "a capability token is constructed outside the minting path"
    );
    assert_eq!(
        bindings_built, 1,
        "a binding witness is constructed outside bind_permission"
    );

    // The whole inventory of files that reach the token at all.
    let mut sites: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        if uses_of(&code_of(&path)?, "CaptureCapabilityToken") > 0 {
            sites.push(relative(&path));
        }
    }
    sites.sort();
    let mut expected: Vec<String> = CAPABILITY_SITES
        .iter()
        .map(|(file, _)| (*file).to_owned())
        .collect();
    expected.sort();
    assert_eq!(
        sites, expected,
        "the capability inventory and the source disagree"
    );
    for (file, reason) in CAPABILITY_SITES {
        assert!(reason.len() >= 80, "{file} has no written reason");
        assert!(workspace_root().join(file).is_file(), "{file} is gone");
    }
    Ok(())
}

#[test]
fn a_status_comes_from_one_derivation_and_absence_is_unknown() -> TestResult {
    let status = fs::read_to_string(crate_root().join("src/status.rs"))?;
    assert_eq!(
        declared_item(&status, "pub fn status_of(")?,
        WHOLE_STATUS_OF,
        "the status derivation changed"
    );
    assert_eq!(
        declared_item(&status, "impl CaptureStatus {")?,
        WHOLE_CAPTURE_STATUS,
        "the status surface changed"
    );

    // The variants, read out of the enum. A sixth status, or a renamed one,
    // fails here rather than silently widening `is_permitting`.
    let variants = enum_variants(&status, "pub enum CaptureStatus {");
    assert_eq!(variants, STATUS_VARIANTS, "the status set changed");

    // `Unknown` is the `Default`, so a value of this type that nobody set is
    // the refusing one. Reading the attribute rather than calling `default()`
    // is deliberate: the call would pass if a later edit moved the attribute
    // and this reads where it is.
    let default_line = status
        .lines()
        .position(|line| line.trim() == "#[default]")
        .ok_or("CaptureStatus has no #[default]")?;
    assert_eq!(
        status
            .lines()
            .nth(default_line + 1)
            .map(str::trim)
            .map(|line| line.trim_end_matches(',')),
        Some("Unknown"),
        "the default status is not Unknown"
    );

    // `status_of` runs on every path that reports a status: the binding, the
    // ledger query, and the append that records a permission. Nothing else
    // derives one, so there is no second reading of the same aggregate.
    let mut derivations = 0_usize;
    for path in crate_product_sources()? {
        let code = without_use_items(&code_of(&path)?);
        derivations += calls_of(&code, "status_of");
    }
    assert_eq!(
        derivations, 3,
        "a status is derived somewhere other than the three pinned readers"
    );

    // The two permitting variants are named outside `status.rs` only inside
    // `bind_permission`, which is pinned whole above. `status.rs` names them
    // through `Self::`, which this count does not reach.
    let mut permitting_mentions = 0_usize;
    for path in crate_product_sources()? {
        if path.ends_with("status.rs") {
            continue;
        }
        let code = code_of(&path)?;
        permitting_mentions += uses_of(&code, "CaptureStatus::Permitted")
            + uses_of(&code, "CaptureStatus::PermittedWithConditions");
    }
    assert_eq!(
        permitting_mentions, 2,
        "a permitting status is named outside the pinned binding"
    );
    Ok(())
}

#[test]
fn an_attestation_has_no_route_into_an_authority() -> TestResult {
    let evidence = fs::read_to_string(crate_root().join("src/evidence.rs"))?;
    assert_eq!(
        declared_item(&evidence, "impl AuthorityGrant {")?,
        WHOLE_AUTHORITY_GRANT,
        "the grant surface changed"
    );
    assert_eq!(
        declared_item(&evidence, "impl AttestationRecord {")?,
        WHOLE_ATTESTATION,
        "the attestation surface changed"
    );

    // The whole set of `impl` blocks naming each type, over the whole package.
    let mut grant_blocks: Vec<String> = Vec::new();
    let mut attestation_blocks: Vec<String> = Vec::new();
    for path in crate_all_sources()? {
        let code = code_of(&path)?;
        grant_blocks.extend(impl_headers_naming(&code, "AuthorityGrant"));
        attestation_blocks.extend(impl_headers_naming(&code, "AttestationRecord"));
    }
    grant_blocks.sort();
    attestation_blocks.sort();
    assert_eq!(
        grant_blocks,
        AUTHORITY_GRANT_IMPL_BLOCKS.to_vec(),
        "an impl block naming AuthorityGrant is not on the inventory"
    );
    assert_eq!(
        attestation_blocks,
        ATTESTATION_IMPL_BLOCKS.to_vec(),
        "an impl block naming AttestationRecord is not on the inventory"
    );

    // The workspace-wide half. `AttestationRecord` is a public type any crate
    // can name, so a conversion written one crate out would satisfy every rule
    // above; this is `no_public_signature_hands_out_ingested_text`'s shape,
    // applied to the other direction of the same mistake.
    let mut surface = 0_usize;
    let mut packages = BTreeSet::new();
    for path in workspace_product_sources()? {
        packages.insert(relative(&path).split('/').nth(1).unwrap_or("").to_owned());
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            surface = surface.saturating_add(1);
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if uses_of(parameters, "AttestationRecord") == 0 {
                continue;
            }
            for produced in PERMITTING_RETURNS {
                assert_eq!(
                    uses_of(returns, produced),
                    0,
                    "{}: a public signature turns an attestation into {produced}: {signature}",
                    relative(&path)
                );
            }
        }
    }
    assert!(
        packages.len() >= 25,
        "the workspace walk reached only {} packages",
        packages.len()
    );
    assert!(
        surface >= 1_200,
        "the workspace signature scan found only {surface} signatures, so it proved nothing"
    );
    Ok(())
}

#[test]
fn no_legal_conclusion_reaches_a_permission() -> TestResult {
    // A legal exception is an external task. The type carries no conclusion, so
    // the rule that keeps it that way is over signatures: nothing anywhere in
    // this workspace takes a legal question or an open review and returns a
    // permission-shaped value.
    let mut examined = 0_usize;
    for path in workspace_product_sources()? {
        let code = code_of(&path)?;
        for signature in public_signatures(&code) {
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if uses_of(parameters, "LegalQuestion") == 0
                && uses_of(parameters, "ExternalReviewTask") == 0
            {
                continue;
            }
            examined = examined.saturating_add(1);
            for produced in PERMITTING_RETURNS {
                assert_eq!(
                    uses_of(returns, produced),
                    0,
                    "{}: a legal question produces {produced}: {signature}",
                    relative(&path)
                );
            }
        }
    }
    assert!(
        examined >= 1,
        "no signature takes a legal question at all, so this proved nothing"
    );

    // And the type has no resolution surface to read a conclusion off.
    let external = fs::read_to_string(crate_root().join("src/external.rs"))?;
    let code = strip_non_code(&external);
    for forbidden in [
        "fn resolve",
        "fn conclude",
        "fn answer",
        "fn determination",
        "fn outcome",
        "fn decide",
    ] {
        assert_eq!(
            occurrences(&code, forbidden),
            0,
            "external.rs declares {forbidden}, which is a conclusion"
        );
    }
    Ok(())
}

#[test]
fn retention_holds_two_independent_bounds_and_narrows_only() -> TestResult {
    let retention = fs::read_to_string(crate_root().join("src/retention.rs"))?;
    assert_eq!(
        declared_item(&retention, "pub struct RetentionTerms {")?,
        WHOLE_RETENTION_TERMS_STRUCT,
        "the retention struct changed"
    );
    assert_eq!(
        declared_item(&retention, "impl RetentionTerms {")?,
        WHOLE_RETENTION_TERMS,
        "the retention surface changed"
    );
    assert_eq!(
        declared_item(&retention, "impl RetentionBound {")?,
        WHOLE_RETENTION_BOUND,
        "the bound ordering changed"
    );

    // Two fields, both bounds, different names. A struct with one field, or
    // with two fields of different types, fails here before any behaviour does.
    let fields: Vec<(String, String)> = retention
        .lines()
        .skip_while(|line| !line.contains("pub struct RetentionTerms {"))
        .skip(1)
        .take_while(|line| !line.starts_with('}'))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//") && !line.starts_with('#'))
        .filter_map(|line| {
            let (name, kind) = line.trim_end_matches(',').split_once(": ")?;
            Some((name.to_owned(), kind.to_owned()))
        })
        .collect();
    assert_eq!(
        fields,
        vec![
            ("audio".to_owned(), "RetentionBound".to_owned()),
            ("transcript".to_owned(), "RetentionBound".to_owned()),
        ],
        "the two retention bounds are no longer two independent fields"
    );

    // The collapse that still compiles is an accessor returning its sibling.
    // Applying it to the pinned text here is the observation that the pin
    // catches it: a mutated body is not the pinned body.
    let collapsed = WHOLE_RETENTION_TERMS.replace(
        "pub const fn transcript(self) -> RetentionBound { self.transcript }",
        "pub const fn transcript(self) -> RetentionBound { self.audio }",
    );
    assert_ne!(
        collapsed, WHOLE_RETENTION_TERMS,
        "the mutation did not apply, so this proved nothing"
    );
    assert_ne!(
        declared_item(&retention, "impl RetentionTerms {")?,
        collapsed,
        "a collapsed accessor would pass the pin"
    );

    // The same for the direction of the inheritance comparison: one character
    // reverses it and the pin is what refuses the character.
    let widened = WHOLE_RETENTION_TERMS.replace(
        "audio: self.audio.stricter(requested.audio)",
        "audio: self.audio.max(requested.audio)",
    );
    assert_ne!(widened, WHOLE_RETENTION_TERMS, "the mutation did not apply");
    assert_ne!(
        declared_item(&retention, "impl RetentionTerms {")?,
        widened,
        "a widening inheritance would pass the pin"
    );

    // One inheritance function, one caller. A second copy of the rule is how
    // the two would drift apart.
    let mut inherits = 0_usize;
    for path in crate_product_sources()? {
        let code = without_use_items(&code_of(&path)?);
        inherits += calls_of(&code, "inherit");
    }
    assert_eq!(
        inherits, 1,
        "a derivative's retention is computed somewhere other than the one rule"
    );
    Ok(())
}

#[test]
fn the_checklist_is_the_seven_dimensions_the_contract_names() -> TestResult {
    let checklist = fs::read_to_string(crate_root().join("src/checklist.rs"))?;
    let variants = enum_variants(&checklist, "pub enum ChecklistDimension {");
    assert_eq!(
        variants, CHECKLIST_VARIANTS,
        "the checklist dimensions changed"
    );

    // The registry array holds every variant, once, in declaration order. A
    // dimension in the enum and not in the array would be invisible to
    // `unanswered`, which is the only thing that makes an omission cost
    // anything.
    let registry: Vec<String> = checklist
        .lines()
        .skip_while(|line| !line.contains("pub const CHECKLIST_DIMENSIONS:"))
        .skip(1)
        .take_while(|line| !line.starts_with(']'))
        .map(str::trim)
        .filter_map(|line| line.strip_prefix("ChecklistDimension::"))
        .map(|line| line.trim_end_matches(',').to_owned())
        .collect();
    assert_eq!(
        registry, CHECKLIST_VARIANTS,
        "the registry array and the enum disagree"
    );
    assert!(
        checklist.contains("pub const CHECKLIST_DIMENSIONS: [ChecklistDimension; 7]"),
        "the registry array is no longer seven long"
    );

    // An entry is evidenced or explicitly not applicable. A third arm meaning
    // "unknown" would make an omission indistinguishable from a decision, which
    // is the whole reason the two arms exist.
    let arms = enum_variants(&checklist, "pub enum ChecklistEntry {");
    assert_eq!(
        arms,
        vec![
            "Evidenced(crate::evidence::EvidenceArtifact)".to_owned(),
            "NotApplicable(NotApplicableReason)".to_owned(),
        ],
        "the checklist entry arms changed"
    );
    Ok(())
}

#[test]
fn an_expiry_cannot_be_applied_without_its_preview() -> TestResult {
    let expiry = fs::read_to_string(crate_root().join("src/expiry.rs"))?;
    assert_eq!(
        declared_item(&expiry, "pub fn preview_expiry(")?,
        WHOLE_PREVIEW,
        "the preview changed"
    );
    assert_eq!(
        declared_item(&expiry, "pub fn apply_expiry(")?,
        WHOLE_APPLY,
        "the expiry action changed"
    );
    assert_eq!(
        declared_item(&expiry, "impl ExpiryPlan {")?,
        WHOLE_EXPIRY_PLAN,
        "the plan surface changed"
    );

    // One constructor, one struct literal. A plan built any other way would be
    // an expiry nobody previewed.
    let mut plans_built = 0_usize;
    let mut appliers = 0_usize;
    for path in crate_product_sources()? {
        let code = without_use_items(&code_of(&path)?);
        plans_built += struct_literals(&code, "ExpiryPlan");
        appliers += calls_of(&code, "apply_expiry");
    }
    // Zero, because the one constructor writes `Self { impact }` inside the
    // pinned `impl ExpiryPlan` block. A literal spelling the type name would be
    // a second construction site outside that pin, and this is what refuses it;
    // the field is private, so a construction outside this crate is a compile
    // error rather than something a scan has to find.
    assert_eq!(
        plans_built, 0,
        "an ExpiryPlan is built outside from_preview"
    );
    assert_eq!(appliers, 0, "nothing inside this crate applies an expiry");

    // The preview walks the whole registry rather than the entries a caller
    // happened to report, so a class with nothing in it is a node saying so.
    let code = strip_non_code(&expiry);
    assert!(
        uses_of(&code, "DERIVATIVE_CLASSES") >= 1,
        "the preview no longer walks the class registry"
    );
    Ok(())
}

#[test]
fn the_two_derivative_vocabularies_are_the_same_list() -> TestResult {
    // This crate restates `academic-retention`'s class list because a product
    // edge to that crate is refused by `rotation_engine_lane_is_not_default`:
    // linking it links a crate that can destroy a key slot, and a consent
    // ledger has no business inside that boundary. The restatement is therefore
    // compared rather than trusted, through a dev edge that reaches no product
    // binary.
    //
    // Both the order and the spellings are compared. The order matters because
    // both sides promise "one node per class, in registry order", and two lists
    // holding the same set in different orders would produce two reports of the
    // same deletion.
    let here: Vec<&str> = DERIVATIVE_CLASSES
        .iter()
        .map(|class| class.as_str())
        .collect();
    let there: Vec<&str> = academic_retention::plan::DERIVATIVE_CLASSES
        .iter()
        .map(|class| class.as_str())
        .collect();
    assert_eq!(
        here, there,
        "the consent and retention derivative-class lists have drifted"
    );
    assert_eq!(here.len(), 7, "the class list is no longer seven long");

    // The variant names are compared too, out of both sources. A class renamed
    // on one side while its spelling stayed would pass the comparison above.
    let mine = enum_variants(
        &fs::read_to_string(crate_root().join("src/expiry.rs"))?,
        "pub enum DerivativeClass {",
    );
    let theirs = enum_variants(
        &fs::read_to_string(workspace_root().join("crates/retention/src/plan.rs"))?,
        "pub enum DerivativeClass {",
    );
    assert_eq!(
        mine, theirs,
        "the two class enums declare different variants"
    );

    // And the dev edge is a dev edge. A product edge here would be the thing
    // `rotation_engine_lane_is_not_default` refuses, and this fails in this
    // crate rather than only in the workspace-wide manifest map.
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml"))?;
    let declarations: Vec<&str> = manifest
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect();
    let section = |name: &str| -> Vec<String> {
        declarations
            .iter()
            .skip_while(|line| line.trim() != name)
            .skip(1)
            .take_while(|line| !line.trim_start().starts_with('['))
            .map(|line| (*line).trim().to_owned())
            .filter(|line| !line.is_empty())
            .collect()
    };
    let product = section("[dependencies]");
    assert!(
        !product
            .iter()
            .any(|line| line.contains("academic-retention")),
        "academic-retention is a product edge of this crate"
    );
    assert!(
        section("[dev-dependencies]")
            .iter()
            .any(|line| line.contains("academic-retention")),
        "the dev edge that makes the comparison above possible is gone"
    );
    Ok(())
}

#[test]
fn the_migration_vocabularies_are_the_rust_ones() -> TestResult {
    let sql = fs::read_to_string(
        workspace_root().join("migrations/store/0006_phase2_consent_and_capture.sql"),
    )?;
    let src = |name: &str| fs::read_to_string(crate_root().join(format!("src/{name}")));

    // Each vocabulary is checked in both directions. The `as_str` spellings are
    // required to be exactly the SQL `CHECK` list, so a value the Rust side can
    // produce and the database refuses fails here rather than at a first
    // insert; and the enum's variant count is compared against the list length,
    // so a variant added with no `as_str` arm cannot pass by having no spelling
    // to compare.
    let vocabularies: [(&str, &str, &str, Vec<String>); 8] = [
        (
            "status",
            "status.rs",
            "pub enum CaptureStatus {",
            [
                CaptureStatus::Unknown,
                CaptureStatus::Prohibited,
                CaptureStatus::Permitted,
                CaptureStatus::PermittedWithConditions,
                CaptureStatus::Expired,
            ]
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        ),
        (
            "grant_authority",
            "evidence.rs",
            "pub enum GrantAuthority {",
            [
                GrantAuthority::Instructor,
                GrantAuthority::Institution,
                GrantAuthority::AccessibilityAccommodation,
            ]
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        ),
        (
            "evidence_kind",
            "evidence.rs",
            "pub enum WrittenEvidenceKind {",
            [
                WrittenEvidenceKind::Syllabus,
                WrittenEvidenceKind::LmsPolicy,
                WrittenEvidenceKind::Correspondence,
                WrittenEvidenceKind::Announcement,
                WrittenEvidenceKind::InstitutionalRule,
                WrittenEvidenceKind::AccessibilityDetermination,
            ]
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        ),
        (
            "medium",
            "permission.rs",
            "pub enum CaptureMedium {",
            [
                CaptureMedium::Audio,
                CaptureMedium::PhotoOfBoard,
                CaptureMedium::ScreenCapture,
                CaptureMedium::Video,
            ]
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        ),
        (
            "processing",
            "permission.rs",
            "pub enum CaptureProcessing {",
            [
                CaptureProcessing::LocalStt,
                CaptureProcessing::LocalOcr,
                CaptureProcessing::ExternalStt,
                CaptureProcessing::ExternalSummarisation,
            ]
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        ),
        (
            "dimension",
            "checklist.rs",
            "pub enum ChecklistDimension {",
            CHECKLIST_DIMENSIONS
                .iter()
                .map(|value| value.as_str().to_owned())
                .collect(),
        ),
        (
            "not_applicable_reason",
            "checklist.rs",
            "pub enum NotApplicableReason {",
            [
                NotApplicableReason::NoStudentParticipationIsCaptured,
                NotApplicableReason::NoVisualCaptureRequested,
                NotApplicableReason::NoAccommodationInEffect,
                NotApplicableReason::MaterialIsTheUsersOwn,
                NotApplicableReason::NoThirdPartyPersonalData,
                NotApplicableReason::InstitutionPublishesNoApplicableRule,
            ]
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        ),
        (
            "event_kind",
            "ledger.rs",
            "pub enum ConsentEventKind {",
            [
                ConsentEventKind::EvidenceRecorded,
                ConsentEventKind::AttestationRecorded,
                ConsentEventKind::PermissionGranted,
                ConsentEventKind::PermissionProhibited,
                ConsentEventKind::ExternalReviewOpened,
                ConsentEventKind::RecheckQueued,
                ConsentEventKind::CaptureCapabilityMinted,
                ConsentEventKind::CaptureCapabilityDenied,
                ConsentEventKind::ExpiryPreviewed,
                ConsentEventKind::ExpiryApplied,
            ]
            .iter()
            .map(|value| value.as_str().to_owned())
            .collect(),
        ),
    ];

    for (column, file, header, mut spellings) in vocabularies {
        spellings.sort();
        assert_eq!(
            sql_check_list(&sql, column),
            spellings,
            "the {column} CHECK list and the Rust spellings disagree"
        );
        let variants = enum_variants(&src(file)?, header);
        assert_eq!(
            variants.len(),
            spellings.len(),
            "{header} has {} variants and {column} lists {} spellings",
            variants.len(),
            spellings.len()
        );
    }

    // The two retention axes are four columns, not one. This is the schema half
    // of `audio_and_transcript_retention_are_independent`: a migration that
    // collapsed them would leave the Rust type intact and the durable record
    // unable to hold what it says.
    for column in [
        "audio_retention_kind",
        "audio_retention_until",
        "transcript_retention_kind",
        "transcript_retention_until",
    ] {
        // A column declaration is a line that opens with the column name and
        // its storage class. A `CHECK` that mentions the same name opens with
        // the name and an operator, so this counts declarations rather than
        // mentions -- which is the difference between "the schema has two
        // retention axes" and "the schema talks about two retention axes".
        let declared = sql
            .lines()
            .map(str::trim_start)
            .filter(|line| {
                ["TEXT", "INTEGER", "BLOB"]
                    .iter()
                    .any(|kind| line.starts_with(&format!("{column} {kind}")))
            })
            .count();
        assert_eq!(declared, 1, "{column} is not declared exactly once");
    }
    assert_eq!(
        sql_check_list(&sql, "audio_retention_kind"),
        sql_check_list(&sql, "transcript_retention_kind"),
        "the two axes admit different bound kinds"
    );
    assert!(
        !sql.contains("retention_until INTEGER NOT NULL"),
        "a retention axis lost its PROHIBITED spelling"
    );

    // Section 3.7's key, and its two defaults of zero.
    assert!(
        sql.contains("UNIQUE (offering_id, permission_seq)"),
        "the section 3.7 key is not unique"
    );
    for column in ["external_processing_allowed", "sharing_allowed"] {
        assert!(
            sql.contains(&format!("{column} INTEGER NOT NULL DEFAULT 0")),
            "{column} does not default to the refusing value"
        );
    }

    // Every table this migration creates is in the authorizer's canonical set
    // and carries the append-only trigger pair. The store crate's
    // `authorizer_covers_every_canonical_table` compares those two layers
    // against each other; this compares both against the file.
    let tables: Vec<&str> = sql
        .lines()
        .filter_map(|line| line.strip_prefix("CREATE TABLE "))
        .map(|line| line.trim_end_matches(" ("))
        .collect();
    assert_eq!(
        tables.len(),
        5,
        "the migration creates {} tables",
        tables.len()
    );
    let authorizer = fs::read_to_string(workspace_root().join("crates/store/src/authorizer.rs"))?;
    for table in tables {
        assert!(
            authorizer.contains(&format!("\"{table}\"")),
            "{table} is missing from the authorizer canonical set"
        );
        for action in ["update", "delete"] {
            assert!(
                sql.contains(&format!("CREATE TRIGGER guard_{table}_{action}")),
                "{table} has no {action} guard"
            );
        }
    }
    Ok(())
}

#[test]
fn every_instant_this_crate_compares_is_an_argument() -> TestResult {
    // The contract says this crate reads no clock, and the reason it matters is
    // determinism: every acceptance row above compares a status at a named
    // instant, and one `SystemTime::now()` inside the decision would make the
    // expiry rows depend on when the suite ran.
    //
    // This is a token list, which `docs/contracts/policy-source-scans.md` warns
    // about — but it is a complete one rather than a list of spellings somebody
    // predicted. `std::time` is the whole of the standard library's clock, and
    // its two entry points are `SystemTime` and `Instant`; this crate's only
    // product dependency is `academic-domain`, whose own surface is types and
    // digests, so there is no third route to reach for. A crate added to the
    // product edge later is the case this would not see, and
    // `workspace_dependency_direction_is_acyclic` is what fails then.
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        scanned = scanned.saturating_add(1);
        let code = code_of(&path)?;
        for spelling in ["SystemTime", "Instant", "UNIX_EPOCH", "std::time", "chrono"] {
            assert_eq!(
                uses_of(&code, spelling),
                0,
                "{}: this crate reads a clock ({spelling})",
                relative(&path)
            );
        }
    }
    assert!(scanned >= 10, "the clock scan read only {scanned} files");

    // The positive half: `now` and `at` are parameters. Every function that
    // compares an instant takes one, which is why the fixtures can name every
    // instant they assert against.
    let status = fs::read_to_string(crate_root().join("src/status.rs"))?;
    assert!(
        declared_item(&status, "pub fn status_of(")?.contains("at: u64"),
        "the status derivation no longer takes its instant as an argument"
    );
    Ok(())
}
