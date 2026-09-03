//! Source scans for the `P2-L1` capture device gate.
//!
//! Five of this crate's claims are shapes of the source rather than behaviours,
//! so nothing at run time would notice the day they stopped being true. The
//! fifth is `T161`'s: a chunk reaches a manifest from one place, and that place
//! is the one that compares its instant.
//! `docs/contracts/policy-source-scans.md` is the page those scans are
//! enumerated on, and this file is written against all five of the empty-scan
//! shapes it names.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends the whole
//! package, not `src` by name, with a floor, a `mod`/`#[path]` tripwire, and a
//! rule that this crate's product source is under `src` and `probes` and
//! nowhere else. The probe is the reason the second directory is named: a walk
//! rooted at `src` would never read the one file in this crate that opens a
//! device.
//!
//! **The checks are not token lists.** The three that could have been are whole
//! sets: the `impl` blocks naming [`QuarantinedArtifact`], the signatures in
//! this crate whose return type names a byte, and the files holding an `unsafe`
//! item. A trait implementation nobody predicted fails as an extra key.
//!
//! **The pins fix their callers.** `WHOLE_OPEN_DEVICE`, `WHOLE_RECORD_CHUNK`
//! and `WHOLE_SEAL` are pinned beside the counts of the three consent calls
//! they make, because `T141` found a pinned check skipped by a condition
//! wrapped around it and `P2-RF10` found a second public path that never called
//! one.
//!
//! **Every inventory counts an identifier, not a spelling.** `P2-RF10` reached
//! a fourth exposure site in another crate by writing `Untrusted::expose(d)`
//! instead of `d.expose()`. The counts here are whole-identifier counts with
//! declarations subtracted.
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

use academic_capture_gate::{DEVICE_CLASSES, REFUSAL_REASONS};

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

/// Every `.rs` file anywhere under this crate's package.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships: everything outside `tests`.
///
/// `probes` is product source here even though it is not in any default build,
/// because it is the file that opens a device and it is exactly the file a walk
/// rooted at `src` would miss. `S-12` on the scans page is that shape.
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

