import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

import Ajv2020 from "ajv/dist/2020.js";
import protobuf from "protobufjs";

const [
  fixtureV1Bytes,
  fixtureV2Text,
  fixtureSchemaV1Text,
  fixtureSchemaV2Text,
  artifactSchemaText,
  artifactCorpusText,
  protoV1Text,
  protoV2Text,
  canonicalSpecBytes,
] = await Promise.all([
  readFile("schemas/fixtures/signed-batch-v1.json"),
  readFile("schemas/fixtures/signed-batch-v2.json", "utf8"),
  readFile("schemas/jsonschema/signed-batch-fixture-v1.schema.json", "utf8"),
  readFile("schemas/jsonschema/signed-batch-fixture-v2.schema.json", "utf8"),
  readFile("schemas/jsonschema/artifact-descriptor-v1.schema.json", "utf8"),
  readFile("schemas/fixtures/artifact-descriptor-parity-v1.json", "utf8"),
  readFile("schemas/proto/academic/v1/ledger.proto", "utf8"),
  readFile("schemas/proto/academic/v2/ledger.proto", "utf8"),
  readFile("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
]);

const fixtureV1Text = fixtureV1Bytes.toString("utf8");
const fixtureV1 = JSON.parse(fixtureV1Text);
const fixtureV2 = JSON.parse(fixtureV2Text);
const fixtureSchemaV1 = JSON.parse(fixtureSchemaV1Text);
const fixtureSchemaV2 = JSON.parse(fixtureSchemaV2Text);
const artifactSchema = JSON.parse(artifactSchemaText);
const artifactCorpus = JSON.parse(artifactCorpusText);
const { assertArtifactDescriptorSemantics, parseFixtureDocument } = await import("../packages/web-contracts/dist/index.js");

const ajv = new Ajv2020({ allErrors: true, strict: true });
const validateFixtureSchemaV1 = ajv.compile(fixtureSchemaV1);
const validateFixtureSchemaV2 = ajv.compile(fixtureSchemaV2);
const validateArtifactSchema = ajv.compile(artifactSchema);
assert.equal(
  createHash("sha256").update(canonicalSpecBytes).digest("hex").toUpperCase(),
  "4830DEBD1A9EE8BE13B10D1E72BA3D2A3943F9D63417051CC123EF51743B2E45",
  "canonical spec bytes must remain the tracked LF representation",
);

assert.equal(
  createHash("sha256").update(fixtureV1Bytes).digest("hex").toUpperCase(),
  "287F7DEA8FD24C3C6EB205C3F1E2873F6AFDF7D6532FE7BE4FCCFB44A0B7E163",
  "signed-batch-v1 golden bytes are immutable",
);
assert.equal(
  createHash("sha256").update(fixtureV2Text).digest("hex").toUpperCase(),
  "41675CC19BFBA5801F93D18EFC4786E5D65A5166F466DA0A2D43B05C379E43A6",
  "signed-batch-v2 must match the repaired deterministic builder",
);
for (const [version, fixture, validateFixtureSchema] of [
  [1, fixtureV1, validateFixtureSchemaV1],
  [2, fixtureV2, validateFixtureSchemaV2],
]) {
  assert.equal(
    validateFixtureSchema(fixture),
    true,
    `committed v${version} fixture must satisfy JSON Schema: ${ajv.errorsText(validateFixtureSchema.errors)}`,
  );
  assert.deepEqual(parseFixtureDocument(fixture), fixture);
}

const validArtifact = artifactCorpus.base;
assert.equal(
  validateArtifactSchema(validArtifact),
  true,
  `artifact descriptor example must satisfy JSON Schema: ${ajv.errorsText(validateArtifactSchema.errors)}`,
);
assert.doesNotThrow(() => assertArtifactDescriptorSemantics(validArtifact));
const extraArtifactProperty = structuredClone(validArtifact);
extraArtifactProperty.unexpected = true;
assert.equal(validateArtifactSchema(extraArtifactProperty), false);

const applyArtifactMutations = (base, mutations) => {
  const candidate = structuredClone(base);
  for (const mutation of mutations) {
    const components = mutation.path
      .split("/")
      .slice(1)
      .map((component) => component.replaceAll("~1", "/").replaceAll("~0", "~"));
    let target = candidate;
    for (const component of components.slice(0, -1)) {
      target = target[component];
    }
    const finalComponent = components.at(-1);
    assert.notEqual(finalComponent, undefined, `${mutation.path} must select a value`);
    if (mutation.op === "replace") {
      assert.ok(Object.hasOwn(target, finalComponent), `${mutation.path} must already exist`);
      target[finalComponent] = structuredClone(mutation.value);
    } else if (mutation.op === "append") {
      assert.ok(Array.isArray(target[finalComponent]), `${mutation.path} must select an array`);
      target[finalComponent].push(structuredClone(mutation.value));
    } else {
      assert.fail(`unsupported artifact mutation operation: ${mutation.op}`);
    }
  }
  return candidate;
};

for (const testCase of artifactCorpus.cases) {
  const candidate = applyArtifactMutations(artifactCorpus.base, testCase.mutations);
  assert.equal(
    validateArtifactSchema(candidate),
    testCase.schema_valid,
    `Ajv artifact parity disagreement: ${testCase.name}: ${ajv.errorsText(validateArtifactSchema.errors)}`,
  );
  let semanticValid = true;
  try {
    assertArtifactDescriptorSemantics(candidate);
  } catch {
    semanticValid = false;
  }
  assert.equal(
    semanticValid,
    testCase.semantic_valid,
    `TypeScript artifact parity disagreement: ${testCase.name}`,
  );
}

const clone = (value) => structuredClone(value);
const invalidFixtures = [
  ["fixture/contract version mismatch", (value) => { value.fixture_version = 1; }],
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
  const candidate = clone(fixtureV2);
  mutate(candidate);
  assert.equal(validateFixtureSchemaV2(candidate), false, `schema accepted negative case: ${name}`);
  assert.throws(
    () => parseFixtureDocument(candidate),
    undefined,
    `TypeScript parser accepted negative case: ${name}`,
  );
}

assert.equal(fixtureSchemaV1.$schema, "https://json-schema.org/draft/2020-12/schema");
assert.equal(fixtureSchemaV2.$schema, "https://json-schema.org/draft/2020-12/schema");
assert.equal(artifactSchema.$schema, "https://json-schema.org/draft/2020-12/schema");
const protoRoots = [protoV1Text, protoV2Text].map((text) => {
  const root = protobuf.parse(text, { keepCase: true }).root;
  root.resolveAll();
  return root;
});
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
const competingKnownPayloads = [
  [Buffer.from([0x52, 0x00]), "artifact_registered"],
  [Buffer.from([0x5a, 0x00]), "evidence_registered"],
  [Buffer.from([0x62, 0x00]), "claim_asserted"],
  [Buffer.from([0x6a, 0x00]), "decision_recorded"],
  [Buffer.from([0x72, 0x00]), "scope_registered"],
];
for (const version of [1, 2]) {
  const protoRoot = protoRoots[version - 1];
  assert.ok(protoRoot);
  const originEventType = protoRoot.lookupType(`academic.v${version}.OriginEvent`);
  assert.equal(originEventType.verify(protoRelationEvent), null);
  const relationWire = Buffer.from(originEventType.encode(protoRelationEvent).finish());
  const protoRoundTrip = originEventType.toObject(
    originEventType.decode(relationWire),
    { bytes: String, enums: String, longs: Number, oneofs: true },
  );
  assert.equal(protoRoundTrip.actor.importer.name, "synthetic.official.fixture");
  assert.equal(protoRoundTrip.claim_related.kind, "CLAIM_RELATION_KIND_SUPERSEDES");
  assert.equal(protoRoundTrip.payload, "claim_related");
  for (const [emptyKnownArm, armName] of competingKnownPayloads) {
    const relationLast = originEventType.toObject(
      originEventType.decode(Buffer.concat([emptyKnownArm, relationWire])),
      { oneofs: true },
    );
    assert.equal(relationLast.payload, "claim_related", `v${version} ${armName} before relation must be overridden`);
    const competingArmLast = originEventType.toObject(
      originEventType.decode(Buffer.concat([relationWire, emptyKnownArm])),
      { oneofs: true },
    );
    assert.equal(competingArmLast.payload, armName, `v${version} ${armName} after relation must override it`);
  }
}
assert.match(protoV1Text, /package academic\.v1;/u);
assert.match(protoV2Text, /package academic\.v2;/u);
assert.match(protoV2Text, /message DecimalValue\s*\{\s*string coefficient = 1;/su);
assert.match(
  protoV2Text,
  /message Actor\s*\{\s*oneof kind\s*\{\s*UserActor user = 1;\s*DeterministicEngineActor deterministic_engine = 2;\s*ModelRunActor model_run = 3;\s*ImporterActor importer = 4;/su,
);
assert.match(
  protoV2Text,
  /message ClaimRelation\s*\{\s*UuidV7 source_claim_id = 1;\s*UuidV7 target_claim_id = 2;\s*ClaimRelationKind kind = 3;\s*UuidV7 scope_id = 4;/su,
);
assert.match(protoV2Text, /Actor actor = 5;/u);
assert.match(
  protoV2Text,
  /ArtifactDescriptor artifact_registered = 10;\s*EvidenceItem evidence_registered = 11;\s*Claim claim_asserted = 12;\s*UserDecision decision_recorded = 13;\s*ScopeDescriptor scope_registered = 14;\s*ClaimRelation claim_related = 15;/su,
);
const userDecisionV1 = protoV1Text.match(/message UserDecision\s*\{(?<body>[\s\S]*?)\n\}/u)?.groups?.body;
const userDecisionV2 = protoV2Text.match(/message UserDecision\s*\{(?<body>[\s\S]*?)\n\}/u)?.groups?.body;
assert.ok(userDecisionV1);
assert.ok(userDecisionV2);
assert.doesNotMatch(userDecisionV1, /= (?:9|10|11|12);/u, "v1 UserDecision wire shape is immutable");
assert.match(userDecisionV2, /UuidV7 subject_entity_id = 9; \/\/ semantic resolution slot subject/u);
assert.match(userDecisionV2, /string predicate_id = 10; \/\/ semantic resolution slot predicate/u);
assert.match(userDecisionV2, /ClaimObject target_object = 11; \/\/ durable assertion semantics across claim IDs/u);
assert.match(userDecisionV2, /ValidInterval valid_time = 12; \/\/ explicit user-controlled applicability/u);
assert.match(protoV2Text, /message ArtifactDescriptor\s*\{\s*UuidV7 id = 1;\s*Sha256Digest content_digest = 2;/su);
assert.match(protoV2Text, /uint64 start_ms = 1;\s*uint64 end_ms = 2;/su);
assert.match(protoV2Text, /string path = 2;/u);
assert.match(protoV2Text, /optional uint32 confidence = 8;/u);
assert.doesNotMatch(protoV2Text, /actor_kind/u);
assert.doesNotMatch(protoV2Text, /optional UuidV7 scope_id/u);
assert.doesNotMatch(protoV2Text, /plaintext_digest|keyed_vault_locator|confidence_permille/u);
assert.match(protoV2Text, /bytes deterministic_payload_cbor = 2;/u);
assert.doesNotMatch(fixtureV1Text, /https?:\/\//u);
assert.doesNotMatch(fixtureV2Text, /https?:\/\//u);

console.log(
  "Immutable v1/upcast and current v2 fixtures, artifact parity, schemas, TypeScript semantics, and Protobuf oneof profiles verified.",
);
