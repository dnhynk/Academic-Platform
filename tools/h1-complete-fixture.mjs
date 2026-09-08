// Tiny complete validator fixture, never a native run or authenticated archive.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { commandPlan, evidencePath, PERL_IDENTITY_ARGS, PERL_MODULE_ARGS, PLATFORMS, prerequisitePlan } from "./h1-plan.mjs";
import { deriveCategories } from "./h1-evidence.mjs";

export const digest = (bytes) => ({ bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") });
export const writeJson = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + "\n");
export const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
export function rehash(bundle) {
  const manifest = readJson(join(bundle, "manifest.json"));
  const inventory = (prefix = "") => readdirSync(join(bundle, prefix), { withFileTypes: true }).flatMap((item) => {
    const path = prefix ? `${prefix}/${item.name}` : item.name;
    return item.isDirectory() ? inventory(path) : path === "manifest.json" ? [] : [{ path, ...digest(readFileSync(join(bundle, path))) }];
  });
  manifest.artifacts = inventory(); writeJson(join(bundle, "manifest.json"), manifest);
}

export function completeFixture(root, pin, platform = "windows-x86_64") {
  const host = PLATFORMS[platform], pathApi = evidencePath(platform), windowsHost = host[1] === "win32", extension = windowsHost ? ".exe" : "";
  const sourceRoot = join(root, "source"), bundle = join(root, "complete");
  const write = (base, path, value) => { mkdirSync(dirname(join(base, path)), { recursive: true }); writeFileSync(join(base, path), value); };
  const source = (path, value) => write(sourceRoot, path, value);
  const bundled = (path, value) => write(bundle, path, value);
  source(".nvmrc", "24.19.0\n");
  source("tools/sqlcipher/windows-toolchain.json", JSON.stringify(pin));
  source("testdata/sqlcipher-canary/store-v2-canaries.txt", "synthetic-canary\n");
  const admission = { bundled_sources: {} }, licenses = [];
  for (const name of ["openssl", "sqlcipher_community"]) {
    const bytes = Buffer.from(`synthetic ${name} notice`), path = `licenses/${name}.txt`;
    bundled(path, bytes);
    admission.bundled_sources[name] = { version: name === "openssl" ? "3.6.3" : "4.14.0", sqlite_version: "3.51.3", license_sha256: digest(bytes).sha256 };
    licenses.push({ name, status: "observed", path, ...digest(bytes), version: admission.bundled_sources[name].version, versionKind: "locked-source-not-runtime-provider" });
  }
  source("docs/security/dependency-admission-phase1.json", JSON.stringify(admission));
  bundled("dependency-admission.json", JSON.stringify(admission)); bundled("licenses.json", JSON.stringify(licenses));
  bundled("canaries.txt", "synthetic-canary\n"); bundled("probe/artifacts/synthetic.sqlite3", Buffer.alloc(32, 42));
  const execution = {
    cwd: "C:\\fixture\\checkout", bundle: "C:\\fixture\\bundle", runnerTemp: "C:\\fixture\\temp",
    targetDir: "C:\\fixture\\temp\\h1-lane\\target", tempDir: "C:\\fixture\\temp\\h1-lane\\temp",
    nodeExecutable: "C:\\fixture\\node.exe", opensslSrcPerl: "C:\\fixture\\temp\\h1-perl\\perl\\bin\\perl.exe", opensslRustUseNasm: "0",
  };
  if (!windowsHost) Object.assign(execution, { cwd: "/fixture/checkout", bundle: "/fixture/bundle", runnerTemp: "/fixture/temp", targetDir: "/fixture/temp/h1-lane/target", tempDir: "/fixture/temp/h1-lane/temp", nodeExecutable: "/fixture/node", opensslSrcPerl: null, opensslRustUseNasm: null });
  const interpreter = execution.opensslSrcPerl;
  const perlRecord = (args, stdout) => ({ executable: interpreter, args, stdout, stderr: "", exitCode: 0, signal: null, error: null });
  const windows = {
    component: pin.component, version: pin.version, archive: pin.archive,
    archiveObservation: { path: pathApi.join(execution.runnerTemp, "h1-perl-download", pin.archive.name), bytes: pin.archive.size_bytes, sha256: pin.archive.sha256 },
    interpreter, interpreterObservation: digest(Buffer.from("synthetic interpreter observation")),
    identity: perlRecord(PERL_IDENTITY_ARGS, `${pin.perl_version_string}\n${pin.perl_archname}\n`), modules: perlRecord(PERL_MODULE_ARGS, "ok\n"),
    pathPolicy: { installRoot: pathApi.join(execution.runnerTemp, "h1-perl"), entriesWithinInstallRoot: [], pathAdded: false, referencedOnlyBy: "OPENSSL_SRC_PERL" },
    opensslRustUseNasm: "0", productArchitectureClaim: false,
  };
  const observed = {
    lane: "sqlcipher-store", adr_002_accepted: false, production_data_allowed: false,
    schema_version: 2, storage_encryption: "SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000",
    cipher_page_size: 4096, kdf_iter: 256000, cipher_hmac_algorithm: "HMAC_SHA512", cipher_kdf_algorithm: "PBKDF2_HMAC_SHA512",
    plaintext_canary_hits: 0, files_scanned: 1, bytes_scanned: 32, canary_count: 1, readable_canary_count: 1,
    cipher_version: "4.14.0 community", sqlite_version: "3.51.3",
  };
  const rows = [], binaries = [];
  const add = (plan, output, stderr = "", extra = {}) => {
    const row = { ...plan, cwd: execution.cwd, started: "2026-09-08T00:00:00.000Z", finished: "2026-09-08T00:00:01.000Z", exitCode: 0, signal: null, error: null, stdout: `logs/${plan.id}.stdout.log`, stderr: `logs/${plan.id}.stderr.log`, status: "passed", tests: (plan.requiredTests || []).map((name) => ({ name, status: "ok" })), ...extra };
    delete row.requiredTests;
    rows.push(row); bundled(row.stdout, output); bundled(row.stderr, stderr); bundled(`commands/${row.id}.json`, JSON.stringify(row));
  };
  for (const plan of prerequisitePlan(platform, execution, pin)) {
    const outputs = { node: "v24.19.0\n", rustc: `rustc 1.98.0\nhost: ${host[3]}\nrelease: 1.98.0\n`, cargo: "cargo 1.98.0\n", perl: `Summary of my perl5 (revision 5 version 42 subversion 2) configuration:\n  osname=${windowsHost ? "MSWin32" : host[1]}\n  archname=${windowsHost ? pin.perl_archname : "synthetic-helper"}\n`, cc: "cc (Synthetic fixture) 1.0.0\n", make: "GNU Make 1.0\n", fetch: "" };
    add(plan, plan.id.startsWith("windows-") ? JSON.stringify(windows) : outputs[plan.id]);
  }
  for (const plan of commandPlan()) {
    const names = { "store-tests": ["encrypted_profile"], "probe-build": ["sqlcipher_store_probe"], "portability-tests": ["encrypted_backup", "encrypted_crash"], "encrypted-crash": ["encrypted_crash"] }[plan.id] || [];
    let output = "", stderr = "";
    for (const [index, name] of names.entries()) {
      const probe = plan.id === "probe-build", store = plan.id === "store-tests" || probe;
      const relative = `crates/${store ? "store" : "portability"}/${probe ? "src/bin" : "tests"}/${name}.rs`;
      source(relative, "// Synthetic compiler correspondence fixture only.\n");
      const executable = pathApi.join(execution.targetDir, "debug", probe ? `${name}${extension}` : `deps/${name}-${plan.id}${extension}`);
      const features = store ? ["sqlcipher-store"] : plan.id === "encrypted-crash" ? ["encrypted-portability", "phase2-fault-injection"] : ["encrypted-portability"];
      const compiler = { reason: "compiler-artifact", executable, features, target: { name, kind: [probe ? "bin" : "test"], crate_types: ["bin"], src_path: pathApi.join(execution.cwd, relative) }, profile: { test: !probe } };
      output += `${JSON.stringify(compiler)}\n`;
      if (!probe) stderr += `     Running ${pathApi.join("tests", `${name}.rs`)} (${executable})\n`;
      const bytes = Buffer.alloc(160);
      if (windowsHost) { bytes.write("MZ"); bytes.writeUInt32LE(64, 0x3c); bytes.write("PE\0\0", 64); bytes.writeUInt16LE(host[2] === "arm64" ? 0xaa64 : 0x8664, 68); }
      else if (host[1] === "linux") { bytes.set([127, 69, 76, 70, 2, 1]); bytes.writeUInt16LE(host[2] === "arm64" ? 183 : 62, 18); }
      else { bytes.writeUInt32LE(0xfeedfacf); bytes.writeUInt32LE(0x0100000c, 4); }
      bytes.write(plan.id + name, 80);
      const path = `binaries/${plan.id}-${index}-${pathApi.basename(executable)}`; bundled(path, bytes);
      binaries.push({ command: plan.id, target: name, executable, features, targetKind: compiler.target.kind, source: relative, testProfile: !probe, path, architecture: host[2], ...digest(bytes) });
    }
    output += (plan.requiredTests || []).map((name) => `test ${name} ... ok\n`).join("");
    add({ ...plan, executable: "cargo" }, output, stderr);
  }
  const probe = binaries.find((binary) => binary.command === "probe-build");
  const measurement = { bytes: probe.bytes, sha256: probe.sha256 };
  add({ id: "probe", executable: probe.executable, args: ["run", pathApi.join(execution.bundle, "probe")] }, JSON.stringify(observed), "", { executableObservation: { before: measurement, after: measurement } });
  bundled("binaries.json", JSON.stringify(binaries));
  const git = (args) => { const result = spawnSync("git", args, { cwd: sourceRoot, encoding: "utf8" }); assert.equal(result.status, 0, result.stderr); return result.stdout.trim(); };
  git(["init", "--quiet"]); git(["add", "."]); git(["-c", "user.name=H1 Synthetic Fixture", "-c", "user.email=h1@example.invalid", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "synthetic validator source"]);
  const commit = git(["rev-parse", "HEAD"]);
  bundled("source.json", JSON.stringify({ commit, tree: git(["rev-parse", "HEAD^{tree}"]), dirty: "", files: git(["ls-files", "-z"]).split("\0").filter(Boolean).map((path) => ({ path, ...digest(readFileSync(join(sourceRoot, path))) })) }));
  const manifest = {
    format: "h1-unsigned-evidence", version: 2, acceptedH1: false, productionDataAllowed: false,
    platform, commit, host: { platform: host[1], arch: host[2], node: "v24.19.0", runnerArch: host[2] === "arm64" ? "ARM64" : "X64" },
    context: { sha: commit, runId: "123", attempt: "1", repository: "synthetic/fixture" }, execution,
    source: "source.json", commands: rows.map((row) => `commands/${row.id}.json`), notRun: [], failure: null,
    observed, categories: deriveCategories(rows, observed), status: "component_checks_passed", artifacts: [],
  };
  bundled("manifest.json", JSON.stringify(manifest)); rehash(bundle);
  return { sourceRoot, bundle, expected: { platform: manifest.platform, commit, ...Object.fromEntries(["runId", "attempt", "repository"].map((key) => [key, manifest.context[key]])) } };
}