/// Every `.rs` file under every workspace package, less each package's `tests`.
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
            if relative.starts_with("tests") {
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
/// Copied from `crates/consent/tests/consent_scans.rs`, raw strings and nested
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

/// How many times `name` is called in `code`, declarations subtracted.
fn calls_of(code: &str, name: &str) -> usize {
    uses_of(code, name).saturating_sub(occurrences(code, &format!("fn {name}(")))
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

/// The lines a braced item declares, in declaration order.
///
/// For an `enum` those are its variants and for a `struct` they are its
/// fields, so the same reader serves both: what a variant list keeps whole is
/// the vocabulary, and what a field list keeps whole is the visibility --
/// a `pub` on a private field opens the type to a literal written anywhere.
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

/// Every function `code` declares, at any visibility, whitespace-collapsed.
///
/// [`public_signatures`] reads only `pub` items, and a second path into a
/// private field does not have to be `pub` to reach one: Rust's field privacy
/// is per module, so any function in the same file can touch it. This reads
/// every one, so a whole-set comparison over a file fails on a function nobody
/// reviewed whatever its visibility is.
fn declared_functions(code: &str) -> Vec<String> {
    const MODIFIERS: [&str; 5] = ["pub", "const", "async", "unsafe", "extern"];
    let lines: Vec<&str> = code.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let Some(before) = trimmed.split_once("fn ").map(|(head, _)| head) else {
            continue;
        };
        if !before
            .split_whitespace()
            .all(|word| MODIFIERS.iter().any(|modifier| word.starts_with(modifier)))
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

/// Every `impl` block header in `code` that names `type_name`.
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

/// One file, as code with comments and literals removed.
fn code_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(strip_non_code(&fs::read_to_string(path)?))
}

/// The relative path of `path` under the crate, with forward slashes.
fn relative_to_crate(path: &Path) -> String {
    path.strip_prefix(crate_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---------------------------------------------------------------------------
// Whole-text pins. Each is compared against the item as the source declares it,
// comment lines dropped and whitespace collapsed, so `cargo fmt` decides layout
// and the pin decides content. What editing one costs is in
// `docs/contracts/policy-source-scans.md`.
// ---------------------------------------------------------------------------

const WHOLE_AUTHORIZE: &str = "pub fn authorize( ledger: &mut ConsentLedger, audit: &mut CaptureAudit, request: &CaptureRequest, now: u64, ) -> Result<CaptureAuthorization, CaptureRefusal> { let subject = AuditSubject { offering_id: request.offering_id, lecture_id: request.lecture_id, digest: None, }; let token = match mint_capture_capability(ledger, request, now) { Ok(token) => token, Err(denial) => { return Err(audit.record_refusal( CaptureRefusal::from_denial(denial, None), subject, now, )); } }; let ruleset = DeviceRuleset::for_token(&token); Ok(CaptureAuthorization { token, ruleset }) }";

const WHOLE_OPEN_DEVICE: &str = "pub fn open_device( ledger: &mut ConsentLedger, audit: &mut CaptureAudit, authorization: CaptureAuthorization, class: DeviceClass, layer: DeviceLayer, now: u64, ) -> Result<CaptureSession, CaptureRefusal> { let token = authorization.token(); let subject = AuditSubject { offering_id: Some(token.bound().offering_id()), lecture_id: Some(token.bound().lecture_id()), digest: Some(*token.token_id()), }; if layer == DeviceLayer::Unavailable { return Err(audit.record_refusal( CaptureRefusal::of(CaptureRefusalReason::DeviceLayerUnavailable, Some(class)), subject, now, )); } if !authorization.ruleset().permits(class) { return Err(audit.record_refusal( CaptureRefusal::of(CaptureRefusalReason::MediumNotOnToken, Some(class)), subject, now, )); } if let Err(denial) = continue_capture(ledger, token, now) { return Err(audit.record_refusal( CaptureRefusal::from_denial(denial, Some(class)), subject, now, )); } let offering_id = token.bound().offering_id(); let lecture_id = token.bound().lecture_id(); let retention = token.bound().retention(); Ok(CaptureSession { token: authorization.into_token(), class, layer, offering_id, lecture_id, retention, accepted_at: now, chunks: Vec::new(), bytes: Vec::new(), gap: None, }) }";

const WHOLE_RECORD_CHUNK: &str = "pub fn record_chunk( &mut self, ledger: &mut ConsentLedger, audit: &mut CaptureAudit, bytes: &[u8], now: u64, ) -> Result<(), CaptureRefusal> { let subject = self.subject(); if self.gap.is_some() { return Err(audit.record_refusal( CaptureRefusal::of( CaptureRefusalReason::SessionAlreadyStopped, Some(self.class), ), subject, now, )); } if now < self.accepted_at { return Err(audit.record_refusal( CaptureRefusal::of(CaptureRefusalReason::ChunkOutOfOrder, Some(self.class)), subject, now, )); } if let Err(denial) = continue_capture(ledger, &self.token, now) { self.gap = Some(TimelineGap::opened( now, CaptureRefusalReason::PermissionRefused, Some(denial.reason()), )); return Err(audit.record_refusal( CaptureRefusal::from_denial(denial, Some(self.class)), subject, now, )); } let seq = u32::try_from(self.chunks.len()).unwrap_or(u32::MAX); self.chunks.push(ChunkRecord::build( seq, now, bytes.len(), ContentDigest::sha256(bytes), )); self.bytes.extend_from_slice(bytes); self.accepted_at = now; Ok(()) } pub fn seal( self, ledger: &ConsentLedger, audit: &mut CaptureAudit, now: u64, ) -> CaptureArtifact { let digest = ContentDigest::sha256(&self.bytes); let byte_len = self.bytes.len(); let subject = AuditSubject { offering_id: Some(self.offering_id), lecture_id: Some(self.lecture_id), digest: Some(digest), }; let violation = self.first_unbound_chunk(ledger); let class = self.class; let manifest = CaptureArtifact::manifest_of(self.chunks, byte_len, digest, self.retention, self.gap); match violation { Some((risk, denial)) => { let _ = audit.record_refusal( CaptureRefusal::from_denial(denial, Some(class)), subject, now, ); CaptureArtifact::quarantined(manifest, risk) } None => CaptureArtifact::releasable(manifest, self.bytes), } } fn first_unbound_chunk( &self, ledger: &ConsentLedger, ) -> Option<(ViolationRisk, CaptureDenial)> { for chunk in &self.chunks { if let Err(denial) = bind_permission(ledger, self.token.request(), chunk.started_at()) { return Some(( ViolationRisk::raised( chunk.seq(), chunk.started_at(), denial.reason(), denial.status(), ), denial, )); } } None } fn subject(&self) -> AuditSubject { AuditSubject { offering_id: Some(self.offering_id), lecture_id: Some(self.lecture_id), digest: Some(*self.token.token_id()), } } }";

const WHOLE_SEAL: &str = "pub fn seal( self, ledger: &ConsentLedger, audit: &mut CaptureAudit, now: u64, ) -> CaptureArtifact { let digest = ContentDigest::sha256(&self.bytes); let byte_len = self.bytes.len(); let subject = AuditSubject { offering_id: Some(self.offering_id), lecture_id: Some(self.lecture_id), digest: Some(digest), }; let violation = self.first_unbound_chunk(ledger); let class = self.class; let manifest = CaptureArtifact::manifest_of(self.chunks, byte_len, digest, self.retention, self.gap); match violation { Some((risk, denial)) => { let _ = audit.record_refusal( CaptureRefusal::from_denial(denial, Some(class)), subject, now, ); CaptureArtifact::quarantined(manifest, risk) } None => CaptureArtifact::releasable(manifest, self.bytes), } } fn first_unbound_chunk( &self, ledger: &ConsentLedger, ) -> Option<(ViolationRisk, CaptureDenial)> { for chunk in &self.chunks { if let Err(denial) = bind_permission(ledger, self.token.request(), chunk.started_at()) { return Some(( ViolationRisk::raised( chunk.seq(), chunk.started_at(), denial.reason(), denial.status(), ), denial, )); } } None } fn subject(&self) -> AuditSubject { AuditSubject { offering_id: Some(self.offering_id), lecture_id: Some(self.lecture_id), digest: Some(*self.token.token_id()), } } }";

const WHOLE_FIRST_UNBOUND: &str = "fn first_unbound_chunk( &self, ledger: &ConsentLedger, ) -> Option<(ViolationRisk, CaptureDenial)> { for chunk in &self.chunks { if let Err(denial) = bind_permission(ledger, self.token.request(), chunk.started_at()) { return Some(( ViolationRisk::raised( chunk.seq(), chunk.started_at(), denial.reason(), denial.status(), ), denial, )); } } None } fn subject(&self) -> AuditSubject { AuditSubject { offering_id: Some(self.offering_id), lecture_id: Some(self.lecture_id), digest: Some(*self.token.token_id()), } } }";

const WHOLE_RELEASABLE_BYTES: &str = "pub fn releasable_bytes<'artifact>( artifact: &'artifact CaptureArtifact, audit: &mut CaptureAudit, now: u64, ) -> Result<&'artifact [u8], CaptureRefusal> { match artifact { CaptureArtifact::Releasable(releasable) => Ok(releasable.bytes()), CaptureArtifact::Quarantined(quarantined) => Err(audit.record_refusal( CaptureRefusal::of(CaptureRefusalReason::ArtifactQuarantined, None), AuditSubject { offering_id: None, lecture_id: None, digest: Some(*quarantined.manifest().digest()), }, now, )), } }";

const WHOLE_DEVICE_CLASS_OF: &str = "pub const fn of(medium: CaptureMedium) -> Option<Self> { match medium { CaptureMedium::Audio => Some(Self::Microphone), CaptureMedium::PhotoOfBoard | CaptureMedium::Video => Some(Self::Camera), CaptureMedium::ScreenCapture => Some(Self::Screen), _ => None, } } }";

const WHOLE_FOR_TOKEN: &str = "pub fn for_token(token: &CaptureCapabilityToken) -> Self { let mut classes = Vec::new(); let mut unclassified = Vec::new(); for medium in token.media() { match DeviceClass::of(*medium) { Some(class) => classes.push(class), None => unclassified.push(*medium), } } classes.sort_unstable(); classes.dedup(); unclassified.sort_unstable(); unclassified.dedup(); Self { classes, unclassified, } } #[must_use] pub fn permits(&self, class: DeviceClass) -> bool { self.classes.contains(&class) } #[must_use] pub fn classes(&self) -> &[DeviceClass] { &self.classes } #[must_use] pub fn unclassified(&self) -> &[CaptureMedium] { &self.unclassified } #[must_use] pub fn is_empty(&self) -> bool { self.classes.is_empty() } }";

const WHOLE_RECORD_REFUSAL: &str = "pub(crate) fn record_refusal( &mut self, refusal: CaptureRefusal, subject: AuditSubject, now: u64, ) -> CaptureRefusal { self.rows.push(CaptureAuditRow { reason: refusal.reason(), denial_reason: refusal.denial_reason(), status: refusal.status(), class: refusal.class(), offering_id: subject.offering_id, lecture_id: subject.lecture_id, subject_digest: subject.digest, recorded_at: now, }); refusal } #[must_use] pub fn rows(&self) -> &[CaptureAuditRow] { &self.rows } #[must_use] pub fn count_of(&self, reason: CaptureRefusalReason) -> usize { self.rows.iter().filter(|row| row.reason == reason).count() } }";

const WHOLE_PROBE_ATTEMPT: &str = "fn attempt(target: &str) -> String { match fs::File::open(target) { Ok(handle) => { drop(handle); String::from(\"OPENED\") } Err(error) => format!(\"REFUSED {}\", error.raw_os_error().unwrap_or(-1)), } }";

/// The whole set of `impl` blocks naming the quarantined arm.
///
/// One entry. A `Deref<Target = [u8]>`, an `AsRef<[u8]>`, a `Borrow`, or any
/// other trait that hands out the bytes appears here as an extra key. An `impl`
/// written in another crate is refused by the orphan rule instead, because both
/// the trait and the type would be foreign there.
const QUARANTINED_IMPL_BLOCKS: [&str; 1] = ["impl QuarantinedArtifact {"];

/// The whole set of functions `src/session.rs` declares, at any visibility.
///
/// A chunk is appended by exactly one of them, and the comparison that keeps
/// the manifest timeline forwards is inside that one's pinned text. **A pin
/// fixes a body, not the set of bodies.** `WHOLE_RECORD_CHUNK` runs from
/// `record_chunk` to the end of its `impl` block, so a second function written
/// *above* it in the same block sits outside every pin in this file -- and a
/// function in this file can reach `CaptureSession`'s private fields, because
/// Rust's field privacy is per module. This set is what fails then: an extra
/// key, whatever it is called and whether or not it is `pub`.
///
/// `C-11` is why the rule exists. The defect it records was the comparison
/// being absent; the shape that would bring it back is a path that appends
/// without it.
const SESSION_FUNCTIONS: [&str; 12] = [
    "pub fn open_device( ledger: &mut ConsentLedger, audit: &mut CaptureAudit, authorization: CaptureAuthorization, class: DeviceClass, layer: DeviceLayer, now: u64, ) -> Result<CaptureSession, CaptureRefusal> {",
    "pub const fn class(&self) -> DeviceClass {",
    "pub const fn layer(&self) -> DeviceLayer {",
    "pub const fn token_id(&self) -> &ContentDigest {",
    "pub const fn not_after(&self) -> u64 {",
    "pub fn chunk_count(&self) -> usize {",
    "pub const fn gap(&self) -> Option<TimelineGap> {",
    "pub fn record_chunk( &mut self, ledger: &mut ConsentLedger, audit: &mut CaptureAudit, bytes: &[u8], now: u64, ) -> Result<(), CaptureRefusal> {",
    "pub fn seal( self, ledger: &ConsentLedger, audit: &mut CaptureAudit, now: u64, ) -> CaptureArtifact {",
    "fn first_unbound_chunk( &self, ledger: &ConsentLedger, ) -> Option<(ViolationRisk, CaptureDenial)> {",
    "fn subject(&self) -> AuditSubject {",
    "pub fn releasable_bytes<'artifact>( artifact: &'artifact CaptureArtifact, audit: &mut CaptureAudit, now: u64, ) -> Result<&'artifact [u8], CaptureRefusal> {",
];

/// `CaptureSession`'s fields, and every one of them is private.
///
/// `accepted_at` is the highest instant the session has accepted and it is what
/// the ordering comparison reads. A `pub` on it would let a caller in any crate
/// lower it back, and a `pub` on all of them would write the struct literal
/// `tests/compile_fail/capture_session_has_no_public_constructor.rs` refuses.
/// Neither is a new function, so neither is something `SESSION_FUNCTIONS` can
/// see.
const SESSION_FIELDS: [&str; 10] = [
    "token: CaptureCapabilityToken",
    "class: DeviceClass",
    "layer: DeviceLayer",
    "offering_id: OfferingId",
    "lecture_id: LectureSessionId",
    "retention: RetentionTerms",
    "accepted_at: u64",
    "chunks: Vec<ChunkRecord>",
    "bytes: Vec<u8>",
    "gap: Option<TimelineGap>",
];

/// Which of `the_capture_gate_appends_a_chunk_from_one_place`'s rules an
/// evasion sample is written against.
///
/// Naming it is the difference between "something caught this" and "the rule
/// this sample exists to test caught it". Three of the four rules could do
/// nothing at all and a sample that trips any one of them would still pass an
/// `any`-shaped assertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Caught {
    /// The whole set of functions `src/session.rs` declares.
    ExtraFunction,
    /// `CaptureSession`'s field list, and that none of them is `pub`.
    PublicField,
    /// The call-site counts on the constructors a chunk reaches a manifest by.
    ExtraSite,
    /// The rule that none of those types is renamed on a `use`.
    Aliased,
}

/// The crate-private constructors a chunk passes through on its way to a
/// manifest, each of which must be reached from exactly one site.
///
/// `record_chunk` is the only place that compares an instant, and it is only
/// the only place a chunk is recorded while these are reached from it alone.
/// Each is `pub(crate)`, so the set of modules that could call one is the whole
/// crate rather than the file that declares it -- and `CaptureManifest`'s
/// fields are private, so a second path has to go through `manifest_of` to
/// assemble one at all.
const CHUNK_CONSTRUCTORS: [(&str, &str); 4] = [
    ("ChunkRecord", "build"),
    ("CaptureArtifact", "manifest_of"),
    ("CaptureArtifact", "releasable"),
    ("CaptureArtifact", "quarantined"),
];

/// The types a `use` may not rename in this crate's product source.
///
/// Every count above is on a path, and `Row::build(..)` after
/// `use ChunkRecord as Row` spells none of them. `academic-capture`'s
/// `SessionClock as ` rule is the precedent.
const UNALIASED_TYPES: [&str; 3] = [
    "ChunkRecord as ",
    "CaptureArtifact as ",
    "CaptureSession as ",
];

/// The whole set of signatures in this crate whose return type names a byte.
///
/// Two, and each is guarded in a different way. `ReleasableArtifact::bytes` is
/// reachable only from the releasable arm, which a quarantined capture is not.
/// `releasable_bytes` takes the sum type and is the one function a caller
/// holding a sealed capture calls; it returns the bytes for one arm and an
/// audited refusal for the other, and its whole text is pinned above.
///
/// This is the whole of "a quarantined artefact hands out no bytes" as a
/// property of the source, and it is a set rather than a search so an accessor
/// nobody predicted fails as an extra key.
const BYTE_RETURNING_SIGNATURES: [&str; 2] = [
    "pub fn bytes(&self) -> &[u8] {",
    "pub fn releasable_bytes<'artifact>( artifact: &'artifact CaptureArtifact, audit: &mut \
     CaptureAudit, now: u64, ) -> Result<&'artifact [u8], CaptureRefusal> {",
];

/// The files in this crate that may hold an `unsafe` item.
///
/// Two, and both are platform backends. The default lane keeps the workspace's
/// `forbid` in practice because nothing else in the crate is allowed one.
const UNSAFE_FILES: [&str; 2] = ["src/native/linux.rs", "src/native/windows.rs"];

/// The three syscalls the Linux backend installs the ruleset with.
///
/// `only_egress_crate_has_a_socket` carries the same list on its allowance for
/// that file. A fourth name, or a bare number, fails both.
const LINUX_SYSCALLS: [&str; 3] = [
    "SYS_landlock_create_ruleset",
    "SYS_landlock_add_rule",
    "SYS_landlock_restrict_self",
];

/// Section 3.7's four media, read from the consent crate and compared against
/// what `DeviceClass::of` classifies.
const CAPTURE_MEDIA: [&str; 4] = ["Audio", "PhotoOfBoard", "ScreenCapture", "Video"];

/// The device classes this crate declares, in declaration order.
///
/// Read out of the enum by the scan and compared against this, and against
/// `DEVICE_CLASSES`, so the three lists cannot drift apart.
const DEVICE_CLASS_VARIANTS: [&str; 3] = ["Microphone", "Camera", "Screen"];

/// The types a signature must not return when it takes a quarantined artefact.
const BYTE_TYPES: [&str; 3] = ["u8", "str", "String"];

#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let all = crate_all_sources()?;
    let product = crate_product_sources()?;
    assert!(
        all.len() >= 8,
        "the crate walk read {} files; it has stopped short",
        all.len()
    );
    assert!(
        product.len() >= 8,
        "the product walk read {} files",
        product.len()
    );

    // Product source lives under `src` and `probes` and nowhere else. A module
    // beside them is `S-12`'s shape, and the tripwire below is the other half.
    for path in &product {
        let relative = relative_to_crate(path);
        assert!(
            relative.starts_with("src/") || relative.starts_with("probes/"),
            "{relative} is product source outside src/ and probes/"
        );
    }

    // Every `mod name;` and every `#[path = "..."]` target resolves to a file
    // the walk read. This fails the day the walk is narrowed.
    let read: BTreeSet<String> = all.iter().map(|path| relative_to_crate(path)).collect();
    let mut declared = 0_usize;
    for path in &all {
        // Read as code rather than as text: this file names the `#[path]`
        // spelling inside a string literal, and a check that read the raw text
        // would fire on the scan itself.
        let source = code_of(path)?;
        assert!(
            !source.contains("#[path"),
            "{} declares a #[path] module; the walk cannot follow one",
            relative_to_crate(path)
        );
        let directory = path.parent().map(relative_to_crate).unwrap_or_default();
        for line in source.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))
            else {
                continue;
            };
            let Some(name) = rest.strip_suffix(';') else {
                continue;
            };
            declared += 1;
            let leaf = if directory.is_empty() {
                format!("{name}.rs")
            } else {
                format!("{directory}/{name}.rs")
            };
            let nested = if directory.is_empty() {
                format!("{name}/mod.rs")
            } else {
                format!("{directory}/{name}/mod.rs")
            };
            assert!(
                read.contains(&leaf) || read.contains(&nested),
                "module {name} declared in {} is a file the walk did not read",
                relative_to_crate(path)
            );
        }
    }
    assert!(
        declared >= 6,
        "only {declared} modules were declared; the tripwire is not bounding anything"
    );
    Ok(())
}

