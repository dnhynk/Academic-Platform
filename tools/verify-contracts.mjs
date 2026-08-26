import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";

const [fixtureText, fixtureSchemaText, artifactSchemaText, protoText] = await Promise.all([
  readFile("schemas/fixtures/signed-batch-v1.json", "utf8"),
  readFile("schemas/jsonschema/signed-batch-fixture-v1.schema.json", "utf8"),
  readFile("schemas/jsonschema/artifact-descriptor-v1.schema.json", "utf8"),
  readFile("schemas/proto/academic/v1/ledger.proto", "utf8"),
]);

const fixture = JSON.parse(fixtureText);
const fixtureSchema = JSON.parse(fixtureSchemaText);
const artifactSchema = JSON.parse(artifactSchemaText);

assert.equal(fixture.fixture_version, 1);
assert.equal(fixture.data_class, "SYNTHETIC_ONLY");
assert.equal(fixture.network_egress, "NONE");
assert.equal(fixtureSchema.$schema, "https://json-schema.org/draft/2020-12/schema");
assert.equal(artifactSchema.$schema, "https://json-schema.org/draft/2020-12/schema");
assert.match(protoText, /package academic\.v1;/u);
assert.match(protoText, /bytes deterministic_payload_cbor = 2;/u);
assert.doesNotMatch(fixtureText, /https?:\/\//u);

console.log("Proto, JSON Schema, and synthetic fixture contract baseline verified.");
