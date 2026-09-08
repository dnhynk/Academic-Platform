import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, mkdirSync, readFileSync, symlinkSync, truncateSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { binaryArchitecture, collect, commandStatus, readProbe, testOutcomes, validate, validateLicenses } from "./h1-evidence.mjs";
import { completeFixture, digest, readJson, rehash, writeJson } from "./h1-complete-fixture.mjs";
import { validateH1Workflow } from "./h1-workflow-policy.mjs";

const workflow = readFileSync(".github/workflows/h1-evidence.yml", "utf8");
const windowsPin = JSON.parse(readFileSync("tools/sqlcipher/windows-toolchain.json", "utf8"));
test("five native platforms and complete workflow execution policy", () => validateH1Workflow(workflow));
for (const [name, before, after] of [
  ["floating action", "actions/upload-artifact@bbbca2ddaa5d8feaa63e36b76fdaad77386f024f", "actions/upload-artifact@v7"],
  ["artifact overwrite", "overwrite: false", "overwrite: true"],
  ["hidden payloads", "include-hidden-files: false", "include-hidden-files: true"],
  ["permission elevation", "contents: read", "contents: write"],
  ["missing ARM", "windows-11-arm", "windows-latest"],
  ["skip upload on failure", "if: always()", "if: success()"],
  ["unlocked prerequisite", "1.98.0", "stable"],
  ["credential persistence", "persist-credentials: false", "persist-credentials: true"],
  ["arbitrary command", "node tools/h1-run.mjs", "cargo test --workspace"],
  ["job injection", "jobs:", "jobs:\n  extra:\n    runs-on: ubuntu-latest\n    steps: []"],
]) test(`workflow rejects ${name}`, () => assert.throws(() => validateH1Workflow(workflow.replace(before, after))));

test("test observations preserve ignored/failed rows and reject declaration-only PASS", () => {
  assert.deepEqual(testOutcomes("PASS\ntest actual ... ok\ntest missing ... ignored, broker absent\ntest failed ... FAILED\n"), [
    { name: "actual", status: "ok" }, { name: "missing", status: "ignored" }, { name: "failed", status: "FAILED" },
  ]);
  const zeroExit = { exitCode: 0, signal: null, error: null };
  assert.equal(commandStatus(zeroExit, "test required ... ok\n", ["required"]), "passed");
  for (const output of ["PASS\n", "test required ... ignored\n", "test another ... ok\n", "test required ... ok\ntest other ... FAILED\n"]) assert.equal(commandStatus(zeroExit, output, ["required"]), "failed");
  assert.equal(commandStatus({ ...zeroExit, exitCode: 1 }, "test required ... ok\n", ["required"]), "failed");
  assert.equal(commandStatus({ ...zeroExit, signal: "SIGTERM" }, "test required ... ok\n", ["required"]), "failed");
});

test("native executable headers distinguish x64 and ARM64", () => {
  for (const [machine, expected] of [[0x8664, "x64"], [0xaa64, "arm64"]]) {
    const pe = Buffer.alloc(128); pe.write("MZ"); pe.writeUInt32LE(64, 0x3c); pe.write("PE\0\0", 64); pe.writeUInt16LE(machine, 68);
    assert.equal(binaryArchitecture(pe), expected);
  }
  const elf = Buffer.alloc(64); elf.set([127, 69, 76, 70, 2, 1]); elf.writeUInt16LE(183, 18);
  assert.equal(binaryArchitecture(elf), "arm64");
  const mach = Buffer.alloc(64); mach.writeUInt32LE(0xfeedfacf); mach.writeUInt32LE(0x0100000c, 4);
  assert.equal(binaryArchitecture(mach), "arm64");
  assert.throws(() => binaryArchitecture(Buffer.alloc(64)));
});

