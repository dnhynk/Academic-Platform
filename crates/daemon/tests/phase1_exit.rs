//! The exact Phase 1 crash, replay, and restore exit.
//!
//! For every identifier in the enumerated fault matrix this harness runs one
//! sequence and nothing else:
//!
//! 1. a fresh profile root begins empty;
//! 2. a harness-owned child ingests only the one allowlisted deterministic
//!    synthetic fixture;
//! 3. the child is killed at that fault point;
//! 4. a real daemon restarts on the same profile;
//! 5. deep doctor and startup reconciliation run;
//! 6. the same command is retried idempotently, twice;
//! 7. the canonical ledger is replayed from its stored signed envelopes;
//! 8. the profile is exported twice;
//! 9. it is backed up;
//! 10. it is restored into another empty profile;
//! 11. that profile's projections are rebuilt from empty;
//! 12. canonical heads, counts, and semantic checksums are compared.
//!
//! The observed outcome letter must equal the letter the matrix assigns, and
//! two stronger invariants are checked first for every fault: no normal-looking
//! partial canonical state, and no normal-looking canonical reference to a
//! missing or corrupt object.
//!
//! # What kills the child
//!
//! Twenty-five of the twenty-six faults are injected failpoints inside the
//! crate that owns the ordering they protect, compiled only by the non-default
//! `phase1-fault-injection` feature. `IPC01` is not, and the difference is
//! stated rather than smoothed over: see `fault_driver::IPC01_REALIZATION` and
//! `docs/development/phase1-exit.md`.
//!
//! # Why the child is this binary
//!
//! `academicd` has no crash switch and must not gain one. The harness child is
//! therefore this same test binary re-entered at
//! [`phase1_exit_fault_child`], which is the pattern
//! `crates/vault/tests/crash.rs` and `crates/portability/tests/crash.rs`
//! already use. It links exactly the crates and features the harness was built
//! with, and it drives the same `LocalService`, `ProjectionRunner`, and
//! portability entry points the product uses.

#![cfg(feature = "phase1-fault-injection")]

#[path = "../../test-support/src/fault_driver.rs"]
mod fault_driver;
#[path = "../../test-support/src/oracle.rs"]
mod oracle;
#[path = "../../test-support/src/process.rs"]
mod process;

use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime},
};

use academic_core::{
    local_service::{LocalService, PHASE1_SYNTHETIC_FIXTURE_ID, mutable_request_digest},
    operations::{
        SYNTHETIC_INGEST_CAPABILITY, backup_synthetic_profile, diagnose_profile,
        ensure_synthetic_profile, export_synthetic_profile, fixture_predicate_policies,
        projection_builder_digest, projection_config_hash, projection_targets,
        restore_synthetic_profile, synthetic_ingest_request,
    },
};
use academic_daemon::{DaemonConfig, LocalEndpoint, RunningDaemon, SessionNonce, WriterQueue};
use academic_portability::{
    backup::{MANIFEST_FILE, find_unpublished_backups, remove_unpublished_backup},
    restore::{PROJECTION_SIDECAR_FILE, find_unpublished_restores, remove_unpublished_restore},
};
use academic_projections::runner::ProjectionRunner;
use academic_rpc::{
    FrameClass, LOCAL_CORE_PROTOCOL_NAME, LOCAL_CORE_PROTOCOL_VERSION,
    generated::{
        ClientHandshake, LocalCoreEnvelope, MutableRequest, MutableResponse, MutationStatus,
        ProtocolVersion, local_core_envelope,
    },
    read_envelope, write_envelope,
};
use academic_store::{
    STORE_DATABASE_FILE, path_policy::NativePathProbe, profile::open_synthetic_profile,
};
use academic_vault::{DomainKeyring, ReconcileOptions, Vault};
use tokio::io::{AsyncRead, AsyncWrite};

use fault_driver::{
    AbortAtAcceptance, AbortAtProjection, FaultSpec, FaultStage, FaultSubject, Outcome,
    PHASE1_EXIT_FAULTS, Reachability, acceptance_point, projection_point,
};
use oracle::{
    CanonicalFacts, Completeness, DoctorFacts, FaultEvidence, ReconcileFacts, RetryFacts,
    SubjectDisposition, VaultInventory, completeness, hex_lower,
};
use process::{CHILD_TIMEOUT, ChildRecord, run_bounded, self_invocation};

type TestResult = Result<(), Box<dyn Error>>;

/// Marks a child invocation of this binary.
const CHILD_VARIABLE: &str = "ACADEMIC_X1_CHILD";
/// Names the fault the child must take.
const FAULT_VARIABLE: &str = "ACADEMIC_X1_FAULT";
/// Names the disposable profile root the child owns.
const PROFILE_VARIABLE: &str = "ACADEMIC_X1_PROFILE";
/// Names the backup directory a restore-stage child reads.
const BACKUP_VARIABLE: &str = "ACADEMIC_X1_BACKUP";
/// Names the destination a backup- or restore-stage child writes.
const DESTINATION_VARIABLE: &str = "ACADEMIC_X1_DESTINATION";
/// Names the file the child creates at its checkpoint.
const READY_VARIABLE: &str = "ACADEMIC_X1_READY";

/// The frozen twenty-six identifiers, in matrix order.
///
/// `crates/test-support/src/lib.rs` and `crates/cli/src/commands/crash_replay.rs`
/// each hold this same list for their own purpose. Repeating it here and
/// asserting it is the repository's existing drift control: three independent
/// copies cannot silently diverge.
const FROZEN_FAULT_IDS: [&str; 26] = [
    "V01", "V02", "V03", "V04", "V05", "V06", "DB01", "DB02", "DB03", "DB04", "DB05", "DB06",
    "DB07", "PR01", "PR02", "PR03", "BK01", "BK02", "BK03", "BK04", "RS01", "RS02", "RS03", "RS04",
    "IPC01", "IPC02",
];

static NEXT_LANE: AtomicU64 = AtomicU64::new(0);

// ---------------------------------------------------------------------------
// Disposable lanes
// ---------------------------------------------------------------------------

/// macOS exposes `$TMPDIR` beneath the `/var` symlink and the native path
/// facade refuses to follow a link component, so lanes are reserved below the
/// real directory.
#[cfg(unix)]
fn temporary_base() -> io::Result<PathBuf> {
    fs::canonicalize(env::temp_dir())
}

/// Windows must not canonicalize: that yields the Win32 verbatim device
/// spelling the facade rejects, trading one refused spelling for another.
#[cfg(windows)]
fn temporary_base() -> io::Result<PathBuf> {
    Ok(env::temp_dir())
}

/// Base for the runtime lane, which the Unix endpoint bound constrains.
///
/// The whole assembled socket path travels in `sun_path`, so the runtime root
/// has to stay short. `/tmp` is the shortest link-free base every Unix offers.
#[cfg(unix)]
fn runtime_base() -> io::Result<PathBuf> {
    fs::canonicalize("/tmp").or_else(|_| temporary_base())
}

/// Windows named-pipe endpoints carry no comparable path bound.
#[cfg(windows)]
fn runtime_base() -> io::Result<PathBuf> {
    temporary_base()
}

/// One disposable directory tree owned by one fault run.
///
/// Every path the harness touches lives below a lane, and the lane is removed
/// when it drops. Nothing is written inside the repository worktree.
#[derive(Debug)]
struct Lane {
    root: PathBuf,
    runtime: PathBuf,
}

