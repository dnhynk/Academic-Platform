// A separate unsigned supplement. Historical encrypted H1 v2 stays immutable.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { closeSync, copyFileSync, lstatSync, mkdirSync, openSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { arch, platform, release } from "node:os";
import { dirname, join, posix, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { binaryArchitecture, commandStatus, testOutcomes } from "./h1-evidence.mjs";

const PROVIDER = "MACOS_KEYCHAIN_DATA_PROTECTION_V1";
export const EXECUTIONS = [
  ["leaf-format", "academic_keystore_platform", "macos_blob::tests::macos_payload_binds_the_label_version_and_exact_generation_length"],
  ["leaf-types", "academic_keystore_platform", "macos::tests::macos_native_result_type_and_length_are_checked_before_recovery"],
  ["leaf-errors", "academic_keystore_platform", "macos::tests::macos_status_categories_do_not_disclose_native_objects"],
  ["facade", "facade", "keystore_leaf_public_facade_exposes_no_raw_handle"],
  ["refusal", "macos_native", "macos_keychain_unprovisioned_identity_refuses_without_fallback"],
  ["crypto-provider", "native_keystore", "macos_data_protection_provider_rejects_foreign_blobs_before_native_access"],
];
export const MISSING = {
  positiveNative: { status: "not_run", tests: ["macos_keychain_positive_contract_and_process_reopen", "macos_data_protection_roundtrip_native"], reason: "No provisioned disposable signed-in test identity; no positive broker execution." },
  controlledLockedContext: { status: "not_run", reason: "A disposable provisioned locked/unavailable context has not been supplied." },
  packaging: { status: "missing", reason: "Main-executable provisioning, entitlement continuity, daemon packaging and distribution/license review are not established." },
  hardwareProtection: { status: "unverified", reason: "Generic-password storage is not Secure Enclave or hardware-protection evidence." },
  memoryReview: { status: "missing", reason: "No native memory/crash-dump inspection; immutable CFData and framework copies are not zeroizable here." },
};
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const digest = (path) => { const info = lstatSync(path); assert(info.isFile() && !info.isSymbolicLink() && info.size <= 256 * 1024 * 1024); const bytes = readFileSync(path); return { bytes: bytes.length, sha256: sha(bytes) }; };
const write = (path, value) => { mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" }); };
const json = (path) => { assert(digest(path).bytes < 16 * 1024 * 1024); return JSON.parse(readFileSync(path, "utf8")); };
const git = (args) => { const row = spawnSync("git", args, { encoding: "utf8", maxBuffer: 128 * 1024 * 1024, windowsHide: true }); assert.equal(row.status, 0); return row.stdout.trim(); };

function inventory(root, prefix = "", state = { bytes: 0, count: 0 }) {
  const directory = lstatSync(join(root, prefix));
  assert(directory.isDirectory() && !directory.isSymbolicLink());
  assert(prefix.split("/").length < 16);
  return readdirSync(join(root, prefix)).sort().flatMap((name) => {
    const path = prefix ? `${prefix}/${name}` : name;
    assert.match(path, /^[a-zA-Z0-9_.\/-]+$/u);
    assert(!path.split("/").some((part) => ["", ".", ".."].includes(part)));
    const info = lstatSync(join(root, path)); assert(!info.isSymbolicLink());
    if (info.isDirectory()) return inventory(root, path, state);
    const measured = digest(join(root, path)); state.bytes += measured.bytes; state.count += 1;
    assert(state.bytes <= 512 * 1024 * 1024 && state.count <= 10000);
    return [{ path, ...measured }];
  });
}

function sourceInventory(commit) {
  const tree = git(["rev-parse", `${commit}^{tree}`]);
  const entries = git(["ls-tree", "-r", "-z", commit]).split("\0").filter(Boolean);
  const files = entries.map((entry) => {
    const [metadata, path] = entry.split("\t");
    const [mode, type] = metadata.split(" ");
    assert(type === "blob" && ["100644", "100755"].includes(mode));
    return { path, ...digest(path) };
  });
  return { commit, tree, files };
}

export function plans() {
  return [
    { id: "source-preflight", executable: process.execPath, args: ["tools/source-preflight.mjs"] },
    { id: "rustc", executable: "rustc", args: ["-vV"] },
    { id: "fetch", executable: "cargo", args: ["fetch", "--locked", "--target", "aarch64-apple-darwin"] },
    { id: "leaf-build", executable: "cargo", args: ["test", "-p", "academic-keystore-platform", "--all-targets", "--no-run", "--locked", "--offline", "--message-format=json"] },
    { id: "crypto-build", executable: "cargo", args: ["test", "-p", "academic-crypto", "--features", "os-keystore", "--test", "native_keystore", "--no-run", "--locked", "--offline", "--message-format=json"] },
    { id: "lint", executable: "cargo", args: ["clippy", "-p", "academic-keystore-platform", "-p", "academic-crypto", "--features", "academic-crypto/os-keystore", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"] },
  ];
}

const BUILD_TARGETS = {
  "leaf-build": ["academic_keystore_platform", "facade", "macos_native"],
  "crypto-build": ["native_keystore"],
};

// Reconcile the entire executable set, including failed builds' partial output.
export function compilerRows(row, output, execution) {
  const found = [];
  for (const line of output.split(/\r?\n/u).filter(Boolean)) {
    const record = JSON.parse(line);
    if (record.reason !== "compiler-artifact" || !record.executable) continue;
    const target = record.target?.name;
    assert(BUILD_TARGETS[row.id]?.includes(target), "unexpected compiler executable");
    const leaf = row.id === "leaf-build", library = target === "academic_keystore_platform";
    const source = `crates/${leaf ? "keystore-platform" : "crypto"}/${library ? "src/lib.rs" : `tests/${target}.rs`}`;
    assert.equal(record.target.src_path, posix.join(execution.cwd, source));
    assert.deepEqual(record.target.kind, [library ? "lib" : "test"]);
    assert.deepEqual(record.target.crate_types, [library ? "lib" : "bin"]);
    assert.equal(record.profile?.test, true);
    assert.deepEqual(record.features?.toSorted(), leaf ? ["default"] : ["default", "os-keystore"]);
    assert(posix.isAbsolute(record.executable));
    const relative = posix.relative(execution.targetDir, record.executable);
    assert(relative && !relative.startsWith("..") && !posix.isAbsolute(relative), "compiler executable outside target directory");
    found.push({ command: row.id, target, executable: record.executable, source,
      features: record.features, targetKind: record.target.kind, testProfile: true });
  }
  assert.equal(new Set(found.map(({ target }) => target)).size, found.length, "duplicate compiler target");
  assert.equal(new Set(found.map(({ executable }) => executable)).size, found.length, "duplicate compiler executable");
  if (row.status === "passed") assert.deepEqual(found.map(({ target }) => target).sort(), BUILD_TARGETS[row.id].toSorted(), "missing compiler executable");
  return found;
}

const commandIds = () => [...plans().map(({ id }) => id), ...EXECUTIONS.map(([id]) => id)];

function run(bundle, plan, required = [], measure = false) {
  const stdout = `logs/${plan.id}.stdout.log`, stderr = `logs/${plan.id}.stderr.log`;
  mkdirSync(join(bundle, "logs"), { recursive: true });
  const out = openSync(join(bundle, stdout), "wx"), err = openSync(join(bundle, stderr), "wx");
  const before = measure ? digest(plan.executable) : null;
  const started = new Date().toISOString();
  const result = spawnSync(plan.executable, plan.args, { stdio: ["ignore", out, err], timeout: 15 * 60 * 1000, windowsHide: true });
  closeSync(out); closeSync(err);
  const output = readFileSync(join(bundle, stdout), "utf8");
  const row = { ...plan, started, finished: new Date().toISOString(), cwd: process.cwd(), exitCode: result.status, signal: result.signal, error: result.error?.message ?? null, stdout, stderr, tests: testOutcomes(output), executableBefore: before, executableAfter: measure ? digest(plan.executable) : null };
  row.status = commandStatus(row, output, required);
  write(join(bundle, `commands/${row.id}.json`), row);
  console.log(`${row.id}: ${row.status} (exit ${row.exitCode})`);
  return row;
}

export function deriveStatus(rows, failure) {
  const required = commandIds();
  return failure || required.some((id) => rows.find((row) => row.id === id)?.status !== "passed") ? "failed" : "refusal_and_api_checks_passed";
}

export function collect(bundle) {
  mkdirSync(bundle); // Must be a new task-owned artifact directory.
  const commit = git(["rev-parse", "HEAD"]);
  const rows = [], binaries = [];
  let failure = null;
  const manifest = {
    format: "h1-native-keystore-evidence", version: 1, acceptedH1: false, productionDataAllowed: false,
    provider: PROVIDER, commit, host: { platform: platform(), arch: arch(), release: release(), node: process.version },
    context: { repository: process.env.GITHUB_REPOSITORY ?? null, runId: process.env.GITHUB_RUN_ID ?? null, attempt: process.env.GITHUB_RUN_ATTEMPT ?? null, sha: process.env.GITHUB_SHA ?? null },
    scope: "disposable-unprovisioned-v1", missing: MISSING,
  };
  try {
    assert.equal(platform(), "darwin"); assert.equal(arch(), "arm64");
    assert.equal(git(["status", "--porcelain"]), "");
    assert.equal(manifest.context.sha, commit);
    assert.equal(process.version, `v${readFileSync(".nvmrc", "utf8").trim()}`);
    write(join(bundle, "source.json"), sourceInventory(commit));
    const lane = join(dirname(bundle), "macos-keystore-lane");
    mkdirSync(join(lane, "temp"), { recursive: true });
    process.env.CARGO_TARGET_DIR = join(lane, "target");
    process.env.TEMP = process.env.TMP = process.env.TMPDIR = join(lane, "temp");
    process.env.CARGO_BUILD_JOBS = "1"; process.env.CARGO_INCREMENTAL = "0"; process.env.RUST_TEST_THREADS = "1";
    process.env.CARGO_TERM_COLOR = "never";
    process.env.ACADEMIC_MACOS_KEYCHAIN_TEST_CONTEXT = manifest.scope;
    manifest.execution = { cwd: process.cwd(), nodeExecutable: process.execPath, targetDir: process.env.CARGO_TARGET_DIR, tempDir: process.env.TEMP };
    for (const plan of plans()) {
      const row = run(bundle, plan); rows.push(row);
      if (plan.id === "rustc") assert(readFileSync(join(bundle, row.stdout), "utf8").includes("host: aarch64-apple-darwin\n"));
      if (plan.id.endsWith("-build")) {
        const records = compilerRows(row, readFileSync(join(bundle, row.stdout), "utf8"), manifest.execution);
        for (const record of records) {
          assert.equal(binaryArchitecture(readFileSync(record.executable), "macos-aarch64"), "arm64");
          const path = `binaries/${record.target}`;
          assert(!binaries.some((binary) => binary.path === path));
          mkdirSync(join(bundle, "binaries"), { recursive: true }); copyFileSync(record.executable, join(bundle, path));
          const measured = digest(record.executable);
          assert.deepEqual(digest(join(bundle, path)), measured);
          binaries.push({ path, ...record, ...measured });
        }
      }
      assert.equal(row.status, "passed", `${plan.id} failed`);
    }
    for (const [id, target, name] of EXECUTIONS) {
      const binary = binaries.find((binary) => binary.target === target); assert(binary, `missing ${target}`);
      assert.deepEqual(digest(binary.executable), { bytes: binary.bytes, sha256: binary.sha256 });
      const args = ["--exact", name, ...(id === "refusal" ? ["--ignored"] : []), "--test-threads=1", "--show-output"];
      rows.push(run(bundle, { id, executable: binary.executable, args }, [name], true));
    }
  } catch (error) { failure = error.message; }
  write(join(bundle, "binaries.json"), binaries);
  manifest.commands = rows.map((row) => `commands/${row.id}.json`);
  manifest.notRun = commandIds().filter((id) => !rows.some((row) => row.id === id));
  manifest.failure = failure; manifest.status = deriveStatus(rows, failure);
  manifest.artifacts = inventory(bundle); write(join(bundle, "manifest.json"), manifest);
  return manifest;
}

export function validate(bundle, expected) {
  const discovered = inventory(bundle);
  const manifest = json(join(bundle, "manifest.json"));
  assert.equal(manifest.format, "h1-native-keystore-evidence"); assert.equal(manifest.version, 1);
  assert.equal(manifest.provider, PROVIDER); assert.equal(manifest.acceptedH1, false); assert.equal(manifest.productionDataAllowed, false);
  assert.equal(manifest.scope, "disposable-unprovisioned-v1"); assert.deepEqual(manifest.missing, MISSING);
  assert.match(expected.repository, /^[\w.-]+\/[\w.-]+$/u);
  for (const key of ["runId", "attempt"]) assert.match(expected[key], /^[1-9][0-9]*$/u);
  for (const key of ["repository", "runId", "attempt"]) assert.equal(manifest.context[key], expected[key]);
  assert.match(expected.commit, /^[a-f0-9]{40}$/u); assert.equal(manifest.commit, expected.commit); assert.equal(manifest.context.sha, expected.commit);
  assert.equal(manifest.host.platform, "darwin"); assert.equal(manifest.host.arch, "arm64");
  assert.deepEqual(manifest.artifacts, discovered.filter((artifact) => artifact.path !== "manifest.json"));
  const paths = new Set(manifest.artifacts.map((artifact) => artifact.path));
  assert.equal(paths.size, manifest.artifacts.length);
  const readArtifact = (path) => { assert(paths.has(path), "uninventoried artifact reference"); return readFileSync(join(bundle, path), "utf8"); };
  const source = json(join(bundle, "source.json"));
  // Compare canonical committed bytes, including the manifest/lock and test source.
  const entries = git(["ls-tree", "-r", "-z", expected.commit]).split("\0").filter(Boolean);
  const objects = entries.map((entry) => {
    const [metadata, path] = entry.split("\t"); const [mode, type, oid] = metadata.split(" ");
    assert(type === "blob" && ["100644", "100755"].includes(mode));
    return { path, oid };
  });
  const result = spawnSync("git", ["cat-file", "--batch"], { input: objects.map(({ oid }) => oid).join("\n") + "\n", maxBuffer: 256 * 1024 * 1024, windowsHide: true });
  assert.equal(result.status, 0);
  let offset = 0;
  const committed = objects.map(({ path, oid }) => {
    const end = result.stdout.indexOf(10, offset);
    const [observedOid, type, length] = result.stdout.subarray(offset, end).toString().split(" ");
    assert.equal(observedOid, oid); assert.equal(type, "blob");
    const bytes = result.stdout.subarray(end + 1, end + 1 + Number(length));
    offset = end + 2 + bytes.length;
    return { path, bytes: bytes.length, sha256: sha(bytes) };
  });
  assert.equal(offset, result.stdout.length, "unreconciled source object bytes");
  assert.deepEqual(source, { commit: expected.commit, tree: git(["rev-parse", `${expected.commit}^{tree}`]), files: committed });
  const sourceText = (path) => git(["show", `${expected.commit}:${path}`]);
  assert.equal(manifest.host.node, `v${sourceText(".nvmrc")}`);
  const rustVersion = /channel\s*=\s*"([0-9.]+)"/u.exec(sourceText("rust-toolchain.toml"))?.[1];
  assert(rustVersion);
  for (const path of [manifest.execution?.cwd, manifest.execution?.nodeExecutable, manifest.execution?.targetDir, manifest.execution?.tempDir]) assert(typeof path === "string" && posix.isAbsolute(path));
  assert.equal(posix.basename(manifest.execution.targetDir), "target");
  assert.equal(manifest.execution.tempDir, posix.join(posix.dirname(manifest.execution.targetDir), "temp"));
  const rows = manifest.commands.map((path) => { assert(paths.has(path)); return json(join(bundle, path)); });
  assert.equal(new Set(rows.map((row) => row.id)).size, rows.length);
  assert.deepEqual(rows.map(({ id }) => id), commandIds().slice(0, rows.length), "command order differs from plan");
  assert.deepEqual(manifest.notRun, commandIds().filter((id) => !rows.some((row) => row.id === id)));
  assert.deepEqual(manifest.commands, rows.map(({ id }) => `commands/${id}.json`));
  const binaries = json(join(bundle, "binaries.json"));
  const compiler = rows.filter(({ id }) => id.endsWith("-build")).flatMap((row) => compilerRows(row, readArtifact(row.stdout), manifest.execution));
  assert.deepEqual(binaries.map(({ path, bytes, sha256, ...identity }) => identity), compiler, "compiler and retained binary inventories differ");
  assert.deepEqual([...paths].filter((path) => path.startsWith("binaries/")).sort(), binaries.map(({ path }) => path).sort());
  assert.equal(new Set(binaries.map(({ path }) => path)).size, binaries.length);
  for (const binary of binaries) {
    assert(paths.has(binary.path)); assert.equal(binary.path, `binaries/${binary.target}`);
    assert.deepEqual(digest(join(bundle, binary.path)), { bytes: binary.bytes, sha256: binary.sha256 });
    assert.equal(binaryArchitecture(readFileSync(join(bundle, binary.path)), "macos-aarch64"), "arm64");
  }
  for (const row of rows) {
    assert.equal(row.cwd, manifest.execution.cwd);
    assert.equal(row.stdout, `logs/${row.id}.stdout.log`); assert.equal(row.stderr, `logs/${row.id}.stderr.log`);
    assert(paths.has(row.stdout) && paths.has(row.stderr));
    assert(manifest.commands.includes(`commands/${row.id}.json`));
    assert(Number.isFinite(Date.parse(row.started)) && Date.parse(row.finished) >= Date.parse(row.started));
    const native = EXECUTIONS.find(([id]) => id === row.id);
    const plan = plans().find((plan) => plan.id === row.id);
    assert(native || plan, "unreviewed command");
    if (plan) {
      assert.deepEqual(row.args, plan.args); assert.equal(row.executable, row.id === "source-preflight" ? manifest.execution.nodeExecutable : plan.executable);
      assert.equal(row.executableBefore, null); assert.equal(row.executableAfter, null);
    }
    if (native) {
      const binary = binaries.find((binary) => binary.target === native[1]); assert(binary);
      assert.equal(row.executable, binary.executable);
      assert.deepEqual(row.args, ["--exact", native[2], ...(row.id === "refusal" ? ["--ignored"] : []), "--test-threads=1", "--show-output"]);
      assert.deepEqual(row.executableBefore, { bytes: binary.bytes, sha256: binary.sha256 });
      assert.deepEqual(row.executableAfter, row.executableBefore);
    }
    const output = readFileSync(join(bundle, row.stdout), "utf8");
    assert.deepEqual(row.tests, testOutcomes(output));
    assert.equal(row.status, commandStatus(row, output, native ? [native[2]] : []));
    if (row.id === "rustc" && row.status === "passed") {
      assert(output.includes("host: aarch64-apple-darwin\n"));
      assert(output.includes(`release: ${rustVersion}\n`));
    }
  }
  assert.equal(manifest.status, deriveStatus(rows, manifest.failure));
  return { integrity: "verified", status: manifest.status, acceptedH1: false };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [verb, directory, commit, runId, attempt, repository] = process.argv.slice(2);
  if (verb === "collect") { const result = collect(resolve(directory)); console.log(JSON.stringify({ status: result.status, acceptedH1: false })); if (result.status === "failed") process.exitCode = 1; }
  else if (verb === "validate") console.log(JSON.stringify(validate(resolve(directory), { commit, runId, attempt, repository })));
  else throw new Error("Usage: macos-keystore-evidence.mjs collect DIR | validate DIR COMMIT RUN_ID ATTEMPT REPOSITORY");
}