test("probe observations reconcile actual retained bytes, versions and counts", () => {
  // Measurement-validator fixture only: these tiny files are not an encryption run.
  const root = mkdtempSync(join(tmpdir(), "h1-probe-validator-"));
  mkdirSync(join(root, "probe/artifacts"), { recursive: true });
  writeFileSync(join(root, "probe/artifacts/synthetic.sqlite3"), Buffer.alloc(32, 42));
  writeFileSync(join(root, "canaries.txt"), "# Test corpus\nsynthetic-canary\n");
  writeFileSync(join(root, "dependency-admission.json"), JSON.stringify({ bundled_sources: { sqlcipher_community: { version: "4.14.0", sqlite_version: "3.51.3" } } }));
  const observation = {
    lane: "sqlcipher-store", adr_002_accepted: false, production_data_allowed: false,
    schema_version: 2, storage_encryption: "SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000",
    cipher_page_size: 4096, kdf_iter: 256000, cipher_hmac_algorithm: "HMAC_SHA512", cipher_kdf_algorithm: "PBKDF2_HMAC_SHA512",
    plaintext_canary_hits: 0, files_scanned: 1, bytes_scanned: 32, canary_count: 1, readable_canary_count: 1,
    cipher_version: "4.14.0 community", sqlite_version: "3.51.3",
  };
  const rows = [{ id: "probe", status: "passed", stdout: "probe.json" }];
  const save = (value) => writeFileSync(join(root, "probe.json"), JSON.stringify(value));
  save(observation); assert.deepEqual(readProbe(root, rows), observation);
  for (const [key, value] of [["files_scanned", 2], ["bytes_scanned", 3], ["plaintext_canary_hits", 1], ["canary_count", 0], ["readable_canary_count", 0], ["cipher_version", "3.0 community"], ["sqlite_version", "0"], ["schema_version", 1], ["kdf_iter", 1], ["adr_002_accepted", true]]) {
    save({ ...observation, [key]: value }); assert.throws(() => readProbe(root, rows), key);
  }
  save({ ...observation, bytes_scanned: 16 });
  writeFileSync(join(root, "probe/artifacts/synthetic.sqlite3"), "synthetic-canary");
  assert.throws(() => readProbe(root, rows), "a claimed zero must fail the independent byte scan");
  writeFileSync(join(root, "probe/artifacts/synthetic.sqlite3"), "SQLite format 3\0");
  assert.throws(() => readProbe(root, rows), /plaintext SQLite database header/u);
  assert.equal(readProbe(root, [{ ...rows[0], status: "failed" }]), null);
  assert.equal(readProbe(root, []), null);
});

test("native notices require the distinct admitted set and source-version metadata", () => {
  const root = mkdtempSync(join(tmpdir(), "h1-notice-validator-"));
  mkdirSync(join(root, "licenses"));
  const admission = { bundled_sources: {} };
  const notices = ["sqlcipher_community", "openssl"].map((name) => {
    const path = `licenses/${name}.txt`, bytes = Buffer.from(`synthetic ${name} notice fixture`);
    const sha256 = createHash("sha256").update(bytes).digest("hex");
    writeFileSync(join(root, path), bytes);
    admission.bundled_sources[name] = { version: "synthetic-version", license_sha256: sha256 };
    return { name, status: "observed", path, bytes: bytes.length, sha256, version: "synthetic-version", versionKind: "locked-source-not-runtime-provider" };
  });
  const paths = notices.map((notice) => notice.path);
  const check = (rows, complete = true) => {
    writeFileSync(join(root, "licenses.json"), JSON.stringify(rows));
    return validateLicenses(root, paths, admission, complete);
  };
  assert.deepEqual(check(notices), notices);
  assert.throws(() => check([notices[0], notices[0]]), /duplicate native notice/u);
  assert.throws(() => check([notices[0]]), /both distinct native notices/u);
  assert.throws(() => check([{ ...notices[0], name: "unknown" }, notices[1]]), /unknown native notice/u);
  assert.throws(() => check([{ ...notices[0], version: "different" }, notices[1]]), /version differs/u);
  assert.throws(() => check([{ ...notices[0], versionKind: "runtime-provider" }, notices[1]]), /must not claim a runtime/u);
  assert.throws(() => check([{ ...notices[0], path: "../../notice.txt" }, notices[1]]));
  assert.throws(() => check([{ name: "openssl", status: "missing", reason: "source unavailable" }]), /missing native notice/u);
  assert.deepEqual(check([], false), []);
});