/// Every refusal this crate returns is a refusal it appended a row for.
///
/// The mechanism is that a `CaptureRefusal` is only constructed as an argument
/// to `record_refusal`, which returns it. So the two counts are equal, and a
/// path that builds a refusal and returns it without a row makes them unequal.
/// `academic-consent`'s `record_capture_denial` is the same shape and the same
/// count.
#[test]
fn the_capture_gate_records_every_refusal_it_returns() -> TestResult {
    let audit = fs::read_to_string(crate_root().join("src/audit.rs"))?;
    assert_eq!(
        declared_item(&audit, "    pub(crate) fn record_refusal(")?,
        WHOLE_RECORD_REFUSAL,
        "the audit append changed"
    );

    let mut constructed = 0_usize;
    let mut recorded = 0_usize;
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        let code = without_use_items(&code_of(&path)?);
        scanned += 1;
        constructed += occurrences(&code, "CaptureRefusal::of(")
            + occurrences(&code, "CaptureRefusal::from_denial(");
        recorded += calls_of(&code, "record_refusal");
    }
    assert!(scanned >= 8, "only {scanned} product files were read");
    assert_eq!(
        constructed, recorded,
        "{constructed} refusals are built and {recorded} rows are appended; a refusing path \
         returns without a row"
    );
    assert!(
        constructed >= REFUSAL_REASONS.len(),
        "fewer refusals are built ({constructed}) than there are reasons to build one"
    );
    Ok(())
}

