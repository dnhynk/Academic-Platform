//! `P2-R1`'s named acceptance evidence.
//!
//! Every fixture here is synthetic and built in-process by a deterministic
//! builder: no repository is committed, no real token exists, and no network
//! call is made. The GitHub arms are driven through an in-memory
//! `GitHubRepositoryReader` and an in-memory `DeviceKeystore`, which are this
//! crate's only implementations of either.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::Path,
};

use academic_crypto::{DeviceKeystore, KeystoreFailure};
use academic_policy::{
    ContentDigest, Decision, PermissionBroker, ProcessCapability, ProcessClass, ReasonCode,
};
use academic_repository::{
    Access, AdmittedPaths, Capture, CommitId, CredentialStore, DirtyKind, DisclosureDecision,
    ExclusionReason, FineGrainedToken, GitHubError, GitHubRepository, GitHubRepositoryReader,
    IndexReceipt, Inventory, LocalStages, MAX_TOKEN_LIFETIME_MILLIS, PathPolicy, PathRule,
    RepositoryError, RepositoryId, RepositorySnapshot, RepositorySource, SecretScanResult,
    SnapshotRequest, SnapshotStages, SnapshotType, SourceEntry, SourceTree, TokenLifetime,
    TokenPermission, TokenScope, ToolVersion, WorkingTreeFacts, analyze, capture, capture_local,
    sealed_document_count,
};
use tempfile::TempDir;
use zeroize::Zeroizing;

type TestResult = Result<(), Box<dyn Error>>;

/// A capture time that is a fixed number rather than a clock reading.
const CAPTURED_AT: u64 = 1_756_000_000_000;

/// The head commit every fixture in this file is checked out from.
const HEAD: &str = "abc1234def5678";

/// The deterministic builder. Writes `files` under `root`, creating parents.
fn write_tree(root: &Path, files: &[(&str, &str)]) -> TestResult {
    for (path, content) in files {
        let absolute = root.join(path);
        if let Some(parent) = absolute.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&absolute, content)?;
    }
    Ok(())
}

/// The clean project every fixture starts from.
///
/// It holds one prose file, one Rust file, one manifest, one migration, and one
/// file section 32.4's point-1 policy removes, so an exclusion is observable in
/// every capture rather than only in the one test about exclusions.
fn clean_files() -> Vec<(&'static str, &'static str)> {
    vec![
        ("README.md", "# Orders\n\nA synthetic project.\n"),
        (
            "src/orders/service.rs",
            "// Places one order.\npub fn place() -> u32 {\n    1\n}\n",
        ),
        ("Cargo.toml", "[package]\nname = \"orders\"\n"),
        (
            "migrations/0001_orders.sql",
            "CREATE TABLE orders(id TEXT);\n",
        ),
        (".env", "DATABASE_URL=postgres://user:pw@localhost/db\n"),
    ]
}

/// The same project as in-memory entries, for the four non-directory inputs.
fn clean_entries() -> Vec<SourceEntry> {
    clean_files()
        .into_iter()
        .map(|(path, content)| SourceEntry::new(path, content.as_bytes().to_vec()))
        .collect()
}

/// A specification-only project: prose and no code.
fn spec_only_entries() -> Vec<SourceEntry> {
    vec![
        SourceEntry::new("README.md", b"# Orders\n".to_vec()),
        SourceEntry::new("docs/architecture.md", b"# Architecture\n".to_vec()),
        SourceEntry::new("docs/adr/0001-split.md", b"# ADR 1\n".to_vec()),
    ]
}

fn head() -> Result<CommitId, Box<dyn Error>> {
    Ok(CommitId::new(HEAD)?)
}

/// Facts for a clean checkout of the fixture above.
fn clean_facts(branch: Option<&str>) -> Result<WorkingTreeFacts, Box<dyn Error>> {
    Ok(WorkingTreeFacts::checkout(
        head()?,
        branch.map(str::to_owned),
        clean_files()
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect(),
        Vec::new(),
        Vec::new(),
    ))
}

/// Facts for the same checkout with one tracked file changed and one untracked
/// file present.
fn dirty_facts() -> Result<WorkingTreeFacts, Box<dyn Error>> {
    Ok(WorkingTreeFacts::checkout(
        head()?,
        Some("main".to_owned()),
        clean_files()
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect(),
        vec!["src/orders/service.rs".to_owned()],
        vec!["notes/scratch.md".to_owned()],
    ))
}

fn repository() -> Result<RepositoryId, Box<dyn Error>> {
    Ok(RepositoryId::new("repo_A")?)
}

fn policy_hash() -> ContentDigest {
    ContentDigest::of(b"analysis-policy-v1")
}

fn tools() -> Result<Vec<ToolVersion>, Box<dyn Error>> {
    Ok(vec![
        ToolVersion::new("academic-repository", "0.1.0")?,
        ToolVersion::new("rustc", "1.98.0")?,
    ])
}

/// A request over `tree` with the fixed identity every test uses.
fn request<'a>(
    source: RepositorySource,
    tree: SourceTree<'a>,
    facts: &'a WorkingTreeFacts,
    policy: &'a PathPolicy,
) -> Result<SnapshotRequest<'a>, Box<dyn Error>> {
    Ok(SnapshotRequest {
        repository: repository()?,
        source,
        tree,
        facts,
        policy,
        captured_at: CAPTURED_AT,
        parent_snapshots: Vec::new(),
        submodule_refs: Vec::new(),
        analysis_policy_hash: policy_hash(),
        tool_versions: tools()?,
    })
}

