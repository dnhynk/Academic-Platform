// The existing verifier fixes a user-machine D: install root. Hosted Windows
// ARM uses a C: workspace; retain every byte/identity pin in a job-local root.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";

assert.equal(process.platform, "win32");
assert(process.env.RUNNER_TEMP);
const pin = JSON.parse(readFileSync("tools/sqlcipher/windows-toolchain.json", "utf8"));
const interpreter = join(process.env.RUNNER_TEMP, "h1-perl", pin.perl_relative_path);
assert.equal(resolve(process.env.OPENSSL_SRC_PERL).toLowerCase(), resolve(interpreter).toLowerCase());
const archive = join(process.env.RUNNER_TEMP, "h1-perl-download", pin.archive.name);
assert.equal(statSync(archive).size, pin.archive.size_bytes);
assert.equal(createHash("sha256").update(readFileSync(archive)).digest("hex"), pin.archive.sha256);
const identity = spawnSync(interpreter, ["-e", "print $^V, qq(\\n), $Config::Config{archname}, qq(\\n)", "-MConfig"], { encoding: "utf8", windowsHide: true });
assert.equal(identity.status, 0, identity.stderr);
assert.deepEqual(identity.stdout.trim().split(/\r?\n/u), [pin.perl_version_string, pin.perl_archname]);
const modules = spawnSync(interpreter, ["-e", "use Locale::Maketext::Simple; use Params::Check; use IPC::Cmd; use Pod::Usage; print qq(ok\\n)"], { encoding: "utf8", windowsHide: true });
assert.equal(modules.status, 0, modules.stderr);
console.log(JSON.stringify({ component: pin.component, version: pin.version, archive: pin.archive, interpreter, identity: identity.stdout.trim().split(/\r?\n/u), modules: "verified", pathAdded: false, productArchitectureClaim: false }));