/// The three consent calls, pinned whole with their call sites counted.
///
/// A pin on a decision says nothing about whether the decision runs. `T141`
/// wrapped a pinned call in a marker-file condition and every guard passed, so
/// the callers are pinned beside it and counted.
#[test]
fn the_capture_gate_re_runs_the_binding_on_every_path() -> TestResult {
    let daemon = fs::read_to_string(crate_root().join("src/daemon.rs"))?;
    let session = fs::read_to_string(crate_root().join("src/session.rs"))?;
    assert_eq!(
        declared_item(&daemon, "pub fn authorize(")?,
        WHOLE_AUTHORIZE,
        "the daemon evaluation changed"
    );
    assert_eq!(
        declared_item(&session, "pub fn open_device(")?,
        WHOLE_OPEN_DEVICE,
        "the device open changed"
    );
    assert_eq!(
        declared_item(&session, "    pub fn record_chunk(")?,
        WHOLE_RECORD_CHUNK,
        "the boundary check changed"
    );
    assert_eq!(
        declared_item(&session, "    pub fn seal(")?,
        WHOLE_SEAL,
        "the seal changed"
    );
    assert_eq!(
        declared_item(&session, "    fn first_unbound_chunk(")?,
        WHOLE_FIRST_UNBOUND,
        "the chunk reconciliation changed"
    );
    assert_eq!(
        declared_item(&session, "pub fn releasable_bytes")?,
        WHOLE_RELEASABLE_BYTES,
        "the one place a sealed capture is asked for bytes changed"
    );

    // The counts. `continue_capture` twice -- once where the device opens and
    // once per chunk -- `bind_permission` once, in the reconciliation, and
    // `mint_capture_capability` once, in the daemon evaluation. A third path to
    // any of them has to edit this count.
    let mut mints = 0_usize;
    let mut continues = 0_usize;
    let mut binds = 0_usize;
    for path in crate_product_sources()? {
        let code = without_use_items(&code_of(&path)?);
        mints += calls_of(&code, "mint_capture_capability");
        continues += calls_of(&code, "continue_capture");
        binds += calls_of(&code, "bind_permission");
    }
    assert_eq!(mints, 1, "there are {mints} minting sites, not one");
    assert_eq!(
        continues, 2,
        "there are {continues} boundary re-checks, not two"
    );
    assert_eq!(binds, 1, "there are {binds} reconciliation sites, not one");
    Ok(())
}