test("directory links and oversized metadata are rejected before JSON parsing", () => {
  const root = mkdtempSync(join(tmpdir(), "h1-bounds-"));
  const bundle = join(root, "bundle"), outside = join(root, "outside");
  mkdirSync(bundle); mkdirSync(outside);
  writeFileSync(join(bundle, "manifest.json"), "intentionally invalid JSON");
  symlinkSync(outside, join(bundle, "linked"), process.platform === "win32" ? "junction" : "dir");
  assert.throws(() => validate(bundle, {}), /symlink forbidden/u);
  const large = join(root, "large"); mkdirSync(large);
  writeFileSync(join(large, "manifest.json"), "intentionally invalid JSON");
  truncateSync(join(large, "manifest.json"), 16 * 1024 * 1024 + 1);
  assert.throws(() => validate(large, {}), /metadata exceeds byte bound/u);
});

test("failed collection is retained and validates only against exact external identity and Git bytes", () => {
  // A tiny isolated source repository is a validator fixture, never hosted evidence.
  const root = mkdtempSync(join(tmpdir(), "h1-validator-"));
  const oldCwd = process.cwd();
  const saved = Object.fromEntries(["GITHUB_SHA", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_REPOSITORY", "RUNNER_TEMP", "OPENSSL_SRC_PERL", "OPENSSL_RUST_USE_NASM", "CARGO_TARGET_DIR", "TEMP", "TMP", "TMPDIR", "CARGO_BUILD_JOBS", "CARGO_INCREMENTAL", "CARGO_TERM_COLOR", "RUST_TEST_THREADS"].map((key) => [key, process.env[key]]));
  try {
    mkdirSync(join(root, "testdata/sqlcipher-canary"), { recursive: true });
    mkdirSync(join(root, "docs/security"), { recursive: true });
    writeFileSync(join(root, "testdata/sqlcipher-canary/store-v2-canaries.txt"), "synthetic-canary\n");
    writeFileSync(join(root, "docs/security/dependency-admission-phase1.json"), '{"bundled_sources":{}}\n');
    mkdirSync(join(root, "tools/sqlcipher"), { recursive: true });
    writeFileSync(join(root, "tools/sqlcipher/windows-toolchain.json"), JSON.stringify(windowsPin));
    writeFileSync(join(root, ".nvmrc"), `${process.version.slice(1)}\n`);
    process.chdir(root);
    for (const args of [["init", "--quiet"], ["add", "."], ["-c", "user.name=H1 Synthetic Fixture", "-c", "user.email=h1@example.invalid", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "synthetic source"]]) {
      assert.equal(spawnSync("git", args, { encoding: "utf8" }).status, 0);
    }
    const commit = spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim();
    process.env.GITHUB_SHA = "0".repeat(40); // Deliberate identity failure before any toolchain command.
    process.env.GITHUB_RUN_ID = "123"; process.env.GITHUB_RUN_ATTEMPT = "1";
    process.env.GITHUB_REPOSITORY = "synthetic/fixture"; process.env.RUNNER_TEMP = root;
    const directory = join(root, "bundle");
    const platform = `${({ win32: "windows", darwin: "macos", linux: "linux" })[process.platform]}-${process.arch === "arm64" ? "aarch64" : "x86_64"}`;
    const manifest = collect(directory, platform);
    assert.equal(manifest.status, "failed"); assert.equal(manifest.commands.length, 0);
    assert(manifest.notRun.includes("encrypted-crash"));
    const expected = { commit, platform, runId: "123", attempt: "1", repository: "synthetic/fixture" };
    assert.throws(() => validate(directory, expected), /mismatch|equal/u);
    // Build a coherent missing-evidence fixture by fixing only its synthetic host/context.
    manifest.context.sha = commit;
    manifest.host.runnerArch = process.arch === "arm64" ? "ARM64" : "X64";
    const save = () => writeFileSync(join(directory, "manifest.json"), JSON.stringify(manifest));
    save();
    assert.equal(validate(directory, expected).status, "failed");
    // Directory traversal order differs from lexical order when both a directory
    // and its dot-suffixed metadata file exist (the first real hosted bundle).
    mkdirSync(join(directory, "licenses"));
    const prefixFixture = Buffer.from("synthetic inventory prefix collision");
    writeFileSync(join(directory, "licenses/fixture.txt"), prefixFixture);
    manifest.artifacts.push({ path: "licenses/fixture.txt", bytes: prefixFixture.length, sha256: createHash("sha256").update(prefixFixture).digest("hex") });
    save(); assert.equal(validate(directory, expected).status, "failed");
    manifest.source = "../../outside.json"; save();
    assert.throws(() => validate(directory, expected), /source reference/u);
    manifest.source = "source.json"; save();
    for (const [key, value] of [["commit", "f".repeat(40)], ["platform", "linux-aarch64"], ["runId", "124"], ["attempt", "2"], ["repository", "another/repository"]]) {
      assert.throws(() => validate(directory, { ...expected, [key]: value }));
    }
    manifest.acceptedH1 = true; save(); assert.throws(() => validate(directory, expected)); manifest.acceptedH1 = false;
    manifest.status = "component_checks_passed"; save(); assert.throws(() => validate(directory, expected)); manifest.status = "failed";
    const notRun = manifest.notRun;
    manifest.notRun = []; save(); assert.throws(() => validate(directory, expected)); manifest.notRun = notRun;
    save();
    writeFileSync(join(directory, "canaries.txt"), "modified-canary\n");
    assert.throws(() => validate(directory, expected), /artifact mismatch/u);
    const artifact = manifest.artifacts.find((item) => item.path === "canaries.txt");
    const bytes = readFileSync(join(directory, "canaries.txt")); artifact.bytes = bytes.length; artifact.sha256 = createHash("sha256").update(bytes).digest("hex"); save();
    assert.throws(() => validate(directory, expected), /equal/u, "rehashed substituted corpus must not match committed source");
  } finally {
    process.chdir(oldCwd);
    for (const [key, value] of Object.entries(saved)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; }
  }
});

test("complete v2 bundles reject inconsistent probe, prerequisites and binary inventories", () => {
  const root = mkdtempSync(join(tmpdir(), "h1-complete-validator-"));
  const fixture = completeFixture(root, windowsPin), oldCwd = process.cwd();
  const edit = (bundle, path, change) => {
    const file = join(bundle, path), value = readJson(file); change(value); writeJson(file, value);
  };
  const command = (id, change) => (bundle) => edit(bundle, `commands/${id}.json`, change);
  const windows = (change) => (bundle) => edit(bundle, "logs/windows-toolchain.stdout.log", change);
  const cases = [
    ["unrelated probe executable", command("probe", (row) => { row.executable = "C:\\unrelated\\nonexistent.exe"; }), /probe invocation/u],
    ["unrelated probe output directory", command("probe", (row) => { row.args[1] = "C:\\unrelated\\output"; }), /probe output directory/u],
    ["same-architecture test substituted for probe", (bundle) => {
      const rows = readJson(join(bundle, "binaries.json"));
      const probe = rows.find((row) => row.command === "probe-build"), test = rows.find((row) => row.command === "store-tests");
      const bytes = readFileSync(join(bundle, test.path)); writeFileSync(join(bundle, probe.path), bytes);
      Object.assign(probe, digest(bytes)); writeJson(join(bundle, "binaries.json"), rows);
    }, /probe retained bytes/u],
    ["missing invocation measurement", command("probe", (row) => { delete row.executableObservation; }), /probe retained bytes/u],
    ["probe changes during invocation", command("probe", (row) => { row.executableObservation.after.sha256 = "0".repeat(64); }), /changed during invocation/u],
    ["wrong compiler feature", (bundle) => {
      const path = join(bundle, "logs/probe-build.stdout.log"), item = readJson(path); item.features = ["bundled-sqlite"]; writeFileSync(path, JSON.stringify(item) + "\n");
    }, /compiler features/u],
    ["wrong compiler target", (bundle) => {
      const path = join(bundle, "logs/probe-build.stdout.log"), item = readJson(path); item.target.kind = ["test"]; writeFileSync(path, JSON.stringify(item) + "\n");
    }, /compiler target kind/u],
    ["wrong compiler executable directory", (bundle) => {
      const path = join(bundle, "logs/probe-build.stdout.log"), item = readJson(path); item.executable = "C:\\unrelated\\sqlcipher_store_probe.exe"; writeFileSync(path, JSON.stringify(item) + "\n");
    }, /outside target directory/u],
    ["unplanned Windows verifier arguments", command("windows-toolchain", (row) => { row.args = ["--version"]; }), /command arguments/u],
    ["Windows declaration without observations", (bundle) => writeJson(join(bundle, "logs/windows-toolchain.stdout.log"), { modules: "verified", pathAdded: false }), /equal|admission/u],
    ["wrong archive digest", windows((value) => { value.archiveObservation.sha256 = "0".repeat(64); }), /measured archive/u],
    ["wrong admitted archive", windows((value) => { value.archive.url = "https://example.invalid/archive.zip"; }), /archive admission/u],
    ["wrong interpreter identity", windows((value) => { value.identity.stdout = "v0.0.0\nunknown\n"; }), /identity observation/u],
    ["missing interpreter bytes", windows((value) => { delete value.interpreterObservation; })],
    ["missing module observation", windows((value) => { delete value.modules; }), /modules executable/u],
    ["failed module observation", windows((value) => { value.modules.exitCode = 1; })],
    ["wrong module command", windows((value) => { value.modules.args = ["--version"]; }), /modules command/u],
    ["bundled tool on PATH", windows((value) => { value.pathPolicy.entriesWithinInstallRoot = ["C:\\fixture\\temp\\h1-perl\\c\\bin"]; }), /no-PATH/u],
    ["claimed PATH addition", windows((value) => { value.pathPolicy.pathAdded = true; }), /no-PATH/u],
    ["wrong Node version", (bundle) => edit(bundle, "manifest.json", (value) => { value.host.node = "v0.0.0"; }), /Node command/u],
    ["coherently wrong Node version", (bundle) => {
      edit(bundle, "manifest.json", (value) => { value.host.node = "v0.0.0"; }); writeFileSync(join(bundle, "logs/node.stdout.log"), "v0.0.0\n");
    }],
    ["wrong Cargo prerequisite executable", command("cargo", (row) => { row.executable = "other-cargo"; }), /command executable/u],
    ["wrong Perl prerequisite arguments", command("perl", (row) => { row.args = ["--version"]; }), /command arguments/u],
    ["missing prerequisite output", (bundle) => writeFileSync(join(bundle, "logs/perl.stdout.log"), ""), /missing prerequisite output/u],
    ["missing prerequisite record", (bundle) => edit(bundle, "manifest.json", (value) => { value.commands = value.commands.filter((path) => path !== "commands/windows-toolchain.json"); })],
    ["failed prerequisite labelled complete", command("windows-toolchain", (row) => { row.exitCode = 1; row.status = "failed"; })],
    ["duplicate binary record", (bundle) => edit(bundle, "binaries.json", (value) => { value.push(value[0]); }), /duplicate retained binary path/u],
    ["duplicate identity with a unique retained path", (bundle) => {
      edit(bundle, "binaries.json", (value) => { const other = { ...value[0], path: "binaries/duplicate.exe" }; cpSync(join(bundle, value[0].path), join(bundle, other.path)); value.push(other); });
    }, /duplicate binary invocation/u],
    ["missing eligible compiler inventory row", (bundle) => edit(bundle, "binaries.json", (value) => { value.splice(value.findIndex((row) => row.command === "portability-tests" && row.target === "encrypted_crash"), 1); })],
    ["duplicate compiler observation", (bundle) => {
      const path = join(bundle, "logs/probe-build.stdout.log"), output = readFileSync(path, "utf8"); writeFileSync(path, output + output);
    }, /duplicate compiler invocation/u],
    ["unexecuted retained test", (bundle) => writeFileSync(join(bundle, "logs/encrypted-crash.stderr.log"), ""), /Cargo execution inventories/u],
    ["duplicate native notice cannot satisfy completeness", (bundle) => edit(bundle, "licenses.json", (value) => { value[1] = value[0]; }), /duplicate native notice/u],
    ["historical v1 relabelling is not compatibility", (bundle) => edit(bundle, "manifest.json", (value) => { value.version = 1; }), /newly collected execution proof/u],
  ];
  try {
    process.chdir(fixture.sourceRoot);
    assert.equal(validate(fixture.bundle, fixture.expected).status, "component_checks_passed");
    for (const [index, [name, mutate, message]] of cases.entries()) {
      const bundle = join(root, `case-${index}`); cpSync(fixture.bundle, bundle, { recursive: true });
      mutate(bundle); rehash(bundle);
      assert.throws(() => validate(bundle, fixture.expected), message, name);
    }
    // Distinct Cargo invocations can rebuild the same executable path. Keep both
    // retained byte observations and their different feature sets, never dedupe.
    const distinct = join(root, "distinct-builds"); cpSync(fixture.bundle, distinct, { recursive: true });
    const binaries = readJson(join(distinct, "binaries.json"));
    const first = binaries.find((row) => row.command === "portability-tests" && row.target === "encrypted_crash");
    const second = binaries.find((row) => row.command === "encrypted-crash"); second.executable = first.executable;
    writeJson(join(distinct, "binaries.json"), binaries);
    const stdoutPath = join(distinct, "logs/encrypted-crash.stdout.log");
    const lines = readFileSync(stdoutPath, "utf8").split("\n");
    const compiler = JSON.parse(lines[0]); compiler.executable = first.executable; lines[0] = JSON.stringify(compiler);
    writeFileSync(stdoutPath, lines.join("\n"));
    writeFileSync(join(distinct, "logs/encrypted-crash.stderr.log"), `Running tests/encrypted_crash.rs (${first.executable})\n`);
    rehash(distinct); assert.equal(validate(distinct, fixture.expected).status, "component_checks_passed");
    assert.equal(binaries.length, 5);
    assert.notEqual(first.sha256, second.sha256);
    const failed = join(root, "failed-prerequisite"); cpSync(fixture.bundle, failed, { recursive: true });
    command("windows-toolchain", (row) => { row.exitCode = 1; row.status = "failed"; })(failed);
    writeJson(join(failed, "logs/windows-toolchain.stdout.log"), { modules: "incomplete" });
    edit(failed, "manifest.json", (value) => { value.status = "failed"; });
    rehash(failed);
    assert.deepEqual(validate(failed, fixture.expected), { integrity: "verified", status: "failed", acceptedH1: false, platform: fixture.expected.platform, commit: fixture.expected.commit });
    const missing = join(root, "missing-prerequisite"); cpSync(fixture.bundle, missing, { recursive: true });
    edit(missing, "manifest.json", (value) => { value.commands = value.commands.filter((path) => path !== "commands/windows-toolchain.json"); value.notRun = ["windows-toolchain"]; value.status = "failed"; });
    rehash(missing); assert.equal(validate(missing, fixture.expected).status, "failed");
  } finally { process.chdir(oldCwd); }
});

for (const platform of ["windows-aarch64", "linux-x86_64", "linux-aarch64", "macos-aarch64"]) test(`complete ${platform} fixture checks native paths and prerequisite plans`, () => {
  const root = mkdtempSync(join(tmpdir(), "h1-platform-validator-"));
  const fixture = completeFixture(root, windowsPin, platform), oldCwd = process.cwd();
  try {
    process.chdir(fixture.sourceRoot);
    assert.equal(validate(fixture.bundle, fixture.expected).status, "component_checks_passed");
    const id = platform.startsWith("windows") ? "windows-prerequisites" : "cc";
    const path = join(fixture.bundle, `commands/${id}.json`), row = readJson(path);
    row.executable = "unplanned-tool"; writeJson(path, row); rehash(fixture.bundle);
    assert.throws(() => validate(fixture.bundle, fixture.expected), /command executable/u);
    if (!platform.startsWith("windows")) {
      row.executable = "cc"; writeJson(path, row);
      const makePath = join(fixture.bundle, "commands/make.json"), make = readJson(makePath);
      make.args = ["all"]; writeJson(makePath, make); rehash(fixture.bundle);
      assert.throws(() => validate(fixture.bundle, fixture.expected), /command arguments/u);
    }
  } finally { process.chdir(oldCwd); }
});

for (const platform of ["windows-x86_64", "windows-aarch64", "linux-x86_64", "linux-aarch64", "macos-aarch64"]) test(`complete ${platform} inventory cannot omit targets, alias executables or change native format`, () => {
  const root = mkdtempSync(join(tmpdir(), "h1-inventory-validator-"));
  const fixture = completeFixture(root, windowsPin, platform), oldCwd = process.cwd();
  const edit = (bundle, path, change) => { const file = join(bundle, path), value = readJson(file); change(value); writeJson(file, value); };
  const compiler = (bundle, change) => {
    const file = join(bundle, "logs/portability-tests.stdout.log");
    const lines = readFileSync(file, "utf8").split("\n").flatMap((line) => {
      let value; try { value = JSON.parse(line); } catch { return [line]; }
      return change(value) === false ? [] : [JSON.stringify(value)];
    });
    writeFileSync(file, lines.join("\n"));
  };
  const omit = (bundle) => {
    edit(bundle, "binaries.json", (rows) => { rows.splice(rows.findIndex((row) => row.command === "portability-tests" && row.target === "encrypted_crash"), 1); });
    compiler(bundle, (row) => row.target?.name === "encrypted_crash" ? false : undefined);
  };
  const cases = [
    ["eligible target omitted from both compiler and binary inventories", omit, /complete eligible target set/u],
    ["eligible target omitted from both inventories and execution log", (bundle) => {
      omit(bundle);
      const path = join(bundle, "logs/portability-tests.stderr.log"); writeFileSync(path, readFileSync(path, "utf8").split("\n").filter((line) => !line.includes("encrypted_crash")).join("\n"));
    }, /complete eligible target set/u],
    ["two targets claim one executable within a command", (bundle) => {
      let executable;
      edit(bundle, "binaries.json", (rows) => {
        executable = rows.find((row) => row.command === "portability-tests" && row.target === "encrypted_backup").executable;
        rows.find((row) => row.command === "portability-tests" && row.target === "encrypted_crash").executable = executable;
      });
      compiler(bundle, (row) => { if (row.target?.name === "encrypted_crash") row.executable = executable; });
    }, /duplicate executable within one command/u],
    ["unreconciled eligible execution", (bundle) => {
      const path = join(bundle, "logs/portability-tests.stderr.log"); writeFileSync(path, readFileSync(path, "utf8") + "Running tests/encrypted_crash.rs (unrecorded-executable)\n");
    }, /Cargo execution inventories/u],
    ["retained executable outside binary inventory", (bundle) => {
      const binary = readJson(join(bundle, "binaries.json"))[0]; cpSync(join(bundle, binary.path), join(bundle, "binaries/unrecorded.exe"));
    }, /retained binary files and inventory/u],
    ["wrong OS format with matching CPU and coherent hashes", (bundle) => {
      edit(bundle, "binaries.json", (rows) => {
        const probe = rows.find((row) => row.command === "probe-build"), bytes = Buffer.alloc(160);
        if (platform.startsWith("linux")) { bytes.write("MZ"); bytes.writeUInt32LE(64, 0x3c); bytes.write("PE\0\0", 64); bytes.writeUInt16LE(probe.architecture === "arm64" ? 0xaa64 : 0x8664, 68); }
        else { bytes.set([127, 69, 76, 70, 2, 1]); bytes.writeUInt16LE(probe.architecture === "arm64" ? 183 : 62, 18); }
        writeFileSync(join(bundle, probe.path), bytes); Object.assign(probe, digest(bytes));
        edit(bundle, "commands/probe.json", (row) => { row.executableObservation = { before: digest(bytes), after: digest(bytes) }; });
      });
    }, /executable format differs from platform/u],
  ];
  try {
    process.chdir(fixture.sourceRoot);
    assert.equal(validate(fixture.bundle, fixture.expected).status, "component_checks_passed");
    for (const [index, [name, mutate, message]] of cases.entries()) {
      const bundle = join(root, `case-${index}`); cpSync(fixture.bundle, bundle, { recursive: true }); mutate(bundle); rehash(bundle);
      assert.throws(() => validate(bundle, fixture.expected), message, name);
    }
    const failed = join(root, "incomplete-failed-command"); cpSync(fixture.bundle, failed, { recursive: true }); omit(failed);
    edit(failed, "commands/portability-tests.json", (row) => { row.exitCode = 1; row.status = "failed"; });
    edit(failed, "manifest.json", (row) => { row.status = "failed"; }); rehash(failed);
    assert.equal(validate(failed, fixture.expected).integrity, "verified");
    assert.equal(validate(failed, fixture.expected).status, "failed");
  } finally { process.chdir(oldCwd); }
});