impl Lane {
    fn new(label: &str) -> io::Result<Self> {
        let sequence = NEXT_LANE.fetch_add(1, Ordering::Relaxed);
        let stamp = format!("{}-{sequence}", std::process::id());
        let root = temporary_base()?.join(format!("academic-x1-{label}-{stamp}"));
        // The runtime lane is separate so a long profile path cannot spend the
        // Unix endpoint budget the socket needs.
        let runtime = runtime_base()?.join(format!("ax1-{stamp}"));
        fs::create_dir_all(&root)?;
        fs::create_dir_all(&runtime)?;
        Ok(Self { root, runtime })
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    fn runtime(&self) -> &Path {
        &self.runtime
    }
}

impl Drop for Lane {
    fn drop(&mut self) {
        // A lane is disposable synthetic state; a failed removal is reported
        // but must not mask the assertion that failed first.
        if let Err(error) = fs::remove_dir_all(&self.root)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!("lane cleanup failed for {}: {error}", self.root.display());
        }
        if let Err(error) = fs::remove_dir_all(&self.runtime)
            && error.kind() != io::ErrorKind::NotFound
        {
            eprintln!(
                "runtime cleanup failed for {}: {error}",
                self.runtime.display()
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Local IPC client
// ---------------------------------------------------------------------------

trait ClientStream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T> ClientStream for T where T: AsyncRead + AsyncWrite + Unpin + Send {}

async fn connect(endpoint: &LocalEndpoint) -> io::Result<Box<dyn ClientStream>> {
    match endpoint {
        #[cfg(windows)]
        LocalEndpoint::NamedPipe(name) => {
            use tokio::net::windows::named_pipe::ClientOptions;
            Ok(Box::new(ClientOptions::new().open(name)?))
        }
        #[cfg(not(windows))]
        LocalEndpoint::NamedPipe(_) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Windows named pipes are not available",
        )),
        #[cfg(unix)]
        LocalEndpoint::UnixSocket(path) => {
            Ok(Box::new(tokio::net::UnixStream::connect(path).await?))
        }
        #[cfg(not(unix))]
        LocalEndpoint::UnixSocket(_) => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Unix sockets are not available",
        )),
    }
}

/// Performs one complete handshake-plus-command exchange over local IPC.
async fn client_exchange(
    endpoint: &LocalEndpoint,
    nonce: &SessionNonce,
    request: MutableRequest,
) -> Result<MutableResponse, Box<dyn Error>> {
    let mut stream = connect(endpoint).await?;
    let client = ClientHandshake {
        protocol_name: LOCAL_CORE_PROTOCOL_NAME.to_owned(),
        protocol_version: Some(ProtocolVersion {
            major: u32::from(LOCAL_CORE_PROTOCOL_VERSION.major),
            minor: u32::from(LOCAL_CORE_PROTOCOL_VERSION.minor),
        }),
        capability_ids: vec![
            SYNTHETIC_INGEST_CAPABILITY.to_owned(),
            nonce.capability_id(),
        ],
    };
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::ClientHandshake(client)),
    };
    write_envelope(&mut stream, &envelope, FrameClass::Handshake).await?;
    let response = read_envelope(&mut stream, FrameClass::Handshake).await?;
    if !matches!(
        response.payload,
        Some(local_core_envelope::Payload::ServerHandshake(_))
    ) {
        return Err("server did not return a handshake".into());
    }
    let envelope = LocalCoreEnvelope {
        payload: Some(local_core_envelope::Payload::MutableRequest(request)),
    };
    write_envelope(&mut stream, &envelope, FrameClass::Command).await?;
    match read_envelope(&mut stream, FrameClass::Command)
        .await?
        .payload
    {
        Some(local_core_envelope::Payload::MutableResponse(response)) => Ok(response),
        _ => Err("server did not return a mutable response".into()),
    }
}

fn retry_facts(response: &MutableResponse) -> RetryFacts {
    RetryFacts {
        status: match MutationStatus::try_from(response.status) {
            Ok(MutationStatus::Accepted) => "ACCEPTED".to_owned(),
            Ok(MutationStatus::Duplicate) => "DUPLICATE".to_owned(),
            Ok(MutationStatus::Rejected) => "REJECTED".to_owned(),
            _ => "UNSPECIFIED".to_owned(),
        },
        reason: response.reason.clone(),
        profile_revision: response.profile_revision,
        receipt_id: response
            .receipt
            .as_ref()
            .map(|receipt| hex_lower(&receipt.receipt_id))
            .unwrap_or_default(),
        response_digest: hex_lower(&response.response_digest),
        acceptance_range: response
            .acceptance_range
            .as_ref()
            .map(|range| (range.accept_seq_start, range.accept_seq_end)),
    }
}

fn idempotency_key_hex(request: &MutableRequest) -> String {
    hex_lower(&request.idempotency_key)
}

// ---------------------------------------------------------------------------
// Child entry point
// ---------------------------------------------------------------------------

/// The harness child. It performs one stage and dies at one named checkpoint.
///
/// It is a `#[test]` so the test binary can be re-entered at it by name, and it
/// returns immediately when the harness did not start it.
#[test]
fn phase1_exit_fault_child() -> TestResult {
    if env::var(CHILD_VARIABLE).ok().as_deref() != Some("1") {
        return Ok(());
    }
    let fault_id = env::var(FAULT_VARIABLE)?;
    let spec = fault_driver::spec(&fault_id).ok_or("child was given an unknown fault")?;
    let profile_root = required_path(PROFILE_VARIABLE)?;
    let ready = required_path(READY_VARIABLE)?;

    match spec.stage {
        FaultStage::Ingest => child_ingest(spec, &profile_root, &ready)?,
        FaultStage::ProjectionBuild => child_projection_build(spec, &profile_root, &ready)?,
        FaultStage::Backup => child_backup(&profile_root)?,
        FaultStage::Restore => child_restore(&profile_root)?,
        FaultStage::Admission => child_admission(spec, &profile_root, &ready)?,
    }
    Err(format!("the selected {fault_id} checkpoint did not terminate the child").into())
}

fn required_path(variable: &str) -> Result<PathBuf, Box<dyn Error>> {
    env::var_os(variable)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{variable} is required").into())
}

/// Opens the sole writer composition on a profile the child owns.
fn child_service(profile_root: &Path) -> Result<LocalService, Box<dyn Error>> {
    ensure_synthetic_profile(profile_root)?;
    let profile = open_synthetic_profile(profile_root, &NativePathProbe::default())?;
    let (service, _startup) = LocalService::open(profile, SystemTime::now())?;
    Ok(service)
}

fn ingest_request() -> Result<MutableRequest, Box<dyn Error>> {
    Ok(synthetic_ingest_request(PHASE1_SYNTHETIC_FIXTURE_ID, None)?)
}

/// Ingest-stage child. Vault faults trip inside the seal; store faults are
/// injected into the identical acceptance body the product uses.
fn child_ingest(spec: &FaultSpec, profile_root: &Path, ready: &Path) -> TestResult {
    let mut service = child_service(profile_root)?;
    let request = ingest_request()?;
    let response = match acceptance_point(spec.id) {
        Some(point) => service.handle_mutable_request_now_with_faults(
            &request,
            &AbortAtAcceptance::new(point, spec.id, ready),
        )?,
        // A vault fault needs no injector: `academic-vault` reads the selection
        // variable the parent set and aborts inside the seal.
        None => service.handle_mutable_request_now(&request)?,
    };
    Err(format!("acceptance returned {} instead of dying", response.status).into())
}

/// Projection-stage child. It accepts the fixture cleanly, then dies inside a
/// generation build.
fn child_projection_build(spec: &FaultSpec, profile_root: &Path, ready: &Path) -> TestResult {
    let mut service = child_service(profile_root)?;
    let accepted = service.handle_mutable_request_now(&ingest_request()?)?;
    if MutationStatus::try_from(accepted.status) != Ok(MutationStatus::Accepted) {
        return Err("the projection child could not accept the fixture".into());
    }
    let point = projection_point(spec.id).ok_or("no projection checkpoint for this fault")?;
    let injector = AbortAtProjection::new(point, spec.id, ready);
    let reader = service.open_reader()?;
    let runner = ProjectionRunner::open(
        &reader,
        profile_root.join(PROJECTION_SIDECAR_FILE),
        projection_builder_digest(),
        projection_config_hash(),
    )?;
    let policies = fixture_predicate_policies()?;
    let facts = CanonicalFacts::read(profile_root)?;
    for target in projection_targets(facts.watermark.accept_seq_head)? {
        runner.rebuild_at_with_faults(
            target.kind,
            target.domain,
            target.coordinates,
            &policies,
            &injector,
        )?;
    }
    Err("no projection generation reached the selected checkpoint".into())
}