/// A chunk reaches a manifest from one place, and that place compares its
/// instant.
///
/// `C-11` was that comparison missing. It is now inside `record_chunk`'s pinned
/// text, and a pin says nothing about whether a *second* path exists beside the
/// pinned one -- which is the lesson `T141` and `P2-RF10` both left. So the two
/// whole sets and the four counts here are about the set of paths rather than
/// about any one body:
///
/// | Rule | The path it refuses |
/// |---|---|
/// | the function set of `src/session.rs` | a second appender in the one file that can reach a session's private fields |
/// | the field set of `CaptureSession` | a `pub` field that lets a caller lower the mark, or write the literal |
/// | `ChunkRecord::build` at one site | a chunk record built anywhere else in the crate |
/// | `CaptureArtifact::manifest_of`, `::releasable`, `::quarantined` at one site each | a manifest assembled from chunks no session ordered |
/// | no `use` alias on the three types | a rename that spells none of the counted identifiers |
#[test]
fn the_capture_gate_appends_a_chunk_from_one_place() -> TestResult {
    let session = code_of(&crate_root().join("src/session.rs"))?;
    assert_eq!(
        declared_functions(&session),
        SESSION_FUNCTIONS,
        "the set of functions declared beside the one that appends a chunk changed"
    );
    let fields = enum_variants(&session, "pub struct CaptureSession {");
    assert_eq!(
        fields, SESSION_FIELDS,
        "the session's field list changed; the ordering mark is one of them"
    );
    for field in &fields {
        assert!(
            !field.starts_with("pub"),
            "{field} is public; the session's state is reachable from outside the crate"
        );
    }

    // The counts. Each is a whole-identifier count with `fn <name>(`
    // subtracted, for `P2-RF10`'s reason: a spelling count is defeated by a
    // spelling nobody listed. **Both spellings of the path are counted**: a
    // constructor called from inside its own `impl` block writes `Self::`, and
    // injection `T-I5` is the second assembly path that walked past the first
    // version of this rule by doing exactly that.
    let mut counted = [0_usize; CHUNK_CONSTRUCTORS.len()];
    let mut building_files = BTreeSet::new();
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        let code = without_use_items(&code_of(&path)?);
        scanned = scanned.saturating_add(1);
        for (index, (owner, name)) in CHUNK_CONSTRUCTORS.iter().enumerate() {
            let here = calls_of(&code, &format!("{owner}::{name}"))
                + calls_of(&code, &format!("Self::{name}"));
            if here > 0 {
                building_files.insert(relative_to_crate(&path));
            }
            counted[index] = counted[index].saturating_add(here);
        }
    }
    assert!(scanned >= 8, "only {scanned} product files were read");
    for (index, (owner, name)) in CHUNK_CONSTRUCTORS.iter().enumerate() {
        assert_eq!(
            counted[index], 1,
            "{owner}::{name} is reached from {} sites, not one",
            counted[index]
        );
    }
    assert_eq!(
        building_files,
        ["src/session.rs".to_owned()].into_iter().collect(),
        "a chunk reaches a manifest from a file other than the one holding the comparison"
    );

    // The imports, read with the `use` items kept: a rename is the way to call
    // a counted identifier without spelling it. A `type` alias is the other,
    // and it is not a `use` item, so it is read in the same pass.
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        for alias in UNALIASED_TYPES {
            assert_eq!(
                occurrences(&code, alias),
                0,
                "{}: `{alias}` renames a type the counts above read",
                relative_to_crate(&path)
            );
        }
        for (owner, _) in CHUNK_CONSTRUCTORS {
            assert_eq!(
                occurrences(&code, &format!("= {owner};")),
                0,
                "{}: a type alias renames {owner}",
                relative_to_crate(&path)
            );
        }
    }

    // The evasions, each run through the rules above, and each naming the rule
    // that has to be the one that catches it. Asserting only that *something*
    // caught a sample would pass while three of the four rules did nothing.
    let evasions: [(&str, &str, Caught); 4] = [
        (
            "a second appender above the pinned one, taking a record somebody else built",
            "impl CaptureSession {\n    \
             fn note(&mut self, row: ChunkRecord) {\n        self.chunks.push(row);\n    }\n}",
            Caught::ExtraFunction,
        ),
        (
            "a public field that lets a caller lower the mark",
            "pub struct CaptureSession {\n    token: CaptureCapabilityToken,\n    \
             pub accepted_at: u64,\n}",
            Caught::PublicField,
        ),
        (
            "a manifest assembled from chunks no session ordered, inside the type's own impl \
             block, so it spells `Self` and none of the counted paths",
            "pub(crate) fn rebuilt(rows: Vec<ChunkRecord>, bytes: Vec<u8>) -> Self {\n    \
             Self::releasable(Self::manifest_of(rows, 0, digest, terms, None), bytes)\n}",
            Caught::ExtraSite,
        ),
        (
            "the same build reached through a use alias, which spells no counted identifier",
            "use crate::artifact::ChunkRecord as Row;\n\
             fn subject(&self) -> AuditSubject {\n    \
             self.chunks.push(Row::build(0, 0, 0, digest));\n}",
            Caught::Aliased,
        ),
    ];
    for (name, sample, expected) in evasions {
        let code = strip_non_code(sample);
        let without_uses = without_use_items(&code);
        let fires = |rule: Caught| match rule {
            Caught::ExtraFunction => declared_functions(&code)
                .iter()
                .any(|signature| !SESSION_FUNCTIONS.contains(&signature.as_str())),
            Caught::PublicField => enum_variants(&code, "pub struct CaptureSession {")
                .iter()
                .any(|field| field.starts_with("pub")),
            Caught::ExtraSite => CHUNK_CONSTRUCTORS.iter().any(|(owner, name)| {
                calls_of(&without_uses, &format!("{owner}::{name}"))
                    + calls_of(&without_uses, &format!("Self::{name}"))
                    > 0
            }),
            Caught::Aliased => UNALIASED_TYPES
                .iter()
                .any(|alias| occurrences(&code, alias) > 0),
        };
        assert!(
            fires(expected),
            "the evasion `{name}` was not caught by {expected:?}, the rule it was written against"
        );
    }

    // And the rules are not vacuous against the real file.
    assert!(session.contains("ChunkRecord::build("));
    assert!(session.contains("self.accepted_at"));
    Ok(())
}

