//! Source scans for `P2-R1`.
//!
//! `docs/contracts/policy-source-scans.md` is this repository's inventory of
//! files that read another file's Rust source text; this is one of them, and
//! it is written against the five shapes that page says make a scan empty.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends from the
//! package root rather than into `src` by name, has a floor, and carries a
//! tripwire requiring every `mod name;` and every `#[path = "…"]` target in the
//! package to be a file the walk read. That is `S-12` on the page: a walk that
//! reads `<crate>/src` misses a `[[bin]]` whose `path` is outside it.
//!
//! **The checks are not token lists.** The claims this crate makes are pinned
//! as *whole text* — [`WHOLE_CAPTURE`] is the stage order, [`WHOLE_RUN_GATE`]
//! is the gate, [`WHOLE_SECRET_FINDING`] is the whole of what a secret finding
//! can be asked for — and the two inventories that could have been token lists
//! are whole sets instead: [`USE_ITEMS`] is every `use` in the crate's product
//! code and [`FILESYSTEM_CALLS`] is every `fs::` name it spells, both compared
//! in both directions. A filesystem write appears in one of them as an extra
//! key whatever it is called.
//!
//! **The pins fix their callers too.** A pin on a body says nothing about
//! whether the body runs, or about whether a second body exists beside it, so
//! [`CALL_SITE_COUNTS`] counts each guarded name's call sites over every file
//! the walk read and names the one file each may be called from. `P2-RF10` and
//! `P2-RF11` are both about exactly that gap.
//!
//! **Every inventory counts an identifier, not a spelling.** [`calls_of`]
//! counts a whole identifier and subtracts that name's own declarations, so
//! `LocalStages::inventory(self, ..)` written through the type path is the same
//! call as `self.inventory(..)`, and a function whose name merely starts with a
//! guarded one does not cancel it.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
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
///
/// The package directory rather than `src`: `S-12` in
/// `docs/contracts/policy-source-scans.md` is the walk that reads `<crate>/src`
/// and stops seeing a target whose `path` is outside it.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships, which is every one outside `tests`.
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

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/untrusted-content/tests/trust_scans.rs`, which copied it
/// from `crates/record/tests/record_scans.rs`, raw strings and nested block
/// comments included. `P2-G4` found that a lexer without raw strings
/// desynchronizes and reads every literal after one as code, so the copy is
/// deliberate rather than a simplification.
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

/// One free function's text, from its signature to the `}` at column zero.
fn free_function(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find("\n}")
        .ok_or_else(|| format!("{signature} has no closing brace at column zero"))?;
    Ok(collapse(&source[start..start + end + 2]))
}

/// One brace-balanced block's text, from `header` to its matching `}`.
///
/// A block rather than a line range, because an `impl` block's closing brace
/// sits at column zero only for a free item; the `impl` blocks this pins are
/// what an added method appears inside.
fn whole_block(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("{header} opens no block"))?;
    let mut depth = 0_usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth = depth.saturating_add(1),
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(collapse(&source[start..open + offset + 1]));
                }
            }
            _ => (),
        }
    }
    Err(format!("{header} is not brace-balanced").into())
}

/// Drops comment lines and collapses whitespace.
fn collapse(body: &str) -> String {
    let kept: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    kept.join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
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

/// Counts declarations of a function whose name is exactly `name`.
///
/// What follows the name has to open a parameter list or a generic list and
/// nothing else, so `fn inventory_later(` is not `inventory` and
/// `fn freeze<'a>(` still is. `T149` walked through the version that read the
/// declaration as a spelling: one function whose name merely starts with the
/// guarded one cancelled its own call.
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

/// The relative path of `path` under the workspace, with forward slashes.
fn relative(path: &Path) -> String {
    path.strip_prefix(workspace_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// One file of this crate, raw.
fn source_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?)
}

/// This crate's product files, as code with comments and literals removed.
fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut found = Vec::new();
    for path in crate_product_sources()? {
        found.push((relative(&path), strip_non_code(&fs::read_to_string(&path)?)));
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// The pins.
// ---------------------------------------------------------------------------

/// Section 17.3's stage order, whole.
///
/// The gate first, its result deciding whether anything else runs, then the
/// inventory, the freeze and the index. A reordering, a removed early return,
/// or a second call to any stage edits this constant.
const WHOLE_CAPTURE: &str = "pub fn capture<S: SnapshotStages + ?Sized>( stages: &mut S, request: &SnapshotRequest<'_>, ) -> Result<Capture, RepositoryError> { let admitted = stages.permission_and_secret_gate(request)?; if admitted.result() == SecretScanResult::Blocked { return Err(RepositoryError::SecretGateBlocked); } let listed = stages.inventory(request, &admitted)?; let snapshot = stages.freeze(request, listed)?; let receipt = stages.index(&snapshot)?; Ok(Capture { snapshot, receipt }) }";

/// The gate, whole: the policy runs over every path before a byte is read, and
/// the content scan runs over exactly what survived.
const WHOLE_RUN_GATE: &str = "pub(crate) fn run_gate( request_digest: ContentDigest, policy: &PathPolicy, paths: &[String], read: &mut dyn FnMut(&str) -> Result<Vec<u8>, RepositoryError>, ) -> Result<AdmittedPaths, RepositoryError> { let mut admitted = Vec::new(); let mut excluded = Vec::new(); for path in paths { match policy.classify(path) { Some(reason) => excluded.push(ExcludedPath { path: path.clone(), reason, }), None => admitted.push(path.clone()), } } admitted.sort(); excluded.sort(); let mut findings = Vec::new(); let mut opaque = Vec::new(); for path in &admitted { let bytes = read(path)?; match scan_secrets(path, &bytes) { ContentVerdict::Clean => (), ContentVerdict::Secret(finding) => findings.push(finding), ContentVerdict::Opaque => opaque.push(path.clone()), } } let result = if findings.is_empty() { SecretScanResult::Pass } else { SecretScanResult::Blocked }; Ok(AdmittedPaths::admit( request_digest, admitted, opaque, excluded, findings, result, )) }";

/// The one derivation of a snapshot type, whole.
///
/// The six version-controlled arms read the tree's dirtiness rather than the
/// request's name. An arm moved out of that group would record a dirty tree as
/// its commit, which is what section 17.2 forbids.
const WHOLE_RESOLVE_SNAPSHOT_TYPE: &str = "pub(crate) fn resolve_snapshot_type( source: RepositorySource, facts: &WorkingTreeFacts, ) -> SnapshotType { match source { RepositorySource::Archive => SnapshotType::Archive, RepositorySource::SpecOnly => SnapshotType::SpecOnly, RepositorySource::LocalDirectory | RepositorySource::GitHubPublic | RepositorySource::GitHubPrivate | RepositorySource::Branch | RepositorySource::Commit | RepositorySource::DirtyWorktree => { if facts.is_dirty() { SnapshotType::DirtyWorktree } else { SnapshotType::GitCommit } } } }";

/// The five detectors and the fail-closed arm above them, whole.
const WHOLE_SCAN_SECRETS: &str = "fn scan_secrets(path: &str, bytes: &[u8]) -> ContentVerdict { if bytes.len() > MAX_SCANNED_BYTES { return ContentVerdict::Opaque; } let Ok(text) = core::str::from_utf8(bytes) else { return ContentVerdict::Opaque; }; if text.contains(\"-----BEGIN \") && text.contains(\" PRIVATE KEY-----\") { return ContentVerdict::Secret(SecretFinding::new( path.to_owned(), ReasonCode::SecretPattern, )); } for prefix in [\"AKIA\", \"ASIA\", \"ghp_\", \"github_pat_\", \"xoxb-\", \"sk-\"] { if text.contains(prefix) { return ContentVerdict::Secret(SecretFinding::new( path.to_owned(), ReasonCode::SecretPattern, )); } } for scheme in [\"postgres://\", \"postgresql://\", \"mysql://\", \"mongodb://\"] { if let Some(rest) = text.split(scheme).nth(1) && let Some(authority) = rest.split('/').next() && authority.contains(':') && authority.contains('@') { return ContentVerdict::Secret(SecretFinding::new( path.to_owned(), ReasonCode::SecretPattern, )); } } for name in [\"api_key\", \"apikey\", \"secret\", \"password\", \"token\"] { for line in text.lines() { let lowered = line.to_ascii_lowercase(); let Some(at) = lowered.find(name) else { continue; }; let tail = lowered.get(at.saturating_add(name.len())..).unwrap_or(\"\"); let assigns = tail.trim_start().starts_with(['=', ':']) || tail.trim_start().starts_with(\"\\\" :\"); if assigns && longest_secret_run(line).len() >= 16 { return ContentVerdict::Secret(SecretFinding::new( path.to_owned(), ReasonCode::SecretPattern, )); } } } let run = longest_secret_run(text); if run.len() >= 32 && distinct_characters(run) >= 24 { return ContentVerdict::Secret(SecretFinding::new( path.to_owned(), ReasonCode::SecretEntropy, )); } ContentVerdict::Clean }";

/// Everything a secret finding can be asked for, and the two places its digest
/// field is written.
///
/// The whole `impl` block, so a setter, a `Default`, a second constructor, or
/// an accessor that computed a digest on demand appears here rather than
/// slipping past a list of forbidden names. `blob_digest: None` and
/// `blob_digest: Some(ContentDigest::of(bytes))` are the only two assignments,
/// and the second is inside the method that takes a [`DisclosureDecision`].
const WHOLE_SECRET_FINDING: &str = "impl SecretFinding { #[must_use] pub(crate) fn new(path: String, reason: ReasonCode) -> Self { Self { path, reason, blob_digest: None, disclosure: None, } } #[must_use] pub fn disclose(self, decision: DisclosureDecision, bytes: &[u8]) -> Self { Self { path: self.path, reason: self.reason, blob_digest: Some(ContentDigest::of(bytes)), disclosure: Some(decision), } } #[must_use] pub fn path(&self) -> &str { &self.path } #[must_use] pub const fn reason(&self) -> ReasonCode { self.reason } #[must_use] pub const fn blob_digest(&self) -> Option<&ContentDigest> { self.blob_digest.as_ref() } #[must_use] pub const fn disclosure(&self) -> Option<&DisclosureDecision> { self.disclosure.as_ref() } }";

/// The three configured rule sources and section 32.4's file defaults, in the
/// order they are applied, whole.
const WHOLE_PATH_POLICY_CLASSIFY: &str = "pub fn classify(&self, path: &str) -> Option<ExclusionReason> { if !self.allow.is_empty() && !self.allow.iter().any(|rule| rule.matches(path)) { return Some(ExclusionReason::DenyRule); } if self.deny.iter().any(|rule| rule.matches(path)) { return Some(ExclusionReason::DenyRule); } if self.gitignore.iter().any(|rule| rule.matches(path)) { return Some(ExclusionReason::GitIgnore); } if self.user_exclusions.iter().any(|rule| rule.matches(path)) { return Some(ExclusionReason::UserExclusion); } if Self::is_secret_file(path) { return Some(ExclusionReason::SecretFilePolicy); } None }";

/// The whole of `impl TokenPermission`: the read-only claim, as a total
/// function over the enum rather than a search for forbidden spellings.
const WHOLE_TOKEN_PERMISSION: &str = "impl TokenPermission { pub const ALL: [Self; 3] = [Self::MetadataRead, Self::ContentsRead, Self::IssuesRead]; #[must_use] pub const fn as_str(self) -> &'static str { match self { Self::MetadataRead => \"metadata:read\", Self::ContentsRead => \"contents:read\", Self::IssuesRead => \"issues:read\", } } #[must_use] pub const fn access(self) -> Access { match self { Self::MetadataRead | Self::ContentsRead | Self::IssuesRead => Access::Read, } } }";

/// The whole of `impl FineGrainedToken`: the three-property check in its fixed
/// order, and the one crate-private accessor of the material.
const WHOLE_FINE_GRAINED_TOKEN: &str = "impl FineGrainedToken { #[must_use] pub fn new(scope: TokenScope, lifetime: TokenLifetime, secret: Vec<u8>) -> Self { Self { scope, lifetime, secret: Zeroizing::new(secret), } } #[must_use] pub const fn scope(&self) -> &TokenScope { &self.scope } #[must_use] pub const fn lifetime(&self) -> TokenLifetime { self.lifetime } #[must_use] pub const fn is_valid_at(&self, now: u64) -> bool { self.lifetime.contains(now) } pub fn authorize( &self, repository: &GitHubRepository, permission: TokenPermission, now: u64, ) -> Result<(), GitHubError> { if !self.is_valid_at(now) { return Err(GitHubError::Expired); } if !self.scope.covers(repository) { return Err(GitHubError::OutOfScope); } if !self.scope.permissions().contains(&permission) { return Err(GitHubError::MissingPermission); } Ok(()) } pub(crate) fn material(&self) -> &[u8] { &self.secret } }";

/// The whole of `impl CredentialStore`: the expiry is checked before the
/// broker is asked, so an unusable token's material never leaves it.
const WHOLE_CREDENTIAL_STORE: &str = "impl<K: DeviceKeystore> CredentialStore<K> { #[must_use] pub const fn new(keystore: K) -> Self { Self { keystore } } pub fn seal(&self, token: &FineGrainedToken) -> Result<SealedCredential, GitHubError> { let label = token.scope.repository.as_label(); let blob = self .keystore .seal(&label, token.material()) .map_err(GitHubError::Keystore)?; Ok(SealedCredential { label, provider: self.keystore.provider().to_owned(), scope: token.scope.clone(), lifetime: token.lifetime, blob, }) } pub fn borrow( &self, sealed: &SealedCredential, now: u64, ) -> Result<FineGrainedToken, GitHubError> { if !sealed.lifetime.contains(now) { return Err(GitHubError::Expired); } let recovered = self .keystore .open(&sealed.label, &sealed.blob) .map_err(GitHubError::Keystore)?; Ok(FineGrainedToken { scope: sealed.scope.clone(), lifetime: sealed.lifetime, secret: Zeroizing::new(recovered.to_vec()), }) } }";

/// Every method of `RepositorySnapshot`, as a whole set of signatures.
///
/// The body of a frozen value's accessor is not the interesting part; the
/// signature is. Every entry here takes `&self` or is the one crate-private
/// constructor, so a method taking `&mut self`, a `set_*`, or an accessor
/// handing back a path appears as an extra key. The block is compared in both
/// directions, so a removed accessor fails too.
const SNAPSHOT_SIGNATURES: [&str; 18] = [
    "pub(crate) fn freeze(identity: SnapshotIdentity, listed: Inventory) -> Self {",
    "pub fn snapshot_id(&self) -> &str {",
    "pub const fn repository(&self) -> &RepositoryId {",
    "pub const fn source(&self) -> RepositorySource {",
    "pub const fn snapshot_type(&self) -> SnapshotType {",
    "pub fn branch(&self) -> Option<&str> {",
    "pub const fn commit(&self) -> Option<&CommitId> {",
    "pub fn parent_snapshots(&self) -> &[String] {",
    "pub const fn captured_at(&self) -> u64 {",
    "pub fn manifest(&self) -> &[ManifestEntry] {",
    "pub const fn manifest_digest(&self) -> &ContentDigest {",
    "pub const fn dirty(&self) -> Option<&DirtyManifest> {",
    "pub fn submodule_refs(&self) -> &[SubmoduleRef] {",
    "pub const fn analysis_policy_hash(&self) -> &ContentDigest {",
    "pub fn tool_versions(&self) -> &[ToolVersion] {",
    "pub const fn secret_scan_result(&self) -> SecretScanResult {",
    "pub fn secret_findings(&self) -> &[SecretFinding] {",
    "pub fn excluded(&self) -> &[ExcludedPath] {",
];

/// Call-site counts over every product file the walk reads, and the one file
/// each name may be called from.
///
/// A count is the claim that this crate has exactly this many ways to do the
/// thing named, and the walk is what makes "this crate" mean the package rather
/// than one file. The caller column is the second half: a count of one is still
/// one if it moves into a new module, and a module that reached the inventory
/// without the gate is the shape this table exists to refuse.
///
/// `scan_secrets` and `admit` are on it for the reason the execution plan calls
/// `secret_gate_precedes_indexer` a call-count spy: a gate whose scan was moved
/// into the indexer spells nothing forbidden and passes every pin on the gate's
/// own body.
const CALL_SITE_COUNTS: [(&str, usize, &str, &str); 8] = [
    (
        "permission_and_secret_gate",
        1,
        "crates/repository/src/lib.rs",
        "a capture path reaches the inventory without running the gate",
    ),
    (
        "inventory",
        1,
        "crates/repository/src/lib.rs",
        "the inventory is entered from more than one place",
    ),
    (
        // Two: `capture` calls the stage, and `LocalStages::freeze` calls the
        // snapshot's own crate-private constructor. Both sites are in
        // `lib.rs`, and a third anywhere fails.
        "freeze",
        2,
        "crates/repository/src/lib.rs",
        "a snapshot is frozen from more than one place",
    ),
    (
        "index",
        1,
        "crates/repository/src/lib.rs",
        "the indexer is reached from more than one place",
    ),
    (
        "scan_secrets",
        1,
        "crates/repository/src/gate.rs",
        "the secret scan runs somewhere other than the gate",
    ),
    (
        "admit",
        1,
        "crates/repository/src/gate.rs",
        "a path set is admitted somewhere other than the gate",
    ),
    (
        "run_gate",
        1,
        "crates/repository/src/lib.rs",
        "the gate body is called from more than one stage",
    ),
    (
        "resolve_snapshot_type",
        1,
        "crates/repository/src/lib.rs",
        "a snapshot type is derived somewhere other than the freeze",
    ),
];

/// Every `fs::` name this crate's product code spells, as a whole set.
///
/// Three, and all three read. This is not a list of forbidden names: it is the
/// list of names that are there, compared in both directions, so `fs::write`,
/// `fs::remove_file`, `fs::create_dir_all` and every other mutation appears as
/// an extra key without anyone having predicted it.
const FILESYSTEM_CALLS: [&str; 3] = ["read", "read_dir", "symlink_metadata"];

/// Every `use` item in this crate's product code, as a whole set.
///
/// The companion to [`FILESYSTEM_CALLS`]. A mutation reached without spelling
/// `fs::` needs an import — `use std::fs::File`, `use std::fs::OpenOptions`,
/// `use std::os::unix::fs::symlink` — and an import appears here. Both
/// inventories are whole sets, so between them there is no route to the
/// filesystem that spells no listed name and adds no listed import.
const USE_ITEMS: [&str; 14] = [
    "academic_crypto::{DeviceKeystore, KeystoreFailure}",
    "academic_policy::ContentDigest",
    "academic_policy::{ContentDigest, Decision, ReasonCode}",
    "academic_untrusted_content::{ IngestedDocument, SourceId, SourceIndex, SourceKind, Untrusted, ingest, }",
    "crate::snapshot::RepositoryError",
    "crate::{ gate::{AdmittedPaths, ExcludedPath, SecretFinding, SecretScanResult}, github::GitHubError, source::{CommitId, RepositorySource, SnapshotType}, }",
    "crate::{SourceEntry, source::CommitId}",
    "std::collections::BTreeSet",
    "std::{ collections::BTreeMap, fs, path::{Component, Path, PathBuf}, }",
    "zeroize::Zeroizing",
    // The two re-export groups. They name no capability of their own; they are
    // here because this inventory is the whole set rather than a filtered one.
    "pub use gate::{ AdmittedPaths, ContentVerdict, DisclosureDecision, ExcludedPath, ExclusionReason, PathPolicy, PathRule, SecretFinding, SecretScanResult, }",
    "pub use github::{ Access, CredentialStore, FineGrainedToken, GitHubError, GitHubRepository, GitHubRepositoryReader, MAX_TOKEN_LIFETIME_MILLIS, SealedCredential, TokenLifetime, TokenPermission, TokenScope, }",
    "pub use snapshot::{ DirtyKind, DirtyManifest, Inventory, Language, ManifestEntry, RepositoryError, RepositoryId, RepositorySnapshot, SnapshotIdentity, SubmoduleRef, ToolVersion, }",
    "pub use source::{CommitId, CommitIdError, RepositorySource, SnapshotType, WorkingTreeFacts}",
];

// ---------------------------------------------------------------------------
// The tests.
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion
    // every other test in this file makes over its result.
    assert!(
        sources.len() >= 6,
        "the walk found only {} files under the package",
        sources.len()
    );

    // Product source lives under `src` and nowhere else. That is the condition
    // `S-12` says a crate has to keep if it does not want to widen every scan
    // that reads it.
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
        let source = source_of(path)?;
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
        }
        for line in source.lines() {
            let trimmed = line.trim();
            // An attribute, in attribute position. A path attribute written
            // inside a string literal -- as this file writes one, in the
            // assertion below -- is prose about the rule rather than an
            // instance of it, and reading those would make the rule fire on
            // its own description.
            let Some(rest) = trimmed.strip_prefix("#[path") else {
                continue;
            };
            let spelling = rest
                .split('"')
                .nth(1)
                .ok_or("a #[path] attribute names no file")?;
            declared += 1;
            let resolved = path
                .parent()
                .map(|parent| parent.join(spelling))
                .ok_or("a #[path] has no parent directory")?;
            assert!(
                sources.iter().any(|read| read == &resolved),
                "{} pulls in {spelling}, which the walk never read",
                relative(path)
            );
        }
    }
    assert!(
        declared >= 4,
        "the tripwire read only {declared} module declarations"
    );
    Ok(())
}

