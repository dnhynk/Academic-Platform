import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import test from "node:test";
import { commandStatus } from "./h1-evidence.mjs";
import { compilerRows, deriveStatus, EXECUTIONS, MISSING, plans, validate } from "./macos-keystore-evidence.mjs";
import { validateMacosKeystoreWorkflow } from "./macos-keystore-workflow-policy.mjs";

test("macOS lane permits only the bounded unprovisioned hosted workflow", () => {
  const text = readFileSync(".github/workflows/macos-keystore.yml", "utf8");
  validateMacosKeystoreWorkflow(text);
  for (const [before, after] of [
    ["contents: read", "contents: write"],
    ["persist-credentials: false", "persist-credentials: true"],
    ["timeout-minutes: 40", "timeout-minutes: 140"],
    ["run: node tools/macos-keystore-run.mjs", "run: security unlock-keychain"],
    ["runs-on: macos-latest", "runs-on: self-hosted"],
  ]) assert.throws(() => validateMacosKeystoreWorkflow(text.replace(before, after)));
});

test("build success, ignored tests, zero matched tests and wrong names cannot count as native execution", () => {
  const row = { exitCode: 0, signal: null, error: null };
  const name = EXECUTIONS.find(([id]) => id === "refusal")[2];
  for (const output of ["Finished test profile", "running 0 tests\ntest result: ok.", `test ${name} ... ignored\n`, "test different_test ... ok\n"]) {
    assert.equal(commandStatus(row, output, [name]), "failed");
  }
  assert.equal(commandStatus(row, `test ${name} ... ok\n`, [name]), "passed");
  assert.equal(commandStatus({ ...row, exitCode: 1 }, `test ${name} ... ok\n`, [name]), "failed");
});

test("refusal completion retains positive, packaging, memory and hardware gaps", () => {
  const rows = ["source-preflight", "rustc", "fetch", "leaf-build", "crypto-build", "lint", ...EXECUTIONS.map(([id]) => id)].map((id) => ({ id, status: "passed" }));
  assert.equal(deriveStatus(rows, null), "refusal_and_api_checks_passed");
  for (const row of rows) assert.equal(deriveStatus(rows.filter((candidate) => candidate !== row), null), "failed");
  assert.equal(deriveStatus(rows, "interrupted"), "failed");
  assert.equal(MISSING.positiveNative.status, "not_run");
  assert.equal(MISSING.packaging.status, "missing");
  assert.equal(MISSING.hardwareProtection.status, "unverified");
  assert.equal(MISSING.memoryReview.status, "missing");
});

const execution = { cwd: "/synthetic-source", nodeExecutable: "/synthetic-node", targetDir: "/synthetic-lane/target", tempDir: "/synthetic-lane/temp" };
function compilerFixture(id) {
  return (id === "leaf-build" ? ["academic_keystore_platform", "facade", "macos_native"] : ["native_keystore"]).map((name) => {
    const library = name === "academic_keystore_platform", leaf = id === "leaf-build";
    return { reason: "compiler-artifact", executable: `${execution.targetDir}/debug/deps/${name}-123`, profile: { test: true },
      features: leaf ? ["default"] : ["default", "os-keystore"],
      target: { name, kind: [library ? "lib" : "test"], crate_types: [library ? "lib" : "bin"],
        src_path: `${execution.cwd}/crates/${leaf ? "keystore-platform" : "crypto"}/${library ? "src/lib.rs" : `tests/${name}.rs`}` } };
  });
}
const lines = (records) => records.map((record) => JSON.stringify(record)).join("\n") + "\n";

test("native compiler inventory requires every planned target and its exact source, features and path", () => {
  const row = { id: "leaf-build", status: "passed" };
  const fixture = compilerFixture(row.id);
  assert.equal(compilerRows(row, lines(fixture), execution).length, 3);
  assert.throws(() => compilerRows(row, lines(fixture.slice(0, 2)), execution));
  assert.equal(compilerRows({ ...row, status: "failed" }, lines(fixture.slice(0, 2)), execution).length, 2);
  for (const mutate of [
    (records) => records.push(records[0]),
    (records) => { records[0].target.src_path = "/other.rs"; },
    (records) => { records[0].features = []; },
    (records) => { records[0].target.kind = ["bin"]; },
    (records) => { records[0].profile.test = false; },
    (records) => { records[0].executable = "/other/target"; },
    (records) => { records[0].executable = records[1].executable; },
  ]) {
    const records = structuredClone(fixture); mutate(records);
    assert.throws(() => compilerRows(row, lines(records), execution));
  }
});

