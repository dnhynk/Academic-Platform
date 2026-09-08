// The existing verifier fixes a user-machine D: install root. Hosted Windows
// ARM uses a C: workspace; retain every byte/identity pin in a job-local root.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { delimiter, isAbsolute, join, relative, resolve } from "node:path";
import { measuredBytes, validateWindowsObservation } from "./h1-correspondence.mjs";
import { PERL_IDENTITY_ARGS, PERL_MODULE_ARGS } from "./h1-plan.mjs";

assert.equal(process.platform, "win32");
assert(process.env.RUNNER_TEMP);
const pin = JSON.parse(readFileSync("tools/sqlcipher/windows-toolchain.json", "utf8"));
const interpreter = join(process.env.RUNNER_TEMP, "h1-perl", pin.perl_relative_path);
assert.equal(resolve(process.env.OPENSSL_SRC_PERL).toLowerCase(), resolve(interpreter).toLowerCase());
const archive = join(process.env.RUNNER_TEMP, "h1-perl-download", pin.archive.name);
const archiveObservation = { path: archive, ...measuredBytes(archive) };
assert.equal(archiveObservation.bytes, pin.archive.size_bytes);
assert.equal(archiveObservation.sha256, pin.archive.sha256);
const invoke = (args) => {
  const result = spawnSync(interpreter, args, { encoding: "utf8", windowsHide: true });
  return { executable: interpreter, args, exitCode: result.status, signal: result.signal, error: result.error?.message ?? null, stdout: result.stdout, stderr: result.stderr };
};
const installRoot = join(process.env.RUNNER_TEMP, "h1-perl");
// Retain only entries inside this admitted tool's root, never the ambient PATH.
const entriesWithinInstallRoot = (process.env.PATH || "").split(delimiter).filter((entry) => {
  const suffix = relative(installRoot, resolve(entry.replace(/^"|"$/gu, "")));
  return !suffix || (!suffix.startsWith("..") && !isAbsolute(suffix));
});
const observation = {
  component: pin.component, version: pin.version, archive: pin.archive,
  archiveObservation,
  interpreter, interpreterObservation: measuredBytes(interpreter),
  identity: invoke(PERL_IDENTITY_ARGS), modules: invoke(PERL_MODULE_ARGS),
  pathPolicy: { installRoot, entriesWithinInstallRoot, pathAdded: false, referencedOnlyBy: "OPENSSL_SRC_PERL" },
  opensslRustUseNasm: process.env.OPENSSL_RUST_USE_NASM ?? null, productArchitectureClaim: false,
};
// Failed observations remain raw stdout plus a nonzero command result.
console.log(JSON.stringify(observation));
validateWindowsObservation(observation, { runnerTemp: process.env.RUNNER_TEMP, opensslSrcPerl: process.env.OPENSSL_SRC_PERL, opensslRustUseNasm: process.env.OPENSSL_RUST_USE_NASM }, pin, "windows-x86_64");
