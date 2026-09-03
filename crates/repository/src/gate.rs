//! The permission and secret gate: section 17.3's first stage.
//!
//! Section 17.3 draws the pipeline as `permission + secret gate` above
//! `inventory and immutable snapshot` above `syntax/semantic indexing`. That
//! ordering is what this module exists to make structural rather than
//! conventional, and it is enforced in three independent ways:
//!
//! * **By type.** [`AdmittedPaths`] is the only argument the inventory stage
//!   accepts and its constructor is crate-private, so an implementation of
//!   [`crate::SnapshotStages`] written outside this crate cannot produce one
//!   without calling the gate. It also carries the request digest it was
//!   admitted for, and the inventory refuses one that names another request, so
//!   a second capture path cannot reuse an earlier admission.
//! * **By count.** [`AdmittedPaths::admit`] and [`scan_secrets`] each have one
//!   call site, both in this file, and
//!   `crates/repository/tests/repository_scans.rs` counts them over every file
//!   of the package.
//! * **By observation.** `secret_gate_precedes_indexer` drives the real stages
//!   through a spy and reads the recorded call order and per-stage counts. On a
//!   repository the gate blocks, the indexer's count is zero — which is what
//!   fails if the scan is moved behind it.
//!
//! ## What the gate applies, and in which order
//!
//! Section 29.6 lists four things a local analyzer applies: file allow/deny
//! rules, `.gitignore`, user exclusions, and a secret scan. Section 32.4 splits
//! the last into a file-level policy (point 1) and a content scan (point 2).
//! [`PathPolicy`] holds the first three plus 32.4's file-level defaults;
//! [`scan_secrets`] is the content half.
//!
//! Exclusion is *not* the same as blocking. A path the policy excludes is never
//! read and never reaches a manifest. A path whose *content* trips the secret
//! scan blocks the whole snapshot: section 32.4's point 5 is fail-closed, and a
//! secret in a file the user did not exclude is a fact about the repository
//! rather than about that one file.

use std::collections::BTreeSet;

use academic_policy::{ContentDigest, Decision, ReasonCode};

use crate::snapshot::RepositoryError;

/// Why a path was removed before the analyzer could see it.
///
/// Four reasons, which are the four inputs section 29.6 names. Every excluded
/// path carries one, so "the analyzer never saw it" is answerable per path
/// rather than in aggregate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ExclusionReason {
    /// A `.gitignore` pattern matched.
    GitIgnore,
    /// A configured deny rule matched.
    DenyRule,
    /// The user excluded this path.
    UserExclusion,
    /// Section 32.4's point-1 file policy matched: an environment file, a
    /// private key, a credential store, a secret mount, or a build artifact.
    SecretFilePolicy,
}

impl ExclusionReason {
    /// Exhaustive order.
    pub const ALL: [Self; 4] = [
        Self::GitIgnore,
        Self::DenyRule,
        Self::UserExclusion,
        Self::SecretFilePolicy,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GitIgnore => "GITIGNORE",
            Self::DenyRule => "DENY_RULE",
            Self::UserExclusion => "USER_EXCLUSION",
            Self::SecretFilePolicy => "SECRET_FILE_POLICY",
        }
    }
}

/// One excluded path and the reason it was excluded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExcludedPath {
    path: String,
    reason: ExclusionReason,
}

impl ExcludedPath {
    /// The relative, forward-slashed path.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Why it was excluded.
    #[must_use]
    pub const fn reason(&self) -> ExclusionReason {
        self.reason
    }
}

/// Section 32.4's point-1 file policy, as suffix and segment rules.
///
/// These are the defaults the specification names: an environment file, a
/// private key, a credential store, a secret mount, and a build artifact. They
/// are applied by [`PathPolicy`] regardless of what the user configured, which
/// is why they are a constant here rather than a default value someone can
/// replace.
const SECRET_FILE_SUFFIXES: [&str; 9] = [
    ".env",
    ".pem",
    ".key",
    ".p12",
    ".pfx",
    ".keystore",
    ".jks",
    ".ppk",
    ".asc",
];

