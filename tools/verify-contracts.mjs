import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

import Ajv2020 from "ajv/dist/2020.js";
import protobuf from "protobufjs";

const [fixtureText, fixtureSchemaText, artifactSchemaText, protoText, canonicalSpecBytes] = await Promise.all([
  readFile("schemas/fixtures/signed-batch-v1.json", "utf8"),
  readFile("schemas/jsonschema/signed-batch-fixture-v1.schema.json", "utf8"),
  readFile("schemas/jsonschema/artifact-descriptor-v1.schema.json", "utf8"),
  readFile("schemas/proto/academic/v1/ledger.proto", "utf8"),
  readFile("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
]);

const fixture = JSON.parse(fixtureText);
const fixtureSchema = JSON.parse(fixtureSchemaText);
const artifactSchema = JSON.parse(artifactSchemaText);
const { parseFixtureDocument } = await import("../packages/web-contracts/dist/index.js");

const ajv = new Ajv2020({ allErrors: true, strict: true });
const validateFixtureSchema = ajv.compile(fixtureSchema);
const validateArtifactSchema = ajv.compile(artifactSchema);
assert.equal(
  createHash("sha256").update(canonicalSpecBytes).digest("hex").toUpperCase(),
  "4830DEBD1A9EE8BE13B10D1E72BA3D2A3943F9D63417051CC123EF51743B2E45",
  "canonical spec bytes must remain the tracked LF representation",
);

assert.equal(
  validateFixtureSchema(fixture),
  true,
  `committed fixture must satisfy JSON Schema: ${ajv.errorsText(validateFixtureSchema.errors)}`,
);
assert.deepEqual(parseFixtureDocument(fixture), fixture);

const validArtifact = {
  id: "01900000-0000-7000-8000-000000000003",
  content_digest: `sha256:${"00".repeat(32)}`,
  media_type: "text/plain",
  byte_length: 9,
  domain_id: "01900000-0000-7000-8000-000000000001",
  confidentiality: "PERSONAL",
  retention_class: "USER_MANAGED",
  permission_lineage_id: "01900000-0000-7000-8000-000000000002",
  format_version: 1,
  vault_locator: `locator:v1:${"11".repeat(32)}`,
  evidence_representations: [
    {
      locator: {
        kind: "TEXT_BYTES",
        source_digest: `sha256:${"00".repeat(32)}`,
        start: 0,
        end: 9,
      },
      content_digest: `sha256:${"00".repeat(32)}`,
      byte_length: 9,
    },
  ],
};
assert.equal(
  validateArtifactSchema(validArtifact),
  true,
  `artifact descriptor example must satisfy JSON Schema: ${ajv.errorsText(validateArtifactSchema.errors)}`,
);
const unsafePathArtifact = structuredClone(validArtifact);
unsafePathArtifact.evidence_representations[0].locator = {
  kind: "REPOSITORY_BYTES",
  snapshot_digest: `sha256:${"22".repeat(32)}`,
  path: "C:/Windows/System32",
  start: 0,
  end: 9,
};
assert.equal(validateArtifactSchema(unsafePathArtifact), false);
const extraArtifactProperty = structuredClone(validArtifact);
extraArtifactProperty.unexpected = true;
assert.equal(validateArtifactSchema(extraArtifactProperty), false);

const clone = (value) => structuredClone(value);
const invalidFixtures = [
  ["fixture const", (value) => { value.fixture_version = 2; }],
  ["nonempty name", (value) => { value.name = ""; }],
  ["policy const", (value) => { value.data_class = "PERSONAL"; }],
  ["top additionalProperties", (value) => { value.unexpected = true; }],
  ["contract const", (value) => { value.contract.envelope = "wrong"; }],
  ["contract additionalProperties", (value) => { value.contract.unexpected = true; }],
  ["positive minimum", (value) => { value.expected_replay.accepted_events = 0; }],
  ["safe integer maximum", (value) => { value.expected_replay.accepted_events = 9007199254740992; }],
  ["replay additionalProperties", (value) => { value.expected_replay.unexpected = true; }],
  ["nonempty signed bytes", (value) => { value.signed_batch_cbor_hex = ""; }],
  ["device UUIDv7", (value) => { value.device_id = "00000000-0000-4000-8000-000000000000"; }],
  [
    "unique arrays",
    (value) => {
      const [first] = value.expected_replay.mastery_active_claim_ids;
      value.expected_replay.mastery_active_claim_ids.push(first);
    },
  ],
];

for (const [name, mutate] of invalidFixtures) {
  const candidate = clone(fixture);
  mutate(candidate);
  assert.equal(validateFixtureSchema(candidate), false, `schema accepted negative case: ${name}`);
  assert.throws(
    () => parseFixtureDocument(candidate),
    undefined,
    `TypeScript parser accepted negative case: ${name}`,
  );
}

assert.equal(fixtureSchema.$schema, "https://json-schema.org/draft/2020-12/schema");
assert.equal(artifactSchema.$schema, "https://json-schema.org/draft/2020-12/schema");
const protoRoot = protobuf.parse(protoText, { keepCase: true }).root;
protoRoot.resolveAll();
const originEventType = protoRoot.lookupType("academic.v1.OriginEvent");
const uuidBytes = (value) => Buffer.from(value.replaceAll("-", ""), "hex");
const protoRelationEvent = {
  id: { value: uuidBytes("01900000-0000-7000-8000-00000000000c") },
  origin_seq: 12,
  origin_observed_at: { unix_epoch_millis: 112 },
  domain_id: { value: uuidBytes("01900000-0000-7000-8000-000000000001") },
  actor: {
    importer: { name: "synthetic.official.fixture", version: "1.0.0" },
  },
  claim_related: {
    source_claim_id: { value: uuidBytes("01900000-0000-7000-8000-000000000206") },
    target_claim_id: { value: uuidBytes("01900000-0000-7000-8000-000000000205") },
    kind: 3,
    scope_id: { value: uuidBytes("01900000-0000-7000-8000-000000000007") },
  },
};
assert.equal(originEventType.verify(protoRelationEvent), null);
const protoRoundTrip = originEventType.toObject(
  originEventType.decode(originEventType.encode(protoRelationEvent).finish()),
  { bytes: String, enums: String, longs: Number, oneofs: true },
);
assert.equal(protoRoundTrip.actor.importer.name, "synthetic.official.fixture");
assert.equal(protoRoundTrip.claim_related.kind, "CLAIM_RELATION_KIND_SUPERSEDES");
assert.equal(protoRoundTrip.payload, "claim_related");
assert.match(protoText, /package academic\.v1;/u);
assert.match(protoText, /message DecimalValue\s*\{\s*string coefficient = 1;/su);
assert.match(
  protoText,
  /message Actor\s*\{\s*oneof kind\s*\{\s*UserActor user = 1;\s*DeterministicEngineActor deterministic_engine = 2;\s*ModelRunActor model_run = 3;\s*ImporterActor importer = 4;/su,
);
assert.match(
  protoText,
  /message ClaimRelation\s*\{\s*UuidV7 source_claim_id = 1;\s*UuidV7 target_claim_id = 2;\s*ClaimRelationKind kind = 3;\s*UuidV7 scope_id = 4;/su,
);
assert.match(protoText, /Actor actor = 5;/u);
assert.match(protoText, /ClaimRelation claim_related = 15;/u);
assert.match(protoText, /UuidV7 scope_id = 5; \/\/ required by semantic validation/u);
assert.match(protoText, /message ArtifactDescriptor\s*\{\s*UuidV7 id = 1;\s*Sha256Digest content_digest = 2;/su);
assert.match(protoText, /uint64 start_ms = 1;\s*uint64 end_ms = 2;/su);
assert.match(protoText, /string path = 2;/u);
assert.match(protoText, /optional uint32 confidence = 8;/u);
assert.doesNotMatch(protoText, /actor_kind/u);
assert.doesNotMatch(protoText, /optional UuidV7 scope_id/u);
assert.doesNotMatch(protoText, /plaintext_digest|keyed_vault_locator|confidence_permille/u);
assert.match(protoText, /bytes deterministic_payload_cbor = 2;/u);
assert.doesNotMatch(fixtureText, /https?:\/\//u);

console.log(
  "Committed fixture, JSON Schemas, TypeScript parser, and Protobuf drift profile verified.",
);