/// Every file under `root`, path to bytes.
fn read_all(root: &Path) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn Error>> {
    let mut found = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                found.insert(relative, fs::read(&path)?);
            }
        }
    }
    Ok(found)
}

// ---------------------------------------------------------------------------
// The in-memory doubles. Neither reaches the operating system or the network.
// ---------------------------------------------------------------------------

/// A `DeviceKeystore` that holds secrets in a map.
///
/// It is the `P2-K1` trait rather than a keystore this crate invented, so the
/// path a token takes here is the path the reviewed native broker is bound
/// into behind that crate's `os-keystore` feature.
#[derive(Debug, Default)]
struct MemoryKeystore {
    held: RefCell<BTreeMap<String, Vec<u8>>>,
}

impl DeviceKeystore for MemoryKeystore {
    fn provider(&self) -> &str {
        "memory-test-double"
    }

    fn seal(&self, label: &str, secret: &[u8]) -> Result<Vec<u8>, KeystoreFailure> {
        self.held
            .borrow_mut()
            .insert(label.to_owned(), secret.to_vec());
        // The blob is a handle, not the secret: it names the label the broker
        // holds the material under and carries none of it.
        Ok(format!("handle:{label}").into_bytes())
    }

    fn open(&self, label: &str, blob: &[u8]) -> Result<Zeroizing<Vec<u8>>, KeystoreFailure> {
        if blob != format!("handle:{label}").as_bytes() {
            return Err(KeystoreFailure::InvalidBlob);
        }
        self.held
            .borrow()
            .get(label)
            .map(|secret| Zeroizing::new(secret.clone()))
            .ok_or(KeystoreFailure::NotFound)
    }
}

/// A `GitHubRepositoryReader` that serves entries from memory.
///
/// It authorizes the token first, so a test that hands it an expired or
/// out-of-scope token observes the refusal rather than the tree.
struct MemoryReader {
    entries: Vec<SourceEntry>,
}

impl GitHubRepositoryReader for MemoryReader {
    fn read_tree(
        &self,
        token: &FineGrainedToken,
        repository: &GitHubRepository,
        _commit: &CommitId,
        now: u64,
    ) -> Result<Vec<SourceEntry>, GitHubError> {
        token.authorize(repository, TokenPermission::ContentsRead, now)?;
        Ok(self
            .entries
            .iter()
            .map(|entry| SourceEntry::new(entry.path(), Vec::new()))
            .collect())
    }
}

/// A token scoped to `repository`, valid for half the maximum lifetime.
fn token_for(repository: &GitHubRepository) -> Result<FineGrainedToken, Box<dyn Error>> {
    let scope = TokenScope::new(
        repository.clone(),
        vec![TokenPermission::MetadataRead, TokenPermission::ContentsRead],
    )?;
    let lifetime = TokenLifetime::new(CAPTURED_AT, CAPTURED_AT + MAX_TOKEN_LIFETIME_MILLIS / 2)?;
    Ok(FineGrainedToken::new(
        scope,
        lifetime,
        b"synthetic-token-material".to_vec(),
    ))
}

// ---------------------------------------------------------------------------
// The spy. It wraps the real stages and records the order it entered them in.
// ---------------------------------------------------------------------------

/// Wraps a real stage set and records each stage as it is entered.
///
/// The inner stages are `LocalStages`, so what the spy observes is the
/// production path rather than a re-implementation of it. Each method records
/// its own name *before* delegating, so a stage that fails is still counted:
/// an ordering claim has to see the calls that error as well as the ones that
/// return.
struct Spy<S> {
    inner: S,
    calls: Vec<&'static str>,
}

impl<S> Spy<S> {
    const fn around(inner: S) -> Self {
        Self {
            inner,
            calls: Vec::new(),
        }
    }

    fn count(&self, stage: &str) -> usize {
        self.calls.iter().filter(|call| **call == stage).count()
    }
}

impl<S: SnapshotStages> SnapshotStages for Spy<S> {
    fn permission_and_secret_gate(
        &mut self,
        request: &SnapshotRequest<'_>,
    ) -> Result<AdmittedPaths, RepositoryError> {
        self.calls.push("gate");
        self.inner.permission_and_secret_gate(request)
    }

    fn inventory(
        &mut self,
        request: &SnapshotRequest<'_>,
        admitted: &AdmittedPaths,
    ) -> Result<Inventory, RepositoryError> {
        self.calls.push("inventory");
        self.inner.inventory(request, admitted)
    }

    fn freeze(
        &mut self,
        request: &SnapshotRequest<'_>,
        inventory: Inventory,
    ) -> Result<RepositorySnapshot, RepositoryError> {
        self.calls.push("freeze");
        self.inner.freeze(request, inventory)
    }

    fn index(&mut self, snapshot: &RepositorySnapshot) -> Result<IndexReceipt, RepositoryError> {
        self.calls.push("index");
        self.inner.index(snapshot)
    }
}

// ---------------------------------------------------------------------------
// eight_source_kinds_snapshot_read_only
// ---------------------------------------------------------------------------