#[test]
fn the_stage_order_and_the_gate_are_pinned() -> TestResult {
    let lib = source_of(&crate_root().join("src/lib.rs"))?;
    let gate = source_of(&crate_root().join("src/gate.rs"))?;
    let source = source_of(&crate_root().join("src/source.rs"))?;

    assert_eq!(
        free_function(&lib, "pub fn capture<S: SnapshotStages + ?Sized>(")?,
        WHOLE_CAPTURE,
        "the stage order changed"
    );
    assert_eq!(
        free_function(&gate, "pub(crate) fn run_gate(")?,
        WHOLE_RUN_GATE,
        "the gate body changed"
    );
    assert_eq!(
        free_function(
            &gate,
            "fn scan_secrets(path: &str, bytes: &[u8]) -> ContentVerdict {"
        )?,
        WHOLE_SCAN_SECRETS,
        "the content scan changed"
    );
    assert_eq!(
        free_function(&source, "pub(crate) fn resolve_snapshot_type(")?,
        WHOLE_RESOLVE_SNAPSHOT_TYPE,
        "the snapshot-type derivation changed"
    );
    assert_eq!(
        whole_block(
            &gate,
            "    pub fn classify(&self, path: &str) -> Option<ExclusionReason> {"
        )?,
        WHOLE_PATH_POLICY_CLASSIFY,
        "the path policy changed"
    );

    // The pins fix their callers too. Each guarded name is counted over every
    // product file the walk read, with its declarations subtracted, and the one
    // file it may be called from is checked separately.
    let product = product_code()?;
    for (name, sites, caller, why) in CALL_SITE_COUNTS {
        let mut calls = 0_usize;
        let mut in_caller = 0_usize;
        for (path, code) in &product {
            let body = without_use_items(code);
            let here = calls_of(&body, name);
            calls += here;
            if path == caller {
                in_caller = here;
            }
        }
        assert_eq!(calls, sites, "{why}");
        assert_eq!(
            in_caller, sites,
            "{name} is called from a file other than {caller}"
        );
    }
    Ok(())
}

