#!/usr/bin/env node
// Asserts that the native Windows encrypted-lane build toolchain is exactly the
// pinned one.
//
// The check is conditional on the lane being in use, not on the host: if
// `OPENSSL_SRC_PERL` is unset this host is not building the encrypted lane
// natively and there is nothing to pin, so the script reports that and exits 0.
// The moment the variable is set, every pinned fact is enforced and any
// mismatch is a failure. That keeps a drifted Perl from silently producing
// evidence attributed to the pinned one.

import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pin = JSON.parse(readFileSync(join(here, "windows-toolchain.json"), "utf8"));

function fail(message) {
  process.stderr.write(`windows toolchain: ${message}\n`);
  process.exit(1);
}

const configured = process.env.OPENSSL_SRC_PERL;
if (!configured) {
  process.stdout.write(
    "windows toolchain: OPENSSL_SRC_PERL is unset, so this host is not building " +
      "the encrypted lane natively; nothing to verify.\n",
  );
  process.exit(0);
}

if (!existsSync(configured)) {
  fail(`OPENSSL_SRC_PERL points at ${configured}, which does not exist`);
}

const expectedPerl = resolve(join(pin.install_root, pin.perl_relative_path));
if (resolve(configured).toLowerCase() !== expectedPerl.toLowerCase()) {
  fail(
    `OPENSSL_SRC_PERL is ${resolve(configured)}, but the pinned interpreter is ` +
      `${expectedPerl}. Unpin deliberately or point at the pinned install.`,
  );
}

// The interpreter answers for its own identity. A same-named directory holding a
// different build is exactly what this catches.
const identity = spawnSync(
  configured,
  ["-e", "print $^V, qq(\\n), $Config::Config{archname}, qq(\\n)", "-MConfig"],
  { encoding: "utf8" },
);
if (identity.status !== 0) {
  fail(`the pinned interpreter did not run: ${identity.stderr || identity.error}`);
}
const [reportedVersion, reportedArch] = identity.stdout.trim().split(/\r?\n/u);
if (reportedVersion !== pin.perl_version_string) {
  fail(
    `pinned Perl is ${pin.perl_version_string} but ${configured} reports ` +
      `${reportedVersion}`,
  );
}
if (reportedArch !== pin.perl_archname) {
  fail(
    `pinned Perl is ${pin.perl_archname} but ${configured} reports ${reportedArch}. ` +
      `A Cygwin or MinGW Perl cannot configure OpenSSL for VC-WIN64A.`,
  );
}

// The modules whose absence stopped the E1 spike. Naming them keeps a future
// trimmed distribution from reintroducing that failure silently.
const required = ["Locale::Maketext::Simple", "Params::Check", "IPC::Cmd", "Pod::Usage"];
const modules = spawnSync(
  configured,
  ["-e", `use ${required.join("; use ")}; print qq(ok\\n)`],
  { encoding: "utf8" },
);
if (modules.status !== 0) {
  fail(
    `the pinned interpreter is missing a module OpenSSL's Configure needs: ` +
      `${modules.stderr.trim()}`,
  );
}

// The archive, when it is still on the host, must be the bytes that were
// verified before extraction.
const archive = join(pin.install_root, "..", "_download", pin.archive.name);
if (existsSync(archive)) {
  const size = statSync(archive).size;
  if (size !== pin.archive.size_bytes) {
    fail(`archive is ${size} bytes; pinned size is ${pin.archive.size_bytes}`);
  }
  const digest = createHash("sha256").update(readFileSync(archive)).digest("hex");
  if (digest !== pin.archive.sha256) {
    fail(`archive sha256 is ${digest}; pinned digest is ${pin.archive.sha256}`);
  }
}

// The lane must stay reachable only through OPENSSL_SRC_PERL. A Perl on PATH
// would make the build depend on shell state instead of on the pin, which is
// the ad-hoc change t068 section 8.2 rejects.
const onPath = spawnSync(process.platform === "win32" ? "where" : "which", ["perl"], {
  encoding: "utf8",
});
if (onPath.status === 0 && onPath.stdout.trim().length > 0) {
  const found = onPath.stdout.trim().split(/\r?\n/u);
  process.stdout.write(
    `windows toolchain: note, a Perl is also on PATH (${found.join(", ")}). ` +
      `The build does not use it: openssl-src reads OPENSSL_SRC_PERL first.\n`,
  );
}

process.stdout.write(
  `windows toolchain: pinned Strawberry Perl ${pin.version} ` +
    `(${reportedVersion}, ${reportedArch}) verified at ${configured}.\n`,
);