/// Path segments whose whole subtree the point-1 policy removes.
const SECRET_FILE_SEGMENTS: [&str; 8] = [
    ".aws",
    ".gnupg",
    ".ssh",
    "credentials",
    "node_modules",
    "run/secrets",
    "secrets",
    "target",
];

/// The largest file the content scan reads in one piece.
///
/// The same bound `academic-untrusted-content` puts on one ingested document,
/// restated as a number rather than imported, because a file over it is
/// [`ContentVerdict::Opaque`] here and would be `IngestError::Oversize` there:
/// two different answers to one question would let a file be scanned and then
/// refused, or refused and then scanned.
const MAX_SCANNED_BYTES: usize = 1 << 20;

/// Exact file names the point-1 policy removes wherever they appear.
const SECRET_FILE_NAMES: [&str; 6] = [
    ".env",
    ".envrc",
    ".netrc",
    "credentials",
    "id_ed25519",
    "id_rsa",
];

/// A single path rule: an exact path, a directory prefix, or a suffix.
///
/// This is deliberately not a full `.gitignore` implementation. What the
/// contract fixes is that the three rule sources are applied before a path
/// reaches the analyzer, and the shapes below are what this crate's fixtures
/// and the repository's own ignore files use. A richer matcher is a change to
/// this type and to nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathRule {
    /// Matches one exact relative path.
    Exact(String),
    /// Matches a path under this directory prefix.
    Prefix(String),
    /// Matches a path ending in this suffix.
    Suffix(String),
}

impl PathRule {
    fn matches(&self, path: &str) -> bool {
        match self {
            Self::Exact(exact) => path == exact,
            Self::Prefix(prefix) => {
                path == prefix.trim_end_matches('/')
                    || path.starts_with(&format!("{}/", prefix.trim_end_matches('/')))
            }
            Self::Suffix(suffix) => path.ends_with(suffix.as_str()),
        }
    }
}

/// The three configured rule sources plus section 32.4's file defaults.
///
/// The order of the fields is the order [`PathPolicy::classify`] applies them,
/// and it is the order section 29.6 writes: allow/deny rules, `.gitignore`,
/// user exclusions. An allow rule is a narrowing rather than an override: a
/// non-empty allow list means only matching paths are considered at all, and a
/// path that survives it still has to survive everything below.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathPolicy {
    allow: Vec<PathRule>,
    deny: Vec<PathRule>,
    gitignore: Vec<PathRule>,
    user_exclusions: Vec<PathRule>,
}

