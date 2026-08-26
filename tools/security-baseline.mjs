import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import { assertRepositorySourcePolicy } from "./source-preflight.mjs";

const [fixtureV1, fixtureV2] = await Promise.all([
  readFile("schemas/fixtures/signed-batch-v1.json", "utf8"),
  readFile("schemas/fixtures/signed-batch-v2.json", "utf8"),
]);

await assertRepositorySourcePolicy();
for (const fixture of [fixtureV1, fixtureV2]) {
  assert.match(fixture, /"data_class": "SYNTHETIC_ONLY"/u);
  assert.match(fixture, /"network_egress": "NONE"/u);
}

console.log("Structural dependency-source and v1/v2 synthetic-fixture baseline passed.");
