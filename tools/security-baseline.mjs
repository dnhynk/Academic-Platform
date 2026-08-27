import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import { assertRepositorySourcePolicy } from "./source-preflight.mjs";

const [fixtureV1, fixtureV2, syntheticManifestSchemaText, dependencyReceiptText] =
  await Promise.all([
    readFile("schemas/fixtures/signed-batch-v1.json", "utf8"),
    readFile("schemas/fixtures/signed-batch-v2.json", "utf8"),
    readFile("schemas/jsonschema/synthetic-ingest-manifest-v1.schema.json", "utf8"),
    readFile("docs/security/dependency-admission-phase1.json", "utf8"),
  ]);

await assertRepositorySourcePolicy();
for (const fixture of [fixtureV1, fixtureV2]) {
  assert.match(fixture, /"data_class": "SYNTHETIC_ONLY"/u);
  assert.match(fixture, /"network_egress": "NONE"/u);
}

const syntheticManifestSchema = JSON.parse(syntheticManifestSchemaText);
assert.equal(syntheticManifestSchema.properties.data_class.const, "SYNTHETIC_ONLY");
assert.equal(syntheticManifestSchema.properties.network_egress.const, "NONE");
assert.equal(syntheticManifestSchema.properties.storage_encryption.const, "NONE");
assert.equal(syntheticManifestSchema.properties.production_data_allowed.const, false);
assert.equal(syntheticManifestSchema.properties.product_network.const, "NONE");

const dependencyReceipt = JSON.parse(dependencyReceiptText);
assert.equal(dependencyReceipt.resolution_budget, 1);
assert.deepEqual(dependencyReceipt.npm_additions, []);
assert.equal(dependencyReceipt.npm_install_scripts_added, false);

console.log("Structural dependency-source and Phase 0/Phase 1 synthetic baseline passed.");