impl PathPolicy {
    /// An empty policy, which still applies section 32.4's file defaults.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            allow: Vec::new(),
            deny: Vec::new(),
            gitignore: Vec::new(),
            user_exclusions: Vec::new(),
        }
    }

    /// Narrows the policy to paths matching at least one of these rules.
    #[must_use]
    pub fn allowing(mut self, rules: Vec<PathRule>) -> Self {
        self.allow = rules;
        self
    }

    /// Adds configured deny rules.
    #[must_use]
    pub fn denying(mut self, rules: Vec<PathRule>) -> Self {
        self.deny = rules;
        self
    }

    /// Adds the rules parsed from a `.gitignore` file.
    #[must_use]
    pub fn ignoring(mut self, rules: Vec<PathRule>) -> Self {
        self.gitignore = rules;
        self
    }

    /// Adds the paths the user excluded.
    #[must_use]
    pub fn excluding(mut self, rules: Vec<PathRule>) -> Self {
        self.user_exclusions = rules;
        self
    }

    /// Parses one `.gitignore` text into rules.
    ///
    /// Blank lines and comments are dropped. A leading `/` anchors, a trailing
    /// `/` makes a directory prefix, a leading `*` makes a suffix rule, and
    /// anything else matches the path or any path under it. Negation (`!`) is
    /// deliberately unsupported and is reported rather than silently ignored,
    /// because a rule this parser mis-reads as an exclusion when it is an
    /// inclusion would hide a file from the analyzer, and one mis-read the
    /// other way would show the analyzer a file the user meant to hide.
    ///
    /// # Errors
    ///
    /// [`RepositoryError::UnsupportedIgnoreRule`] when a line begins with `!`.
    pub fn parse_gitignore(text: &str) -> Result<Vec<PathRule>, RepositoryError> {
        let mut rules = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if let Some(negated) = trimmed.strip_prefix('!') {
                return Err(RepositoryError::UnsupportedIgnoreRule(negated.to_owned()));
            }
            if let Some(suffix) = trimmed.strip_prefix('*') {
                rules.push(PathRule::Suffix(suffix.to_owned()));
                continue;
            }
            let anchored = trimmed.trim_start_matches('/');
            if let Some(directory) = anchored.strip_suffix('/') {
                rules.push(PathRule::Prefix(directory.to_owned()));
                continue;
            }
            rules.push(PathRule::Prefix(anchored.to_owned()));
        }
        Ok(rules)
    }

    /// Whether section 32.4's point-1 file policy removes this path.
    fn is_secret_file(path: &str) -> bool {
        let name = path.rsplit('/').next().unwrap_or(path);
        SECRET_FILE_NAMES.contains(&name)
            || SECRET_FILE_SUFFIXES
                .iter()
                .any(|suffix| name.ends_with(suffix))
            || SECRET_FILE_SEGMENTS
                .iter()
                .any(|segment| PathRule::Prefix((*segment).to_owned()).matches(path))
            || path
                .split('/')
                .any(|segment| SECRET_FILE_SEGMENTS.contains(&segment))
    }

    /// The reason this path is excluded, or `None` when it survives.
    ///
    /// This is the whole of "`.gitignore`, allow/deny rules, and user
    /// exclusions are applied before the analyzer sees a path": nothing else in
    /// this crate decides whether a path is read, and
    /// `analyzer_never_sees_an_excluded_path` reads the set of paths the
    /// inventory opened against the set this returns `None` for.
    #[must_use]
    pub fn classify(&self, path: &str) -> Option<ExclusionReason> {
        if !self.allow.is_empty() && !self.allow.iter().any(|rule| rule.matches(path)) {
            return Some(ExclusionReason::DenyRule);
        }
        if self.deny.iter().any(|rule| rule.matches(path)) {
            return Some(ExclusionReason::DenyRule);
        }
        if self.gitignore.iter().any(|rule| rule.matches(path)) {
            return Some(ExclusionReason::GitIgnore);
        }
        if self.user_exclusions.iter().any(|rule| rule.matches(path)) {
            return Some(ExclusionReason::UserExclusion);
        }
        if Self::is_secret_file(path) {
            return Some(ExclusionReason::SecretFilePolicy);
        }
        None
    }
}

/// Section 17.2's `secretScanResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SecretScanResult {
    /// Nothing the content scan recognises is in the admitted set.
    Pass,
    /// Something is, and no snapshot is produced.
    Blocked,
}

impl SecretScanResult {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Blocked => "BLOCKED",
        }
    }

    /// The broker decision this result is.
    ///
    /// `P2-G1` owns the two-valued decision vocabulary and this reuses it
    /// rather than adding a third spelling of the same thing.
    #[must_use]
    pub const fn decision(self) -> Decision {
        match self {
            Self::Pass => Decision::Allow,
            Self::Blocked => Decision::Deny,
        }
    }
}

/// The recorded decision that permits one secret file's digest to be stored.
///
/// Section 17.2 says a secret file's *hash* has its disclosure scope reviewed.
/// The default in this crate is that there is no such review and therefore no
/// digest: [`SecretFinding`] is constructed without one and there is no setter.
/// The only way a digest is ever computed is [`SecretFinding::disclose`], which
/// takes one of these by value.
///
/// What makes it a *record* rather than a flag is that every field is required
/// and none can be empty, and that migration `0012` stores it in its own table
/// with the finding's digest column bound to its identifier by a trigger. A
/// digest with no decision row is refused by the database as well as by this
/// type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureDecision {
    decision_id: String,
    actor_id: String,
    reason: String,
    recorded_at: u64,
}