/// All eight of section 17.1's inputs produce a read-only snapshot.
///
/// The eight are *enumerated*, not counted: the table below is compared with
/// `RepositorySource::ALL` as a whole set in both directions, so removing one
/// arm fails as a missing key and adding a ninth to the enum without a row here
/// fails as an extra one. A count of eight would pass with one arm exercised
/// twice.
#[test]
fn eight_source_kinds_snapshot_read_only() -> TestResult {
    let clean = clean_facts(Some("main"))?;
    let dirty = dirty_facts()?;
    let nothing = WorkingTreeFacts::none();
    let policy = PathPolicy::new();
    let entries = clean_entries();
    let spec_entries = spec_only_entries();

    // The two GitHub arms read their tree through the credential path rather
    // than being handed one: the token is sealed into the keystore, borrowed
    // back, and presented to the reader, which authorizes it before serving.
    let repository = GitHubRepository::new("dnhynk", "orders")?;
    let store = CredentialStore::new(MemoryKeystore::default());
    let sealed = store.seal(&token_for(&repository)?)?;
    let borrowed = store.borrow(&sealed, CAPTURED_AT)?;
    let reader = MemoryReader {
        entries: clean_entries(),
    };
    let served = reader.read_tree(&borrowed, &repository, &head()?, CAPTURED_AT)?;
    assert_eq!(
        served.len(),
        clean_entries().len(),
        "the reader served a different number of entries than the fixture holds"
    );

    let mut covered: BTreeSet<RepositorySource> = BTreeSet::new();
    let mut observed: BTreeMap<RepositorySource, SnapshotType> = BTreeMap::new();

    for (source, facts) in [
        (RepositorySource::LocalDirectory, &clean),
        (RepositorySource::Branch, &clean),
        (RepositorySource::Commit, &clean),
        (RepositorySource::DirtyWorktree, &dirty),
    ] {
        let directory = TempDir::new()?;
        write_tree(directory.path(), &clean_files())?;
        if source == RepositorySource::DirtyWorktree {
            write_tree(directory.path(), &[("notes/scratch.md", "scratch\n")])?;
        }
        let before = read_all(directory.path())?;
        let (captured, documents) = capture_local(&request(
            source,
            SourceTree::Directory(directory.path()),
            facts,
            &policy,
        )?)?;
        let after = read_all(directory.path())?;
        assert_eq!(
            before,
            after,
            "{} changed the bytes under its root",
            source.as_str()
        );
        assert!(
            sealed_document_count(&documents) > 0,
            "{} sealed no document",
            source.as_str()
        );
        covered.insert(source);
        observed.insert(source, captured.snapshot.snapshot_type());
    }

    for (source, entry_set, facts) in [
        (RepositorySource::GitHubPublic, &entries, &clean),
        (RepositorySource::GitHubPrivate, &entries, &clean),
        (RepositorySource::Archive, &entries, &nothing),
        (RepositorySource::SpecOnly, &spec_entries, &nothing),
    ] {
        let (captured, _) = capture_local(&request(
            source,
            SourceTree::Entries(entry_set),
            facts,
            &policy,
        )?)?;
        covered.insert(source);
        observed.insert(source, captured.snapshot.snapshot_type());
    }

    assert_eq!(
        covered,
        RepositorySource::ALL.into_iter().collect::<BTreeSet<_>>(),
        "the eight inputs and the exercised set differ"
    );

    // And each one resolved to the snapshot type section 17.2 gives it. This is
    // the half a count cannot express: `Archive` is `ARCHIVE` and not
    // `GIT_COMMIT` however it was reached.
    let expected: BTreeMap<RepositorySource, SnapshotType> = [
        (RepositorySource::LocalDirectory, SnapshotType::GitCommit),
        (RepositorySource::GitHubPublic, SnapshotType::GitCommit),
        (RepositorySource::GitHubPrivate, SnapshotType::GitCommit),
        (RepositorySource::Archive, SnapshotType::Archive),
        (RepositorySource::Branch, SnapshotType::GitCommit),
        (RepositorySource::Commit, SnapshotType::GitCommit),
        (RepositorySource::DirtyWorktree, SnapshotType::DirtyWorktree),
        (RepositorySource::SpecOnly, SnapshotType::SpecOnly),
    ]
    .into_iter()
    .collect();
    assert_eq!(
        observed, expected,
        "an input resolved to the wrong snapshot type"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// analyzer_cannot_mutate_source_or_open_a_socket
// ---------------------------------------------------------------------------

/// Three separate claims at three separate levels, each tested where it holds.
///
/// This test deliberately does not claim the operating system refuses anything.
/// `P2-G4` measured that, per platform, with error numbers, in
/// `docs/contracts/worker-sandbox.md`. What is here is what this crate and
/// `P2-G7`'s matrix decide.
#[test]
fn analyzer_cannot_mutate_source_or_open_a_socket() -> TestResult {
    // Level one: a whole capture and analysis leaves the tree byte-identical,
    // with the same set of paths.
    let directory = TempDir::new()?;
    write_tree(directory.path(), &clean_files())?;
    let before = read_all(directory.path())?;
    let facts = clean_facts(Some("main"))?;
    let policy = PathPolicy::new();
    let (captured, _) = capture_local(&request(
        RepositorySource::LocalDirectory,
        SourceTree::Directory(directory.path()),
        &facts,
        &policy,
    )?)?;
    let receipt = analyze(&captured.snapshot);
    assert_eq!(receipt.snapshot_id(), captured.snapshot.snapshot_id());
    let after = read_all(directory.path())?;
    assert_eq!(before, after, "a capture and analysis changed the source");
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>(),
        "a capture and analysis added or removed a path"
    );

    // Level two: `P2-G7`'s matrix, observed through `P2-G1`'s broker rather
    // than restated. The three capabilities the class holds are minted; the
    // three it does not are denied with an audit row. Asserting both halves is
    // what keeps a matrix that refuses everything from passing this.
    let broker = PermissionBroker::new_profile()?;
    for capability in [
        ProcessCapability::ReadArtifactRange,
        ProcessCapability::AnalyzeRepository,
        ProcessCapability::CreateClaim,
    ] {
        broker.mint_process_capability(
            "actor-analyzer",
            ProcessClass::RepositoryAnalyzer,
            capability,
            CAPTURED_AT,
        )?;
    }
    for capability in [
        ProcessCapability::OpenOutboundSocket,
        ProcessCapability::WriteStagedArtifact,
        ProcessCapability::ReadKeyMaterial,
    ] {
        let refused = broker.mint_process_capability(
            "actor-analyzer",
            ProcessClass::RepositoryAnalyzer,
            capability,
            CAPTURED_AT,
        );
        assert!(
            refused.is_err(),
            "the broker minted {} for the repository analyzer",
            capability.as_str()
        );
        let denials: Vec<_> = broker
            .audit_rows()?
            .into_iter()
            .filter(|row| {
                row.process_class == ProcessClass::RepositoryAnalyzer
                    && row.capability == capability
                    && row.decision == Decision::Deny
            })
            .collect();
        assert_eq!(
            denials.len(),
            1,
            "the refusal of {} left no single DENY audit row",
            capability.as_str()
        );
        assert_eq!(denials[0].reason_code, Some(ReasonCode::NoGrant));
    }

    // Level three: the analysis entry point holds nothing to write through. It
    // is handed a frozen snapshot and the snapshot carries no root: reading
    // every accessor it has yields owned data and no path back to the tree.
    assert!(
        captured
            .snapshot
            .manifest()
            .iter()
            .all(|entry| !entry.path().starts_with('/')),
        "a manifest row carries an absolute path"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// secret_gate_precedes_indexer
// ---------------------------------------------------------------------------

/// The gate runs before the indexer, observed as a call order and two counts.
///
/// The spy wraps the real `LocalStages`, so the sequence it records is the
/// production one. Two claims are made, and the second is the one that fails if
/// the scan is moved behind the indexer:
///
/// * on a clean source the recorded sequence is exactly gate, inventory,
///   freeze, index, each once;
/// * on a blocked source the indexer's count is **zero** — the pipeline stops
///   at the gate, so there is no order in which indexing has already happened.
#[test]
fn secret_gate_precedes_indexer() -> TestResult {
    let clean = clean_facts(Some("main"))?;
    let policy = PathPolicy::new();
    let entries = clean_entries();

    // Every one of the eight inputs, so the ordering is not a property of one
    // arm. The four directory arms and the four in-memory ones are driven the
    // same way.
    let mut covered: BTreeSet<RepositorySource> = BTreeSet::new();
    for source in RepositorySource::ALL {
        let directory = TempDir::new()?;
        write_tree(directory.path(), &clean_files())?;
        let tree = if matches!(
            source,
            RepositorySource::Archive
                | RepositorySource::SpecOnly
                | RepositorySource::GitHubPublic
                | RepositorySource::GitHubPrivate
        ) {
            SourceTree::Entries(&entries)
        } else {
            SourceTree::Directory(directory.path())
        };
        let mut spy = Spy::around(LocalStages::new());
        capture(&mut spy, &request(source, tree, &clean, &policy)?)?;
        assert_eq!(
            spy.calls,
            vec!["gate", "inventory", "freeze", "index"],
            "{} ran the stages in another order",
            source.as_str()
        );
        covered.insert(source);
    }
    assert_eq!(
        covered,
        RepositorySource::ALL.into_iter().collect::<BTreeSet<_>>(),
        "the ordering was not observed for every input"
    );

    // The blocked half. A file the point-1 policy does not remove, holding
    // something the content scan recognises.
    let blocked = TempDir::new()?;
    write_tree(blocked.path(), &clean_files())?;
    write_tree(
        blocked.path(),
        &[(
            "src/orders/config.rs",
            "pub const TOKEN: &str = \"ghp_0123456789abcdefghijklmnopqrstuvwxyz\";\n",
        )],
    )?;
    let mut spy = Spy::around(LocalStages::new());
    let refused = capture(
        &mut spy,
        &request(
            RepositorySource::LocalDirectory,
            SourceTree::Directory(blocked.path()),
            &clean,
            &policy,
        )?,
    );
    assert_eq!(refused.err(), Some(RepositoryError::SecretGateBlocked));
    assert_eq!(spy.count("gate"), 1, "the gate did not run exactly once");
    assert_eq!(
        spy.count("index"),
        0,
        "the indexer ran on a source the gate blocked"
    );
    assert_eq!(
        spy.count("inventory"),
        0,
        "the inventory ran on a source the gate blocked"
    );
    Ok(())
}

/// An admission cannot be carried from one capture into another.
///
/// This is the other half of "there is no second path": the type stops a stage
/// from being skipped, and the request digest stops an earlier stage's answer
/// from standing in for this one's.
#[test]
fn an_admission_cannot_be_reused_for_another_request() -> TestResult {
    let directory = TempDir::new()?;
    write_tree(directory.path(), &clean_files())?;
    let facts = clean_facts(Some("main"))?;
    let policy = PathPolicy::new();
    let mut stages = LocalStages::new();

    let first = request(
        RepositorySource::LocalDirectory,
        SourceTree::Directory(directory.path()),
        &facts,
        &policy,
    )?;
    let admitted = stages.permission_and_secret_gate(&first)?;

    // A second request differing only in its capture time.
    let mut second = request(
        RepositorySource::LocalDirectory,
        SourceTree::Directory(directory.path()),
        &facts,
        &policy,
    )?;
    second.captured_at = CAPTURED_AT + 1;
    assert_ne!(first.digest(), second.digest());
    assert_eq!(
        stages.inventory(&second, &admitted).err(),
        Some(RepositoryError::AdmissionMismatch),
        "an admission decided for one request admitted another"
    );
    // And the admission still works for the request it was decided for.
    stages.inventory(&first, &admitted)?;
    Ok(())
}

/// A path the policy removes is never opened.
///
/// The observed read set is compared against the set the policy admits,
/// computed independently of the walk, so this is not the walk agreeing with
/// itself. All four exclusion reasons are exercised.
#[test]
fn analyzer_never_sees_an_excluded_path() -> TestResult {
    let directory = TempDir::new()?;
    write_tree(directory.path(), &clean_files())?;
    write_tree(
        directory.path(),
        &[
            (".gitignore", "/build/\n*.log\n"),
            ("build/artifact.bin", "ignored\n"),
            ("run.log", "ignored\n"),
            ("vendor/copy.rs", "// vendored\n"),
            ("private/notes.md", "user excluded\n"),
        ],
    )?;
    let ignore_text = fs::read_to_string(directory.path().join(".gitignore"))?;
    let policy = PathPolicy::new()
        .ignoring(PathPolicy::parse_gitignore(&ignore_text)?)
        .denying(vec![PathRule::Prefix("vendor".to_owned())])
        .excluding(vec![PathRule::Prefix("private".to_owned())]);

    let facts = clean_facts(Some("main"))?;
    let mut stages = LocalStages::new();
    let capture_request = request(
        RepositorySource::LocalDirectory,
        SourceTree::Directory(directory.path()),
        &facts,
        &policy,
    )?;
    let admitted = stages.permission_and_secret_gate(&capture_request)?;
    let inventory = stages.inventory(&capture_request, &admitted)?;

    let opened: BTreeSet<String> = inventory.read_paths().iter().cloned().collect();
    let expected = academic_repository::gate::admitted_paths(
        &policy,
        &SourceTree::Directory(directory.path()).paths()?,
    );
    assert_eq!(
        opened, expected,
        "the inventory opened a path the policy removed"
    );

    // Every reason is exercised, as a whole set rather than as a count.
    let reasons: BTreeSet<ExclusionReason> = admitted
        .excluded()
        .iter()
        .map(academic_repository::ExcludedPath::reason)
        .collect();
    assert_eq!(
        reasons,
        ExclusionReason::ALL.into_iter().collect::<BTreeSet<_>>(),
        "an exclusion reason was never exercised"
    );
    for excluded in [
        "build/artifact.bin",
        "run.log",
        "vendor/copy.rs",
        "private/notes.md",
        ".env",
    ] {
        assert!(
            !opened.contains(excluded),
            "{excluded} was opened despite being excluded"
        );
    }
    // A negated ignore rule is refused rather than mis-read.
    assert!(matches!(
        PathPolicy::parse_gitignore("!keep.md"),
        Err(RepositoryError::UnsupportedIgnoreRule(_))
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// dirty_worktree_is_not_head
// ---------------------------------------------------------------------------

/// A dirty working tree is never recorded as the commit it was checked out
/// from, whichever of the eight inputs named it.
#[test]
fn dirty_worktree_is_not_head() -> TestResult {
    let policy = PathPolicy::new();
    let clean = clean_facts(Some("main"))?;
    let dirty = dirty_facts()?;

    let clean_directory = TempDir::new()?;
    write_tree(clean_directory.path(), &clean_files())?;
    let (at_head, _) = capture_local(&request(
        RepositorySource::Commit,
        SourceTree::Directory(clean_directory.path()),
        &clean,
        &policy,
    )?)?;

    let dirty_directory = TempDir::new()?;
    write_tree(dirty_directory.path(), &clean_files())?;
    write_tree(
        dirty_directory.path(),
        &[
            (
                "src/orders/service.rs",
                "// Places one order, twice.\npub fn place() -> u32 {\n    2\n}\n",
            ),
            ("notes/scratch.md", "scratch\n"),
        ],
    )?;
    let (at_worktree, _) = capture_local(&request(
        RepositorySource::Commit,
        SourceTree::Directory(dirty_directory.path()),
        &dirty,
        &policy,
    )?)?;

    assert_eq!(at_head.snapshot.snapshot_type(), SnapshotType::GitCommit);
    assert_eq!(
        at_worktree.snapshot.snapshot_type(),
        SnapshotType::DirtyWorktree,
        "a dirty tree named as a commit was recorded as one"
    );
    // Both name the same HEAD, which is what makes the distinction meaningful:
    // the difference is not that one has no commit.
    assert_eq!(at_head.snapshot.commit(), at_worktree.snapshot.commit());
    assert_ne!(
        at_head.snapshot.manifest_digest(),
        at_worktree.snapshot.manifest_digest(),
        "the two snapshots have the same manifest digest"
    );
    assert_ne!(
        at_head.snapshot.snapshot_id(),
        at_worktree.snapshot.snapshot_id()
    );
    assert!(at_head.snapshot.dirty().is_none());
    assert!(at_worktree.snapshot.dirty().is_some());

    // And no input at all resolves a dirty tree to `GIT_COMMIT`. This is the
    // enumeration: the claim is about the whole vocabulary, not about the arm
    // the test above happened to use.
    for source in RepositorySource::ALL {
        let (captured, _) = capture_local(&request(
            source,
            SourceTree::Directory(dirty_directory.path()),
            &dirty,
            &policy,
        )?)?;
        assert_ne!(
            captured.snapshot.snapshot_type(),
            SnapshotType::GitCommit,
            "{} recorded a dirty tree as its commit",
            source.as_str()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// tracked_and_untracked_are_both_manifested
// ---------------------------------------------------------------------------

/// Both halves of the dirty manifest are present, and each holds what it should.
#[test]
fn tracked_and_untracked_are_both_manifested() -> TestResult {
    let directory = TempDir::new()?;
    write_tree(directory.path(), &clean_files())?;
    write_tree(
        directory.path(),
        &[
            (
                "src/orders/service.rs",
                "// Places one order, twice.\npub fn place() -> u32 {\n    2\n}\n",
            ),
            ("notes/scratch.md", "scratch\n"),
        ],
    )?;
    let facts = dirty_facts()?;
    let policy = PathPolicy::new();
    let (captured, _) = capture_local(&request(
        RepositorySource::DirtyWorktree,
        SourceTree::Directory(directory.path()),
        &facts,
        &policy,
    )?)?;
    let dirty = captured
        .snapshot
        .dirty()
        .ok_or("a dirty tree produced no dirty manifest")?;

    assert_eq!(
        dirty
            .tracked()
            .iter()
            .map(academic_repository::ManifestEntry::path)
            .collect::<Vec<_>>(),
        vec!["src/orders/service.rs"],
        "the tracked half is not what version control reported as modified"
    );
    assert_eq!(
        dirty
            .untracked()
            .iter()
            .map(academic_repository::ManifestEntry::path)
            .collect::<Vec<_>>(),
        vec!["notes/scratch.md"],
        "the untracked half is not what version control reported as untracked"
    );

    // Both labels appear, as a whole set. A manifest that recorded only one
    // half would satisfy a count of two entries and fails this.
    let kinds: BTreeSet<DirtyKind> = dirty.entries().into_iter().map(|(kind, _)| kind).collect();
    assert_eq!(
        kinds,
        DirtyKind::ALL.into_iter().collect::<BTreeSet<_>>(),
        "one half of the dirty manifest is empty"
    );

    // The patch digest covers both halves: a change to either moves it.
    let untracked_only = WorkingTreeFacts::checkout(
        head()?,
        Some("main".to_owned()),
        facts.tracked().to_vec(),
        Vec::new(),
        vec!["notes/scratch.md".to_owned()],
    );
    let (other, _) = capture_local(&request(
        RepositorySource::DirtyWorktree,
        SourceTree::Directory(directory.path()),
        &untracked_only,
        &policy,
    )?)?;
    let other_dirty = other.snapshot.dirty().ok_or("no dirty manifest")?;
    assert!(other_dirty.tracked().is_empty());
    assert_ne!(
        dirty.patch_digest(),
        other_dirty.patch_digest(),
        "the patch digest does not cover the tracked half"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// secret_hash_disclosure_requires_a_recorded_decision
// ---------------------------------------------------------------------------

/// A secret file's digest exists only where a decision was recorded.
#[test]
fn secret_hash_disclosure_requires_a_recorded_decision() -> TestResult {
    let directory = TempDir::new()?;
    write_tree(directory.path(), &clean_files())?;
    write_tree(
        directory.path(),
        &[
            (
                "src/orders/config.rs",
                "pub const TOKEN: &str = \"ghp_0123456789abcdefghijklmnopqrstuvwxyz\";\n",
            ),
            (
                "deploy/key.txt",
                "-----BEGIN RSA PRIVATE KEY-----\nAAAA\n-----END RSA PRIVATE KEY-----\n",
            ),
        ],
    )?;
    let facts = clean_facts(Some("main"))?;
    let policy = PathPolicy::new();
    let capture_request = request(
        RepositorySource::LocalDirectory,
        SourceTree::Directory(directory.path()),
        &facts,
        &policy,
    )?;

    // The whole capture refuses, so no snapshot holding a digest is produced at
    // all.
    assert_eq!(
        capture_local(&capture_request).err(),
        Some(RepositoryError::SecretGateBlocked)
    );

    let mut stages = LocalStages::new();
    let admitted = stages.permission_and_secret_gate(&capture_request)?;
    assert_eq!(admitted.result(), SecretScanResult::Blocked);
    assert_eq!(admitted.result().decision(), Decision::Deny);
    assert_eq!(
        admitted.findings().len(),
        2,
        "the two secret files were not both found"
    );

    // Default deny: every finding the gate produced holds no digest and no
    // decision. This is a whole-set assertion over the findings rather than a
    // check of one of them.
    for finding in admitted.findings() {
        assert_eq!(
            finding.blob_digest(),
            None,
            "{} carries a digest with no recorded decision",
            finding.path()
        );
        assert_eq!(finding.disclosure(), None);
    }
    // The reasons, as a whole set. `ReasonCode` is `P2-G1`'s closed vocabulary
    // and carries no `Ord`, so the set is built over the stable spellings that
    // crate publishes rather than over the values.
    assert_eq!(
        admitted
            .findings()
            .iter()
            .map(|finding| finding.reason().as_str())
            .collect::<BTreeSet<_>>(),
        [ReasonCode::SecretPattern.as_str()]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );

    // And with one, exactly the file the decision names gains one.
    let bytes = fs::read(directory.path().join("deploy/key.txt"))?;
    let decision = DisclosureDecision::record(
        "decision-1",
        "actor-user",
        "the user asked for the digest so a rotation can be verified",
        CAPTURED_AT,
    )?;
    let disclosed = admitted
        .findings()
        .iter()
        .find(|finding| finding.path() == "deploy/key.txt")
        .ok_or("the private key file was not found")?
        .clone()
        .disclose(decision.clone(), &bytes);
    assert_eq!(disclosed.blob_digest(), Some(&ContentDigest::of(&bytes)));
    assert_eq!(
        disclosed.disclosure().map(DisclosureDecision::decision_id),
        Some("decision-1")
    );
    // The finding it was derived from is unchanged: disclosure consumes a copy
    // and returns a new value rather than mutating one in place.
    for finding in admitted.findings() {
        assert_eq!(finding.blob_digest(), None);
    }

    // A decision missing any field is not a record.
    for (id, actor, reason) in [
        ("", "actor-user", "why"),
        ("decision-2", "", "why"),
        ("decision-2", "actor-user", ""),
    ] {
        assert_eq!(
            DisclosureDecision::record(id, actor, reason, CAPTURED_AT).err(),
            Some(RepositoryError::EmptyDecisionField)
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// snapshot_is_immutable_after_source_change
// ---------------------------------------------------------------------------

/// A frozen snapshot does not change when the tree it was taken from does.
#[test]
fn snapshot_is_immutable_after_source_change() -> TestResult {
    let directory = TempDir::new()?;
    write_tree(directory.path(), &clean_files())?;
    let facts = clean_facts(Some("main"))?;
    let policy = PathPolicy::new();
    let (captured, _) = capture_local(&request(
        RepositorySource::LocalDirectory,
        SourceTree::Directory(directory.path()),
        &facts,
        &policy,
    )?)?;
    let Capture { snapshot, receipt } = captured;

    let before_id = snapshot.snapshot_id().to_owned();
    let before_manifest = snapshot.manifest().to_vec();
    let before_digest = snapshot.manifest_digest().clone();
    let before_receipt = receipt.clone();

    // Change the tree three ways: rewrite a file, add one, remove one.
    write_tree(
        directory.path(),
        &[
            ("README.md", "# Orders\n\nRewritten.\n"),
            ("src/orders/refund.rs", "pub fn refund() {}\n"),
        ],
    )?;
    fs::remove_file(directory.path().join("Cargo.toml"))?;

    // The value is unchanged, field by field.
    assert_eq!(snapshot.snapshot_id(), before_id);
    assert_eq!(snapshot.manifest(), before_manifest.as_slice());
    assert_eq!(snapshot.manifest_digest(), &before_digest);
    assert_eq!(analyze(&snapshot), before_receipt);

    // And the tree really did change, so the assertion above is not vacuous: a
    // fresh capture of the same directory differs.
    let (fresh, _) = capture_local(&request(
        RepositorySource::LocalDirectory,
        SourceTree::Directory(directory.path()),
        &facts,
        &policy,
    )?)?;
    assert_ne!(
        fresh.snapshot.manifest_digest(),
        &before_digest,
        "the source did not actually change"
    );
    assert_ne!(fresh.snapshot.snapshot_id(), before_id);
    Ok(())
}

// ---------------------------------------------------------------------------
// github_token_is_repo_scoped_read_only_and_expiring
// ---------------------------------------------------------------------------

/// The three properties, tested one at a time.
#[test]
fn github_token_is_repo_scoped_read_only_and_expiring() -> TestResult {
    let owned = GitHubRepository::new("dnhynk", "orders")?;
    let other = GitHubRepository::new("dnhynk", "invoices")?;
    let token = token_for(&owned)?;

    // Repo-scoped. Exact equality, not a prefix and not an owner scope.
    assert!(token.scope().covers(&owned));
    assert!(!token.scope().covers(&other));
    assert_eq!(
        token.authorize(&other, TokenPermission::ContentsRead, CAPTURED_AT),
        Err(GitHubError::OutOfScope)
    );

    // Read-only. Every permission in the vocabulary maps to `Access::Read`
    // through a total function, and the mapping is compared as a whole set so a
    // fourth permission that returned something else would fail here as well as
    // failing to compile.
    let accesses: BTreeSet<Access> = TokenPermission::ALL
        .into_iter()
        .map(TokenPermission::access)
        .collect();
    assert_eq!(
        accesses,
        [Access::Read].into_iter().collect::<BTreeSet<_>>()
    );
    assert!(
        TokenPermission::ALL
            .into_iter()
            .all(|permission| permission.as_str().ends_with(":read")),
        "a permission spelling is not a read"
    );
    // A permission the scope does not carry is refused even though it is a read.
    assert_eq!(
        token.authorize(&owned, TokenPermission::IssuesRead, CAPTURED_AT),
        Err(GitHubError::MissingPermission)
    );

    // Expiring. The interval is half-open and bounded.
    let lifetime = token.lifetime();
    assert!(token.is_valid_at(lifetime.issued_at()));
    assert!(token.is_valid_at(lifetime.expires_at() - 1));
    assert!(!token.is_valid_at(lifetime.expires_at()));
    assert_eq!(
        token.authorize(&owned, TokenPermission::ContentsRead, lifetime.expires_at()),
        Err(GitHubError::Expired)
    );
    assert_eq!(
        TokenLifetime::new(CAPTURED_AT, CAPTURED_AT).err(),
        Some(GitHubError::MalformedLifetime),
        "an empty lifetime was accepted"
    );
    assert_eq!(
        TokenLifetime::new(CAPTURED_AT, CAPTURED_AT - 1).err(),
        Some(GitHubError::MalformedLifetime),
        "a backwards lifetime was accepted"
    );
    assert_eq!(
        TokenLifetime::new(CAPTURED_AT, CAPTURED_AT + MAX_TOKEN_LIFETIME_MILLIS + 1).err(),
        Some(GitHubError::MalformedLifetime),
        "a lifetime longer than the bound was accepted"
    );
    TokenLifetime::new(CAPTURED_AT, CAPTURED_AT + MAX_TOKEN_LIFETIME_MILLIS)?;

    // The material rests in the operating-system credential store. What the
    // sealed value carries is the label, the provider, the scope and the
    // lifetime; borrowing it back after expiry refuses before the broker is
    // asked at all.
    let store = CredentialStore::new(MemoryKeystore::default());
    let sealed = store.seal(&token)?;
    assert_eq!(sealed.label(), "dnhynk/orders");
    assert_eq!(sealed.provider(), "memory-test-double");
    assert_eq!(sealed.lifetime(), lifetime);
    let borrowed = store.borrow(&sealed, CAPTURED_AT)?;
    assert_eq!(borrowed.scope(), token.scope());
    assert_eq!(
        store.borrow(&sealed, lifetime.expires_at()).err(),
        Some(GitHubError::Expired)
    );

    // And the token's `Debug` prints a length rather than the material. The
    // material itself is the fixture's, so a leak would be visible as its own
    // bytes.
    let rendered = format!("{borrowed:?}");
    assert!(
        !rendered.contains("synthetic-token-material"),
        "the token's Debug printed its material"
    );
    assert!(rendered.contains("<redacted:"));

    // A reader is refused an expired or out-of-scope token before it serves a
    // byte.
    let reader = MemoryReader {
        entries: clean_entries(),
    };
    assert_eq!(
        reader
            .read_tree(&borrowed, &other, &head()?, CAPTURED_AT)
            .err(),
        Some(GitHubError::OutOfScope)
    );
    assert_eq!(
        reader
            .read_tree(&borrowed, &owned, &head()?, lifetime.expires_at())
            .err(),
        Some(GitHubError::Expired)
    );
    reader.read_tree(&borrowed, &owned, &head()?, CAPTURED_AT)?;
    Ok(())
}

/// Every byte, against the class the doc comments state.
///
/// `P2-A5`'s F6 measured this gap: widening the admitted character class by
/// exactly one byte in `src/snapshot.rs` and `src/github.rs` left this crate at 0 failed, and the workspace
/// byte-identical to its baseline. `P2-R4` had the measurement already; this is
/// that test, ported. It walks the whole byte range and compares admitted
/// against belongs, so it is a measurement rather than three examples, and the
/// class it compares against is **written here** rather than read from the
/// crate -- an oracle that asked the code what it admits would agree with the
/// code whatever the code says.
#[test]
fn every_identifier_is_the_shape_this_crate_admits() -> TestResult {
    // `[A-Za-z0-9._/-]` for a repository identity; the same without `/` for
    // each half of a GitHub coordinate.
    let repository_class =
        |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/');
    let github_class =
        |byte: u8| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-');

    for byte in 0_u8..=127 {
        let candidate = format!("a{}b", char::from(byte));
        let taken = RepositoryId::new(candidate.clone()).is_ok();
        assert_eq!(
            taken,
            repository_class(byte),
            "byte {byte} in {candidate:?} is admitted {taken} by RepositoryId and belongs {}",
            repository_class(byte)
        );
        for (owner, name) in [(candidate.as_str(), "n"), ("o", candidate.as_str())] {
            let taken = GitHubRepository::new(owner, name).is_ok();
            assert_eq!(
                taken,
                github_class(byte),
                "byte {byte} in {candidate:?} is admitted {taken} by GitHubRepository and belongs {}",
                github_class(byte)
            );
        }
    }

    // Beyond ASCII, where a byte-wise reader and a character-wise one disagree.
    for outside in [
        "\u{ac1c}\u{b150}",
        "a\u{ac1c}b",
        "a\u{00e9}b",
        "a\u{1f600}b",
    ] {
        assert!(
            RepositoryId::new(outside).is_err(),
            "{outside:?} was admitted as a repository identity"
        );
        assert!(
            GitHubRepository::new(outside, "n").is_err(),
            "{outside:?} was admitted as a GitHub owner"
        );
    }

    // The length boundaries the doc comments state, on both sides, and empty.
    assert!(RepositoryId::new("a".repeat(128)).is_ok());
    assert!(RepositoryId::new("a".repeat(129)).is_err());
    assert!(RepositoryId::new(String::new()).is_err());
    assert!(GitHubRepository::new("a".repeat(100), "n").is_ok());
    assert!(GitHubRepository::new("a".repeat(101), "n").is_err());
    assert!(GitHubRepository::new("o", "a".repeat(101)).is_err());
    assert!(GitHubRepository::new(String::new(), "n").is_err());
    assert!(GitHubRepository::new("o", String::new()).is_err());
    Ok(())
}
