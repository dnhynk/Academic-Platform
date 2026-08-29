import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

import { assertRepositorySourcePolicy } from "./source-preflight.mjs";

const [
  fixtureV1,
  fixtureV2,
  syntheticManifestSchemaText,
  dependencyReceiptText,
  keyDependencyReceiptText,
  scenarioDependencyReceiptText,
] = await Promise.all([
  readFile("schemas/fixtures/signed-batch-v1.json", "utf8"),
  readFile("schemas/fixtures/signed-batch-v2.json", "utf8"),
  readFile("schemas/jsonschema/synthetic-ingest-manifest-v1.schema.json", "utf8"),
  readFile("docs/security/dependency-admission-phase1.json", "utf8"),
  readFile("docs/security/dependency-admission-phase2-k1.json", "utf8"),
  readFile("docs/security/dependency-admission-phase2-c7.json", "utf8"),
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

// P2-K1 admitted a cryptographic key hierarchy, not real data. The receipt must
// stay complete: every admitted crate carries the four CONTRIBUTING items plus
// the reason it belongs inside its trust boundary, and no second async runtime
// or npm package entered with it.
const keyDependencyReceipt = JSON.parse(keyDependencyReceiptText);
assert.equal(keyDependencyReceipt.task, "P2-K1");
assert.equal(keyDependencyReceipt.resolution_budget, 1);
assert.deepEqual(keyDependencyReceipt.summary.npm_additions, []);
assert.equal(keyDependencyReceipt.summary.npm_install_scripts_added, false);
assert.equal(keyDependencyReceipt.summary.second_async_runtime_added, false);
assert.equal(keyDependencyReceipt.digest_tree_unification.duplicate_trait_generations, 0);
assert.equal(
  keyDependencyReceipt.admissions.length,
  keyDependencyReceipt.summary.added_external_crate_count,
);
assert.ok(keyDependencyReceipt.admissions.length > 0);
for (const admission of keyDependencyReceipt.admissions) {
  for (const field of ["name", "version", "checksum", "source", "license", "role"]) {
    assert.ok(
      typeof admission[field] === "string" && admission[field].length > 0,
      `${admission.name}: ${field} must be a non-empty string`,
    );
  }
  assert.match(admission.source, /^registry\+https:\/\/github\.com\/rust-lang\/crates\.io-index$/u);
  assert.match(admission.license, /MIT|Apache-2\.0/u);
  assert.ok(
    typeof admission.trust_boundary_justification === "string" &&
      admission.trust_boundary_justification.length >= 40,
    `${admission.name}: a trust-boundary justification is required`,
  );
}

// P2-C7 admitted a compile-fail harness, which is a build-time tool and never
// part of a shipping graph. The receipt must keep saying so for every crate in
// it: a later edit that promoted one of these to a product dependency has to
// fail here as well as in the crate-graph gate.
const scenarioDependencyReceipt = JSON.parse(scenarioDependencyReceiptText);
assert.equal(scenarioDependencyReceipt.task, "P2-C7");
assert.equal(scenarioDependencyReceipt.resolution_budget, 1);
assert.deepEqual(scenarioDependencyReceipt.summary.npm_additions, []);
assert.equal(scenarioDependencyReceipt.summary.npm_install_scripts_added, false);
assert.equal(scenarioDependencyReceipt.summary.linked_into_binary_count, 0);
assert.equal(
  scenarioDependencyReceipt.admissions.length,
  scenarioDependencyReceipt.summary.added_external_crate_count,
);
assert.equal(
  scenarioDependencyReceipt.admissions.length,
  scenarioDependencyReceipt.summary.build_time_only_count,
);
assert.ok(scenarioDependencyReceipt.admissions.length > 0);
for (const admission of scenarioDependencyReceipt.admissions) {
  for (const field of ["name", "version", "checksum", "source", "license", "role"]) {
    assert.ok(
      typeof admission[field] === "string" && admission[field].length > 0,
      `${admission.name}: ${field} must be a non-empty string`,
    );
  }
  assert.match(admission.source, /^registry\+https:\/\/github\.com\/rust-lang\/crates\.io-index$/u);
  assert.match(admission.license, /MIT|Apache-2\.0|Unlicense/u);
  assert.equal(
    admission.role,
    "build-time only, never linked into a product binary",
    `${admission.name}: the compile-fail harness must not be admitted as a shipping dependency`,
  );
  assert.ok(
    typeof admission.trust_boundary_justification === "string" &&
      admission.trust_boundary_justification.length >= 40,
    `${admission.name}: a trust-boundary justification is required`,
  );
}

console.log("Structural dependency-source and Phase 0/Phase 1 synthetic baseline passed.");
console.log(
  `P2-K1 key dependency admission receipt passed for ${keyDependencyReceipt.admissions.length} crates.`,
);
console.log(
  `P2-C7 compile-fail harness admission receipt passed for ${scenarioDependencyReceipt.admissions.length} crates.`,
);