impl DisclosureDecision {
    /// Records a user's decision to permit one secret file's digest.
    ///
    /// # Errors
    ///
    /// [`RepositoryError::EmptyDecisionField`] when any field is empty. A
    /// decision with no actor or no reason is not a record of anything.
    pub fn record(
        decision_id: impl Into<String>,
        actor_id: impl Into<String>,
        reason: impl Into<String>,
        recorded_at: u64,
    ) -> Result<Self, RepositoryError> {
        let decision_id = decision_id.into();
        let actor_id = actor_id.into();
        let reason = reason.into();
        if decision_id.is_empty() || actor_id.is_empty() || reason.is_empty() {
            return Err(RepositoryError::EmptyDecisionField);
        }
        Ok(Self {
            decision_id,
            actor_id,
            reason,
            recorded_at,
        })
    }

    /// The decision identifier migration `0012` keys the record on.
    #[must_use]
    pub fn decision_id(&self) -> &str {
        &self.decision_id
    }

    /// Who decided.
    #[must_use]
    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    /// Why.
    #[must_use]
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// When, in the caller's recorded milliseconds.
    #[must_use]
    pub const fn recorded_at(&self) -> u64 {
        self.recorded_at
    }
}

/// One file the content scan recognised, and what may be stored about it.
///
/// The digest is `None` until a [`DisclosureDecision`] is presented. That is
/// the default-deny half: a finding is constructible and a digest is not, so
/// there is no order of calls that stores one without a decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretFinding {
    path: String,
    reason: ReasonCode,
    blob_digest: Option<ContentDigest>,
    disclosure: Option<DisclosureDecision>,
}

impl SecretFinding {
    /// A finding with no digest and no disclosure.
    ///
    /// This is the only constructor, and it takes no digest and no bytes. It is
    /// called from [`scan_secrets`] and from nowhere else.
    #[must_use]
    pub(crate) fn new(path: String, reason: ReasonCode) -> Self {
        Self {
            path,
            reason,
            blob_digest: None,
            disclosure: None,
        }
    }

    /// Attaches the digest of `bytes` under a recorded decision.
    ///
    /// This is the one place a secret file's digest is ever computed. It
    /// consumes the finding and returns a new one, so a caller holding the
    /// undisclosed value cannot observe it changing underneath.
    #[must_use]
    pub fn disclose(self, decision: DisclosureDecision, bytes: &[u8]) -> Self {
        Self {
            path: self.path,
            reason: self.reason,
            blob_digest: Some(ContentDigest::of(bytes)),
            disclosure: Some(decision),
        }
    }

    /// The path, which is metadata this system chose and not file content.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Which of section 3.5's reason codes the content scan raised.
    #[must_use]
    pub const fn reason(&self) -> ReasonCode {
        self.reason
    }

    /// The digest, when a decision permitted one.
    #[must_use]
    pub const fn blob_digest(&self) -> Option<&ContentDigest> {
        self.blob_digest.as_ref()
    }

    /// The decision that permitted it.
    #[must_use]
    pub const fn disclosure(&self) -> Option<&DisclosureDecision> {
        self.disclosure.as_ref()
    }
}

/// A relative path admitted for reading, with the gate's decision attached.
///
/// The constructor is crate-private and there is exactly one call site, so an
/// implementation of [`crate::SnapshotStages`] written outside this crate has
/// no way to produce this value: it can only obtain one by calling the gate.
/// That is what makes the stage order structural rather than conventional.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedPaths {
    request_digest: ContentDigest,
    admitted: Vec<String>,
    opaque: Vec<String>,
    excluded: Vec<ExcludedPath>,
    findings: Vec<SecretFinding>,
    result: SecretScanResult,
}

impl AdmittedPaths {
    /// The one construction site. Called from [`run_gate`] and nowhere else.
    fn admit(
        request_digest: ContentDigest,
        admitted: Vec<String>,
        opaque: Vec<String>,
        excluded: Vec<ExcludedPath>,
        findings: Vec<SecretFinding>,
        result: SecretScanResult,
    ) -> Self {
        Self {
            request_digest,
            admitted,
            opaque,
            excluded,
            findings,
            result,
        }
    }

    /// The admitted paths the scanner could not read as bounded text.
    ///
    /// They are manifested by digest and are not ingested. See
    /// [`ContentVerdict::Opaque`].
    #[must_use]
    pub fn opaque(&self) -> &[String] {
        &self.opaque
    }