/// Backup-stage child. It accepts the fixture cleanly, then dies inside the
/// backup the parent selected with the portability environment variables.
fn child_backup(profile_root: &Path) -> TestResult {
    let mut service = child_service(profile_root)?;
    let accepted = service.handle_mutable_request_now(&ingest_request()?)?;
    if MutationStatus::try_from(accepted.status) != Ok(MutationStatus::Accepted) {
        return Err("the backup child could not accept the fixture".into());
    }
    drop(service);
    let destination = required_path(DESTINATION_VARIABLE)?;
    backup_synthetic_profile(profile_root, &destination)?;
    Err("the backup published without reaching the selected checkpoint".into())
}

/// Restore-stage child. It accepts, backs up cleanly, then dies inside the
/// restore. The clean backup cannot trip a restore checkpoint, because the
/// owning crate compares the selection variable against its own identifier.
fn child_restore(profile_root: &Path) -> TestResult {
    let mut service = child_service(profile_root)?;
    let accepted = service.handle_mutable_request_now(&ingest_request()?)?;
    if MutationStatus::try_from(accepted.status) != Ok(MutationStatus::Accepted) {
        return Err("the restore child could not accept the fixture".into());
    }
    drop(service);
    let backup = required_path(BACKUP_VARIABLE)?;
    let destination = required_path(DESTINATION_VARIABLE)?;
    backup_synthetic_profile(profile_root, &backup)?;
    fs::create_dir_all(&destination)?;
    restore_synthetic_profile(&backup, &destination)?;
    Err("the restore published without reaching the selected checkpoint".into())
}

/// Admission-stage child for `IPC01`.
///
/// The daemon carries no failpoint between reading a complete request and
/// admitting it, and must not gain one. The child therefore composes the same
/// two public steps in the same order — read one complete command frame to
/// completion, then admit it to the bounded writer lane — and aborts between
/// them. `fault_driver::IPC01_REALIZATION` states what that does and does not
/// prove.
fn child_admission(spec: &FaultSpec, profile_root: &Path, ready: &Path) -> TestResult {
    ensure_synthetic_profile(profile_root)?;
    let profile = open_synthetic_profile(profile_root, &NativePathProbe::default())?;
    let (writer, _startup) = WriterQueue::start(profile)?;
    let request = ingest_request()?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?;
    let framed = runtime.block_on(async {
        let mut buffer = Vec::new();
        let envelope = LocalCoreEnvelope {
            payload: Some(local_core_envelope::Payload::MutableRequest(
                request.clone(),
            )),
        };
        write_envelope(&mut buffer, &envelope, FrameClass::Command).await?;
        Ok::<_, academic_rpc::RpcError>(buffer)
    })?;
    let decoded = runtime.block_on(async {
        let mut source = framed.as_slice();
        read_envelope(&mut source, FrameClass::Command).await
    })?;
    let Some(local_core_envelope::Payload::MutableRequest(decoded)) = decoded.payload else {
        return Err("the framed command did not decode as a mutable request".into());
    };

    // The read completed: the decoded frame is the whole request, byte for
    // byte. Asserting it here is what makes this checkpoint "after a complete
    // request read" rather than "somewhere during one".
    if decoded != request {
        return Err("the decoded request is not the request that was framed".into());
    }
    // The writer lane is live and has committed nothing, so the next statement
    // the daemon would execute is the admission that never happens.
    if writer.current_revision() != 0 {
        return Err("the admission child observed a revision it never wrote".into());
    }

    fault_driver::write_ready_marker(ready, spec.id)?;
    std::process::abort();
}

// ---------------------------------------------------------------------------
// Parent: one fault run
// ---------------------------------------------------------------------------

/// Spawns and reaps the child that takes one fault.
fn take_fault(
    spec: &FaultSpec,
    lane: &Lane,
    profile: &Path,
) -> Result<ChildRecord, Box<dyn Error>> {
    let ready = lane.path(&format!("{}.ready", spec.id));
    let mut command = self_invocation("phase1_exit_fault_child")?;
    command
        .env(CHILD_VARIABLE, "1")
        .env(FAULT_VARIABLE, spec.id)
        .env(PROFILE_VARIABLE, profile)
        .env(READY_VARIABLE, &ready)
        .env(BACKUP_VARIABLE, lane.path("child-backup"))
        .env(DESTINATION_VARIABLE, lane.path("child-destination"));
    for (name, value) in spec.environment(&ready) {
        command.env(name, value);
    }
    let record = run_bounded(&mut command, spec.id, profile, CHILD_TIMEOUT)?;
    assert!(
        !record.end.timed_out(),
        "{} child exceeded its bounded wait: {}",
        spec.id,
        record.receipt_line()
    );
    assert!(
        record.end.is_failure(),
        "{} child exited successfully, so the checkpoint was never reached: {}",
        spec.id,
        record.receipt_line()
    );
    let marker = fs::read_to_string(&ready).map_err(|error| {
        format!(
            "{} child left no ready marker at {}: {error}",
            spec.id,
            ready.display()
        )
    })?;
    assert_eq!(
        marker, spec.id,
        "{} child stopped at a different checkpoint",
        spec.id
    );
    Ok(record)
}

/// Reads the reconciliation decisions one restart recorded.
fn reconcile_facts(daemon: &RunningDaemon) -> ReconcileFacts {
    ReconcileFacts {
        states: daemon
            .startup()
            .reconciliation()
            .records()
            .iter()
            .map(|record| format!("{:?}", record.state()))
            .collect(),
    }
}

