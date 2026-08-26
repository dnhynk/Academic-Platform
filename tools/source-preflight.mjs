import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

import { assertCargoLockSourcePolicy } from "./cargo-lock-source-policy.mjs";
import { assertPnpmLockSourcePolicy } from "./dependency-source-policy.mjs";

/**
 * Runs the dependency-free lock source gate used before package-manager or
 * compiler execution. Only Node built-ins and checked-in parser code are used.
 */
export async function assertRepositorySourcePolicy(root = process.cwd()) {
  const [cargoLock, pnpmLock] = await Promise.all([
    readFile(resolve(root, "Cargo.lock"), "utf8"),
    readFile(resolve(root, "pnpm-lock.yaml"), "utf8"),
  ]);
  assertCargoLockSourcePolicy(cargoLock);
  assertPnpmLockSourcePolicy(pnpmLock);
}

const invokedPath = process.argv[1];
if (invokedPath !== undefined && import.meta.url === pathToFileURL(resolve(invokedPath)).href) {
  await assertRepositorySourcePolicy();
  console.log("Dependency-free Cargo/pnpm source preflight passed.");
}