    /// The digest of the request this admission was decided for.
    ///
    /// The inventory compares it against the request it is given, so an
    /// admission cannot be carried from one capture to another.
    #[must_use]
    pub const fn request_digest(&self) -> &ContentDigest {
        &self.request_digest
    }

    /// The paths an analyzer may be shown, in sorted order.
    #[must_use]
    pub fn admitted(&self) -> &[String] {
        &self.admitted
    }

    /// The paths the policy removed, each with its reason.
    #[must_use]
    pub fn excluded(&self) -> &[ExcludedPath] {
        &self.excluded
    }

    /// What the content scan found.
    #[must_use]
    pub fn findings(&self) -> &[SecretFinding] {
        &self.findings
    }

    /// Section 17.2's `secretScanResult`.
    #[must_use]
    pub const fn result(&self) -> SecretScanResult {
        self.result
    }
}

/// How many distinct characters a run holds.
///
/// This is the entropy detector's criterion, and it is integer arithmetic on
/// purpose. Shannon entropy over the same run is the textbook formula and needs
/// `log2`, whose last bits are not guaranteed identical between two targets;
/// a snapshot whose `secretScanResult` differed between Windows and Linux for a
/// file near the threshold would be a worse defect than the sharper detector is
/// worth. What this counts is therefore a proxy for entropy rather than
/// entropy, and it is named for what it counts.
///
/// The finding it raises is [`ReasonCode::SecretEntropy`] because that is
/// section 3.5's code for this class of finding; the code names the class, not
/// the formula.
fn distinct_characters(run: &str) -> usize {
    let mut seen = [false; 128];
    let mut distinct = 0_usize;
    for byte in run.bytes() {
        let slot = usize::from(byte);
        if slot < seen.len() && !seen[slot] {
            seen[slot] = true;
            distinct = distinct.saturating_add(1);
        }
    }
    distinct
}

/// The longest run of secret-alphabet characters in `text`.
fn longest_secret_run(text: &str) -> &str {
    let bytes = text.as_bytes();
    let is_run =
        |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'-' | b'_');
    let mut best = 0_usize..0_usize;
    let mut start = 0_usize;
    for at in 0..=bytes.len() {
        let inside = at < bytes.len() && is_run(bytes[at]);
        if !inside {
            if at.saturating_sub(start) > best.end.saturating_sub(best.start) {
                best = start..at;
            }
            start = at.saturating_add(1);
        }
    }
    text.get(best).unwrap_or("")
}

/// What the content scan concluded about one admitted file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentVerdict {
    /// Nothing was recognised, and the text is available to be ingested.
    Clean,
    /// Something was, and no snapshot is produced.
    Secret(SecretFinding),
    /// The scanner could not read the bytes as text, or there were too many.
    ///
    /// Section 32.4's point 5 is fail-closed about what such a file may be used
    /// *for*: no external transmission. It is not a reason to refuse a
    /// snapshot, so the file is manifested by digest and is not ingested — the
    /// bytes exist in no value this crate hands on, so there is nothing for a
    /// later stage to transmit.
    Opaque,
}

