import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";

import { assertPnpmLockSourcePolicy } from "./dependency-source-policy.mjs";

function run(command, commandArguments, cwd) {
  const result = spawnSync(command, commandArguments, {
    cwd,
    encoding: "utf8",
    env: { ...process.env, npm_config_update_notifier: "false" },
  });
  assert.equal(
    result.status,
    0,
    `${command} ${commandArguments.join(" ")} failed\n${result.stdout}\n${result.stderr}`,
  );
}

function runExpectingFailure(command, commandArguments, cwd) {
  const result = spawnSync(command, commandArguments, {
    cwd,
    encoding: "utf8",
    env: { ...process.env, npm_config_update_notifier: "false" },
  });
  assert.notEqual(
    result.status,
    0,
    `${command} ${commandArguments.join(" ")} unexpectedly succeeded`,
  );
  return result;
}

const temporaryBase = resolve(tmpdir());
const root = await mkdtemp(join(temporaryBase, "academic-pnpm-key-policy-"));
assert.ok(resolve(root).startsWith(`${temporaryBase}\\`) || resolve(root).startsWith(`${temporaryBase}/`));

try {
  const dependencyRoot = join(root, "dependency");
  const consumerRoot = join(root, "consumer");
  const variationConsumerRoot = join(root, "variation-consumer");
  await Promise.all([mkdir(dependencyRoot), mkdir(consumerRoot), mkdir(variationConsumerRoot)]);
  await writeFile(
    join(dependencyRoot, "package.json"),
    `${JSON.stringify({ name: "t017-local-git", version: "1.0.0", main: "index.js" }, null, 2)}\n`,
  );
  await writeFile(join(dependencyRoot, "index.js"), "export const synthetic = true;\n");
  run("git", ["init", "--quiet"], dependencyRoot);
  run("git", ["config", "user.name", "Academic Phase 0 Fixture"], dependencyRoot);
  run("git", ["config", "user.email", "fixture@academic.invalid"], dependencyRoot);
  run("git", ["config", "core.autocrlf", "false"], dependencyRoot);
  run("git", ["add", "package.json", "index.js"], dependencyRoot);
  run("git", ["commit", "--quiet", "-m", "synthetic fixture"], dependencyRoot);

  const dependencyUrl = `git+${pathToFileURL(dependencyRoot).href}`;
  await writeFile(
    join(consumerRoot, "package.json"),
    `${JSON.stringify({
      name: "t017-pnpm-policy-consumer",
      version: "1.0.0",
      private: true,
      dependencies: { "t017-local-git": dependencyUrl },
    }, null, 2)}\n`,
  );
  run(
    "pnpm",
    ["install", "--lockfile-only", "--offline", "--ignore-scripts"],
    consumerRoot,
  );
  await rm(join(consumerRoot, "node_modules"), { recursive: true, force: true });

  const lockPath = join(consumerRoot, "pnpm-lock.yaml");
  const generatedLock = await readFile(lockPath, "utf8");
  let decoratedLock = generatedLock;
  for (const section of ["importers", "packages", "snapshots"]) {
    const replacement = `&hide_${section} ${section}:`;
    const next = decoratedLock.replace(new RegExp(`^${section}:`, "mu"), replacement);
    assert.notEqual(next, decoratedLock, `pnpm-generated lock must contain ${section}`);
    decoratedLock = next;
  }
  await writeFile(lockPath, decoratedLock);
  assert.throws(
    () => assertPnpmLockSourcePolicy(decoratedLock, "anchored-real-pnpm-lock.yaml"),
    /anchors, aliases, and tags are forbidden on mapping keys/u,
  );

  const before = createHash("sha256").update(decoratedLock).digest("hex");
  run(
    "pnpm",
    ["install", "--frozen-lockfile", "--offline", "--ignore-scripts"],
    consumerRoot,
  );
  const installed = JSON.parse(
    await readFile(join(consumerRoot, "node_modules", "t017-local-git", "package.json"), "utf8"),
  );
  assert.equal(installed.name, "t017-local-git");
  const afterText = await readFile(lockPath, "utf8");
  const after = createHash("sha256").update(afterText).digest("hex");
  assert.equal(after, before, "frozen pnpm consumption must leave the concealed-source lock unchanged");

  await writeFile(
    join(variationConsumerRoot, "package.json"),
    `${JSON.stringify({
      name: "phase0-synthetic-runtime-variation",
      version: "1.0.0",
      private: true,
      devDependencies: { node: "runtime:24.19.0" },
    }, null, 2)}\n`,
  );
  const variationLock = [
    "lockfileVersion: '9.0'",
    "",
    "importers:",
    "  .:",
    "    devDependencies:",
    "      node:",
    "        specifier: runtime:24.19.0",
    "        version: runtime:24.19.0",
    "",
    "packages:",
    "  node@runtime:24.19.0:",
    "    version: 24.19.0",
    "    resolution:",
    "      type: variations",
    "      variants:",
    "        - targets: [{os: win32, cpu: x64}]",
    "          resolution:",
    "            type: binary",
    "            archive: zip",
    "            bin: node.exe",
    "            integrity: sha512-AA==",
    "            url: http://127.0.0.1:9/node.zip",
    "        - targets: [{os: linux, cpu: x64}]",
    "          resolution:",
    "            type: binary",
    "            archive: tarball",
    "            bin: bin/node",
    "            integrity: sha512-AA==",
    "            url: http://127.0.0.1:9/node.tar.gz",
    "",
    "snapshots:",
    "  node@runtime:24.19.0: {}",
    "",
  ].join("\n");
  const variationLockPath = join(variationConsumerRoot, "pnpm-lock.yaml");
  await writeFile(variationLockPath, variationLock);
  assert.throws(
    () => assertPnpmLockSourcePolicy(variationLock, "runtime-variations-lock.yaml"),
    /insecure HTTP or Git dependency sources are forbidden/u,
  );
  const variationBefore = createHash("sha256").update(variationLock).digest("hex");
  const variationResult = runExpectingFailure(
    "pnpm",
    ["install", "--frozen-lockfile", "--offline", "--ignore-scripts"],
    variationConsumerRoot,
  );
  const variationOutput = `${variationResult.stdout}\n${variationResult.stderr}`;
  assert.match(variationOutput, /ERR_PNPM_CANNOT_DOWNLOAD_BINARY_OFFLINE/u);
  assert.match(variationOutput, /http:\/\/127\.0\.0\.1:9\/node\.(?:zip|tar\.gz)/u);
  const variationAfterText = await readFile(variationLockPath, "utf8");
  assert.equal(
    createHash("sha256").update(variationAfterText).digest("hex"),
    variationBefore,
    "frozen pnpm consumption must leave the rejected variation lock unchanged",
  );

  console.log(
    "Restricted YAML and recursive variation policy rejected concealed sources before real frozen/offline pnpm consumption.",
  );
} finally {
  await rm(root, { recursive: true, force: true });
}
