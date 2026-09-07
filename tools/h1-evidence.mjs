import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { closeSync, copyFileSync, existsSync, lstatSync, mkdirSync, openSync, readFileSync, readdirSync, realpathSync, statSync, writeFileSync } from "node:fs";
import { arch, homedir, machine, platform, release } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { CATEGORIES, PLATFORMS, commandPlan } from "./h1-plan.mjs";

const MAX_FILE = 512 * 1024 * 1024;
const MAX_TOTAL = 1024 * 1024 * 1024;
const sha = (bytes) => createHash("sha256").update(bytes).digest("hex");
const json = (path) => {
  const info = lstatSync(path);
  assert(info.isFile() && !info.isSymbolicLink(), "metadata must be a regular file");
  assert(info.size <= 16 * 1024 * 1024, "metadata exceeds byte bound");
  return JSON.parse(readFileSync(path, "utf8"));
};
function write(path, value) { mkdirSync(dirname(path), { recursive: true }); writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`, { flag: "wx" }); }
function files(root, prefix = "", budget = { count: 0, bytes: 0 }) {
  if (prefix === "") {
    const info = lstatSync(root);
    assert(info.isDirectory() && !info.isSymbolicLink(), "bundle root must be a real directory");
  }
  assert(prefix.split("/").length <= 32, "directory depth exceeds bound");
  return readdirSync(join(root, prefix)).sort().flatMap((name) => {
    const path = prefix ? `${prefix}/${name}` : name;
    const info = lstatSync(join(root, path));
    assert(!info.isSymbolicLink(), `symlink forbidden: ${path}`);
    if (info.isDirectory()) return files(root, path, budget);
    assert(info.isFile() && info.size <= MAX_FILE, `unbounded/non-file: ${path}`);
    budget.count += 1; budget.bytes += info.size;
    assert(budget.count <= 10000 && budget.bytes <= MAX_TOTAL, "bundle inventory exceeds bound");
    return [path];
  });
}
function digestFile(path) {
  const info = lstatSync(path);
  assert(info.isFile() && !info.isSymbolicLink() && info.size <= MAX_FILE, `unbounded/non-file: ${path}`);
  return { bytes: info.size, sha256: sha(readFileSync(path)) };
}
function git(args) {
  const result = spawnSync("git", args, { encoding: "utf8", maxBuffer: 32 * 1024 * 1024 });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
}
function verifySource(source) {
  assert.equal(git(["rev-parse", `${source.commit}^{tree}`]), source.tree);
  const entries = git(["ls-tree", "-r", "-z", source.commit]).split("\0").filter(Boolean).map((entry) => {
    const [metadata, path] = entry.split("\t");
    const [mode, type, oid] = metadata.split(" ");
    assert(type === "blob" && ["100644", "100755"].includes(mode), "source symlinks/submodules unsupported");
    return { path, oid };
  });
  assert.deepEqual(source.files.map((item) => item.path).sort(), entries.map((item) => item.path).sort(), "source inventory differs from commit");
  const result = spawnSync("git", ["cat-file", "--batch"], { input: entries.map((entry) => entry.oid).join("\n") + "\n", maxBuffer: 256 * 1024 * 1024 });
  assert.equal(result.status, 0);
  let offset = 0;
  const byPath = new Map(source.files.map((item) => [item.path, item]));
  for (const entry of entries) {
    const end = result.stdout.indexOf(10, offset);
    const [oid, type, length] = result.stdout.subarray(offset, end).toString().split(" ");
    assert.equal(oid, entry.oid); assert.equal(type, "blob");
    const bytes = result.stdout.subarray(end + 1, end + 1 + Number(length));
    assert.deepEqual(byPath.get(entry.path), { path: entry.path, bytes: bytes.length, sha256: sha(bytes) }, `source bytes differ from commit: ${entry.path}`);
    offset = end + 2 + bytes.length;
  }
}
export function binaryArchitecture(bytes) {
  if (bytes.subarray(0, 2).toString() === "MZ") {
    const offset = bytes.readUInt32LE(0x3c);
    assert.equal(bytes.subarray(offset, offset + 4).toString(), "PE\0\0");
    return ({ 0x8664: "x64", 0xaa64: "arm64" })[bytes.readUInt16LE(offset + 4)];
  }
  if (bytes.subarray(0, 4).equals(Buffer.from([127, 69, 76, 70]))) {
    assert.equal(bytes[4], 2); assert.equal(bytes[5], 1);
    return ({ 62: "x64", 183: "arm64" })[bytes.readUInt16LE(18)];
  }
  if (bytes.readUInt32LE(0) === 0xfeedfacf) return ({ 0x01000007: "x64", 0x0100000c: "arm64" })[bytes.readUInt32LE(4)];
  throw new Error("unsupported executable format");
}
export function testOutcomes(log) {
  return [...log.matchAll(/^test (\S+) \.\.\. (ok|FAILED|ignored)(?:[^\r\n]*)$/gmu)].map((match) => ({ name: match[1], status: match[2] }));
}
export function commandStatus(row, output, requiredTests = []) {
  if (row.exitCode !== 0 || row.signal !== null || row.error !== null) return "failed";
  const tests = testOutcomes(output);
  if (tests.some((test) => test.status === "FAILED")) return "failed";
  if (requiredTests.some((name) => !tests.some((test) => test.name === name && test.status === "ok"))) return "failed";
  return "passed";
}
function run(bundle, id, executable, args, requiredTests = []) {
  const stdout = `logs/${id}.stdout.log`, stderr = `logs/${id}.stderr.log`;
  mkdirSync(join(bundle, "logs"), { recursive: true });
  const out = openSync(join(bundle, stdout), "wx"), err = openSync(join(bundle, stderr), "wx");
  const started = new Date().toISOString();
  const result = spawnSync(executable, args, { stdio: ["ignore", out, err], timeout: 45 * 60 * 1000, windowsHide: true });
  closeSync(out); closeSync(err);
  const output = readFileSync(join(bundle, stdout), "utf8");
  const row = { id, executable, args, started, finished: new Date().toISOString(), exitCode: result.status, signal: result.signal, error: result.error?.message ?? null, stdout, stderr };
  row.status = commandStatus(row, output, requiredTests);
  row.tests = testOutcomes(output);
  write(join(bundle, `commands/${id}.json`), row);
  console.log(`${id}: ${row.status} (exit ${row.exitCode})`);
  return row;
}
function deriveCategories(rows, probe) {
  const refs = (ids) => ids.filter((id) => rows.some((row) => row.id === id)).map((id) => `commands/${id}.json`);
  const categories = Object.fromEntries(CATEGORIES.map((id) => [id, { status: "not_run", artifacts: [], reason: "Outside unsigned encrypted component preparation; no H1 acceptance claim." }]));
  categories.platform_build_and_license_receipt = { status: "partial", artifacts: ["source.json", "licenses.json", ...refs(["probe-build", "store-lint", "portability-lint"])], reason: "Native component build and admitted licence bytes only; installer, signing, size budget, updater and distribution/security review missing." };
  categories.platform_zero_canary = { status: probe ? "observed" : "missing", artifacts: refs(["probe"]), reason: "Only the schema-2 probe artifact scan; no full H1 memory/crash-dump or object-format packaging claim." };
  const faults = refs(["store-tests", "portability-tests", "encrypted-crash"]);
  categories.platform_fault_and_restore = { status: faults.length ? "partial" : "not_run", artifacts: faults, reason: "Named encrypted component faults and fresh-machine synthetic recovery are bounded by each linked command result; full Phase 2 exit and physical power-loss evidence missing." };
  categories.platform_keystore_native = { status: "missing", artifacts: [], reason: "Native broker positive tests not run; macOS backend absent; hardware protection unverified." };
  categories.five_platform_receipt_is_complete = { status: "missing", artifacts: [], reason: "Unsigned evidence is not the signed v1 admission receipt; no acceptance key or signer operation." };
  return categories;
}
export function readProbe(bundle, rows) {
  const row = rows.find((item) => item.id === "probe");
  if (!row || row.status !== "passed") return null;
  const observed = json(join(bundle, row.stdout));
  assert.equal(observed.lane, "sqlcipher-store");
  assert.equal(observed.adr_002_accepted, false);
  assert.equal(observed.production_data_allowed, false);
  assert.equal(observed.schema_version, 2);
  assert.equal(observed.storage_encryption, "SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000");
  assert.equal(observed.cipher_page_size, 4096);
  assert.equal(observed.kdf_iter, 256000);
  assert.equal(observed.cipher_hmac_algorithm, "HMAC_SHA512");
  assert.equal(observed.cipher_kdf_algorithm, "PBKDF2_HMAC_SHA512");
  assert.equal(observed.plaintext_canary_hits, 0);
  assert(observed.files_scanned > 0 && observed.bytes_scanned > 0 && observed.canary_count > 0);
  assert.equal(observed.readable_canary_count, observed.canary_count);
  const admitted = json(join(bundle, "dependency-admission.json")).bundled_sources.sqlcipher_community;
  assert.equal(observed.cipher_version, `${admitted.version} community`);
  assert.equal(observed.sqlite_version, admitted.sqlite_version);
  const artifacts = files(join(bundle, "probe/artifacts"));
  assert.equal(artifacts.length, observed.files_scanned);
  assert.equal(artifacts.reduce((total, path) => total + statSync(join(bundle, "probe/artifacts", path)).size, 0), observed.bytes_scanned);
  const canaries = readFileSync(join(bundle, "canaries.txt"), "utf8").split(/\r?\n/u).filter((line) => line && !line.startsWith("#"));
  assert.equal(canaries.length, observed.canary_count);
  let hits = 0;
  for (const path of artifacts) {
    const bytes = readFileSync(join(bundle, "probe/artifacts", path));
    if (path.endsWith(".sqlite3")) {
      assert(bytes.length >= 16, "database artifact is shorter than its header");
      assert(!bytes.subarray(0, 16).equals(Buffer.from("SQLite format 3\0")), "plaintext SQLite database header in probe artifacts");
    }
    for (const canary of canaries) { let offset = 0; while ((offset = bytes.indexOf(canary, offset)) !== -1) { hits += 1; offset += 1; } }
  }
  assert.equal(hits, observed.plaintext_canary_hits);
  return observed;
}
function nativeLicenses(bundle) {
  const receipt = json("docs/security/dependency-admission-phase1.json");
  const registry = join(process.env.CARGO_HOME || join(homedir(), ".cargo"), "registry/src");
  const output = [];
  for (const [name, crate, relative] of [["sqlcipher_community", "libsqlite3-sys-0.38.2", "sqlcipher/LICENSE"], ["openssl", "openssl-src-300.6.1+3.6.3", "openssl/LICENSE.txt"]]) {
    const candidates = existsSync(registry) ? readdirSync(registry).map((entry) => join(registry, entry, crate, relative)).filter(existsSync) : [];
    if (candidates.length !== 1) { output.push({ name, status: "missing", reason: "Exact fetched native source notice unavailable" }); continue; }
    const path = `licenses/${name}.txt`;
    mkdirSync(join(bundle, "licenses"), { recursive: true });
    copyFileSync(candidates[0], join(bundle, path));
    const observed = digestFile(join(bundle, path));
    assert.equal(observed.sha256, receipt.bundled_sources[name].license_sha256);
    output.push({ name, status: "observed", path, ...observed, version: receipt.bundled_sources[name].version, versionKind: "locked-source-not-runtime-provider" });
  }
  write(join(bundle, "licenses.json"), output);
}
export function validateLicenses(bundle, paths, admission, complete) {
  const notices = json(join(bundle, "licenses.json"));
  assert(Array.isArray(notices), "native notices must be an array");
  const required = ["openssl", "sqlcipher_community"];
  const names = notices.map((notice) => notice.name);
  assert.equal(new Set(names).size, names.length, "duplicate native notice name");
  for (const notice of notices) {
    assert(required.includes(notice.name), "unknown native notice name");
    assert(["observed", "missing"].includes(notice.status), "unknown native notice status");
    if (notice.status === "missing") { assert(!complete && typeof notice.reason === "string" && notice.reason.length > 0, "missing native notice"); continue; }
    assert.equal(notice.version, admission.bundled_sources[notice.name].version, "native notice version differs from admission");
    assert.equal(notice.versionKind, "locked-source-not-runtime-provider", "native notice must not claim a runtime provider version");
    assert.equal(notice.path, `licenses/${notice.name}.txt`);
    assert(paths.includes(notice.path), "notice reference must be an inventoried artifact");
    assert.equal(notice.sha256, admission.bundled_sources[notice.name].license_sha256);
    assert.deepEqual(digestFile(join(bundle, notice.path)), { bytes: notice.bytes, sha256: notice.sha256 });
  }
  if (complete) assert.deepEqual(names.toSorted(), required, "complete component evidence requires both distinct native notices");
  return notices;
}
function retainTestBinaries(bundle, rows, targetArch) {
  const binaries = [];
  for (const row of rows) {
    for (const line of readFileSync(join(bundle, row.stdout), "utf8").split(/\r?\n/u)) {
      if (!line.startsWith("{")) continue;
      let item; try { item = JSON.parse(line); } catch { continue; }
      if (item.reason !== "compiler-artifact" || !item.executable || !["encrypted_profile", "encrypted_backup", "encrypted_crash"].includes(item.target?.name)) continue;
      const path = `binaries/${row.id}-${basename(item.executable)}`;
      if (binaries.some((binary) => binary.path === path)) continue;
      mkdirSync(join(bundle, "binaries"), { recursive: true });
      copyFileSync(item.executable, join(bundle, path));
      const architecture = binaryArchitecture(readFileSync(join(bundle, path)));
      assert.equal(architecture, targetArch);
      binaries.push({ command: row.id, target: item.target.name, executable: item.executable, path, architecture, ...digestFile(join(bundle, path)) });
    }
  }
  write(join(bundle, "binaries.json"), binaries);
}
export function collect(bundle, target) {
  assert(PLATFORMS[target], "unknown platform");
  assert(!existsSync(bundle), "bundle directory must be new");
  mkdirSync(bundle, { recursive: true });
  if (platform() !== "win32") bundle = realpathSync(bundle);
  const commit = git(["rev-parse", "HEAD"]);
  const paths = git(["ls-files", "-z"]).split("\0").filter(Boolean);
  const source = { commit, tree: git(["rev-parse", "HEAD^{tree}"]), dirty: git(["status", "--porcelain", "--untracked-files=no"]), files: paths.map((path) => ({ path, ...digestFile(path) })) };
  write(join(bundle, "source.json"), source);
  copyFileSync("testdata/sqlcipher-canary/store-v2-canaries.txt", join(bundle, "canaries.txt"));
  copyFileSync("docs/security/dependency-admission-phase1.json", join(bundle, "dependency-admission.json"));
  const host = { platform: platform(), arch: arch(), machine: machine(), release: release(), node: process.version, runnerArch: process.env.RUNNER_ARCH ?? null, imageOS: process.env.ImageOS ?? null, imageVersion: process.env.ImageVersion ?? null };
  const context = { repository: process.env.GITHUB_REPOSITORY ?? null, runId: process.env.GITHUB_RUN_ID ?? null, attempt: process.env.GITHUB_RUN_ATTEMPT ?? null, job: process.env.GITHUB_JOB ?? null, sha: process.env.GITHUB_SHA ?? null };
  const rows = [];
  const lane = resolve(process.env.RUNNER_TEMP || process.env.TEMP || dirname(bundle), "h1-lane");
  mkdirSync(join(lane, "temp"), { recursive: true });
  process.env.CARGO_TARGET_DIR = join(lane, "target");
  process.env.TEMP = process.env.TMP = process.env.TMPDIR = realpathSync(join(lane, "temp"));
  process.env.CARGO_BUILD_JOBS = "1"; process.env.CARGO_INCREMENTAL = "0"; process.env.CARGO_TERM_COLOR = "never";
  if (platform() === "win32") process.env.OPENSSL_RUST_USE_NASM = "0";
  let failure = null;
  try {
    assert.equal(source.dirty, "", "tracked source must be clean");
    verifySource(source);
    assert.equal(context.sha, commit, "checked-out commit differs from hosted identity");
    const expected = PLATFORMS[target];
    assert.equal(host.platform, expected[1]); assert.equal(host.arch, expected[2]);
    assert.equal(host.runnerArch, expected[2] === "arm64" ? "ARM64" : "X64");
    assert.equal(process.version, `v${readFileSync(".nvmrc", "utf8").trim()}`);
    if (platform() === "win32" && !process.env.OPENSSL_SRC_PERL) {
      rows.push(run(bundle, "windows-prerequisites", "pwsh", ["-NoProfile", "-File", "tools/h1-prerequisites.ps1"]));
      assert.equal(rows.at(-1).status, "passed", "pinned native Perl preparation failed");
      const pin = json("tools/sqlcipher/windows-toolchain.json");
      process.env.OPENSSL_SRC_PERL = join(process.env.RUNNER_TEMP, "h1-perl", pin.perl_relative_path);
    }
    rows.push(run(bundle, "rustc", "rustc", ["-vV"]));
    assert.equal(rows.at(-1).status, "passed");
    assert(readFileSync(join(bundle, rows.at(-1).stdout), "utf8").includes(`host: ${expected[3]}`));
    assert(readFileSync(join(bundle, rows.at(-1).stdout), "utf8").replaceAll("\r\n", "\n").includes("release: 1.98.0\n"));
    rows.push(run(bundle, "cargo", "cargo", ["--version"]));
    rows.push(run(bundle, "perl", process.env.OPENSSL_SRC_PERL || "perl", ["-V"]));
    if (platform() !== "win32") {
      rows.push(run(bundle, "cc", "cc", ["--version"]));
      rows.push(run(bundle, "make", "make", ["--version"]));
    }
    if (platform() === "win32") rows.push(run(bundle, "windows-toolchain", process.execPath, ["tools/h1-windows-toolchain.mjs"]));
    rows.push(run(bundle, "fetch", "cargo", ["fetch", "--locked"]));
    assert(rows.every((row) => row.status === "passed"), "prerequisite failed");
    nativeLicenses(bundle);
    for (const command of commandPlan()) rows.push(run(bundle, command.id, "cargo", command.args, command.requiredTests));
    if (rows.find((row) => row.id === "probe-build")?.status === "passed") {
      const binary = join(process.env.CARGO_TARGET_DIR, "debug", `sqlcipher_store_probe${platform() === "win32" ? ".exe" : ""}`);
      mkdirSync(join(bundle, "binaries"), { recursive: true }); copyFileSync(binary, join(bundle, "binaries", basename(binary)));
      assert.equal(binaryArchitecture(readFileSync(binary)), expected[2]);
      rows.push(run(bundle, "probe", binary, ["run", join(bundle, "probe")]));
    }
  } catch (error) { failure = error.message; }
  try { retainTestBinaries(bundle, rows, PLATFORMS[target][2]); } catch (error) { failure = error.message; }
  if (!existsSync(join(bundle, "licenses.json"))) write(join(bundle, "licenses.json"), []);
  let observed = null;
  try { observed = readProbe(bundle, rows); } catch (error) { failure = error.message; }
  const expectedIds = [...commandPlan().map((command) => command.id), "probe"];
  const notRun = expectedIds.filter((id) => !rows.some((row) => row.id === id));
  const manifest = { format: "h1-unsigned-evidence", version: 1, acceptedH1: false, productionDataAllowed: false, platform: target, commit, host, context, source: "source.json", commands: rows.map((row) => `commands/${row.id}.json`), notRun, failure, observed, categories: deriveCategories(rows, observed) };
  manifest.status = failure || notRun.length || rows.some((row) => row.status !== "passed") ? "failed" : "component_checks_passed";
  manifest.artifacts = files(bundle).map((path) => ({ path, ...digestFile(join(bundle, path)) }));
  write(join(bundle, "manifest.json"), manifest);
  return manifest;
}

export function validate(bundle, expected) {
  // Inspect the whole tree before opening untrusted metadata or following any
  // manifest reference. Hash validation later is not permission to read outside.
  const discovered = files(bundle);
  const manifest = json(join(bundle, "manifest.json"));
  assert.equal(manifest.source, "source.json", "source reference must be source.json");
  assert.equal(manifest.format, "h1-unsigned-evidence"); assert.equal(manifest.version, 1);
  assert.equal(manifest.acceptedH1, false); assert.equal(manifest.productionDataAllowed, false);
  assert.match(expected.commit, /^[a-f0-9]{40}$/u);
  for (const key of ["commit", "platform"]) assert.equal(manifest[key], expected[key], `${key} mismatch`);
  assert.equal(manifest.context.sha, expected.commit);
  for (const key of ["runId", "attempt", "repository"]) assert.equal(manifest.context[key], expected[key], `${key} mismatch`);
  const host = PLATFORMS[manifest.platform]; assert(host);
  assert.equal(manifest.host.platform, host[1]); assert.equal(manifest.host.arch, host[2]);
  assert.equal(manifest.host.runnerArch, host[2] === "arm64" ? "ARM64" : "X64");
  const paths = manifest.artifacts.map((item) => item.path);
  assert.equal(new Set(paths).size, paths.length);
  assert(paths.length <= 10000);
  let total = 0;
  for (const artifact of manifest.artifacts) {
    assert.match(artifact.path, /^[a-zA-Z0-9_./-]+$/u);
    assert(!artifact.path.split("/").some((part) => !part || part === "." || part === ".."));
    const actual = digestFile(join(bundle, artifact.path));
    assert.deepEqual(actual, { bytes: artifact.bytes, sha256: artifact.sha256 }, `artifact mismatch: ${artifact.path}`);
    total += actual.bytes;
  }
  assert(total <= MAX_TOTAL);
  assert.deepEqual(discovered.toSorted(), [...paths, "manifest.json"].sort(), "unlisted artifact");
  const source = json(join(bundle, manifest.source));
  assert.equal(source.commit, manifest.commit); assert.equal(source.dirty, "");
  assert.equal(new Set(source.files.map((item) => item.path)).size, source.files.length);
  verifySource(source);
  for (const [original, retained] of [["testdata/sqlcipher-canary/store-v2-canaries.txt", "canaries.txt"], ["docs/security/dependency-admission-phase1.json", "dependency-admission.json"]]) {
    const item = source.files.find((file) => file.path === original); assert(item);
    assert.deepEqual(digestFile(join(bundle, retained)), { bytes: item.bytes, sha256: item.sha256 });
  }
  const admission = json(join(bundle, "dependency-admission.json"));
  validateLicenses(bundle, paths, admission, manifest.status === "component_checks_passed");
  const rows = manifest.commands.map((path) => { assert(paths.includes(path)); return json(join(bundle, path)); });
  assert.equal(new Set(rows.map((row) => row.id)).size, rows.length);
  const allowed = new Set(["rustc", "cargo", "perl", "cc", "make", "windows-prerequisites", "windows-toolchain", "fetch", "probe", ...commandPlan().map((command) => command.id)]);
  for (const row of rows) {
    assert(allowed.has(row.id), "unexpected command");
    assert(paths.includes(row.stdout) && paths.includes(row.stderr));
    const plan = commandPlan().find((command) => command.id === row.id);
    if (plan) { assert.equal(row.executable, "cargo"); assert.deepEqual(row.args, plan.args); }
    const output = readFileSync(join(bundle, row.stdout), "utf8");
    assert.equal(row.status, commandStatus(row, output, plan?.requiredTests));
    assert.deepEqual(row.tests, testOutcomes(output));
  }
  for (const binary of json(join(bundle, "binaries.json"))) {
    assert(paths.includes(binary.path));
    assert.deepEqual(digestFile(join(bundle, binary.path)), { bytes: binary.bytes, sha256: binary.sha256 });
    assert.equal(binary.architecture, host[2]);
    assert.equal(binaryArchitecture(readFileSync(join(bundle, binary.path))), host[2]);
    const command = rows.find((row) => row.id === binary.command); assert(command);
    const compilerArtifacts = readFileSync(join(bundle, command.stdout), "utf8").split(/\r?\n/u).flatMap((line) => { try { return [JSON.parse(line)]; } catch { return []; } });
    assert(compilerArtifacts.some((item) => item.reason === "compiler-artifact" && item.executable === binary.executable && item.target.name === binary.target), "binary missing from compiler output");
  }
  const notRun = [...commandPlan().map((command) => command.id), "probe"].filter((id) => !rows.some((row) => row.id === id));
  assert.deepEqual(manifest.notRun, notRun);
  const observed = readProbe(bundle, rows);
  assert.deepEqual(manifest.observed, observed);
  assert.deepEqual(manifest.categories, deriveCategories(rows, observed));
  const status = manifest.failure || notRun.length || rows.some((row) => row.status !== "passed") ? "failed" : "component_checks_passed";
  assert.equal(manifest.status, status);
  if (status === "component_checks_passed") {
    for (const id of ["rustc", "cargo", "perl", "fetch", ...(host[1] === "win32" ? ["windows-toolchain"] : ["cc", "make"])]) assert(rows.some((row) => row.id === id), `missing prerequisite ${id}`);
    const rust = rows.find((row) => row.id === "rustc");
    assert.deepEqual(rust.args, ["-vV"]); assert.equal(rust.executable, "rustc");
    const text = readFileSync(join(bundle, rust.stdout), "utf8").replaceAll("\r\n", "\n");
    assert(text.includes(`host: ${host[3]}\n`) && text.includes("release: 1.98.0\n"));
    const probe = rows.find((row) => row.id === "probe");
    assert.equal(probe.args.length, 2); assert.equal(probe.args[0], "run");
    const binary = `binaries/sqlcipher_store_probe${host[1] === "win32" ? ".exe" : ""}`;
    assert(paths.includes(binary));
    assert.equal(binaryArchitecture(readFileSync(join(bundle, binary))), host[2]);
    const binaries = json(join(bundle, "binaries.json"));
    for (const [command, target] of [["store-tests", "encrypted_profile"], ["portability-tests", "encrypted_backup"], ["encrypted-crash", "encrypted_crash"]]) assert(binaries.some((binary) => binary.command === command && binary.target === target), "required test binary missing");
  }
  return { integrity: "verified", status, acceptedH1: false, platform: manifest.platform, commit: manifest.commit };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const [verb, directory, target, commit, runId, attempt, repository] = process.argv.slice(2);
  if (verb === "collect") { const manifest = collect(resolve(directory), target); console.log(JSON.stringify({ status: manifest.status, acceptedH1: false })); if (manifest.status !== "component_checks_passed") process.exitCode = 1; }
  else if (verb === "validate") console.log(JSON.stringify(validate(resolve(directory), { platform: target, commit, runId, attempt, repository })));
  else throw new Error("Usage: h1-evidence.mjs collect DIR PLATFORM | validate DIR PLATFORM COMMIT RUN_ID ATTEMPT REPOSITORY");
}
