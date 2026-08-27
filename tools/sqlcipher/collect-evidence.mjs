#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, extname, isAbsolute, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const EXPECTED = Object.freeze({
  node: "24.19.0",
  rustc: "1.98.0",
  cargo: "1.98.0",
  pnpm: "11.22.0",
});
const repository = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

function fail(message) {
  process.stderr.write(`sqlcipher evidence: ${message}\n`);
  process.exitCode = 1;
  throw new Error(message);
}

function parseArguments(arguments_) {
  const parsed = {};
  for (let index = 0; index < arguments_.length; index += 2) {
    const option = arguments_[index];
    const value = arguments_[index + 1];
    if (!value || !["--artifact-root", "--receipt"].includes(option)) {
      fail(
        "usage: collect-evidence.mjs --artifact-root <new-dir> --receipt <new-json>",
      );
    }
    parsed[option.slice(2)] = value;
  }
  if (!parsed["artifact-root"] || !parsed.receipt) {
    fail("both --artifact-root and --receipt are required");
  }
  return parsed;
}

function isWithin(parent, candidate) {
  const pathFromParent = relative(parent, candidate);
  return pathFromParent === "" || (!pathFromParent.startsWith("..") && !isAbsolute(pathFromParent));
}

function requireFreshExternalPath(label, value) {
  const path = resolve(value);
  if (isWithin(repository, path)) {
    fail(`${label} must be outside the repository: ${path}`);
  }
  if (existsSync(path)) {
    fail(`${label} must not already exist: ${path}`);
  }
  return path;
}

function commandText(command, arguments_) {
  return [command, ...arguments_]
    .map((part) => (/^[A-Za-z0-9_./:=+-]+$/u.test(part) ? part : JSON.stringify(part)))
    .join(" ");
}