/// Runs the whole exit sequence for one fault and returns what it observed.
fn run_exit_sequence(
    spec: &FaultSpec,
    reference: &CanonicalFacts,
) -> Result<FaultEvidence, Box<dyn Error>> {
    let lane = Lane::new(&spec.id.to_ascii_lowercase())?;
    let profile = lane.path("profile");
    let child = take_fault(spec, &lane, &profile)?;

    // (a) What the kill left behind, before anything recovers it.
    let pre_restart_vault = VaultInventory::read(&profile)?;
    let pre_restart_canonical = CanonicalFacts::read(&profile)?;

    // (b) Reconcile the temp lane as the expiry window would.
    //
    //     The vault deliberately keeps an unexpired `*.partial`, because a live
    //     one may belong to a concurrent ingest, and the daemon always
    //     reconciles at the current clock. A leftover from a kill seconds ago is
    //     therefore `TempLive` and stays. The matrix row says *expired* temp
    //     removed, so the harness asks the product's own reconciliation the
    //     question the row asks, with the expiry threshold set to zero and no
    //     descriptors supplied: only the temp lane is in scope, and a sealed
    //     object is left for the daemon's own pass to dispose of.
    let expiry = expire_vault_temps(&profile)?;

    // (b) Restart a real daemon: it validates the profile, acquires the
    //     singleton, reconciles the vault, and only then binds an endpoint.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_io()
        .enable_time()
        .build()?;
    let daemon = runtime.block_on(RunningDaemon::start(DaemonConfig::new(
        &profile,
        lane.runtime(),
    )))?;
    let mut reconcile = reconcile_facts(&daemon);
    reconcile.states.extend(expiry);
    let unknown = reconcile.unknown_states();
    assert!(
        unknown.is_empty(),
        "{} restart produced reconciliation states outside the frozen vocabulary: {unknown:?}",
        spec.id
    );

    // (c) Deep doctor and reconciliation, before any retry.
    let post_restart_canonical = CanonicalFacts::read(&profile)?;
    let post_restart_doctor = DoctorFacts::from_diagnosis(&diagnose_profile(&profile, true)?);

    // (d) Idempotent retry, twice, over local IPC.
    let request = ingest_request()?;
    let key = idempotency_key_hex(&request);
    let first = retry_facts(&runtime.block_on(client_exchange(
        daemon.endpoint(),
        daemon.session_nonce(),
        request.clone(),
    ))?);
    let second = retry_facts(&runtime.block_on(client_exchange(
        daemon.endpoint(),
        daemon.session_nonce(),
        ingest_request()?,
    ))?);
    let post_retry_canonical = CanonicalFacts::read(&profile)?;
    runtime.block_on(daemon.shutdown())?;

    // (e) Replay the canonical ledger from its own stored signed envelopes.
    assert_ledger_replays(&profile)?;

    // (f) Export twice and compare semantics and per-file hashes.
    let first_export = export_synthetic_profile(&profile, &lane.path("export-1"))?;
    let second_export = export_synthetic_profile(&profile, &lane.path("export-2"))?;
    let export_file_manifests_match =
        first_export.manifest.semantic.files == second_export.manifest.semantic.files;
    let exported_object_count = first_export.manifest.semantic.objects.len();

    // (g) Back up, restore into another empty profile, rebuild projections.
    let backup_root = lane.path("backup");
    backup_synthetic_profile(&profile, &backup_root)?;
    let restored_root = lane.path("restored");
    fs::create_dir_all(&restored_root)?;
    let restore = restore_synthetic_profile(&backup_root, &restored_root)?;
    let restored_canonical = CanonicalFacts::read(&restored_root)?;
    let restored_doctor = DoctorFacts::from_diagnosis(&diagnose_profile(&restored_root, true)?);

    // (h) The disposition of the artifact this fault's termination point
    //     protects, measured where that artifact actually lives.
    let subject = observe_subject(
        spec,
        &lane,
        &profile,
        reference,
        &pre_restart_vault,
        &reconcile,
        &post_restart_canonical,
        &post_restart_doctor,
        &first,
        &key,
        exported_object_count,
    )?;

    Ok(FaultEvidence {
        fault_id: spec.id.to_owned(),
        child,
        ready_marker_matched: true,
        pre_restart_vault,
        pre_restart_canonical,
        reconcile,
        post_restart_canonical,
        post_restart_doctor,
        subject,
        first_retry: first,
        second_retry: second,
        post_retry_canonical,
        exported_object_count,
        export_semantic_digests: (
            first_export.manifest.semantic_digest.clone(),
            second_export.manifest.semantic_digest.clone(),
        ),
        export_file_manifests_match,
        restored_semantic_digest: restore.canonical_semantic_digest.clone(),
        restored_canonical,
        restored_doctor,
    })
}

/// Measures the disposition of the artifact one fault's termination point
/// protects.
#[allow(clippy::too_many_arguments)]
fn observe_subject(
    spec: &FaultSpec,
    lane: &Lane,
    profile: &Path,
    reference: &CanonicalFacts,
    pre_restart_vault: &VaultInventory,
    reconcile: &ReconcileFacts,
    post_restart_canonical: &CanonicalFacts,
    post_restart_doctor: &DoctorFacts,
    first_retry: &RetryFacts,
    idempotency_key: &str,
    exported_object_count: usize,
) -> Result<SubjectDisposition, Box<dyn Error>> {
    let canonical_absent = post_restart_canonical.is_empty();
    let source_unchanged = post_restart_canonical.matches_reference(reference);
    Ok(match spec.subject {
        FaultSubject::VaultTemp => SubjectDisposition::VaultTemp {
            partials_after_restart: post_restart_doctor.orphan_temp_entries.len(),
            canonical_absent,
        },
        FaultSubject::SealedObject => SubjectDisposition::SealedObject {
            sealed_before_restart: !pre_restart_vault.sealed_objects.is_empty(),
            // The restart's own pass decided this, not the harness: a sealed
            // object matching a retry candidate is a `ValidOrphan`, one past its
            // grace window is a `QuarantinedOrphan`, and both are the explicit
            // disposition the row requires. A file simply left lying with no
            // record is neither.
            explicit_disposition: reconcile.disposed_an_orphan()
                || !post_restart_doctor.quarantined_entries.is_empty(),
            canonical_absent,
        },
        FaultSubject::CanonicalTransaction => SubjectDisposition::CanonicalTransaction {
            completeness: completeness(post_restart_canonical, reference),
            object_closure_holds: exported_object_count == reference.artifact_ids.len(),
        },
        FaultSubject::ProjectionGeneration => {
            let inconsistent_generation_active =
                !post_restart_doctor.quarantined_entries.is_empty()
                    || projections_inconsistent(&post_restart_doctor.findings);
            SubjectDisposition::ProjectionGeneration {
                inconsistent_generation_active,
                clean_rebuild_activated: rebuild_projections(profile)?,
                canonical_unchanged: source_unchanged,
            }
        }
        FaultSubject::BackupDirectory => {
            let destination = lane.path("child-destination");
            SubjectDisposition::BackupDirectory {
                destination_published: destination.join(MANIFEST_FILE).is_file(),
                unpublished_recoverable: unpublished_backups_recoverable(&destination)?,
                source_unchanged,
                fresh_publication_succeeded: backup_synthetic_profile(
                    profile,
                    &lane.path("fresh-backup"),
                )
                .is_ok(),
            }
        }
        FaultSubject::RestoreDestination => {
            let destination = lane.path("child-destination");
            let backup = lane.path("child-backup");
            let fresh = lane.path("fresh-restore");
            fs::create_dir_all(&fresh)?;
            SubjectDisposition::RestoreDestination {
                destination_publishable: destination.join(STORE_DATABASE_FILE).is_file(),
                unpublished_recoverable: unpublished_restores_recoverable(&destination)?,
                source_unchanged,
                backup_unchanged: backup.join(MANIFEST_FILE).is_file(),
                fresh_publication_succeeded: restore_synthetic_profile(&backup, &fresh).is_ok(),
            }
        }
        FaultSubject::QueuedRequest => SubjectDisposition::QueuedRequest {
            canonical_absent,
            retry_admitted_fresh: first_retry.is_accepted(),
            same_idempotency_key: idempotency_key == idempotency_key_hex(&ingest_request()?),
        },
    })
}

/// Returns whether any deep-doctor finding says a projection disagrees with the
/// canonical head.
fn projections_inconsistent(findings: &[String]) -> bool {
    findings
        .iter()
        .any(|finding| finding.starts_with("PROJECTION_LAG") && finding.contains("RepairRequired"))
}

/// Reconciles the vault temp lane as the expiry window would, and reports what
/// the product's own pass decided.
///
/// Only the temp threshold is moved. No referenced descriptor and no retry
/// candidate is supplied, and the orphan grace keeps its product default, so a
/// sealed object is untouched here and is disposed of by the daemon's own
/// startup pass a moment later. The clock itself is not moved either: setting
/// the expiry to zero asks exactly "what happens once this temp is expired?"
/// without pretending the machine is in the future.
fn expire_vault_temps(profile: &Path) -> Result<Vec<String>, Box<dyn Error>> {
    let vault = Vault::open(profile, DomainKeyring::new())?;
    let report = vault
        .reconcile(&ReconcileOptions::new(SystemTime::now()).with_temp_expiry(Duration::ZERO))?;
    Ok(report
        .records()
        .iter()
        .map(|record| format!("{:?}", record.state()))
        .collect())
}