#[test]
fn a_secret_digest_has_exactly_two_writers_and_one_needs_a_decision() -> TestResult {
    let gate = source_of(&crate_root().join("src/gate.rs"))?;
    assert_eq!(
        whole_block(&gate, "impl SecretFinding {")?,
        WHOLE_SECRET_FINDING,
        "what a secret finding can be asked for changed"
    );

    // And the digest field is written in exactly two places across the whole
    // package: the constructor that writes `None`, and the disclosure that
    // writes `Some`. A third assignment anywhere fails, wherever it is.
    let mut none_sites = 0_usize;
    let mut some_sites = 0_usize;
    for (_, code) in product_code()? {
        none_sites += code.matches("blob_digest: None").count();
        some_sites += code
            .matches("blob_digest: Some(ContentDigest::of(bytes))")
            .count();
    }
    assert_eq!(none_sites, 1, "the undisclosed default is written twice");
    assert_eq!(
        some_sites, 1,
        "a secret file's digest is computed somewhere besides the disclosure"
    );

    // The disclosure is the only public method that takes a `DisclosureDecision`
    // by value, so there is no second door into the same field.
    let mut takers = 0_usize;
    for (_, code) in product_code()? {
        takers += code.matches("decision: DisclosureDecision").count();
    }
    assert_eq!(
        takers, 1,
        "more than one function takes a recorded decision by value"
    );
    Ok(())
}