function run(command, arguments_, { capture = false } = {}) {
  const display = commandText(command, arguments_);
  process.stderr.write(`+ ${display}\n`);
  const result = spawnSync(command, arguments_, {
    cwd: repository,
    encoding: "utf8",
    env: { ...process.env, CARGO_NET_OFFLINE: "true" },
    maxBuffer: 64 * 1024 * 1024,
    stdio: capture ? "pipe" : "inherit",
  });
  if (result.error) {
    fail(`${display}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    if (capture) {
      process.stderr.write(result.stdout ?? "");
      process.stderr.write(result.stderr ?? "");
    }
    fail(`${display} exited ${result.status}`);
  }
  return capture ? (result.stdout ?? "").trim() : "";
}

function exactVersion(command, expected, pattern) {
  const output = run(command, ["--version"], { capture: true });
  const match = pattern.exec(output);
  if (!match || match[1] !== expected) {
    fail(`${command} must be exactly ${expected}; got ${JSON.stringify(output)}`);
  }
  return output;
}

function parseJsonOutput(label, output) {
  const line = output.split(/\r?\n/u).findLast((candidate) => candidate.trim().startsWith("{"));
  if (!line) {
    fail(`${label} did not emit a JSON receipt`);
  }
  try {
    return JSON.parse(line);
  } catch (error) {
    fail(`${label} emitted invalid JSON: ${error.message}`);
  }
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

function localizeGitPath(path) {
  const windowsAbsolute = /^([A-Za-z]):[\\/](.*)$/u.exec(path);
  if (process.platform !== "win32" && windowsAbsolute) {
    return `/mnt/${windowsAbsolute[1].toLowerCase()}/${windowsAbsolute[2].replaceAll("\\", "/")}`;
  }
  return path;
}

function repositoryHead() {
  const dotGit = resolve(repository, ".git");
  const gitDirectory = statSync(dotGit).isDirectory()
    ? dotGit
    : resolve(
        repository,
        localizeGitPath(
          readFileSync(dotGit, "utf8").trim().slice("gitdir:".length).trim(),
        ),
      );
  const head = readFileSync(resolve(gitDirectory, "HEAD"), "utf8").trim();
  if (!head.startsWith("ref: ")) {
    return head;
  }
  const reference = head.slice("ref: ".length);
  const commonDirectoryFile = resolve(gitDirectory, "commondir");
  const commonDirectory = existsSync(commonDirectoryFile)
    ? resolve(gitDirectory, readFileSync(commonDirectoryFile, "utf8").trim())
    : gitDirectory;
  for (const root of [gitDirectory, commonDirectory]) {
    const looseReference = resolve(root, reference);
    if (existsSync(looseReference)) {
      return readFileSync(looseReference, "utf8").trim();
    }
  }
  const packedReferences = resolve(commonDirectory, "packed-refs");
  if (existsSync(packedReferences)) {
    const suffix = ` ${reference}`;
    const match = readFileSync(packedReferences, "utf8")
      .split(/\r?\n/u)
      .find((line) => line.endsWith(suffix));
    if (match) {
      return match.slice(0, -suffix.length);
    }
  }
  fail(`cannot resolve Git HEAD reference ${reference}`);
}

function filesBelow(root, current = root) {
  const output = [];
  for (const entry of readdirSync(current, { withFileTypes: true })) {
    const path = resolve(current, entry.name);
    if (entry.isDirectory()) {
      output.push(...filesBelow(root, path));
    } else if (entry.isFile()) {
      output.push({
        path: relative(root, path).replaceAll("\\", "/"),
        bytes: statSync(path).size,
        sha256: sha256(path),
      });
    } else {
      fail(`unexpected non-file artifact: ${path}`);
    }
  }
  return output.sort((left, right) => left.path.localeCompare(right.path));
}

function binaryPath() {
  const targetRoot = resolve(process.env.CARGO_TARGET_DIR ?? resolve(repository, "target"));
  const suffix = process.platform === "win32" ? ".exe" : "";
  return resolve(targetRoot, "debug", `sqlcipher_spike${suffix}`);
}

function requiredSqlcipherSymbols(executable) {
  const required = ["sqlcipher_version", "sqlite3_key", "sqlite3_key_v2", "sqlite3_rekey_v2"];
  const output = run("nm", ["-g", "--defined-only", executable], { capture: true });
  const exported = new Set(
    output
      .split(/\r?\n/u)
      .map((line) => line.trim().split(/\s+/u).at(-1))
      .filter(Boolean),
  );
  const missing = required.filter((symbol) => !exported.has(symbol));
  if (missing.length > 0) {
    fail(`evidence binary is missing SQLCipher symbols: ${missing.join(", ")}`);
  }
  return required;
}

function main() {
  const parsed = parseArguments(process.argv.slice(2));
  const artifactRoot = requireFreshExternalPath("artifact root", parsed["artifact-root"]);
  const receiptPath = requireFreshExternalPath("receipt", parsed.receipt);
  if (extname(receiptPath).toLowerCase() !== ".json") {
    fail("receipt path must end in .json");
  }

  if (process.versions.node !== EXPECTED.node) {
    fail(`node must be exactly ${EXPECTED.node}; got ${process.versions.node}`);
  }
  const toolchain = {
    node: process.version,
    rustc: exactVersion("rustc", EXPECTED.rustc, /^rustc\s+(\d+\.\d+\.\d+)/u),
    cargo: exactVersion("cargo", EXPECTED.cargo, /^cargo\s+(\d+\.\d+\.\d+)/u),
    pnpm: exactVersion("pnpm", EXPECTED.pnpm, /^(\d+\.\d+\.\d+)/u),
  };

  const defaultTestArguments = [
    "test",
    "--locked",
    "--offline",
    "-p",
    "academic-store",
    "--test",
    "sqlcipher_spike",
    "plaintext_default_binary_has_no_cipher_claim",
    "--",
    "--exact",
  ];
  run("cargo", defaultTestArguments);
  const defaultPosture = {
    spike_binary_built: false,
    required_features: ["sqlcipher-spike"],
    storage_mode: "PLAINTEXT_TEMPORARY_SQLITE",
    storage_encryption: "NONE",
    production_data_allowed: false,
    adr_002_accepted: false,
  };

  const testArguments = [
    "test",
    "--locked",
    "--offline",
    "-p",
    "academic-store",
    "--test",
    "sqlcipher_spike",
    "--no-default-features",
    "--features",
    "sqlcipher-spike",
    "--",
    "--nocapture",
  ];
  run("cargo", testArguments);

  const harnessOutput = run(
    "cargo",
    [
      "run",
      "--locked",
      "--offline",
      "--quiet",
      "-p",
      "academic-store",
      "--no-default-features",
      "--features",
      "sqlcipher-spike",
      "--bin",
      "sqlcipher_spike",
      "--",
      "run",
      artifactRoot,
    ],
    { capture: true },
  );
  const harness = parseJsonOutput("SQLCipher harness", harnessOutput);
  if (
    harness.plaintext_canary_hits !== 0 ||
    harness.adr_002_accepted !== false ||
    harness.production_data_allowed !== false
  ) {
    fail("the harness reported plaintext leakage or an unauthorized posture change");
  }

  const executable = binaryPath();
  if (!existsSync(executable)) {
    fail(`compiled evidence binary is missing: ${executable}`);
  }
  const admission = JSON.parse(
    readFileSync(resolve(repository, "docs/security/dependency-admission-phase1.json"), "utf8"),
  );
  const receipt = {
    receipt_version: 1,
    evidence_lane: "sqlcipher-spike",
    generated_at_utc: new Date().toISOString(),
    repository_head: repositoryHead(),
    platform: { platform: process.platform, arch: process.arch },
    toolchain,
    environment: {
      cargo_target_dir: process.env.CARGO_TARGET_DIR ?? null,
      rustflags: process.env.RUSTFLAGS ?? null,
      cflags: process.env.CFLAGS ?? null,
      cppflags: process.env.CPPFLAGS ?? null,
      ldflags: process.env.LDFLAGS ?? null,
    },
    commands: {
      default_posture: commandText("cargo", defaultTestArguments),
      seven_test_suite: commandText("cargo", testArguments),
      harness: "cargo run --locked --offline -p academic-store --no-default-features --features sqlcipher-spike --bin sqlcipher_spike -- run <artifact-root>",
    },
    source_admission: {
      sqlcipher_community: admission.bundled_sources.sqlcipher_community,
      openssl: admission.bundled_sources.openssl,
      libsqlite3_sys: admission.native_transitives.find(
        (entry) => entry.name === "libsqlite3-sys",
      ),
      openssl_src: admission.native_transitives.find((entry) => entry.name === "openssl-src"),
      openssl_sys: admission.native_transitives.find((entry) => entry.name === "openssl-sys"),
    },
    default_posture: defaultPosture,
    harness,
    binary: {
      path: executable,
      bytes: statSync(executable).size,
      sha256: sha256(executable),
      required_symbols: requiredSqlcipherSymbols(executable),
    },
    artifacts: filesBelow(artifactRoot),
    network_or_fetch_actions: [],
    advisory_scan_performed: false,
    production_data_allowed: false,
    adr_002_accepted: false,
  };

  mkdirSync(dirname(receiptPath), { recursive: true });
  writeFileSync(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`, {
    encoding: "utf8",
    flag: "wx",
  });
  process.stdout.write(`${JSON.stringify({ receipt: receiptPath, ...harness })}\n`);
}

try {
  main();
} catch (error) {
  if (process.exitCode !== 1) {
    process.stderr.write(`sqlcipher evidence: ${error.stack ?? error}\n`);
    process.exitCode = 1;
  }
}
