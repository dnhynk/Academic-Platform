import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, symlinkSync, truncateSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { binaryArchitecture, collect, commandStatus, readProbe, testOutcomes, validate } from "./h1-evidence.mjs";
import { validateH1Workflow } from "./h1-workflow-policy.mjs";

const workflow = readFileSync(".github/workflows/h1-evidence.yml", "utf8");
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
  writeFileSync(join(root, "probe/artifacts/synthetic.bin"), Buffer.from([1, 2, 3, 4]));
  writeFileSync(join(root, "canaries.txt"), "# Test corpus\nsynthetic-canary\n");
  writeFileSync(join(root, "dependency-admission.json"), JSON.stringify({ bundled_sources: { sqlcipher_community: { version: "4.14.0", sqlite_version: "3.51.3" } } }));
  const observation = {
    lane: "sqlcipher-store", adr_002_accepted: false, production_data_allowed: false,
    schema_version: 2, storage_encryption: "SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000",
    cipher_page_size: 4096, kdf_iter: 256000, cipher_hmac_algorithm: "HMAC_SHA512", cipher_kdf_algorithm: "PBKDF2_HMAC_SHA512",
    plaintext_canary_hits: 0, files_scanned: 1, bytes_scanned: 4, canary_count: 1, readable_canary_count: 1,
    cipher_version: "4.14.0 community", sqlite_version: "3.51.3",
  };
  const rows = [{ id: "probe", status: "passed", stdout: "probe.json" }];
  const save = (value) => writeFileSync(join(root, "probe.json"), JSON.stringify(value));
  save(observation); assert.deepEqual(readProbe(root, rows), observation);
  for (const [key, value] of [["files_scanned", 2], ["bytes_scanned", 3], ["plaintext_canary_hits", 1], ["canary_count", 0], ["readable_canary_count", 0], ["cipher_version", "3.0 community"], ["sqlite_version", "0"], ["schema_version", 1], ["kdf_iter", 1], ["adr_002_accepted", true]]) {
    save({ ...observation, [key]: value }); assert.throws(() => readProbe(root, rows), key);
  }
  save({ ...observation, bytes_scanned: 16 });
  writeFileSync(join(root, "probe/artifacts/synthetic.bin"), "synthetic-canary");
  assert.throws(() => readProbe(root, rows), "a claimed zero must fail the independent byte scan");
  assert.equal(readProbe(root, [{ ...rows[0], status: "failed" }]), null);
  assert.equal(readProbe(root, []), null);
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
  const saved = Object.fromEntries(["GITHUB_SHA", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_REPOSITORY", "RUNNER_TEMP"].map((key) => [key, process.env[key]]));
  try {
    mkdirSync(join(root, "testdata/sqlcipher-canary"), { recursive: true });
    mkdirSync(join(root, "docs/security"), { recursive: true });
    writeFileSync(join(root, "testdata/sqlcipher-canary/store-v2-canaries.txt"), "synthetic-canary\n");
    writeFileSync(join(root, "docs/security/dependency-admission-phase1.json"), '{"bundled_sources":{}}\n');
    process.chdir(root);
    for (const args of [["init", "--quiet"], ["add", "."], ["-c", "user.name=H1 Synthetic Fixture", "-c", "user.email=h1@example.invalid", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "synthetic source"]]) {
      assert.equal(spawnSync("git", args, { encoding: "utf8" }).status, 0);
    }
    const commit = spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim();
    process.env.GITHUB_SHA = "0".repeat(40); // Deliberate identity failure before any toolchain command.
    process.env.GITHUB_RUN_ID = "123"; process.env.GITHUB_RUN_ATTEMPT = "1";
    process.env.GITHUB_REPOSITORY = "synthetic/fixture"; process.env.RUNNER_TEMP = root;
    const directory = join(root, "bundle");
    const manifest = collect(directory, "linux-x86_64");
    assert.equal(manifest.status, "failed"); assert.equal(manifest.commands.length, 0);
    assert(manifest.notRun.includes("encrypted-crash"));
    const expected = { commit, platform: "linux-x86_64", runId: "123", attempt: "1", repository: "synthetic/fixture" };
    assert.throws(() => validate(directory, expected), /mismatch|equal/u);
    // Build a coherent missing-evidence fixture by fixing only its synthetic host/context.
    manifest.context.sha = commit;
    manifest.host = { ...manifest.host, platform: "linux", arch: "x64", runnerArch: "X64" };
    const save = () => writeFileSync(join(directory, "manifest.json"), JSON.stringify(manifest));
    save();
    assert.equal(validate(directory, expected).status, "failed");
    manifest.source = "../../outside.json"; save();
    assert.throws(() => validate(directory, expected), /source reference/u);
    manifest.source = "source.json"; save();
    for (const [key, value] of [["commit", "f".repeat(40)], ["platform", "linux-aarch64"], ["runId", "124"], ["attempt", "2"], ["repository", "another/repository"]]) {
      assert.throws(() => validate(directory, { ...expected, [key]: value }));
    }
    manifest.acceptedH1 = true; save(); assert.throws(() => validate(directory, expected)); manifest.acceptedH1 = false;
    manifest.status = "component_checks_passed"; save(); assert.throws(() => validate(directory, expected)); manifest.status = "failed";
    manifest.notRun = []; save(); assert.throws(() => validate(directory, expected)); manifest.notRun = ["store-lint", "store-tests", "probe-build", "portability-lint", "portability-tests", "encrypted-crash", "probe"];
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