#[test]
fn the_snapshot_hands_back_owned_data_and_nothing_else() -> TestResult {
    let snapshot = source_of(&crate_root().join("src/snapshot.rs"))?;
    let block = whole_block_raw(&snapshot, "impl RepositorySnapshot {")?;
    let lines: Vec<&str> = block.lines().collect();
    let mut signatures = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if !["pub fn ", "pub const fn ", "pub(crate) fn ", "fn "]
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
        signatures.push(signature.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    assert_eq!(
        signatures,
        SNAPSHOT_SIGNATURES.to_vec(),
        "the snapshot's method set changed"
    );

    // No `&mut self` anywhere on the frozen value, and no public field on it.
    assert_eq!(
        signatures
            .iter()
            .filter(|signature| signature.contains("&mut self"))
            .count(),
        0,
        "a frozen snapshot gained a mutator"
    );
    let definition = whole_block_raw(&snapshot, "pub struct RepositorySnapshot {")?;
    assert_eq!(
        definition
            .lines()
            .skip(1)
            .filter(|line| line.trim_start().starts_with("pub "))
            .count(),
        0,
        "a frozen snapshot gained a public field"
    );
    Ok(())
}

#[test]
fn the_credential_is_repo_scoped_read_only_and_expiring_in_source() -> TestResult {
    let github = source_of(&crate_root().join("src/github.rs"))?;
    assert_eq!(
        whole_block(&github, "impl TokenPermission {")?,
        WHOLE_TOKEN_PERMISSION,
        "the permission vocabulary changed"
    );
    assert_eq!(
        whole_block(&github, "impl FineGrainedToken {")?,
        WHOLE_FINE_GRAINED_TOKEN,
        "what a token checks, or what can reach its material, changed"
    );
    assert_eq!(
        whole_block(&github, "impl<K: DeviceKeystore> CredentialStore<K> {")?,
        WHOLE_CREDENTIAL_STORE,
        "the credential store changed"
    );

    // The material has one accessor, and it is called once: from `seal`.
    let product = product_code()?;
    let mut material_calls = 0_usize;
    for (_, code) in &product {
        material_calls += calls_of(&without_use_items(code), "material");
    }
    assert_eq!(
        material_calls, 1,
        "the token's material is read from more than one place"
    );

    // This crate ships no implementation of the reader trait. A product file
    // implementing it would be the first thing that needed a transport.
    let mut implementations = 0_usize;
    for (_, code) in &product {
        implementations += code.matches("impl GitHubRepositoryReader for").count()
            + code.matches("GitHubRepositoryReader for").count();
    }
    assert_eq!(
        implementations, 0,
        "a product file implements the GitHub reader; that is where a transport would go"
    );
    Ok(())
}

#[test]
fn the_crate_touches_the_filesystem_only_to_read_it() -> TestResult {
    let product = product_code()?;

    // Every `fs::` name, as a whole set.
    let mut named: BTreeSet<String> = BTreeSet::new();
    for (_, code) in &product {
        for occurrence in code.match_indices("fs::") {
            let tail = code.get(occurrence.0 + 4..).unwrap_or("");
            let name: String = tail
                .chars()
                .take_while(|character| character.is_alphanumeric() || *character == '_')
                .collect();
            if !name.is_empty() {
                named.insert(name);
            }
        }
    }
    assert_eq!(
        named,
        FILESYSTEM_CALLS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        "this crate spells a filesystem call its read-only claim does not cover"
    );

    // Every `use` item, as a whole set. A mutation reached without spelling
    // `fs::` needs one of these.
    let mut imports: BTreeSet<String> = BTreeSet::new();
    for (_, code) in &product {
        let mut collecting: Option<String> = None;
        for line in code.lines() {
            let trimmed = line.trim();
            let opens = trimmed.starts_with("use ")
                || (trimmed.starts_with("pub use ") || trimmed.starts_with("pub(crate) use "));
            if collecting.is_none() && !opens {
                continue;
            }
            let mut current = collecting.take().unwrap_or_default();
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(trimmed);
            if trimmed.ends_with(';') {
                let cleaned = current
                    .trim_end_matches(';')
                    .trim_start_matches("pub(crate) use ")
                    .trim_start_matches("use ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                imports.insert(cleaned);
            } else {
                collecting = Some(current);
            }
        }
    }
    assert_eq!(
        imports,
        USE_ITEMS
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<String>>(),
        "this crate's import set changed; a filesystem or transport type may have arrived"
    );
    Ok(())
}

/// The vacuity control. Every helper this file relies on is exercised against a
/// sample it must accept and one it must refuse, so a helper that matched
/// nothing would fail here rather than making every assertion above pass over
/// an empty set.
#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    assert_eq!(uses_of("a.expose(); expose_rendered();", "expose"), 1);
    assert_eq!(declarations_of("fn expose_rendered(", "expose"), 0);
    assert_eq!(declarations_of("fn expose(&self)", "expose"), 1);
    assert_eq!(declarations_of("fn freeze<'a>(", "freeze"), 1);
    assert_eq!(
        calls_of("fn index() {} index(); Local::index(x);", "index"),
        2
    );
    assert_eq!(
        without_use_items("use a::b;\nlet index = 1;\n").trim(),
        "let index = 1;"
    );
    assert_eq!(
        without_use_items("use a::{\n  b,\n};\nkeep;\n").trim(),
        "keep;"
    );
    assert_eq!(
        strip_non_code("let a = \"fs::write\"; // fs::remove_file\n").trim(),
        "let a =  ;"
    );
    assert_eq!(
        strip_non_code("let a = r#\"fs::write\"#;").trim(),
        "let a =  ;"
    );
    assert_eq!(
        whole_block("impl A { fn b() { if c { d } } }", "impl A {")?,
        "impl A { fn b() { if c { d } } }"
    );
    assert!(free_function("fn a() {\n    1\n}\n", "fn a()").is_ok());
    Ok(())
}

/// `whole_block`, without the comment-and-whitespace collapse.
fn whole_block_raw(source: &str, header: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(header)
        .ok_or_else(|| format!("{header} is not in the source"))?;
    let open = source[start..]
        .find('{')
        .map(|offset| start + offset)
        .ok_or_else(|| format!("{header} opens no block"))?;
    let mut depth = 0_usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth = depth.saturating_add(1),
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Ok(source[start..open + offset + 1].to_owned());
                }
            }
            _ => (),
        }
    }
    Err(format!("{header} is not brace-balanced").into())
}

