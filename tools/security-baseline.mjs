import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import { assertPnpmLockSourcePolicy } from "./dependency-source-policy.mjs";

const [cargoLock, pnpmLock, fixtureV1, fixtureV2] = await Promise.all([
  readFile("Cargo.lock", "utf8"),
  readFile("pnpm-lock.yaml", "utf8"),
  readFile("schemas/fixtures/signed-batch-v1.json", "utf8"),
  readFile("schemas/fixtures/signed-batch-v2.json", "utf8"),
]);

assert.doesNotMatch(cargoLock, /source = "git\+/u, "Cargo git dependencies require explicit review");
assertPnpmLockSourcePolicy(pnpmLock);
for (const fixture of [fixtureV1, fixtureV2]) {
  assert.match(fixture, /"data_class": "SYNTHETIC_ONLY"/u);
  assert.match(fixture, /"network_egress": "NONE"/u);
}

console.log("Structural dependency-source and v1/v2 synthetic-fixture baseline passed.");