/// A quarantined artefact hands out no bytes, and nothing in the workspace adds
/// a signature that does.
#[test]
fn no_public_signature_hands_out_a_quarantined_capture() -> TestResult {
    let artifact = fs::read_to_string(crate_root().join("src/artifact.rs"))?;
    let headers = impl_headers_naming(&artifact, "QuarantinedArtifact");
    assert_eq!(
        headers, QUARANTINED_IMPL_BLOCKS,
        "the set of impl blocks naming QuarantinedArtifact changed"
    );

    // The whole set of byte-returning signatures in this crate.
    let mut byte_returning = Vec::new();
    let mut scanned = 0_usize;
    for path in crate_product_sources()? {
        scanned += 1;
        for signature in public_signatures(&fs::read_to_string(&path)?) {
            let Some((_, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if uses_of(returns, "u8") > 0 {
                byte_returning.push(signature.trim().to_owned());
            }
        }
    }
    assert!(scanned >= 8, "only {scanned} product files were read");
    assert_eq!(
        byte_returning, BYTE_RETURNING_SIGNATURES,
        "the set of signatures returning bytes changed"
    );

    // And the workspace-wide half. `QuarantinedArtifact` is a public type any
    // crate can name, so the rule that refuses a byte accessor has to reach
    // every package. This is `no_public_signature_hands_out_ingested_text`
    // applied to the other quarantine.
    let mut packages = BTreeSet::new();
    let mut signatures = 0_usize;
    for path in workspace_product_sources()? {
        if let Some(package) = path
            .strip_prefix(workspace_root().join("crates"))
            .ok()
            .and_then(|rest| rest.components().next())
        {
            packages.insert(package.as_os_str().to_string_lossy().into_owned());
        }
        let source = fs::read_to_string(&path)?;
        for signature in public_signatures(&source) {
            signatures += 1;
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if uses_of(parameters, "QuarantinedArtifact") == 0 {
                continue;
            }
            for byte_type in BYTE_TYPES {
                assert_eq!(
                    uses_of(returns, byte_type),
                    0,
                    "{} hands a {byte_type} out of a quarantined capture: {signature}",
                    path.display()
                );
            }
        }
    }
    assert!(
        packages.len() >= 25,
        "only {} packages were walked",
        packages.len()
    );
    assert!(
        signatures >= 1_200,
        "only {signatures} public signatures were read"
    );
    Ok(())
}

/// Every section 3.7 medium has a device class, and the map is read out of the
/// consent crate rather than restated here.
#[test]
fn every_capture_medium_is_classified() -> TestResult {
    let device = fs::read_to_string(crate_root().join("src/device.rs"))?;
    assert_eq!(
        declared_item(&device, "    pub const fn of(medium: CaptureMedium)")?,
        WHOLE_DEVICE_CLASS_OF,
        "the medium-to-device map changed"
    );
    assert_eq!(
        declared_item(&device, "    pub fn for_token(")?,
        WHOLE_FOR_TOKEN,
        "the one ruleset constructor changed"
    );

    // The media are read out of the enum the other crate declares, so a fifth
    // one added there fails here rather than falling into the wildcard arm.
    let permission = fs::read_to_string(workspace_root().join("crates/consent/src/permission.rs"))?;
    let mut variants = enum_variants(&permission, "pub enum CaptureMedium {");
    variants.sort();
    assert_eq!(
        variants,
        CAPTURE_MEDIA.to_vec(),
        "CaptureMedium's variants changed; DeviceClass::of has a wildcard arm and a medium \
         that falls into it opens nothing, so the classification has to be revisited"
    );
    for medium in CAPTURE_MEDIA {
        assert!(
            uses_of(WHOLE_DEVICE_CLASS_OF, medium) > 0,
            "{medium} is not named in the classification"
        );
    }

    // The wildcard is `None`, not a device. A `_ => Some(..)` would open one for
    // a medium nobody classified.
    assert!(
        WHOLE_DEVICE_CLASS_OF.contains("_ => None"),
        "the unclassified arm is not fail-closed"
    );

    // And the device classes this crate declares are the ones the map produces.
    // The variant list is read out of the enum rather than compared against
    // `DEVICE_CLASSES`, for `L-I15b`'s reason: comparing an array against
    // itself is true of any array, and a class dropped from `DEVICE_CLASSES`
    // would leave every loop that walks it one class short and still green.
    let declared = enum_variants(&device, "pub enum DeviceClass {");
    assert_eq!(
        declared,
        DEVICE_CLASS_VARIANTS.to_vec(),
        "DeviceClass's variants changed"
    );
    assert_eq!(
        DEVICE_CLASSES.len(),
        declared.len(),
        "DEVICE_CLASSES holds {} of {} declared classes",
        DEVICE_CLASSES.len(),
        declared.len()
    );
    for (index, class) in DEVICE_CLASSES.iter().enumerate() {
        assert_eq!(
            format!("{class:?}"),
            declared[index],
            "DEVICE_CLASSES is not the enum's declaration order"
        );
        assert!(
            uses_of(&device, class.as_str()) > 0,
            "{class:?} has no spelling in device.rs"
        );
    }
    Ok(())
}

/// `unsafe` is confined to the two platform backends.
#[test]
fn unsafe_is_confined_to_the_device_backends() -> TestResult {
    let mut holding = Vec::new();
    let mut scanned = 0_usize;
    for path in crate_all_sources()? {
        let code = code_of(&path)?;
        scanned += 1;
        if uses_of(&code, "unsafe") > 0 {
            holding.push(relative_to_crate(&path));
        }
    }
    assert!(scanned >= 8, "only {scanned} files were read");
    holding.sort();
    assert_eq!(
        holding,
        UNSAFE_FILES.to_vec(),
        "the set of files holding an unsafe item changed"
    );
    Ok(())
}

/// The Linux backend names three syscalls and no fourth, and every
/// `libc::syscall(` call names one of them as its first argument.
///
/// This is `only_egress_crate_has_a_socket`'s rule for
/// `crates/worker/src/sandbox/linux.rs`, applied to this file with this file's
/// own three-name list. `P2-RF11` is why the first-argument half exists: a bare
/// `libc::syscall(41, 2, 1, 0)` opens an `AF_INET` socket, spells no forbidden
/// name, and compiles clean.
#[test]
fn the_linux_backend_names_only_the_three_syscalls_it_installs() -> TestResult {
    let path = crate_root().join("src/native/linux.rs");
    let code = code_of(&path)?;

    // Every `SYS_` spelling in the file is one of the three.
    let mut index = 0_usize;
    let mut seen = BTreeSet::new();
    while let Some(at) = code[index..].find("SYS_") {
        let start = index + at;
        let rest = &code[start..];
        let end = rest
            .find(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .unwrap_or(rest.len());
        let name = &rest[..end];
        assert!(
            LINUX_SYSCALLS.contains(&name),
            "{name} is a syscall this file names and this scan has not reviewed"
        );
        seen.insert(name.to_owned());
        index = start + end;
    }
    assert_eq!(
        seen.len(),
        LINUX_SYSCALLS.len(),
        "the file names {} of its three syscalls",
        seen.len()
    );

    // Every `libc::syscall(` call names one of them as its first argument.
    let calls = occurrences(&code, "libc::syscall(");
    assert!(calls >= 4, "only {calls} syscall sites; the file has four");
    let mut cursor = 0_usize;
    let mut checked = 0_usize;
    while let Some(at) = code[cursor..].find("libc::syscall(") {
        let start = cursor + at + "libc::syscall(".len();
        let rest = &code[start..];
        let first = rest
            .split(',')
            .next()
            .unwrap_or("")
            .trim()
            .trim_start_matches("libc::");
        assert!(
            LINUX_SYSCALLS.iter().any(|name| first.starts_with(name)),
            "a syscall site's first argument is `{first}`, which is not one of the three"
        );
        checked += 1;
        cursor = start;
    }
    assert_eq!(checked, calls, "not every syscall site was read");

    // And no file in this crate imports `libc::syscall`, so a call has to spell
    // the path and reach the rule above.
    for source in crate_all_sources()? {
        let text = code_of(&source)?;
        for line in text.lines() {
            let trimmed = line.trim_start();
            if !trimmed.starts_with("use ") && !trimmed.starts_with("extern crate ") {
                continue;
            }
            assert!(
                uses_of(line, "syscall") == 0,
                "{} imports syscall: {line}",
                relative_to_crate(&source)
            );
        }
    }
    Ok(())
}

/// The probe opens a handle and reads no sample, and it is in no default build.
#[test]
fn the_probe_opens_a_handle_and_reads_no_sample() -> TestResult {
    let probe_path = crate_root().join("probes/capture_probe.rs");
    let probe = fs::read_to_string(&probe_path)?;
    assert_eq!(
        declared_item(&probe, "fn attempt(target: &str)")?,
        WHOLE_PROBE_ATTEMPT,
        "the probe's device open changed"
    );

    // Nothing in the probe reads from what it opened. These are the shapes a
    // read takes; the handle is dropped and that is the whole of it.
    let code = code_of(&probe_path)?;
    for reader in [
        "read_to_end",
        "read_to_string",
        "read_exact",
        "BufReader",
        "copy",
    ] {
        assert_eq!(
            uses_of(&code, reader),
            0,
            "the probe names {reader}; it must open a handle and drop it"
        );
    }

    // The manifest keeps it out of every default build.
    let manifest = fs::read_to_string(crate_root().join("Cargo.toml"))?;
    assert!(
        manifest.contains("required-features = [\"native-capture\"]"),
        "the probe target has no required-features"
    );
    assert!(
        manifest.contains("path = \"probes/capture_probe.rs\""),
        "the probe target has no explicit path outside src"
    );
    assert!(
        manifest.contains("default = []"),
        "the default feature set is not empty"
    );
    Ok(())
}