/// Rebuilds every projection generation cleanly and reports full activation.
fn rebuild_projections(profile: &Path) -> Result<bool, Box<dyn Error>> {
    let synthetic = open_synthetic_profile(profile, &NativePathProbe::default())?;
    let reader = synthetic.open_reader()?;
    let runner = ProjectionRunner::open(
        &reader,
        profile.join(PROJECTION_SIDECAR_FILE),
        projection_builder_digest(),
        projection_config_hash(),
    )?;
    let policies = fixture_predicate_policies()?;
    let facts = CanonicalFacts::read(profile)?;
    for target in projection_targets(facts.watermark.accept_seq_head)? {
        runner.rebuild_at(target.kind, target.domain, target.coordinates, &policies)?;
        if runner
            .active_generation(target.kind, target.domain)?
            .is_none()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Returns whether every unpublished backup staging root can be found and removed.
fn unpublished_backups_recoverable(destination: &Path) -> Result<bool, Box<dyn Error>> {
    let staged = find_unpublished_backups(destination)?;
    for staging in &staged {
        remove_unpublished_backup(destination, staging)?;
    }
    Ok(find_unpublished_backups(destination)?.is_empty())
}

/// Returns whether every unpublished restore staging root can be found and removed.
fn unpublished_restores_recoverable(destination: &Path) -> Result<bool, Box<dyn Error>> {
    let staged = find_unpublished_restores(destination)?;
    for staging in &staged {
        remove_unpublished_restore(destination, staging)?;
    }
    Ok(find_unpublished_restores(destination)?.is_empty())
}

/// Replays the canonical ledger from the profile's own stored signed envelopes.
///
/// The trust anchor comes from this build, never from the stored bytes, which
/// is the same rule restore applies.
fn assert_ledger_replays(profile: &Path) -> TestResult {
    use academic_portability::verify::{CanonicalDatabase, replay_signed_batches};

    let database = CanonicalDatabase::open_source(&profile.join(STORE_DATABASE_FILE))?;
    let authorizations = academic_core::operations::fixture_authorizations()?;
    let report = replay_signed_batches(&database, &authorizations)?;
    let facts = CanonicalFacts::read(profile)?;
    assert_eq!(
        report.verified_batches, facts.counts.batches,
        "ledger replay disagreed with the stored batch count"
    );
    assert_eq!(
        report.verified_events, facts.counts.events,
        "ledger replay disagreed with the stored event count"
    );
    Ok(())
}

/// Runs one fault and asserts every invariant the exit contract states.
fn assert_fault(
    spec: &FaultSpec,
    reference: &CanonicalFacts,
) -> Result<FaultEvidence, Box<dyn Error>> {
    let evidence = run_exit_sequence(spec, reference)?;
    let report = evidence.receipt_lines(spec, reference).join("\n");

    // The two invariants that outrank every letter.
    let observed_completeness = completeness(&evidence.post_restart_canonical, reference);
    assert!(
        !matches!(observed_completeness, Completeness::Partial(_)),
        "{} left a normal-looking partial canonical state: {observed_completeness}\n{report}",
        spec.id
    );
    if !evidence.post_restart_canonical.is_empty() {
        assert_eq!(
            evidence.exported_object_count,
            reference.artifact_ids.len(),
            "{} left a canonical reference to a missing or corrupt object\n{report}",
            spec.id
        );
    }

    // The profile must be healthy and serve a deterministic, restorable export.
    assert!(
        evidence.post_restart_doctor.health_checks_passed(),
        "{} failed a deep-doctor health check\n{report}",
        spec.id
    );
    assert_eq!(
        evidence.export_semantic_digests.0, evidence.export_semantic_digests.1,
        "{} produced two exports with different semantic digests\n{report}",
        spec.id
    );
    assert!(
        evidence.export_file_manifests_match,
        "{} produced two exports with different per-file hash manifests\n{report}",
        spec.id
    );
    assert!(
        evidence
            .restored_canonical
            .matches_lineage(&evidence.post_retry_canonical),
        "{} restored a profile whose canonical semantics differ from its source\n{report}",
        spec.id
    );
    assert_eq!(
        evidence.restored_semantic_digest, evidence.post_retry_canonical.semantic_digest,
        "{} restore reported a semantic digest its own rows do not produce\n{report}",
        spec.id
    );
    assert!(
        evidence.restored_doctor.projections_current,
        "{} restored a profile whose projections were not rebuilt from empty\n{report}",
        spec.id
    );
    assert!(
        !evidence.restored_doctor.repair_required,
        "{} restored a profile that needs repair\n{report}",
        spec.id
    );

    // Every fault retries idempotently, whether or not its own row is about
    // that. A profile that already holds the fixture must replay its stored
    // receipt; one that does not must admit the retry as a fresh command and
    // then replay it. Either way one idempotency key ends with one receipt.
    if evidence.committed(reference) {
        assert!(
            evidence.first_retry.is_duplicate() && evidence.second_retry.is_duplicate(),
            "{} accepted an already-committed batch again instead of replaying it\n{report}",
            spec.id
        );
    } else {
        assert!(
            evidence.first_retry.is_accepted(),
            "{} refused the retry of a command that never committed\n{report}",
            spec.id
        );
        assert!(
            evidence.second_retry.is_duplicate(),
            "{} accepted the same idempotency key twice\n{report}",
            spec.id
        );
    }
    assert_eq!(
        evidence.first_retry.receipt_id, evidence.second_retry.receipt_id,
        "{} returned two different receipts for one idempotency key\n{report}",
        spec.id
    );
    assert_eq!(
        evidence.post_retry_canonical.counts.command_receipts, 1,
        "{} stored more than one receipt for one idempotency key\n{report}",
        spec.id
    );

    // Finally the matrix letter itself.
    let observed = evidence.observed(spec, reference);
    assert_eq!(
        Outcome::render(&observed),
        Outcome::render(spec.expected),
        "{} observed outcome does not equal the enumerated matrix letter\n{report}",
        spec.id
    );
    println!("{report}");
    Ok(evidence)
}

/// Establishes the fault-free canonical reference every fault is measured against.
fn reference_profile() -> Result<(Lane, CanonicalFacts), Box<dyn Error>> {
    let lane = Lane::new("reference")?;
    let profile = lane.path("profile");
    let mut service = child_service(&profile)?;
    let accepted = service.handle_mutable_request_now(&ingest_request()?)?;
    assert_eq!(
        MutationStatus::try_from(accepted.status),
        Ok(MutationStatus::Accepted),
        "the fault-free reference profile could not accept the fixture"
    );
    drop(service);
    let facts = CanonicalFacts::read(&profile)?;
    Ok((lane, facts))
}

// ---------------------------------------------------------------------------
// The six named tests
// ---------------------------------------------------------------------------

/// Records that one named exit test completed every assertion it makes.
///
/// Called as the last statement of a named test, so the row cannot be emitted
/// by a run that failed earlier.
fn record_named_test(name: &str) {
    // The leading newline matters. With `--nocapture` cargo writes
    // `test <name> ... ` without a newline first, so a test whose only output
    // is this row would otherwise emit it on that same line and a line-anchored
    // reader would never see it.
    println!(
        "
{}{}",
        oracle::RESULT_TEST_PREFIX,
        oracle::json_object(&[
            ("name", oracle::JsonValue::Text(name)),
            ("status", oracle::JsonValue::Text("PASS")),
            ("host_family", oracle::JsonValue::Text(HOST_FAMILY)),
        ])
    );
}

/// The exit sequence with no fault injected at all.
///
/// This is both the control and the source of the canonical reference: if the
/// clean path did not produce a complete acceptance, a deterministic pair of
/// exports, and a restore that matches, no fault result would mean anything.
#[test]
fn phase1_exit_without_fault() -> TestResult {
    let lane = Lane::new("nofault")?;
    let profile = lane.path("profile");
    let mut service = child_service(&profile)?;
    let accepted = service.handle_mutable_request_now(&ingest_request()?)?;
    assert_eq!(
        MutationStatus::try_from(accepted.status),
        Ok(MutationStatus::Accepted)
    );
    drop(service);

    let reference = CanonicalFacts::read(&profile)?;
    assert!(
        !reference.is_empty(),
        "the fault-free run accepted nothing: {}",
        reference.receipt_line()
    );
    assert_eq!(reference.counts.batches, 1);
    assert_eq!(reference.counts.command_receipts, 1);
    assert_eq!(reference.counts.device_heads, 1);
    assert_eq!(reference.watermark.profile_revision, 1);
    assert_eq!(
        reference.watermark.accept_seq_head, reference.counts.events,
        "every accepted event must consume exactly one acceptance sequence"
    );

    let doctor = DoctorFacts::from_diagnosis(&diagnose_profile(&profile, true)?);
    assert!(doctor.health_checks_passed(), "{}", doctor.receipt_line());
    assert!(!doctor.repair_required, "{}", doctor.receipt_line());
    assert!(
        doctor.orphan_temp_entries.is_empty(),
        "{}",
        doctor.receipt_line()
    );
    assert!(
        doctor.quarantined_entries.is_empty(),
        "{}",
        doctor.receipt_line()
    );

    assert_ledger_replays(&profile)?;

    let first = export_synthetic_profile(&profile, &lane.path("export-1"))?;
    let second = export_synthetic_profile(&profile, &lane.path("export-2"))?;
    assert_eq!(
        first.manifest.semantic_digest, second.manifest.semantic_digest,
        "two exports of one watermark must agree semantically"
    );
    assert_eq!(
        first.manifest.semantic.files, second.manifest.semantic.files,
        "two exports of one watermark must agree file by file"
    );
    assert!(!first.manifest.semantic.encrypted);
    assert!(!first.manifest.semantic.projections_included);

    let backup = lane.path("backup");
    backup_synthetic_profile(&profile, &backup)?;
    let restored = lane.path("restored");
    fs::create_dir_all(&restored)?;
    let receipt = restore_synthetic_profile(&backup, &restored)?;
    let restored_facts = CanonicalFacts::read(&restored)?;
    assert!(
        restored_facts.matches_lineage(&reference),
        "restore changed canonical semantics: {} vs {}",
        restored_facts.receipt_line(),
        reference.receipt_line()
    );
    assert_eq!(receipt.canonical_semantic_digest, reference.semantic_digest);
    let restored_doctor = DoctorFacts::from_diagnosis(&diagnose_profile(&restored, true)?);
    assert!(
        restored_doctor.projections_current,
        "{}",
        restored_doctor.receipt_line()
    );
    record_named_test("phase1_exit_without_fault");
    Ok(())
}

/// Every enumerated fault, run through the whole exit sequence.
#[test]
fn phase1_exit_at_every_fault_point() -> TestResult {
    let observed_ids = PHASE1_EXIT_FAULTS
        .iter()
        .map(|fault| fault.id)
        .collect::<Vec<_>>();
    assert_eq!(
        observed_ids, FROZEN_FAULT_IDS,
        "the executable matrix drifted from the frozen identifier list"
    );

    // The rows the exit corpus cannot reach are frozen here as well, so a
    // second unreachable row can never appear silently: it would have to be
    // added to this list, in the open, before the suite would go green again.
    let unreachable = fault_driver::not_run_in_exit_corpus()
        .into_iter()
        .map(|fault| fault.id)
        .collect::<Vec<_>>();
    assert_eq!(
        unreachable,
        ["BK03"],
        "the set of rows the exit corpus cannot reach changed"
    );

    let (_reference_lane, reference) = reference_profile()?;
    let mut lines = Vec::new();
    let mut rows = Vec::new();
    let (mut passed, mut not_run) = (0_u64, 0_u64);
    for spec in PHASE1_EXIT_FAULTS {
        if let Reachability::NotRunInExitCorpus { reason, covered_by } = spec.reachability {
            not_run += 1;
            lines.push(format!(
                "{} expected={} observed=NOT_RUN reason={reason} covered_by={covered_by}",
                spec.id,
                Outcome::render(spec.expected)
            ));
            rows.push(result_row(
                spec, "NOT_RUN", "NOT_RUN", reason, covered_by, None,
            ));
            continue;
        }
        let evidence = assert_fault(spec, &reference)?;
        passed += 1;
        let observed = Outcome::render(&evidence.observed(spec, &reference));
        lines.push(format!(
            "{} expected={} observed={} PASS",
            spec.id,
            Outcome::render(spec.expected),
            observed
        ));
        rows.push(result_row(spec, &observed, "PASS", "", "", Some(&evidence)));
    }
    assert_eq!(lines.len(), FROZEN_FAULT_IDS.len());
    assert_eq!(passed + not_run, FROZEN_FAULT_IDS.len() as u64);

    // The normalized rows the exit receipt is assembled from. They are printed
    // by the run that made the assertions, so a receipt can never describe a
    // matrix the suite did not actually execute.
    for row in &rows {
        println!("{}{row}", oracle::RESULT_ROW_PREFIX);
    }
    println!(
        "{}{}",
        oracle::RESULT_SUMMARY_PREFIX,
        oracle::json_object(&[
            ("schema", oracle::JsonValue::Text(oracle::RESULT_SCHEMA)),
            ("matrix_size", oracle::JsonValue::Number(rows.len() as u64)),
            ("passed", oracle::JsonValue::Number(passed)),
            ("not_run", oracle::JsonValue::Number(not_run)),
            ("failed", oracle::JsonValue::Number(0)),
            ("host_family", oracle::JsonValue::Text(HOST_FAMILY)),
            (
                "ipc01_realization",
                oracle::JsonValue::Text(fault_driver::IPC01_REALIZATION)
            ),
        ])
    );
    println!("phase1 exit matrix:\n{}", lines.join("\n"));
    record_named_test("phase1_exit_at_every_fault_point");
    Ok(())
}

/// The operating-system family this lane's evidence belongs to.
///
/// Windows named-pipe evidence and Unix domain-socket evidence are separate
/// claims, so every emitted row says which one produced it and neither lane's
/// receipt can be read as the other's.
const HOST_FAMILY: &str = if cfg!(windows) { "windows" } else { "unix" };

/// Renders one normalized result row.
fn result_row(
    spec: &FaultSpec,
    observed: &str,
    status: &str,
    reason: &str,
    covered_by: &str,
    evidence: Option<&FaultEvidence>,
) -> String {
    oracle::json_object(&[
        ("id", oracle::JsonValue::Text(spec.id)),
        ("owner", oracle::JsonValue::Text(spec.owner.as_str())),
        ("stage", oracle::JsonValue::Text(spec.stage.as_str())),
        ("subject", oracle::JsonValue::Text(spec.subject.as_str())),
        (
            "expected",
            oracle::JsonValue::Text(&Outcome::render(spec.expected)),
        ),
        ("observed", oracle::JsonValue::Text(observed)),
        ("status", oracle::JsonValue::Text(status)),
        ("host_family", oracle::JsonValue::Text(HOST_FAMILY)),
        (
            "child_pid",
            oracle::JsonValue::Number(evidence.map_or(0, |value| u64::from(value.child.pid))),
        ),
        (
            "child_profile",
            oracle::JsonValue::Text(&evidence.map_or(String::new(), |value| {
                value.child.profile_root.display().to_string()
            })),
        ),
        (
            "child_end",
            oracle::JsonValue::Text(
                &evidence.map_or(String::new(), |value| value.child.end.to_string()),
            ),
        ),
        (
            "elapsed_ms",
            oracle::JsonValue::Number(evidence.map_or(0, |value| {
                u64::try_from(value.child.elapsed_ms).unwrap_or(0)
            })),
        ),
        (
            "export_semantic_digest",
            oracle::JsonValue::Text(
                evidence.map_or("", |value| value.export_semantic_digests.0.as_str()),
            ),
        ),
        (
            "exports_agree",
            oracle::JsonValue::Bool(evidence.is_some_and(|value| {
                value.export_semantic_digests.0 == value.export_semantic_digests.1
                    && value.export_file_manifests_match
            })),
        ),
        (
            "restored_semantic_digest",
            oracle::JsonValue::Text(
                evidence.map_or("", |value| value.restored_semantic_digest.as_str()),
            ),
        ),
        (
            "restored_projections_current",
            oracle::JsonValue::Bool(
                evidence.is_some_and(|value| value.restored_doctor.projections_current),
            ),
        ),
        ("not_run_reason", oracle::JsonValue::Text(reason)),
        ("covered_by", oracle::JsonValue::Text(covered_by)),
    ])
}

/// The two post-commit faults, checked specifically for receipt identity.
///
/// `phase1_exit_at_every_fault_point` already covers `DB07` and `IPC02`. This
/// test states the lost-acknowledgement property on its own, so a regression in
/// receipt replay names itself instead of appearing as one row of a long matrix.
#[test]
fn phase1_exit_idempotent_retry_after_lost_ack() -> TestResult {
    let (_reference_lane, reference) = reference_profile()?;
    for id in ["DB07", "IPC02"] {
        let spec = fault_driver::spec(id).ok_or("post-commit fault is missing from the matrix")?;
        assert_eq!(
            spec.expected,
            [Outcome::Complete, Outcome::IdempotentRetry].as_slice(),
            "{id} must require a complete transaction and an idempotent retry"
        );
        let evidence = run_exit_sequence(spec, &reference)?;
        let report = evidence.receipt_lines(spec, &reference).join("\n");

        assert!(
            evidence.committed(&reference),
            "{id} lost a transaction that had already committed\n{report}"
        );
        assert!(
            evidence.first_retry.is_duplicate() && evidence.second_retry.is_duplicate(),
            "{id} accepted the batch again instead of replaying its receipt\n{report}"
        );
        assert!(
            evidence.first_retry.is_identical_to(&evidence.second_retry),
            "{id} returned two different receipts for one idempotency key\n{report}"
        );
        assert_eq!(
            evidence.post_retry_canonical, evidence.post_restart_canonical,
            "{id} retry changed canonical state\n{report}"
        );
        assert_eq!(
            evidence.post_restart_canonical.counts.command_receipts, 1,
            "{id} stored more than one receipt for one key\n{report}"
        );
    }
    record_named_test("phase1_exit_idempotent_retry_after_lost_ack");
    Ok(())
}

/// Deep doctor, canonical replay, and restore-into-empty on their own.
///
/// The matrix run exercises these after every fault. This test exercises them
/// against a profile that was crashed once, so the doctor, the replay, and the
/// restore each have a named home in the suite.
#[test]
fn phase1_exit_doctor_replay_restore() -> TestResult {
    let (_reference_lane, reference) = reference_profile()?;
    let spec = fault_driver::spec("DB03").ok_or("DB03 is missing from the matrix")?;
    let evidence = run_exit_sequence(spec, &reference)?;
    let report = evidence.receipt_lines(spec, &reference).join("\n");

    assert!(
        evidence.post_restart_doctor.deep,
        "the deep pass did not run\n{report}"
    );
    assert_eq!(evidence.post_restart_doctor.integrity_check, Some(true));
    assert_eq!(evidence.post_restart_doctor.foreign_key_check, Some(true));
    assert!(
        evidence.post_restart_doctor.synthetic_marker_present,
        "the synthetic-only marker vanished\n{report}"
    );
    assert!(
        evidence.post_restart_doctor.orphan_temp_entries.is_empty(),
        "startup reconciliation left a stale temp behind\n{report}"
    );
    assert!(
        !evidence.reconcile.states.is_empty(),
        "the restart recorded no explicit reconciliation disposition\n{report}"
    );
    assert!(
        evidence
            .restored_canonical
            .matches_lineage(&evidence.post_retry_canonical),
        "the restored profile does not match its source\n{report}"
    );
    assert!(
        evidence.restored_doctor.projections_current,
        "the restored profile's projections were not rebuilt from empty\n{report}"
    );
    assert_eq!(
        evidence.export_semantic_digests.0, evidence.export_semantic_digests.1,
        "two exports disagreed\n{report}"
    );
    record_named_test("phase1_exit_doctor_replay_restore");
    Ok(())
}

// ---------------------------------------------------------------------------
// Substantive negative proofs
// ---------------------------------------------------------------------------

/// Repository root, resolved from this crate's manifest directory.
fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .to_path_buf()
}

/// Collects every `.rs` file below a directory.
fn rust_sources(root: &Path) -> io::Result<Vec<(PathBuf, String)>> {
    let mut sources = Vec::new();
    collect_rust_sources(root, &mut sources)?;
    sources.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(sources)
}

fn collect_rust_sources(root: &Path, sources: &mut Vec<(PathBuf, String)>) -> io::Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_rust_sources(&path, sources)?;
        } else if path.extension().is_some_and(|value| value == "rs") {
            sources.push((path.clone(), fs::read_to_string(&path)?));
        }
    }
    Ok(())
}