/// This crate is named in the repository's policy source-scan inventory.
///
/// `tools/policy-source-scan-inventory.test.mjs` executes the page's claim that
/// it enumerates every file reading this repository's Rust source text. That
/// test runs in the pnpm lane; this one fails in the Rust lane too, so a scan
/// added here without a row on the page is caught on both.
#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    for named in [
        "crates/repository/tests/repository_scans.rs",
        "crates/repository/tests/snapshot.rs",
    ] {
        assert!(
            page.contains(named),
            "{named} is not named in docs/contracts/policy-source-scans.md"
        );
    }
    // And the counted names are on the page, so the table there and this file
    // cannot drift into describing different guards.
    let mut missing = Vec::new();
    for (name, _, _, _) in CALL_SITE_COUNTS {
        if !page.contains(name) {
            missing.push(name);
        }
    }
    assert_eq!(
        missing,
        Vec::<&str>::new(),
        "a counted call site is not described on the inventory page"
    );
    Ok(())
}

/// The counted names are exactly the names this crate's stages have.
///
/// Without this, a stage renamed in `lib.rs` and in `CALL_SITE_COUNTS` together
/// would keep every count passing while the guard stopped covering the pipeline.
#[test]
fn every_stage_of_the_seam_is_counted() -> TestResult {
    let lib = source_of(&crate_root().join("src/lib.rs"))?;
    let trait_block = whole_block_raw(&lib, "pub trait SnapshotStages {")?;
    let declared: BTreeSet<String> = trait_block
        .lines()
        .filter_map(|line| {
            line.trim_start()
                .strip_prefix("fn ")
                .and_then(|rest| rest.split(['(', '<']).next())
                .map(str::to_owned)
        })
        .collect();
    let counted: BTreeSet<String> = CALL_SITE_COUNTS
        .into_iter()
        .map(|(name, _, _, _)| name.to_owned())
        .filter(|name| declared.contains(name))
        .collect();
    assert_eq!(
        declared, counted,
        "a stage of the seam is not in CALL_SITE_COUNTS"
    );
    assert_eq!(declared.len(), 4, "the seam no longer has four stages");
    Ok(())
}

