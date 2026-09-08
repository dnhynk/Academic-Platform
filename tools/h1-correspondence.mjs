// Internal correspondence only. Unsigned records need external archive provenance.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { evidencePath, PERL_IDENTITY_ARGS, PERL_MODULE_ARGS, PLATFORMS } from "./h1-plan.mjs";

export function measuredBytes(path) {
  const bytes = readFileSync(path);
  return { bytes: bytes.length, sha256: createHash("sha256").update(bytes).digest("hex") };
}

const selectedTests = ["encrypted_profile", "encrypted_backup", "encrypted_crash"];
const eligibleTargets = { "store-tests": ["encrypted_profile"], "probe-build": ["sqlcipher_store_probe"], "portability-tests": ["encrypted_backup", "encrypted_crash"], "encrypted-crash": ["encrypted_crash"] };
export function compilerInventory(row, output, execution, platform, stderr) {
  const path = evidencePath(platform);
  const records = [];
  for (const line of output.split(/\r?\n/u)) {
    let item; try { item = JSON.parse(line); } catch { continue; }
    if (item?.reason !== "compiler-artifact" || !item.executable) continue;
    const probe = row.id === "probe-build" && item.target?.name === "sqlcipher_store_probe";
    if (!probe && !selectedTests.includes(item.target?.name)) continue;
    const store = row.id === "store-tests" || row.id === "probe-build";
    assert(eligibleTargets[row.id]?.includes(item.target.name), "unexpected eligible compiler target");
    const features = store ? ["sqlcipher-store"] : row.id === "encrypted-crash" ? ["encrypted-portability", "phase2-fault-injection"] : ["encrypted-portability"];
    assert.deepEqual(item.features?.toSorted(), features, "compiler features differ from command plan");
    assert.deepEqual(item.target.kind, [probe ? "bin" : "test"], "compiler target kind mismatch");
    assert.deepEqual(item.target.crate_types, ["bin"]);
    assert.equal(item.profile?.test, !probe, "compiler test profile mismatch");
    const source = `crates/${store ? "store" : "portability"}/${probe ? "src/bin" : "tests"}/${item.target.name}.rs`;
    assert.equal(item.target.src_path, path.join(execution.cwd, source), "compiler target source mismatch");
    assert(path.isAbsolute(item.executable), "compiler executable must be absolute");
    const relative = path.relative(execution.targetDir, item.executable);
    assert(relative && !relative.startsWith("..") && !path.isAbsolute(relative), "compiler executable outside target directory");
    if (probe) assert.equal(item.executable, path.join(execution.targetDir, "debug", `sqlcipher_store_probe${PLATFORMS[platform][1] === "win32" ? ".exe" : ""}`), "probe compiler executable mismatch");
    records.push({ command: row.id, target: item.target.name, executable: item.executable, features: item.features, targetKind: item.target.kind, source, testProfile: item.profile.test });
  }
  const identities = records.map(binaryIdentity);
  assert.equal(new Set(identities).size, identities.length, "duplicate compiler invocation/target/executable");
  const executablePaths = records.map((record) => PLATFORMS[platform][1] === "win32" ? path.normalize(record.executable).toLowerCase() : path.normalize(record.executable));
  assert.equal(new Set(executablePaths).size, executablePaths.length, "duplicate executable within one command");
  if (row.status === "passed" && eligibleTargets[row.id]) {
    assert.deepEqual([...new Set(records.map((record) => record.target))].sort(), eligibleTargets[row.id].toSorted(), "complete eligible target set missing from compiler output");
    if (row.id !== "probe-build") {
      const executed = [...stderr.matchAll(/^\s*Running tests[/\\](encrypted_profile|encrypted_backup|encrypted_crash)\.rs \(([^\r\n]+)\)\r?$/gmu)].map((match) => ({ target: match[1], executable: match[2] }));
      const identity = (record) => JSON.stringify([record.target, record.executable]);
      assert.deepEqual(executed.map(identity).sort(), records.map(identity).sort(), "eligible compiler and Cargo execution inventories differ");
    }
  }
  return records;
}
export const binaryIdentity = (record) => JSON.stringify([record.command, record.target, record.executable]);

export function retainBinaries(bundle, row, records, architectureOf, platform) {
  return records.map((record, index) => {
    const path = `binaries/${row.id}-${index}-${evidencePath(platform).basename(record.executable)}`;
    mkdirSync(join(bundle, "binaries"), { recursive: true });
    const measured = measuredBytes(record.executable);
    copyFileSync(record.executable, join(bundle, path));
    assert.deepEqual(measuredBytes(join(bundle, path)), measured, "retained executable differs from measured compiler executable");
    const architecture = architectureOf(readFileSync(join(bundle, path)), platform);
    assert.equal(architecture, PLATFORMS[platform][2]);
    return { ...record, path, architecture, ...measured };
  });
}

export function validateWindowsObservation(observation, execution, pin, platform) {
  const path = evidencePath(platform);
  const installRoot = path.join(execution.runnerTemp, "h1-perl");
  const interpreter = path.join(installRoot, pin.perl_relative_path);
  assert.equal(pin.referenced_only_by, "OPENSSL_SRC_PERL");
  assert.equal(pin.path_change, false); assert.equal(pin.openssl_asm, false);
  assert.equal(observation.component, pin.component); assert.equal(observation.version, pin.version);
  assert.deepEqual(observation.archive, pin.archive, "Windows archive admission mismatch");
  assert.deepEqual(observation.archiveObservation, {
    path: path.join(execution.runnerTemp, "h1-perl-download", pin.archive.name),
    bytes: pin.archive.size_bytes, sha256: pin.archive.sha256,
  }, "Windows measured archive mismatch");
  assert.equal(observation.interpreter, interpreter, "Windows interpreter mismatch");
  assert.equal(execution.opensslSrcPerl, interpreter);
  assert(Number.isSafeInteger(observation.interpreterObservation?.bytes) && observation.interpreterObservation.bytes > 0);
  assert.match(observation.interpreterObservation.sha256, /^[a-f0-9]{64}$/u);
  for (const [name, args, expected] of [["identity", PERL_IDENTITY_ARGS, [pin.perl_version_string, pin.perl_archname]], ["modules", PERL_MODULE_ARGS, ["ok"]]]) {
    const record = observation[name];
    assert.equal(record?.executable, interpreter, `Windows ${name} executable mismatch`);
    assert.deepEqual(record.args, args, `Windows ${name} command mismatch`);
    assert.equal(record.exitCode, 0); assert.equal(record.signal, null); assert.equal(record.error, null);
    assert.equal(record.stderr, "");
    assert.deepEqual(record.stdout.trim().split(/\r?\n/u), expected, `Windows ${name} observation mismatch`);
  }
  assert.deepEqual(observation.pathPolicy, { installRoot, entriesWithinInstallRoot: [], pathAdded: false, referencedOnlyBy: pin.referenced_only_by }, "Windows no-PATH observation mismatch");
  assert.equal(execution.opensslRustUseNasm, "0", "Windows no-assembly environment mismatch");
  assert.equal(observation.opensslRustUseNasm, "0");
  assert.equal(observation.productArchitectureClaim, false);
}