test("unsigned fixture validation reconciles execution, source, artifacts and missing claims", () => {
  // Validator fixture only: these constructed logs and header bytes are never
  // collected, uploaded or described as a native execution or acceptance run.
  const original = process.cwd(), root = mkdtempSync(join(tmpdir(), "t251-validator-"));
  const bundle = join(root, "bundle"), repo = join(root, "source");
  mkdirSync(repo); mkdirSync(bundle);
  const measured = (bytes) => ({ bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") });
  const save = (path, value) => { mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, typeof value === "string" || Buffer.isBuffer(value) ? value : JSON.stringify(value)); };
  const read = (path) => JSON.parse(readFileSync(join(bundle, path), "utf8"));
  const git = (args) => { const row = spawnSync("git", args, { encoding: "utf8", windowsHide: true }); assert.equal(row.status, 0, row.stderr); return row.stdout.trim(); };
  try {
    process.chdir(repo);
    const files = { ".nvmrc": "24.19.0\n", "rust-toolchain.toml": '[toolchain]\nchannel = "1.98.0"\n' };
    for (const [path, value] of Object.entries(files)) save(path, value);
    git(["init", "--quiet"]); git(["add", "."]);
    git(["-c", "user.name=Synthetic validator fixture", "-c", "user.email=synthetic@example.invalid", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "Synthetic validator fixture only"]);
    const expected = { commit: git(["rev-parse", "HEAD"]), repository: "synthetic/fixture", runId: "1", attempt: "1" };
    save(join(bundle, "source.json"), { commit: expected.commit, tree: git(["rev-parse", "HEAD^{tree}"]), files: Object.entries(files).map(([path, value]) => ({ path, ...measured(Buffer.from(value)) })) });
    const binaries = [], rows = [];
    for (const plan of plans()) {
      const records = plan.id.endsWith("-build") ? compilerFixture(plan.id) : [];
      const output = records.length ? lines(records) : plan.id === "rustc" ? "host: aarch64-apple-darwin\nrelease: 1.98.0\n" : "synthetic command output\n";
      const row = { ...plan, executable: plan.id === "source-preflight" ? execution.nodeExecutable : plan.executable,
        cwd: execution.cwd, started: "2026-09-08T00:00:00Z", finished: "2026-09-08T00:00:01Z", exitCode: 0, signal: null, error: null,
        stdout: `logs/${plan.id}.stdout.log`, stderr: `logs/${plan.id}.stderr.log`, tests: [], status: "passed", executableBefore: null, executableAfter: null };
      rows.push(row); save(join(bundle, row.stdout), output); save(join(bundle, row.stderr), "");
      for (const identity of records.length ? compilerRows(row, output, execution) : []) {
        const bytes = Buffer.alloc(64); bytes.writeUInt32LE(0xfeedfacf); bytes.writeUInt32LE(0x0100000c, 4);
        const binary = { path: `binaries/${identity.target}`, ...identity, ...measured(bytes) };
        binaries.push(binary); save(join(bundle, binary.path), bytes);
      }
    }
    for (const [id, target, name] of EXECUTIONS) {
      const binary = binaries.find((candidate) => candidate.target === target);
      const measurement = { bytes: binary.bytes, sha256: binary.sha256 };
      const row = { id, executable: binary.executable, args: ["--exact", name, ...(id === "refusal" ? ["--ignored"] : []), "--test-threads=1", "--show-output"],
        cwd: execution.cwd, started: "2026-09-08T00:00:02Z", finished: "2026-09-08T00:00:03Z", exitCode: 0, signal: null, error: null,
        stdout: `logs/${id}.stdout.log`, stderr: `logs/${id}.stderr.log`, tests: [{ name, status: "ok" }], status: "passed", executableBefore: measurement, executableAfter: measurement };
      rows.push(row); save(join(bundle, row.stdout), `test ${name} ... ok\n`); save(join(bundle, row.stderr), "");
    }
    for (const row of rows) save(join(bundle, `commands/${row.id}.json`), row);
    save(join(bundle, "binaries.json"), binaries);
    const manifest = { format: "h1-native-keystore-evidence", version: 1, provider: "MACOS_KEYCHAIN_DATA_PROTECTION_V1", acceptedH1: false, productionDataAllowed: false,
      commit: expected.commit, context: { ...expected, sha: expected.commit }, host: { platform: "darwin", arch: "arm64", node: "v24.19.0" },
      scope: "disposable-unprovisioned-v1", missing: MISSING, execution, commands: rows.map(({ id }) => `commands/${id}.json`), notRun: [], failure: null, status: "refusal_and_api_checks_passed" };
    const inventory = (prefix = "") => readdirSync(join(bundle, prefix), { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : 1).flatMap((entry) => {
      const path = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (path === "manifest.json") return [];
      return entry.isDirectory() ? inventory(path) : [{ path, ...measured(readFileSync(join(bundle, path))) }];
    });
    const rehash = (value) => save(join(bundle, "manifest.json"), { ...value, artifacts: inventory() });
    rehash(manifest);
    assert.deepEqual(validate(bundle, expected), { integrity: "verified", status: "refusal_and_api_checks_passed", acceptedH1: false });
    const baseline = new Map(inventory().map(({ path }) => [path, readFileSync(join(bundle, path))]));
    for (const mutate of [
      (value) => { value.acceptedH1 = true; },
      (value) => { value.host.node = "v0.0.0"; },
      (value) => { value.notRun = ["refusal"]; },
      (value) => { value.commands.reverse(); },
      () => { const row = read("commands/refusal.json"); row.executableAfter.sha256 = "0".repeat(64); save(join(bundle, "commands/refusal.json"), row); },
      () => { const binaryRows = read("binaries.json"); binaryRows.pop(); save(join(bundle, "binaries.json"), binaryRows); },
      () => save(join(bundle, "logs/refusal.stdout.log"), "running 0 tests\ntest result: ok.\n"),
      () => save(join(bundle, "logs/rustc.stdout.log"), "host: aarch64-apple-darwin\nrelease: 0.0.0\n"),
      () => { const row = read("commands/leaf-build.json"); row.stdout = "../outside.log"; save(join(bundle, "commands/leaf-build.json"), row); },
      () => { const source = read("source.json"); source.files[0].sha256 = "0".repeat(64); save(join(bundle, "source.json"), source); },
    ]) {
      for (const [path, bytes] of baseline) save(join(bundle, path), bytes);
      const value = structuredClone(manifest); mutate(value); rehash(value);
      assert.throws(() => validate(bundle, expected));
    }
    for (const [path, bytes] of baseline) save(join(bundle, path), bytes);
    const failed = read("commands/refusal.json"); failed.exitCode = 1; failed.status = "failed";
    save(join(bundle, "commands/refusal.json"), failed);
    rehash({ ...manifest, status: "failed" });
    assert.equal(validate(bundle, expected).status, "failed");
  } finally { process.chdir(original); }
});