/// The eight inputs and the four snapshot types are what the specification says.
///
/// The vocabularies are read out of the authoritative specification rather than
/// transcribed here, so a change to section 17.1 or 17.2 that this crate has
/// not followed fails rather than passing silently.
#[test]
fn the_vocabularies_match_the_specification() -> TestResult {
    let specification = fs::read_to_string(
        workspace_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?;
    let section = specification
        .split("### 17.2 Snapshot")
        .nth(1)
        .ok_or("section 17.2 is not in the specification")?;
    let declared = section
        .lines()
        .find(|line| line.trim_start().starts_with("sourceType:"))
        .ok_or("section 17.2 declares no sourceType")?;
    let spelled: BTreeSet<String> = declared
        .split(':')
        .nth(1)
        .unwrap_or("")
        .split('|')
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect();
    let source = source_of(&crate_root().join("src/source.rs"))?;
    let implemented: BTreeSet<String> = ["GIT_COMMIT", "DIRTY_WORKTREE", "ARCHIVE", "SPEC_ONLY"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(
        spelled, implemented,
        "section 17.2's sourceType values and this crate's SnapshotType differ"
    );
    for value in &implemented {
        assert!(
            source.contains(&format!("\"{value}\"")),
            "{value} is not spelled in source.rs"
        );
    }

    // The eight inputs, read out of section 17.1's own sentence.
    let inputs = specification
        .split("### 17.1")
        .nth(1)
        .ok_or("section 17.1 is not in the specification")?;
    for phrase in [
        "local directory",
        "GitHub public/private repo",
        "archive",
        "branch",
        "commit",
        "dirty working tree",
        "spec-only project",
    ] {
        assert!(
            inputs.contains(phrase),
            "section 17.1 no longer names {phrase}; RepositorySource has to follow it"
        );
    }
    // Seven phrases because the specification writes the two GitHub arms as
    // one; the enum splits them because a private repository needs a
    // credential and a public one does not.
    let listed = source
        .split("pub const ALL: [Self; 8] = [")
        .nth(1)
        .and_then(|rest| rest.split("];").next())
        .ok_or("RepositorySource::ALL is not in source.rs")?;
    assert_eq!(
        listed.matches("Self::").count(),
        8,
        "RepositorySource::ALL no longer lists eight inputs"
    );
    Ok(())
}