/// The one file in the workspace allowed an outbound socket construct.
///
/// It is the process `P2-G4`'s sandbox contains: proving that the operating
/// system refuses a socket means asking it for one. What keeps it scoped is
/// read from `cargo metadata` rather than from this comment --
/// `only_egress_crate_has_a_socket` in `tools/phase1-scaffold-policy.test.mjs`
/// asserts that the target is `required-features = ["native-sandbox"]`, that it
/// is the worker's only binary, and that no workspace crate depends on
/// `academic-worker`, so no default build and no product crate reaches it.
const SANDBOX_PROBE: &str = "crates/worker/probes/worker_probe.rs";

/// The product binary built with default plaintext features has no product
/// networking, proved by source scan and by link scan.
///
/// Two independent proofs, because either alone is weak. The source scan
/// catches a call that exists but is unreachable in this build; the link scan
/// catches a transitive dependency that ships networking a source scan of this
/// repository would never see.
#[test]
fn phase1_exit_has_no_product_network() -> TestResult {
    let root = repository_root();

    // (1) Source scan of every product crate, excluding the harness.
    //
    // The whole package rather than `<crate>/src`. `T146` put
    // `std::net::TcpStream::connect` in `crates/record/examples/emit_harness.rs`
    // and this scan read nothing: the example has no feature gate, is compiled
    // by `cargo clippy --workspace --all-targets`, and is run by the documented
    // `pnpm harness:emit` script, so it is product-shaped code that a walk
    // rooted at `src` never saw. `tests` and `benches` stay out, as they always
    // were -- this crate's own suite opens the local IPC seam on purpose.
    let prohibited = [
        "TcpListener",
        "TcpStream",
        "TcpSocket",
        "UdpSocket",
        "ToSocketAddrs",
        "lookup_host",
        "getaddrinfo",
        "hyper::",
        "reqwest::",
        "tonic::",
    ];
    let crates_dir = root.join("crates");
    let mut scanned = 0_usize;
    for entry in fs::read_dir(&crates_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() || entry.file_name() == "test-support" {
            continue;
        }
        let package = entry.path();
        for sub in fs::read_dir(&package)? {
            let sub = sub?;
            let name = sub.file_name();
            if name == "tests" || name == "benches" {
                continue;
            }
            let sources = if sub.file_type()?.is_dir() {
                rust_sources(&sub.path())?
            } else if sub.path().extension().is_some_and(|value| value == "rs") {
                vec![(sub.path(), fs::read_to_string(sub.path())?)]
            } else {
                Vec::new()
            };
            for (path, source) in sources {
                let relative = path.strip_prefix(&root).unwrap_or(&path);
                let spelled = relative.to_string_lossy().replace('\\', "/");
                if spelled == SANDBOX_PROBE {
                    continue;
                }
                scanned += 1;
                for needle in prohibited {
                    assert!(
                        !source.contains(needle),
                        "product source {} names {needle}",
                        path.display()
                    );
                }
            }
        }
    }
    assert!(
        scanned >= 200,
        "the product source scan found only {scanned} files, so it proved little"
    );

    // (2) Link scan of the default-feature `academicd` binary itself.
    let binary = default_feature_daemon_binary(&root)?;
    let image = fs::read(&binary)?;
    assert!(
        !image.is_empty(),
        "{} is empty, so the link scan proved nothing",
        binary.display()
    );
    // Symbol and import names a networking stack cannot avoid exporting. They
    // are matched as ASCII byte sequences so the scan works on a PE image and
    // an ELF image alike.
    let linked_network_symbols: [&[u8]; 8] = [
        b"getaddrinfo",
        b"WSAStartup",
        b"gethostbyname",
        b"SSL_connect",
        b"hyper",
        b"reqwest",
        b"rustls",
        b"native_tls",
    ];
    for symbol in linked_network_symbols {
        assert!(
            !contains_bytes(&image, symbol),
            "{} links {}",
            binary.display(),
            String::from_utf8_lossy(symbol)
        );
    }
    // The scan is only meaningful if this image really is the daemon, so it has
    // to find something it must contain.
    assert!(
        contains_bytes(&image, b"SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED"),
        "{} does not carry the synthetic-only data policy, so it is not the \
         product daemon and the link scan proved nothing",
        binary.display()
    );
    record_named_test("phase1_exit_has_no_product_network");
    Ok(())
}

