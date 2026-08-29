import { spawn, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

/**
 * Assembles the Phase 1 exit receipt from one run of the exit lane.
 *
 * This tool proves nothing by itself. Every claim it emits is read back out of
 * a command it actually ran, and the exact command receipt is part of the
 * output, so a reader can re-run the same argv and compare. It never re-derives
 * a fault outcome: the per-fault rows come from the harness run that made the
 * assertions, so a receipt cannot describe a matrix the suite did not execute.
 *
 * Usage:
 *   node tools/phase1-exit.mjs --all-faults --format json
 *   node tools/phase1-exit.mjs --format human            (skips the fault matrix)
 *
 * Options:
 *   --all-faults      run the enumerated fault matrix (slow; the whole point)
 *   --format json     emit the normalized result document (default)
 *   --format human    emit the same facts as readable lines
 *   --lane <dir>      disposable lane for profiles and exports
 *                     (default: a fresh directory below the system temp root)
 *   --keep-lane       leave the lane in place instead of removing it
 *   --commit <sha>    commit this source came from, for an archive lane where
 *                     git metadata is absent and cannot be measured
 *   --tree <sha>      tree this source came from, for the same reason
 *
 * Nothing here writes inside the repository. Profiles, exports, and backups all
 * live in the lane, and `CARGO_TARGET_DIR` is whatever the caller set.
 */

/** Schema name of the normalized result document. */
export const RESULT_SCHEMA = "learning-platform.phase1-exit-result.v1";
/** Marker the harness prefixes to each machine-readable fault row. */
const ROW_PREFIX = "PHASE1_EXIT_ROW ";
/** Marker the harness prefixes to each named-test row. */
const TEST_PREFIX = "PHASE1_EXIT_TEST ";
/** Marker the harness prefixes to its single summary row. */
const SUMMARY_PREFIX = "PHASE1_EXIT_SUMMARY ";
/** The six named tests the Phase 1 exit contract requires. */
const REQUIRED_NAMED_TESTS = [
  "phase1_exit_without_fault",
  "phase1_exit_at_every_fault_point",
  "phase1_exit_idempotent_retry_after_lost_ack",
  "phase1_exit_doctor_replay_restore",
  "phase1_exit_has_no_product_network",
  "phase1_exit_rejects_real_data",
];
/** The one repository-allowlisted synthetic fixture. */
const FIXTURE_ID = "phase0-synthetic-bitemporal-ledger-v2";
/** Feature selection that compiles the exit harness lane. */
const FAULT_FEATURE = "phase1-fault-injection";
/** The exact banner every data-bearing command must print first. */
const REQUIRED_BANNER =
  "PLAINTEXT SYNTHETIC-ONLY PROFILE — REAL OR PRODUCTION DATA IS FORBIDDEN";
/** The exact policy object every machine-readable surface must repeat. */
const REQUIRED_POLICY = {
  data_policy: "SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED",
  storage_mode: "PLAINTEXT_TEMPORARY_SQLITE",
  storage_encryption: "NONE",
  production_data_allowed: false,
  product_network: "NONE",
};
/** Bounded wait for one command, in milliseconds. */
const COMMAND_TIMEOUT_MS = 20 * 60 * 1000;
/** Bounded wait for the foreground daemon to publish its session, in ms. */
const DAEMON_READY_TIMEOUT_MS = 60 * 1000;

/** Records every command this run executed, in order. */
const commandReceipt = [];

/**
 * Runs one command to completion under a bounded wait and records its receipt.
 *
 * The receipt holds the argv, the working directory, the exit status, and the
 * duration. Output is returned to the caller for parsing but is never placed in
 * the receipt: a receipt is evidence about what ran, not a transcript of what a
 * synthetic profile contained.
 */
function run(argv, { cwd = process.cwd(), env = {}, label = null } = {}) {
  const startedAt = Date.now();
  const result = spawnSync(argv[0], argv.slice(1), {
    cwd,
    encoding: "utf8",
    timeout: COMMAND_TIMEOUT_MS,
    maxBuffer: 256 * 1024 * 1024,
    env: { ...process.env, ...env },
  });
  const entry = {
    label,
    argv,
    cwd,
    exit_code: result.status,
    signal: result.signal ?? null,
    timed_out: result.error?.code === "ETIMEDOUT",
    duration_ms: Date.now() - startedAt,
  };
  commandReceipt.push(entry);
  return { ...entry, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

/** Runs a command and fails loudly when it does not succeed. */
function runOrThrow(argv, options = {}) {
  const result = run(argv, options);
  if (result.exit_code !== 0) {
    throw new Error(
      `${argv.join(" ")} exited ${result.exit_code}${result.signal ? ` (${result.signal})` : ""}`,
    );
  }
  return result;
}

/**
 * Parses one CLI invocation's JSON document and checks both banner channels.
 *
 * In JSON mode the CLI keeps stdout a clean document and writes the human
 * banner to stderr, so a machine caller can pipe stdout straight into a parser
 * and still cannot silence the banner. Both are required here: the stderr line
 * and the document's own `banner` field. A run missing either is a policy
 * failure, not a parsing inconvenience.
 */
function parseCliJson(result, label) {
  const stdout = typeof result === "string" ? result : result.stdout;
  const stderr = typeof result === "string" ? "" : (result.stderr ?? "");
  const brace = stdout.indexOf("{");
  if (brace < 0) {
    throw new Error(`${label} produced no JSON document`);
  }
  const preamble = stdout.slice(0, brace).trim();
  if (!preamble.includes(REQUIRED_BANNER) && !stderr.includes(REQUIRED_BANNER)) {
    throw new Error(`${label} did not print the mandatory policy banner on either channel`);
  }
  const document = JSON.parse(stdout.slice(brace));
  assertPolicy(document, label);
  return document;
}

/** Requires one CLI document to carry the exact frozen banner and policy. */
function assertPolicy(document, label) {
  if (document.banner !== REQUIRED_BANNER) {
    throw new Error(`${label} carries a different banner`);
  }
  for (const [key, value] of Object.entries(REQUIRED_POLICY)) {
    if (document.policy?.[key] !== value) {
      throw new Error(
        `${label} policy.${key} is ${JSON.stringify(document.policy?.[key])}, not ${JSON.stringify(value)}`,
      );
    }
  }
}

/**
 * Requires that every named field was actually read out of a successful command.
 *
 * A receipt whose field is `null` because this tool read a key the CLI does not
 * emit is worse than no receipt: it looks like evidence and is not. This turns
 * that into a loud failure at the moment of extraction.
 */
function requireExtracted(extracted, names, label) {
  const missing = names.filter((name) => extracted[name] === null || extracted[name] === undefined);
  if (missing.length > 0) {
    throw new Error(
      `${label} evidence is missing ${missing.join(", ")}; the command's JSON contract changed`,
    );
  }
  return extracted;
}

/** Reduces one CLI document to the policy evidence the receipt carries. */
function policyEvidence(label, document) {
  return {
    surface: label,
    banner: document.banner,
    policy: document.policy,
    status: document.status,
    exit_code: document.exit_code,
  };
}

// ---------------------------------------------------------------------------
// Commit, tools, and the default feature graph
// ---------------------------------------------------------------------------

/**
 * Reports which exact source this run was made from.
 *
 * A `git archive` mirror carries no repository metadata by design, and running
 * the exit from one is the point of the archive lane. So git is best-effort: in
 * a checkout the commit, tree, and worktree cleanliness are measured, and in an
 * archive they are whatever the caller passed with `--commit` and `--tree`, or
 * `null`. The receipt says which of the two it is and never presents a
 * caller-supplied hash as a measured one.
 */
function commitIdentity(root, options) {
  const probe = run(["git", "rev-parse", "--is-inside-work-tree"], {
    cwd: root,
    label: "repository probe",
  });
  if (probe.exit_code !== 0 || probe.stdout.trim() !== "true") {
    return {
      source: "archive",
      commit: options.commit,
      tree: options.tree,
      clean_worktree: null,
      dirty_entries: [],
      tracked_files: null,
      note:
        "extracted from a git archive, which carries no repository metadata; " +
        "commit and tree are as supplied on the command line, not measured here",
    };
  }
  const commit = runOrThrow(["git", "rev-parse", "HEAD"], { cwd: root, label: "commit" });
  const tree = runOrThrow(["git", "rev-parse", "HEAD^{tree}"], { cwd: root, label: "tree" });
  const status = run(["git", "status", "--porcelain=v2", "--branch", "--untracked-files=all"], {
    cwd: root,
    label: "worktree status",
  });
  const tracked = runOrThrow(["git", "ls-files"], { cwd: root, label: "tracked files" });
  const dirty = status.stdout
    .split(/\r?\n/u)
    .filter((line) => line.length > 0 && !line.startsWith("#"));
  return {
    source: "worktree",
    commit: commit.stdout.trim(),
    tree: tree.stdout.trim(),
    clean_worktree: dirty.length === 0,
    dirty_entries: dirty,
    tracked_files: tracked.stdout.split(/\r?\n/u).filter((line) => line.length > 0).length,
  };
}

function toolVersions(root) {
  const versions = {};
  for (const [tool, argv] of [
    ["rustc", ["rustc", "--version"]],
    ["cargo", ["cargo", "--version"]],
    ["node", ["node", "--version"]],
    ["pnpm", ["pnpm", "--version"]],
  ]) {
    const result = run(argv, { cwd: root, label: `version ${tool}` });
    versions[tool] = result.exit_code === 0 ? result.stdout.trim() : null;
  }
  return versions;
}

/**
 * Returns the resolved default feature set of every workspace crate.
 *
 * This is the "default Cargo feature graph" the receipt has to state, and it is
 * read from the resolver rather than from the manifests, so a feature enabled
 * by unification shows up here even though no manifest mentions it.
 */
function defaultFeatureGraph(root) {
  const result = runOrThrow(
    ["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
    { cwd: root, label: "default feature graph" },
  );
  const metadata = JSON.parse(result.stdout);
  const byId = new Map(metadata.packages.map((pkg) => [pkg.id, pkg]));
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const workspace = metadata.workspace_members
    .map((id) => ({
      package: byId.get(id).name,
      features: (nodes.get(id)?.features ?? []).toSorted(),
    }))
    .toSorted((left, right) => left.package.localeCompare(right.package));
  const faultLanes = workspace.filter((entry) => entry.features.includes(FAULT_FEATURE));
  return {
    workspace,
    fault_injection_enabled_by_default: faultLanes.map((entry) => entry.package),
  };
}

// ---------------------------------------------------------------------------
// The live CLI lane
// ---------------------------------------------------------------------------

function cliArgv(root, args) {
  return ["cargo", "run", "--locked", "--offline", "--quiet", "-p", "academic-cli", "--", ...args];
}

/** Waits, bounded, for the foreground daemon to publish its session metadata. */
function waitForDaemon(root, profile, runtime) {
  const deadline = Date.now() + DAEMON_READY_TIMEOUT_MS;
  let last = null;
  while (Date.now() < deadline) {
    const attempt = run(
      cliArgv(root, [
        "daemon",
        "status",
        "--profile",
        profile,
        "--runtime",
        runtime,
        "--format",
        "json",
      ]),
      { cwd: root, label: "daemon status probe" },
    );
    if (attempt.exit_code === 0) {
      return attempt;
    }
    last = attempt;
    // A short synchronous pause; the daemon binds in well under a second once
    // it has been built, and this loop is bounded either way.
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 250);
  }
  throw new Error(
    `the foreground daemon did not publish a session within ${DAEMON_READY_TIMEOUT_MS}ms` +
      (last ? ` (last status exit ${last.exit_code})` : ""),
  );
}

/**
 * Drives one disposable synthetic profile through every data-bearing command.
 *
 * This is where the receipt's banner and policy evidence comes from: each
 * surface is invoked for real and its own document is read back. The daemon is
 * then terminated abruptly, so the crash-replay row is a real post-kill deep
 * doctor rather than a description of one.
 */
function driveLane(root, lane) {
  const profile = join(lane, "profile");
  const runtime = join(lane, "runtime");
  const restored = join(lane, "restored");
  // `daemon serve` creates an absent profile root but requires its runtime root
  // to exist, and `restore` requires a new empty destination directory.
  mkdirSync(runtime, { recursive: true });
  mkdirSync(restored, { recursive: true });
  const surfaces = [];
  const evidence = {};

  const doctor = runOrThrow(cliArgv(root, ["doctor", "--format", "json"]), {
    cwd: root,
    label: "doctor",
  });
  surfaces.push(policyEvidence("doctor", parseCliJson(doctor, "doctor")));

  const daemonProcess = spawn(
    "cargo",
    cliArgv(root, [
      "daemon",
      "serve",
      "--profile",
      profile,
      "--runtime",
      runtime,
      "--format",
      "json",
    ]).slice(1),
    { cwd: root, stdio: "ignore", detached: false },
  );
  commandReceipt.push({
    label: "daemon serve",
    argv: cliArgv(root, [
      "daemon",
      "serve",
      "--profile",
      profile,
      "--runtime",
      runtime,
      "--format",
      "json",
    ]),
    cwd: root,
    exit_code: null,
    signal: null,
    timed_out: false,
    duration_ms: 0,
    note: "foreground daemon; terminated abruptly later in this run",
  });

  try {
    const status = waitForDaemon(root, profile, runtime);
    const statusDocument = parseCliJson(status, "daemon status");
    surfaces.push(policyEvidence("daemon handshake", statusDocument));
    evidence.handshake = statusDocument.result?.handshake ?? null;

    const ingest = runOrThrow(
      cliArgv(root, [
        "ingest",
        "--profile",
        profile,
        "--runtime",
        runtime,
        "--fixture",
        FIXTURE_ID,
        "--format",
        "json",
      ]),
      { cwd: root, label: "ingest" },
    );
    const ingestDocument = parseCliJson(ingest, "ingest");
    surfaces.push(policyEvidence("ingest", ingestDocument));
    const acceptance = ingestDocument.result?.acceptance ?? {};

    const retry = runOrThrow(
      cliArgv(root, [
        "ingest",
        "--profile",
        profile,
        "--runtime",
        runtime,
        "--fixture",
        FIXTURE_ID,
        "--format",
        "json",
      ]),
      { cwd: root, label: "ingest retry" },
    );
    const retryAcceptance = parseCliJson(retry, "ingest retry").result?.acceptance ?? {};

    evidence.accepted_fixture = {
      fixture_ids: [FIXTURE_ID],
      status: acceptance.status ?? null,
      accept_seq_range: acceptance.acceptance_range ?? null,
      profile_revision: acceptance.profile_revision ?? null,
      receipt_id: acceptance.receipt?.receipt_id ?? null,
      idempotency_key: acceptance.receipt?.idempotency_key ?? null,
      request_digest: acceptance.receipt?.request_digest ?? null,
      response_digest: acceptance.response_digest ?? null,
      retry_status: retryAcceptance.status ?? null,
      retry_receipt_id: retryAcceptance.receipt?.receipt_id ?? null,
      idempotent_retry_returns_original_receipt:
        retryAcceptance.status === "DUPLICATE" &&
        retryAcceptance.receipt?.receipt_id === acceptance.receipt?.receipt_id,
    };
    requireExtracted(
      evidence.accepted_fixture,
      [
        "status",
        "accept_seq_range",
        "profile_revision",
        "receipt_id",
        "idempotency_key",
        "request_digest",
        "response_digest",
        "retry_status",
        "retry_receipt_id",
      ],
      "ingest",
    );

    const deep = run(
      cliArgv(root, ["doctor", "--profile", profile, "--deep", "--format", "json"]),
      { cwd: root, label: "deep doctor" },
    );
    const deepDocument = parseCliJson(deep, "deep doctor");
    surfaces.push(policyEvidence("deep doctor", deepDocument));
    evidence.deep_doctor = summarizeDoctor(deepDocument);

    const firstExport = runOrThrow(
      cliArgv(root, [
        "export",
        "--profile",
        profile,
        "--destination",
        join(lane, "export-1"),
        "--runtime",
        runtime,
        "--format",
        "json",
      ]),
      { cwd: root, label: "export 1" },
    );
    surfaces.push(policyEvidence("export", parseCliJson(firstExport, "export")));
    runOrThrow(
      cliArgv(root, [
        "export",
        "--profile",
        profile,
        "--destination",
        join(lane, "export-2"),
        "--runtime",
        runtime,
        "--format",
        "json",
      ]),
      { cwd: root, label: "export 2" },
    );
    evidence.exports = compareExports(join(lane, "export-1"), join(lane, "export-2"));
    evidence.exports.daemon_owns_profile =
      parseCliJson(firstExport, "export").result?.ownership?.daemon_owns_profile ?? null;
    requireExtracted(
      evidence.exports,
      ["semantic_digest", "file_count", "object_count", "canonical_semantic_digest"],
      "export",
    );

    const backup = runOrThrow(
      cliArgv(root, [
        "backup",
        "--profile",
        profile,
        "--destination",
        join(lane, "backup"),
        "--runtime",
        runtime,
        "--format",
        "json",
      ]),
      { cwd: root, label: "backup" },
    );
    const backupDocument = parseCliJson(backup, "backup");
    surfaces.push(policyEvidence("backup", backupDocument));
    // Field names are the CLI's own contract, not this tool's guess: see
    // `crates/cli/src/commands/backup.rs`. Reading a name the command does not
    // emit would put a silent `null` in the receipt where evidence belongs.
    evidence.backup = {
      format: backupDocument.result?.format ?? null,
      encrypted: backupDocument.result?.encrypted ?? null,
      confidentiality_warning: backupDocument.result?.confidentiality_warning ?? null,
      semantic_digest: backupDocument.result?.semantic_digest ?? null,
      canonical_semantic_digest: backupDocument.result?.canonical_semantic_digest ?? null,
      watermark: backupDocument.result?.watermark ?? null,
      object_count: backupDocument.result?.object_count ?? null,
      device_head_count: backupDocument.result?.device_head_count ?? null,
      daemon_owns_profile: backupDocument.result?.ownership?.daemon_owns_profile ?? null,
    };
    requireExtracted(
      evidence.backup,
      [
        "format",
        "confidentiality_warning",
        "semantic_digest",
        "canonical_semantic_digest",
        "watermark",
        "object_count",
        "device_head_count",
      ],
      "backup",
    );

    // The abrupt termination the crash-replay evidence is about.
    daemonProcess.kill("SIGKILL");
    commandReceipt.push({
      label: "abrupt daemon termination",
      argv: ["kill", "SIGKILL", String(daemonProcess.pid)],
      cwd: root,
      exit_code: null,
      signal: "SIGKILL",
      timed_out: false,
      duration_ms: 0,
    });

    const afterCrash = run(
      cliArgv(root, ["doctor", "--profile", profile, "--deep", "--format", "json"]),
      { cwd: root, label: "deep doctor after crash" },
    );
    const afterCrashDocument = parseCliJson(afterCrash, "crash-replay deep doctor");
    surfaces.push(policyEvidence("crash-replay", afterCrashDocument));
    evidence.after_crash_doctor = summarizeDoctor(afterCrashDocument);

    const restore = runOrThrow(
      cliArgv(root, [
        "restore",
        "--backup",
        join(lane, "backup"),
        "--new-profile",
        restored,
        "--runtime",
        runtime,
        "--format",
        "json",
      ]),
      { cwd: root, label: "restore" },
    );
    const restoreDocument = parseCliJson(restore, "restore");
    surfaces.push(policyEvidence("restore", restoreDocument));
    evidence.restore = {
      mode: restoreDocument.result?.mode ?? null,
      canonical_semantic_digest: restoreDocument.result?.canonical_semantic_digest ?? null,
      watermark: restoreDocument.result?.watermark ?? null,
      verified_batches: restoreDocument.result?.replay?.verified_batches ?? null,
      verified_events: restoreDocument.result?.replay?.verified_events ?? null,
      device_heads: restoreDocument.result?.replay?.device_heads ?? null,
      projections: restoreDocument.result?.projections ?? null,
    };
    requireExtracted(
      evidence.restore,
      [
        "mode",
        "canonical_semantic_digest",
        "watermark",
        "verified_batches",
        "verified_events",
        "device_heads",
        "projections",
      ],
      "restore",
    );

    const restoredDoctor = run(
      cliArgv(root, ["doctor", "--profile", restored, "--deep", "--format", "json"]),
      { cwd: root, label: "restored deep doctor" },
    );
    evidence.restored_doctor = summarizeDoctor(
      parseCliJson(restoredDoctor, "restored deep doctor"),
    );
  } finally {
    daemonProcess.kill("SIGKILL");
  }

  const matrix = runOrThrow(cliArgv(root, ["crash-replay", "--all", "--format", "json"]), {
    cwd: root,
    label: "crash-replay matrix",
  });
  const matrixDocument = parseCliJson(matrix, "crash-replay matrix");
  surfaces.push(policyEvidence("crash-replay matrix", matrixDocument));
  evidence.declared_matrix = (matrixDocument.result?.faults ?? []).map((fault) => ({
    id: fault.id,
    expected: (fault.required_restart_outcomes ?? []).join("+"),
    injectable_by_this_build: fault.injectable_by_this_build,
  }));

  return { surfaces, evidence };
}

/** Reduces one deep-doctor document to the receipt's fields. */
function summarizeDoctor(document) {
  const profile = document.result?.profile ?? null;
  if (profile === null) {
    return null;
  }
  return {
    ready: document.result?.ready ?? null,
    synthetic_marker_present: profile.synthetic_marker_present,
    store_schema: profile.store,
    canonical: profile.canonical,
    integrity_check: profile.integrity_check,
    foreign_key_check: profile.foreign_key_check,
    orphan_temp_entries: profile.orphan_temp_entries?.length ?? null,
    quarantined_entries: profile.quarantined_entries?.length ?? null,
    projections: profile.projections ?? [],
    findings: (profile.findings ?? []).map((finding) => `${finding.code}:${finding.severity}`),
  };
}

/** Compares two export directories by manifest semantics and per-file hashes. */
function compareExports(first, second) {
  const read = (directory) => JSON.parse(readFileSync(join(directory, "manifest.json"), "utf8"));
  const left = read(first);
  const right = read(second);
  const fileList = (manifest) =>
    (manifest.semantic?.files ?? []).map((file) => `${file.path}:${file.sha256}`);
  return {
    semantic_digest: left.semantic_digest,
    semantic_digests_equal: left.semantic_digest === right.semantic_digest,
    file_manifests_equal: JSON.stringify(fileList(left)) === JSON.stringify(fileList(right)),
    file_count: fileList(left).length,
    object_count: (left.semantic?.objects ?? []).length,
    encrypted: left.semantic?.encrypted ?? null,
    projections_included: left.semantic?.projections_included ?? null,
    canonical_semantic_digest: left.semantic?.canonical_semantic_digest ?? null,
  };
}

// ---------------------------------------------------------------------------
// The fault matrix
// ---------------------------------------------------------------------------

/**
 * Runs the enumerated fault matrix and returns the harness's own rows.
 *
 * The rows are parsed out of the run that made the assertions. Nothing is
 * recomputed here, so a row can only say `PASS` if the harness asserted it.
 */
function runFaultMatrix(root) {
  const argv = [
    "cargo",
    "test",
    "-p",
    "academic-daemon",
    "--test",
    "phase1_exit",
    "--locked",
    "--offline",
    "--features",
    FAULT_FEATURE,
    "--",
    "--nocapture",
    "--test-threads",
    "1",
  ];
  const result = run(argv, { cwd: root, label: "phase1 exit harness" });
  const rows = [];
  const completed = new Map();
  let summary = null;
  // The marker is located anywhere in the line rather than at its start.
  // `--nocapture` writes `test <name> ... ` without a newline before a test's
  // own output, so a marker can legitimately share a line with cargo's prefix.
  for (const line of result.stdout.split(/\r?\n/u)) {
    for (const [prefix, take] of [
      [ROW_PREFIX, (value) => rows.push(value)],
      [TEST_PREFIX, (value) => completed.set(value.name, value)],
      [SUMMARY_PREFIX, (value) => (summary = value)],
    ]) {
      const at = line.indexOf(prefix);
      if (at >= 0) {
        take(JSON.parse(line.slice(at + prefix.length)));
      }
    }
  }
  // A named test prints its row as its last statement, so a row present means
  // every assertion in that test held. A row absent means the test panicked,
  // was filtered out, or never ran, and all three are failures here. Reading
  // cargo's own `test NAME ... ok` line would not work: `--nocapture`
  // interleaves the test's output between the name and the verdict.
  const named = REQUIRED_NAMED_TESTS.map((name) => ({
    name,
    status: completed.has(name) ? "PASS" : "FAIL",
  }));
  return {
    harness_exit_code: result.exit_code,
    rows,
    summary,
    named_tests: named,
  };
}

/** Cross-checks the harness rows against the CLI's own declared matrix. */
function reconcileMatrix(rows, declared) {
  if (declared.length === 0) {
    return { checked: false, disagreements: [] };
  }
  const declaredById = new Map(declared.map((fault) => [fault.id, fault.expected]));
  const disagreements = rows
    .filter((row) => declaredById.get(row.id) !== row.expected)
    .map((row) => ({
      id: row.id,
      harness_expected: row.expected,
      cli_expected: declaredById.get(row.id) ?? null,
    }));
  return { checked: true, disagreements };
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

function parseArguments(argv) {
  const options = {
    allFaults: false,
    format: "json",
    lane: null,
    keepLane: false,
    commit: null,
    tree: null,
  };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--all-faults") {
      options.allFaults = true;
    } else if (argument === "--format") {
      index += 1;
      options.format = argv[index];
    } else if (argument === "--lane") {
      index += 1;
      options.lane = argv[index];
    } else if (argument === "--keep-lane") {
      options.keepLane = true;
    } else if (argument === "--commit") {
      index += 1;
      options.commit = argv[index];
    } else if (argument === "--tree") {
      index += 1;
      options.tree = argv[index];
    } else {
      throw new Error(`unknown option: ${argument}`);
    }
  }
  if (!["json", "human"].includes(options.format)) {
    throw new Error(`--format must be json or human, not ${options.format}`);
  }
  return options;
}

/** Builds the normalized result document for one exit run. */
export function assemblePhase1ExitReceipt(root, options) {
  commandReceipt.length = 0;
  const lane = options.lane ?? mkdtempSync(join(tmpdir(), "academic-x1-lane-"));
  try {
    const identity = commitIdentity(root, options);
    const tools = toolVersions(root);
    const features = defaultFeatureGraph(root);
    const { surfaces, evidence } = driveLane(root, lane);
    const matrix = options.allFaults
      ? runFaultMatrix(root)
      : { harness_exit_code: null, rows: [], summary: null, named_tests: [] };
    const reconciliation = reconcileMatrix(matrix.rows, evidence.declared_matrix ?? []);

    const passed = matrix.rows.filter((row) => row.status === "PASS").length;
    const notRun = matrix.rows.filter((row) => row.status === "NOT_RUN");
    const failed = matrix.rows.filter((row) => row.status === "FAIL");

    return {
      schema: RESULT_SCHEMA,
      generated_by: "tools/phase1-exit.mjs",
      host: { platform: process.platform, arch: process.arch },
      policy: REQUIRED_POLICY,
      banner: REQUIRED_BANNER,
      identity,
      tools,
      cargo_default_features: features,
      policy_surfaces: surfaces,
      accepted_fixture: evidence.accepted_fixture ?? null,
      handshake: evidence.handshake ?? null,
      deep_doctor: evidence.deep_doctor ?? null,
      after_crash_doctor: evidence.after_crash_doctor ?? null,
      exports: evidence.exports ?? null,
      backup: evidence.backup ?? null,
      restore: evidence.restore ?? null,
      restored_doctor: evidence.restored_doctor ?? null,
      fault_matrix: {
        ran: options.allFaults,
        harness_exit_code: matrix.harness_exit_code,
        summary: matrix.summary,
        rows: matrix.rows,
        named_tests: matrix.named_tests,
        reconciliation,
        totals: {
          declared: (evidence.declared_matrix ?? []).length,
          observed: matrix.rows.length,
          pass: passed,
          not_run: notRun.length,
          fail: failed.length,
        },
        not_run: notRun.map((row) => ({
          id: row.id,
          reason: row.not_run_reason,
          covered_by: row.covered_by,
        })),
      },
      claims: {
        synthetic_only: true,
        real_or_personal_input_accepted: false,
        product_network_present: false,
        default_linked_sqlcipher: false,
        adr_002_accepted: false,
        permitted_success_wording:
          identity.commit === null
            ? "synthetic throwaway Phase 1 local core passed at the commit this source " +
              "was archived from; encrypted-at-rest and production-data gates remain open"
            : `synthetic throwaway Phase 1 local core passed at commit ${identity.commit}; ` +
              "encrypted-at-rest and production-data gates remain open",
      },
      open_gates: [
        "ADR-002 encrypted transactional store",
        "ADR-004 encrypted artifact vault format",
        "ADR-005 key hierarchy and recovery",
        "SQLCipher five-platform packaging, leakage, rekey, recovery and restore matrix",
        "hosted CI on Windows and Linux (H1)",
        "macOS, Android and iOS native evidence",
        "production or personal data admission",
      ],
      command_receipt: [...commandReceipt],
    };
  } finally {
    if (!options.keepLane && options.lane === null) {
      rmSync(lane, { recursive: true, force: true });
    }
  }
}

/** Renders the normalized document as readable lines. */
function renderHuman(document) {
  const lines = [
    document.banner,
    `source: ${document.identity.source}` +
      ` commit ${document.identity.commit ?? "unstated"}` +
      ` tree ${document.identity.tree ?? "unstated"}`,
    document.identity.source === "worktree"
      ? `clean worktree: ${document.identity.clean_worktree} (${document.identity.tracked_files} tracked files)`
      : `worktree cleanliness: not measurable from an archive (${document.identity.note})`,
    `tools: ${Object.entries(document.tools)
      .map(([tool, version]) => `${tool}=${version}`)
      .join(" ")}`,
    `fault injection enabled by default: ${
      document.cargo_default_features.fault_injection_enabled_by_default.length === 0
        ? "none"
        : document.cargo_default_features.fault_injection_enabled_by_default.join(", ")
    }`,
    `policy surfaces carrying banner and policy object: ${document.policy_surfaces
      .map((surface) => surface.surface)
      .join(", ")}`,
  ];
  if (document.accepted_fixture) {
    lines.push(
      `fixture ${document.accepted_fixture.fixture_ids.join(",")} ${document.accepted_fixture.status}` +
        ` accept_seq ${JSON.stringify(document.accepted_fixture.accept_seq_range)}` +
        ` revision ${document.accepted_fixture.profile_revision}` +
        ` receipt ${document.accepted_fixture.receipt_id}`,
      `idempotent retry returns the original receipt: ${document.accepted_fixture.idempotent_retry_returns_original_receipt}`,
    );
  }
  if (document.exports) {
    lines.push(
      `two exports agree: semantic=${document.exports.semantic_digests_equal} files=${document.exports.file_manifests_equal}`,
    );
  }
  const totals = document.fault_matrix.totals;
  lines.push(
    `fault matrix: ${totals.pass} PASS, ${totals.not_run} NOT_RUN, ${totals.fail} FAIL of ${totals.observed} observed`,
  );
  for (const row of document.fault_matrix.rows) {
    lines.push(`  ${row.id} expected=${row.expected} observed=${row.observed} ${row.status}`);
  }
  for (const row of document.fault_matrix.not_run) {
    lines.push(`  NOT_RUN ${row.id}: ${row.reason} (covered by ${row.covered_by})`);
  }
  for (const test of document.fault_matrix.named_tests) {
    lines.push(`  ${test.name}: ${test.status}`);
  }
  lines.push(`open gates: ${document.open_gates.join("; ")}`);
  lines.push(document.claims.permitted_success_wording);
  return lines.join("\n");
}

const invokedPath = process.argv[1];
if (invokedPath !== undefined && import.meta.url === pathToFileURL(resolve(invokedPath)).href) {
  const options = parseArguments(process.argv.slice(2));
  const root = process.cwd();
  if (!existsSync(join(root, "Cargo.toml"))) {
    throw new Error("run this from the repository root");
  }
  const document = assemblePhase1ExitReceipt(root, options);
  process.stdout.write(
    options.format === "json"
      ? `${JSON.stringify(document, null, 2)}\n`
      : `${renderHuman(document)}\n`,
  );
  // This is the gate, not a report. A failed row, a named test that did not
  // complete, a harness that exited non-zero, and a matrix that disagrees with
  // the CLI's own declared expectations each fail the run.
  const matrix = document.fault_matrix;
  const failedTests = matrix.named_tests.filter((test) => test.status !== "PASS");
  const failed =
    matrix.totals.fail > 0 ||
    matrix.reconciliation.disagreements.length > 0 ||
    (options.allFaults && (matrix.harness_exit_code !== 0 || failedTests.length > 0));
  if (failed) {
    for (const test of failedTests) {
      process.stderr.write(`named test did not complete: ${test.name}\n`);
    }
    for (const row of matrix.reconciliation.disagreements) {
      process.stderr.write(
        `${row.id}: harness expects ${row.harness_expected}, CLI declares ${row.cli_expected}\n`,
      );
    }
    process.exitCode = 1;
  }
}