/// Section 32.4's point-2 content scan, as five independent detectors.
///
/// The five are the ones the specification names: token pattern, entropy, known
/// key format, connection string, and cloud credential. Each maps to one of
/// section 3.5's closed reason codes rather than to a vocabulary of its own.
///
/// This function has one call site, in [`run_gate`]. Moving the scan behind the
/// inventory or the indexer changes that call site's file, and
/// `crates/repository/tests/repository_scans.rs` reads the file each counted
/// name may be called from.
fn scan_secrets(path: &str, bytes: &[u8]) -> ContentVerdict {
    if bytes.len() > MAX_SCANNED_BYTES {
        return ContentVerdict::Opaque;
    }
    let Ok(text) = core::str::from_utf8(bytes) else {
        return ContentVerdict::Opaque;
    };

    // Known key format. A PEM header names what follows it.
    if text.contains("-----BEGIN ") && text.contains(" PRIVATE KEY-----") {
        return ContentVerdict::Secret(SecretFinding::new(
            path.to_owned(),
            ReasonCode::SecretPattern,
        ));
    }
    // Cloud credential. The provider prefixes are structural, not guesses.
    for prefix in ["AKIA", "ASIA", "ghp_", "github_pat_", "xoxb-", "sk-"] {
        if text.contains(prefix) {
            return ContentVerdict::Secret(SecretFinding::new(
                path.to_owned(),
                ReasonCode::SecretPattern,
            ));
        }
    }
    // Connection string. A scheme with an embedded password.
    for scheme in ["postgres://", "postgresql://", "mysql://", "mongodb://"] {
        if let Some(rest) = text.split(scheme).nth(1)
            && let Some(authority) = rest.split('/').next()
            && authority.contains(':')
            && authority.contains('@')
        {
            return ContentVerdict::Secret(SecretFinding::new(
                path.to_owned(),
                ReasonCode::SecretPattern,
            ));
        }
    }
    // Token pattern. An assignment whose name says what the value is.
    for name in ["api_key", "apikey", "secret", "password", "token"] {
        for line in text.lines() {
            let lowered = line.to_ascii_lowercase();
            let Some(at) = lowered.find(name) else {
                continue;
            };
            let tail = lowered.get(at.saturating_add(name.len())..).unwrap_or("");
            let assigns =
                tail.trim_start().starts_with(['=', ':']) || tail.trim_start().starts_with("\" :");
            if assigns && longest_secret_run(line).len() >= 16 {
                return ContentVerdict::Secret(SecretFinding::new(
                    path.to_owned(),
                    ReasonCode::SecretPattern,
                ));
            }
        }
    }
    // Entropy. A long run drawing on far more of its alphabet than a word,
    // an identifier, or a hexadecimal digest does.
    let run = longest_secret_run(text);
    if run.len() >= 32 && distinct_characters(run) >= 24 {
        return ContentVerdict::Secret(SecretFinding::new(
            path.to_owned(),
            ReasonCode::SecretEntropy,
        ));
    }
    ContentVerdict::Clean
}

/// The gate. Section 17.3's first stage, entire.
///
/// `paths` is every relative path under the root, `read` yields the bytes of
/// one. The policy runs first and decides what is *read at all*; the content
/// scan runs over exactly what survived, and its result decides whether a
/// snapshot exists.
///
/// `crates/repository/tests/repository_scans.rs` pins this function whole and
/// counts its callers, because a pin on a decision says nothing about whether
/// the decision runs.
pub(crate) fn run_gate(
    request_digest: ContentDigest,
    policy: &PathPolicy,
    paths: &[String],
    read: &mut dyn FnMut(&str) -> Result<Vec<u8>, RepositoryError>,
) -> Result<AdmittedPaths, RepositoryError> {
    let mut admitted = Vec::new();
    let mut excluded = Vec::new();
    for path in paths {
        match policy.classify(path) {
            Some(reason) => excluded.push(ExcludedPath {
                path: path.clone(),
                reason,
            }),
            None => admitted.push(path.clone()),
        }
    }
    admitted.sort();
    excluded.sort();

    let mut findings = Vec::new();
    let mut opaque = Vec::new();
    for path in &admitted {
        let bytes = read(path)?;
        match scan_secrets(path, &bytes) {
            ContentVerdict::Clean => (),
            ContentVerdict::Secret(finding) => findings.push(finding),
            ContentVerdict::Opaque => opaque.push(path.clone()),
        }
    }
    let result = if findings.is_empty() {
        SecretScanResult::Pass
    } else {
        SecretScanResult::Blocked
    };
    Ok(AdmittedPaths::admit(
        request_digest,
        admitted,
        opaque,
        excluded,
        findings,
        result,
    ))
}

/// The set of paths a policy admits out of `paths`, for callers that want the
/// classification without reading a byte.
///
/// Used by `analyzer_never_sees_an_excluded_path` as the independent expectation
/// the observed read set is compared against.
#[must_use]
pub fn admitted_paths(policy: &PathPolicy, paths: &[String]) -> BTreeSet<String> {
    paths
        .iter()
        .filter(|path| policy.classify(path).is_none())
        .cloned()
        .collect()
}