/// Builds `academicd` with default features only and returns its path.
///
/// The harness itself is compiled with `phase1-fault-injection`, so the binary
/// beside this test is not a default build and cannot answer the question. A
/// separate locked, offline, default-feature build in its own target directory
/// is what the claim is actually about.
fn default_feature_daemon_binary(root: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let lane = default_build_lane()?;
    let status = std::process::Command::new(env!("CARGO"))
        .current_dir(root)
        .args([
            "build",
            "--locked",
            "--offline",
            "--quiet",
            "-p",
            "academic-daemon",
            "--bin",
            "academicd",
        ])
        .env("CARGO_TARGET_DIR", &lane)
        .stdin(std::process::Stdio::null())
        .status()?;
    assert!(
        status.success(),
        "the default-feature daemon build failed, so the link scan cannot run"
    );
    let binary = lane.join("debug").join(if cfg!(windows) {
        "academicd.exe"
    } else {
        "academicd"
    });
    assert!(
        binary.is_file(),
        "the default-feature build produced no {}",
        binary.display()
    );
    Ok(binary)
}

/// A stable target directory for the default-feature build, outside the tree.
///
/// It is deliberately reused rather than made unique per run: the build is
/// expensive and its inputs are the committed sources, so a warm lane keeps the
/// scan affordable without ever writing inside the worktree.
fn default_build_lane() -> io::Result<PathBuf> {
    let lane = temporary_base()?.join("academic-x1-default-features");
    fs::create_dir_all(&lane)?;
    Ok(lane)
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Builds an ingest request naming an arbitrary fixture identifier.
///
/// Only the name is substituted. Everything else is the canonical request the
/// exit sequence sends, so a refusal is about the name and nothing else.
fn fixture_named(candidate: &str) -> Result<MutableRequest, Box<dyn Error>> {
    let mut request = synthetic_ingest_request(PHASE1_SYNTHETIC_FIXTURE_ID, None)?;
    match request.command.as_mut() {
        Some(academic_rpc::generated::mutable_request::Command::SyntheticIngest(command)) => {
            command.synthetic_fixture_id = candidate.to_owned();
        }
        _ => return Err("the canonical ingest request lost its command".into()),
    }
    Ok(request)
}

/// The exit path refuses anything that is not an allowlisted synthetic fixture.
///
/// Three independent refusals, because the allowlist has to hold at each of the
/// three places a caller could try to get past it: the request builder, the
/// daemon's own validation, and the profile the exit sequence will accept.
#[test]
fn phase1_exit_rejects_real_data() -> TestResult {
    // (1) The allowlist itself admits exactly one identifier.
    assert_eq!(
        academic_core::operations::synthetic_ingest_request(PHASE1_SYNTHETIC_FIXTURE_ID, None)?
            .capability_id,
        SYNTHETIC_INGEST_CAPABILITY
    );

    // (2) A daemon refuses every non-allowlisted name over real local IPC,
    //     including names shaped like real personal data and like file paths.
    let lane = Lane::new("realdata")?;
    let profile = lane.path("profile");
    ensure_synthetic_profile(&profile)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_io()
        .enable_time()
        .build()?;
    let daemon = runtime.block_on(RunningDaemon::start(DaemonConfig::new(
        &profile,
        lane.runtime(),
    )))?;

    // (2a) A name the protocol itself cannot carry never reaches the daemon:
    //      the bounded nonempty field is refused while the request is still
    //      being digested, which is the earliest of the three guards.
    let oversized = "x".repeat(4096);
    for candidate in ["", oversized.as_str()] {
        let request = fixture_named(candidate)?;
        assert!(
            mutable_request_digest(&request).is_err(),
            "a protocol-invalid fixture name of {} bytes passed request validation",
            candidate.len()
        );
    }

    // (2b) A well-formed but non-allowlisted name reaches the daemon and is
    //      refused there, including names shaped like real personal data and
    //      like file paths on either host.
    let refused = [
        "real-data",
        "production",
        "phase0-synthetic-bitemporal-ledger-v1",
        "PHASE0-SYNTHETIC-BITEMPORAL-LEDGER-V2",
        "../../etc/passwd",
        "C:/Users/someone/transcript.pdf",
        "/home/someone/grades.csv",
        "\\\\server\\share\\lecture.mp4",
        "student-record-2026",
        "2026-fall-transcript.pdf",
    ];
    for candidate in refused {
        let mut request = fixture_named(candidate)?;
        request.request_digest = mutable_request_digest(&request)?.as_bytes().to_vec();
        let response = retry_facts(&runtime.block_on(client_exchange(
            daemon.endpoint(),
            daemon.session_nonce(),
            request,
        ))?);
        assert_eq!(
            response.status,
            "REJECTED",
            "the daemon did not refuse {candidate:?}: {}",
            response.receipt_line()
        );
        assert_eq!(
            response.reason,
            "FIXTURE_NOT_ALLOWLISTED",
            "the daemon refused {candidate:?} for the wrong reason: {}",
            response.receipt_line()
        );
    }

    // (3) Nothing was written by any refusal.
    let facts = CanonicalFacts::read(&profile)?;
    assert!(
        facts.is_empty(),
        "a refused fixture still changed canonical state: {}",
        facts.receipt_line()
    );
    let doctor = DoctorFacts::from_diagnosis(&diagnose_profile(&profile, true)?);
    assert!(
        doctor.orphan_temp_entries.is_empty(),
        "a refused fixture left vault residue: {}",
        doctor.receipt_line()
    );
    runtime.block_on(daemon.shutdown())?;

    // (4) The one allowlisted identifier is the only entry, and it is not a path.
    assert!(!PHASE1_SYNTHETIC_FIXTURE_ID.contains('/'));
    assert!(!PHASE1_SYNTHETIC_FIXTURE_ID.contains('\\'));
    assert!(!PHASE1_SYNTHETIC_FIXTURE_ID.contains(':'));
    record_named_test("phase1_exit_rejects_real_data");
    Ok(())
}
