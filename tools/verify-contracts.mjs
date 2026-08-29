import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";
import { join } from "node:path";

import Ajv2020 from "ajv/dist/2020.js";
import protobuf from "protobufjs";

import { parsePnpmLockYaml } from "./restricted-yaml.mjs";
import {
  GENERATED_PATH,
  REGISTRY_PATH,
  predicateId,
  renderRustModule,
} from "./predicate-registry.mjs";
import {
  GENERATED_PATH as ENGINE_GENERATED_PATH,
  REGISTRY_PATH as ENGINE_REGISTRY_PATH,
  engineId,
  renderRustModule as renderEngineModule,
  specName,
} from "./engine-registry.mjs";

async function readRustSourceTree(root, relative = "") {
  const directory = join(root, relative);
  const entries = await readdir(directory, { withFileTypes: true });
  const sources = new Map();
  for (const entry of entries.toSorted((left, right) => left.name.localeCompare(right.name))) {
    const childRelative = relative.length === 0 ? entry.name : `${relative}/${entry.name}`;
    if (entry.isDirectory()) {
      for (const [path, source] of await readRustSourceTree(root, childRelative)) {
        sources.set(path, source);
      }
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      sources.set(childRelative, await readFile(join(root, childRelative), "utf8"));
    } else {
      assert.fail(`unreviewed contracts source entry is forbidden: ${childRelative}`);
    }
  }
  return sources;
}

const [
  fixtureV1Bytes,
  fixtureV2Bytes,
  fixtureV3Bytes,
  fixtureSchemaV1Bytes,
  fixtureSchemaV2Text,
  fixtureSchemaV3Text,
  artifactSchemaText,
  syntheticManifestSchemaText,
  artifactCorpusText,
  fixtureRawCorpusText,
  fixtureIntegerCorpusText,
  fixtureByteCorpusText,
  predictionMetadataCorpusText,
  toolVersionCorpusText,
  nodePinText,
  rustToolchainText,
  packageJsonText,
  protoV1Bytes,
  protoV2Text,
  protoV3Text,
  canonicalSpecBytes,
  rustProtoContractText,
  rustDomainText,
  rustContractsText,
  rustCoreText,
  rustCliText,
  rustCliDoctorText,
  rustCliFixtureText,
  bootstrapText,
  sourcePreflightText,
  dependencySourcePolicyText,
  cargoLockSourcePolicyText,
  restrictedYamlText,
  ciText,
] = await Promise.all([
  readFile("schemas/fixtures/signed-batch-v1.json"),
  readFile("schemas/fixtures/signed-batch-v2.json"),
  readFile("schemas/fixtures/signed-batch-v3.json"),
  readFile("schemas/jsonschema/signed-batch-fixture-v1.schema.json"),
  readFile("schemas/jsonschema/signed-batch-fixture-v2.schema.json", "utf8"),
  readFile("schemas/jsonschema/signed-batch-fixture-v3.schema.json", "utf8"),
  readFile("schemas/jsonschema/artifact-descriptor-v1.schema.json", "utf8"),
  readFile("schemas/jsonschema/synthetic-ingest-manifest-v1.schema.json", "utf8"),
  readFile("schemas/fixtures/artifact-descriptor-parity-v1.json", "utf8"),
  readFile("schemas/fixtures/signed-batch-raw-parity-v1.json", "utf8"),
  readFile("schemas/fixtures/signed-batch-integer-lexeme-parity-v1.json", "utf8"),
  readFile("schemas/fixtures/signed-batch-byte-parity-v1.json", "utf8"),
  readFile("schemas/fixtures/prediction-metadata-parity-v1.json", "utf8"),
  readFile("tools/fixtures/tool-version-conformance-v1.json", "utf8"),
  readFile(".nvmrc", "utf8"),
  readFile("rust-toolchain.toml", "utf8"),
  readFile("package.json", "utf8"),
  readFile("schemas/proto/academic/v1/ledger.proto"),
  readFile("schemas/proto/academic/v2/ledger.proto", "utf8"),
  readFile("schemas/proto/academic/v3/ledger.proto", "utf8"),
  readFile("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
  readFile("crates/contracts/src/proto_contract.rs", "utf8"),
  readFile("crates/domain/src/lib.rs", "utf8"),
  readFile("crates/contracts/src/lib.rs", "utf8"),
  readFile("crates/core/src/lib.rs", "utf8"),
  readFile("crates/cli/src/main.rs", "utf8"),
  readFile("crates/cli/src/commands/doctor.rs", "utf8"),
  readFile("crates/cli/src/commands/fixture.rs", "utf8"),
  readFile("tools/bootstrap.mjs", "utf8"),
  readFile("tools/source-preflight.mjs", "utf8"),
  readFile("tools/dependency-source-policy.mjs", "utf8"),
  readFile("tools/cargo-lock-source-policy.mjs", "utf8"),
  readFile("tools/restricted-yaml.mjs", "utf8"),
  readFile(".github/workflows/ci.yml", "utf8"),
]);
const rustContractsSources = await readRustSourceTree("crates/contracts/src");
assert.deepEqual(
  [...rustContractsSources.keys()],
  ["lib.rs", "proto_contract.rs"],
  "academic-contracts source inventory must remain explicit and complete",
);
assert.equal(rustContractsSources.get("lib.rs"), rustContractsText);
assert.equal(rustContractsSources.get("proto_contract.rs"), rustProtoContractText);

const fixtureSchemaV1Text = fixtureSchemaV1Bytes.toString("utf8");
const protoV1Text = protoV1Bytes.toString("utf8");
const fixtureSchemaV1 = JSON.parse(fixtureSchemaV1Text);
const fixtureSchemaV2 = JSON.parse(fixtureSchemaV2Text);
const fixtureSchemaV3 = JSON.parse(fixtureSchemaV3Text);
const artifactSchema = JSON.parse(artifactSchemaText);
const syntheticManifestSchema = JSON.parse(syntheticManifestSchemaText);
const artifactCorpus = JSON.parse(artifactCorpusText);
const fixtureRawCorpus = JSON.parse(fixtureRawCorpusText);
const fixtureIntegerCorpus = JSON.parse(fixtureIntegerCorpusText);
const fixtureByteCorpus = JSON.parse(fixtureByteCorpusText);
const predictionMetadataCorpus = JSON.parse(predictionMetadataCorpusText);
const toolVersionCorpus = JSON.parse(toolVersionCorpusText);
const packageJson = JSON.parse(packageJsonText);
assert.equal(fixtureRawCorpus.schema_version, 1, "raw fixture corpus schema version");
assert.equal(fixtureIntegerCorpus.schema_version, 1, "integer fixture corpus schema version");
assert.equal(fixtureByteCorpus.schema_version, 1, "byte fixture corpus schema version");
assert.equal(
  predictionMetadataCorpus.schema_version,
  1,
  "prediction metadata corpus schema version",
);
const {
  assertArtifactDescriptorSemantics,
  assertCanonicalArtifactJsonNumberTokens,
  decodePortableFixtureJsonBytes,
  parseArtifactDescriptorJson,
  parseFixtureDocument,
  parseFixtureDocumentJson,
} = await import("../packages/web-contracts/dist/index.js");
const { assertToolVersionConformanceCorpus } = await import("./tool-version-policy.mjs");

const fixtureV1Text = decodePortableFixtureJsonBytes(fixtureV1Bytes);
const fixtureV2Text = decodePortableFixtureJsonBytes(fixtureV2Bytes);
const fixtureV3Text = decodePortableFixtureJsonBytes(fixtureV3Bytes);
const fixtureV1 = JSON.parse(fixtureV1Text);
const fixtureV2 = JSON.parse(fixtureV2Text);
const fixtureV3 = JSON.parse(fixtureV3Text);
assertToolVersionConformanceCorpus(toolVersionCorpus);
const rustPin = rustToolchainText.match(/^channel = "(?<version>[^"]+)"$/mu)?.groups?.version;
assert.ok(rustPin, "rust-toolchain.toml must contain one exact channel pin");
assert.deepEqual(
  Object.fromEntries(toolVersionCorpus.tools.map((tool) => [tool.name, tool.expected])),
  {
    rustc: `rustc ${rustPin}`,
    cargo: `cargo ${rustPin}`,
    node: `v${nodePinText.trim()}`,
    pnpm: packageJson.packageManager.replace("pnpm@", ""),
  },
  "shared conformance outputs must be derived token-exactly from repository pins",
);
assert.equal(packageJson.engines.node, nodePinText.trim(), "package Node engine must match .nvmrc");
assert.equal(
  packageJson.engines.pnpm,
  packageJson.packageManager.replace("pnpm@", ""),
  "package pnpm engine and packageManager pins must agree",
);
const expectedDoctorScript =
  "cargo run --locked --offline --quiet -p academic-cli -- doctor --format json";
assert.equal(
  packageJson.scripts.doctor,
  expectedDoctorScript,
  "the pnpm doctor wrapper must force locked, offline Cargo execution",
);
const doctorWithoutOffline = structuredClone(packageJson);
doctorWithoutOffline.scripts.doctor =
  "cargo run --locked --quiet -p academic-cli -- doctor --format json";
assert.throws(
  () => assert.equal(doctorWithoutOffline.scripts.doctor, expectedDoctorScript),
  undefined,
  "a doctor wrapper without --offline must fail exact script verification",
);
assert.match(
  bootstrapText,
  /const versionCorpus = await loadToolVersionConformanceCorpus\(\);[\s\S]*!isSupportedToolVersion\(tool, observed\)/u,
  "bootstrap must enforce the shared token-exact corpus at the executable probe",
);
assert.doesNotMatch(
  bootstrapText,
  /observed\.startsWith|observed\.starts_with/u,
  "bootstrap must not use unrestricted version-prefix acceptance",
);
assert.match(
  rustCliDoctorText,
  /include_str!\("\.\.\/\.\.\/\.\.\/\.\.\/tools\/fixtures\/tool-version-conformance-v1\.json"\)/u,
  "Rust doctor must consume the same committed conformance corpus as bootstrap",
);
assert.match(
  rustCliDoctorText,
  /is_some_and\(\|value\| is_supported_tool_version\(specification, value\)\)/u,
  "Rust doctor executable probes must use token-exact conformance",
);
assert.doesNotMatch(
  rustCliDoctorText,
  /value\.starts_with\([^\n]*expected|value\.starts_with\([^\n]*EXPECTED/u,
  "Rust doctor must not use unrestricted expected-version prefix acceptance",
);

const ajv = new Ajv2020({ allErrors: true, strict: true });
const validateFixtureSchemaV1 = ajv.compile(fixtureSchemaV1);
const validateFixtureSchemaV2 = ajv.compile(fixtureSchemaV2);
const validateFixtureSchemaV3 = ajv.compile(fixtureSchemaV3);
const validateArtifactSchema = ajv.compile(artifactSchema);
const validateSyntheticManifestSchema = ajv.compile(syntheticManifestSchema);
const syntheticManifest = structuredClone(syntheticManifestSchema.examples[0]);
assert.equal(
  validateSyntheticManifestSchema(syntheticManifest),
  true,
  `synthetic ingest example must satisfy JSON Schema: ${ajv.errorsText(validateSyntheticManifestSchema.errors)}`,
);
assert.equal(syntheticManifest.fixture_byte_length, fixtureV2Bytes.length);
assert.equal(
  syntheticManifest.fixture_sha256,
  createHash("sha256").update(fixtureV2Bytes).digest("hex"),
  "synthetic manifest must bind the frozen v2 fixture the store lane ingests",
);
for (const [field, value] of [
  ["data_class", "PERSONAL"],
  ["network_egress", "HTTPS"],
  ["storage_encryption", "SQLCIPHER"],
  ["production_data_allowed", true],
  ["product_network", "TCP"],
]) {
  assert.equal(
    validateSyntheticManifestSchema({ ...syntheticManifest, [field]: value }),
    false,
    `synthetic manifest schema accepted ${field}=${String(value)}`,
  );
}
const sha256Upper = (bytes) => createHash("sha256").update(bytes).digest("hex").toUpperCase();
const assertImmutableV1Bytes = (bytes, expected, label) => {
  assert.equal(sha256Upper(bytes), expected, `${label} bytes are immutable`);
};
const replaceBytesOnce = (source, needleUtf8, replacementHex, label) => {
  assert.match(replacementHex, /^(?:[0-9a-f]{2})+$/u, `${label}: canonical replacement hex`);
  const sourceBuffer = Buffer.from(source);
  const needle = Buffer.from(needleUtf8, "utf8");
  const index = sourceBuffer.indexOf(needle);
  assert.ok(index >= 0, `${label}: byte needle must exist`);
  assert.equal(
    sourceBuffer.indexOf(needle, index + needle.length),
    -1,
    `${label}: byte needle must be unique`,
  );
  return Buffer.concat([
    sourceBuffer.subarray(0, index),
    Buffer.from(replacementHex, "hex"),
    sourceBuffer.subarray(index + needle.length),
  ]);
};
assert.equal(
  sha256Upper(canonicalSpecBytes),
  "4830DEBD1A9EE8BE13B10D1E72BA3D2A3943F9D63417051CC123EF51743B2E45",
  "canonical spec bytes must remain the tracked LF representation",
);

assertImmutableV1Bytes(
  fixtureV1Bytes,
  "287F7DEA8FD24C3C6EB205C3F1E2873F6AFDF7D6532FE7BE4FCCFB44A0B7E163",
  "signed-batch-v1 golden",
);
assertImmutableV1Bytes(
  fixtureSchemaV1Bytes,
  "9588EE9B439C9DBCF864A8F07BD64BD6353ECC8F1D46151348C9B3283B36E6BD",
  "signed-batch-fixture-v1 JSON Schema",
);
assertImmutableV1Bytes(
  protoV1Bytes,
  "8BC58C574E0BEC84F6BC3D6BB3A7E006E45DE69B1793C407BBCAC57FD29C507A",
  "academic.v1 Proto",
);
for (const [label, mutated, expected] of [
  [
    "fixture byte mutation",
    Buffer.from(fixtureV1Text.replace("phase0-synthetic", "phase0-Synthetic"), "utf8"),
    "287F7DEA8FD24C3C6EB205C3F1E2873F6AFDF7D6532FE7BE4FCCFB44A0B7E163",
  ],
  [
    "JSON Schema shape mutation",
    Buffer.from(fixtureSchemaV1Text.replace('"additionalProperties": false', '"additionalProperties": true'), "utf8"),
    "9588EE9B439C9DBCF864A8F07BD64BD6353ECC8F1D46151348C9B3283B36E6BD",
  ],
  [
    "Proto tag mutation",
    Buffer.from(protoV1Text.replace("ClaimRelation claim_related = 15;", "ClaimRelation claim_related = 16;"), "utf8"),
    "8BC58C574E0BEC84F6BC3D6BB3A7E006E45DE69B1793C407BBCAC57FD29C507A",
  ],
]) {
  assert.throws(
    () => assertImmutableV1Bytes(mutated, expected, label),
    undefined,
    `${label} must fail the immutable-v1 contract guard`,
  );
}
assert.equal(
  createHash("sha256").update(fixtureV2Bytes).digest("hex").toUpperCase(),
  "F94DFCF7E3E376E54B5514CEB3016B0B7D97D17366562F7AC4A16286D3AA367D",
  "signed-batch-v2 must match the repaired deterministic builder",
);
// Ajv, TypeScript, and Rust all read all three fixtures. A fixture may satisfy
// only its own version's schema: the wrapper version, payload label, and event
// schema version are `const` in each, so a cross-version match is drift.
const fixtureSchemaValidators = [
  [1, validateFixtureSchemaV1],
  [2, validateFixtureSchemaV2],
  [3, validateFixtureSchemaV3],
];
for (const [version, fixture, rawFixture] of [
  [1, fixtureV1, fixtureV1Bytes],
  [2, fixtureV2, fixtureV2Bytes],
  [3, fixtureV3, fixtureV3Bytes],
]) {
  for (const [schemaVersion, validateFixtureSchema] of fixtureSchemaValidators) {
    assert.equal(
      validateFixtureSchema(fixture),
      schemaVersion === version,
      `committed v${version} fixture against the v${schemaVersion} JSON Schema: ${ajv.errorsText(validateFixtureSchema.errors)}`,
    );
  }
  assert.deepEqual(parseFixtureDocumentJson(rawFixture), fixture);
}

for (const testCase of [...fixtureRawCorpus.cases, ...fixtureIntegerCorpus.cases]) {
  const baseText = testCase.fixture === 1 ? fixtureV1Text : fixtureV2Text;
  const validateFixtureSchema = testCase.fixture === 1
    ? validateFixtureSchemaV1
    : validateFixtureSchemaV2;
  let rawFixture = baseText;
  for (const replacement of testCase.replacements) {
    const next = rawFixture.replace(replacement.needle, replacement.replacement);
    assert.notEqual(next, rawFixture, `${testCase.name}: replacement must mutate fixture text`);
    rawFixture = next;
  }
  const rawFixtureBytes = Buffer.from(rawFixture, "utf8");
  let schemaBoundaryValid = true;
  try {
    const decoded = decodePortableFixtureJsonBytes(rawFixtureBytes);
    schemaBoundaryValid = validateFixtureSchema(JSON.parse(decoded));
  } catch {
    schemaBoundaryValid = false;
  }
  assert.equal(
    schemaBoundaryValid,
    testCase.valid,
    `raw fixture/Ajv boundary disagreement: ${testCase.name}`,
  );
  let typescriptValid = true;
  try {
    parseFixtureDocumentJson(rawFixtureBytes);
  } catch {
    typescriptValid = false;
  }
  assert.equal(
    typescriptValid,
    testCase.valid,
    `raw fixture/TypeScript boundary disagreement: ${testCase.name}`,
  );
}

for (const testCase of fixtureByteCorpus.cases) {
  let bytes = Buffer.from(testCase.fixture === 1 ? fixtureV1Bytes : fixtureV2Bytes);
  for (const replacement of testCase.replacements) {
    bytes = replaceBytesOnce(
      bytes,
      replacement.needle_utf8,
      replacement.replacement_hex,
      testCase.name,
    );
  }
  const validateFixtureSchema = testCase.fixture === 1
    ? validateFixtureSchemaV1
    : validateFixtureSchemaV2;
  let schemaBoundaryValid = true;
  try {
    const decoded = decodePortableFixtureJsonBytes(bytes);
    schemaBoundaryValid = validateFixtureSchema(JSON.parse(decoded));
  } catch {
    schemaBoundaryValid = false;
  }
  assert.equal(
    schemaBoundaryValid,
    testCase.valid,
    `strict UTF-8/Ajv boundary disagreement: ${testCase.name}`,
  );
  let typescriptValid = true;
  try {
    parseFixtureDocumentJson(bytes);
  } catch {
    typescriptValid = false;
  }
  assert.equal(
    typescriptValid,
    testCase.valid,
    `strict UTF-8/TypeScript boundary disagreement: ${testCase.name}`,
  );
  if (!testCase.valid) {
    assert.throws(
      () => decodePortableFixtureJsonBytes(bytes),
      TypeError,
      `malformed UTF-8 must reject before JSON/Ajv: ${testCase.name}`,
    );
  }
}

for (const testCase of predictionMetadataCorpus.cases) {
  const candidate = structuredClone(fixtureV2);
  candidate.expected_replay.prediction_claims = [structuredClone(testCase.disclosure)];
  assert.equal(
    validateFixtureSchemaV2(candidate),
    testCase.schema_valid,
    `prediction metadata/Ajv parity disagreement: ${testCase.name}: ${ajv.errorsText(validateFixtureSchemaV2.errors)}`,
  );
  let typescriptValid = true;
  try {
    parseFixtureDocument(candidate);
  } catch {
    typescriptValid = false;
  }
  assert.equal(
    typescriptValid,
    testCase.semantic_valid,
    `prediction metadata/TypeScript parity disagreement: ${testCase.name}`,
  );
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
    } else if (mutation.op === "add") {
      assert.ok(!Object.hasOwn(target, finalComponent), `${mutation.path} must be a new property`);
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
  let typescriptRawValid = true;
  try {
    parseArtifactDescriptorJson(JSON.stringify(candidate));
  } catch {
    typescriptRawValid = false;
  }
  assert.equal(
    typescriptRawValid,
    testCase.schema_valid && testCase.semantic_valid,
    `TypeScript raw artifact parity disagreement: ${testCase.name}`,
  );
}

for (const testCase of artifactCorpus.raw_number_cases) {
  const candidate = applyArtifactMutations(artifactCorpus.base, testCase.mutations);
  const components = testCase.path
    .split("/")
    .slice(1)
    .map((component) => component.replaceAll("~1", "/").replaceAll("~0", "~"));
  let target = candidate;
  for (const component of components.slice(0, -1)) {
    target = target[component];
  }
  const finalComponent = components.at(-1);
  assert.notEqual(finalComponent, undefined, `${testCase.path} must select a raw number`);
  assert.ok(Object.hasOwn(target, finalComponent), `${testCase.path} must already exist`);
  target[finalComponent] = "__RAW_INTEGER_TOKEN__";
  const template = JSON.stringify(candidate);
  const rawJson = template.replace('"__RAW_INTEGER_TOKEN__"', testCase.token);

  let ajvRawValid = true;
  try {
    assertCanonicalArtifactJsonNumberTokens(rawJson);
    ajvRawValid = validateArtifactSchema(JSON.parse(rawJson));
  } catch {
    ajvRawValid = false;
  }
  assert.equal(
    ajvRawValid,
    testCase.valid,
    `Ajv raw-number parity disagreement: ${testCase.name}`,
  );

  let typescriptRawValid = true;
  try {
    parseArtifactDescriptorJson(rawJson);
  } catch {
    typescriptRawValid = false;
  }
  assert.equal(
    typescriptRawValid,
    testCase.valid,
    `TypeScript raw-number parity disagreement: ${testCase.name}`,
  );
}

for (const testCase of artifactCorpus.raw_json_cases) {
  let schemaRawValid = true;
  try {
    assertCanonicalArtifactJsonNumberTokens(testCase.raw_json);
    const candidate = JSON.parse(testCase.raw_json);
    schemaRawValid = validateArtifactSchema(candidate);
    if (schemaRawValid) {
      assertArtifactDescriptorSemantics(candidate);
    }
  } catch {
    schemaRawValid = false;
  }
  assert.equal(
    schemaRawValid,
    testCase.valid,
    `JSON Schema raw artifact parity disagreement: ${testCase.name}`,
  );

  let typescriptRawValid = true;
  try {
    parseArtifactDescriptorJson(testCase.raw_json);
  } catch {
    typescriptRawValid = false;
  }
  assert.equal(
    typescriptRawValid,
    testCase.valid,
    `TypeScript raw artifact parity disagreement: ${testCase.name}`,
  );
}

const assertRustArtifactUnknownFieldDenial = (source) => {
  for (const name of ["ArtifactRepresentation", "ArtifactDescriptor"]) {
    assert.match(
      source,
      new RegExp(
        `#\\[serde\\(deny_unknown_fields\\)\\]\\s*pub struct ${name}\\s*\\{`,
        "u",
      ),
      `${name} Rust typed deserialization must deny unknown properties`,
    );
  }
};
assertRustArtifactUnknownFieldDenial(rustDomainText);
for (const name of ["ArtifactRepresentation", "ArtifactDescriptor"]) {
  const mutated = rustDomainText.replace(
    `#[serde(deny_unknown_fields)]\npub struct ${name}`,
    `pub struct ${name}`,
  );
  assert.notEqual(mutated, rustDomainText, `${name} deny-attribute mutation must alter source`);
  assert.throws(
    () => assertRustArtifactUnknownFieldDenial(mutated),
    undefined,
    `${name} deny-attribute mutation must fail contract verification`,
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

for (const [invalidKind, invalidUuid] of [
  ["version-four", "00000000-0000-4000-8000-000000000000"],
  ["NCS-variant version-seven", "01900000-0000-7000-0000-000000000001"],
  ["Microsoft-variant version-seven", "01900000-0000-7000-c000-000000000001"],
  ["future-variant version-seven", "01900000-0000-7000-e000-000000000001"],
]) {
  for (const [version, fixture, validateFixtureSchema] of [
    [1, fixtureV1, validateFixtureSchemaV1],
    [2, fixtureV2, validateFixtureSchemaV2],
  ]) {
    const candidate = clone(fixture);
    candidate.device_id = invalidUuid;
    assert.equal(
      validateFixtureSchema(candidate),
      false,
      `v${version} schema accepted ${invalidKind} UUID`,
    );
    assert.throws(
      () => parseFixtureDocument(candidate),
      undefined,
      `v${version} TypeScript parser accepted ${invalidKind} UUID`,
    );
  }
  const artifactCandidate = clone(artifactCorpus.base);
  artifactCandidate.id = invalidUuid;
  assert.equal(
    validateArtifactSchema(artifactCandidate),
    false,
    `artifact schema accepted ${invalidKind} UUID`,
  );
  assert.throws(
    () => parseArtifactDescriptorJson(JSON.stringify(artifactCandidate)),
    undefined,
    `TypeScript artifact parser accepted ${invalidKind} UUID`,
  );
}

assert.equal(fixtureSchemaV1.$schema, "https://json-schema.org/draft/2020-12/schema");
assert.equal(fixtureSchemaV2.$schema, "https://json-schema.org/draft/2020-12/schema");
assert.equal(artifactSchema.$schema, "https://json-schema.org/draft/2020-12/schema");
assert.equal(
  syntheticManifestSchema.$schema,
  "https://json-schema.org/draft/2020-12/schema",
);
const maskRustCommentsAndLiterals = (source) => {
  const masked = source.split("");
  const blank = (index) => {
    if (masked[index] !== "\n" && masked[index] !== "\r") masked[index] = " ";
  };
  const blankRange = (start, end) => {
    for (let index = start; index < end; index += 1) blank(index);
  };
  let index = 0;
  while (index < source.length) {
    if (source.startsWith("//", index)) {
      const end = source.indexOf("\n", index + 2);
      const boundedEnd = end < 0 ? source.length : end;
      blankRange(index, boundedEnd);
      index = boundedEnd;
      continue;
    }
    if (source.startsWith("/*", index)) {
      const start = index;
      let depth = 1;
      index += 2;
      while (index < source.length && depth > 0) {
        if (source.startsWith("/*", index)) {
          depth += 1;
          index += 2;
        } else if (source.startsWith("*/", index)) {
          depth -= 1;
          index += 2;
        } else {
          index += 1;
        }
      }
      blankRange(start, index);
      continue;
    }
    const raw = source.slice(index).match(/^(?:br|cr|r)(?<hashes>#+)?"/u);
    if (raw !== null) {
      const hashes = raw.groups?.hashes ?? "";
      const terminator = `"${hashes}`;
      const end = source.indexOf(terminator, index + raw[0].length);
      const boundedEnd = end < 0 ? source.length : end + terminator.length;
      blankRange(index, boundedEnd);
      index = boundedEnd;
      continue;
    }
    const stringPrefix = source.startsWith('b"', index) || source.startsWith('c"', index)
      ? 2
      : source[index] === '"' ? 1 : 0;
    if (stringPrefix > 0) {
      const start = index;
      index += stringPrefix;
      while (index < source.length) {
        if (source[index] === "\\") {
          index += 2;
        } else if (source[index] === '"') {
          index += 1;
          break;
        } else {
          index += 1;
        }
      }
      blankRange(start, index);
      continue;
    }
    if (source[index] === "'") {
      let end = index + 1;
      let escaped = false;
      while (end < source.length && source[end] !== "\n" && source[end] !== "\r") {
        if (!escaped && source[end] === "'") break;
        escaped = !escaped && source[end] === "\\";
        if (source[end] !== "\\") escaped = false;
        end += 1;
      }
      if (source[end] === "'") {
        blankRange(index, end + 1);
        index = end + 1;
        continue;
      }
    }
    index += 1;
  }
  return masked.join("");
};
const stripRustAttributes = (prefix) => {
  let result = "";
  let index = 0;
  while (index < prefix.length) {
    if (prefix[index] === "#" && prefix[index + 1] === "[") {
      let depth = 1;
      index += 2;
      while (index < prefix.length && depth > 0) {
        if (prefix[index] === "[") depth += 1;
        if (prefix[index] === "]") depth -= 1;
        index += 1;
      }
    } else {
      result += prefix[index];
      index += 1;
    }
  }
  return result;
};
const rustFunctionEnd = (masked, fnIndex) => {
  const openingBrace = masked.indexOf("{", fnIndex);
  const declarationEnd = masked.indexOf(";", fnIndex);
  if (openingBrace < 0 || (declarationEnd >= 0 && declarationEnd < openingBrace)) {
    assert.ok(declarationEnd >= 0, "root Rust function declaration must terminate");
    return declarationEnd + 1;
  }
  let depth = 1;
  for (let index = openingBrace + 1; index < masked.length; index += 1) {
    if (masked[index] === "{") depth += 1;
    if (masked[index] === "}") depth -= 1;
    if (depth === 0) return index + 1;
  }
  assert.fail("root Rust function body must terminate");
};
const rustRootFunctionBodies = (source) => {
  const masked = maskRustCommentsAndLiterals(source);
  const functions = new Map();
  let itemStart = 0;
  let braceDepth = 0;
  let index = 0;
  while (index < masked.length) {
    const character = masked[index];
    if (braceDepth === 0 && character === "!" && masked[index + 1] !== "=") {
      assert.fail("unreviewed root Rust macros and includes are forbidden");
    }
    if (character === "{") {
      braceDepth += 1;
      index += 1;
      continue;
    }
    if (character === "}") {
      braceDepth -= 1;
      assert.ok(braceDepth >= 0, "Rust root item braces must remain balanced");
      index += 1;
      if (braceDepth === 0) itemStart = index;
      continue;
    }
    if (braceDepth === 0 && character === ";") {
      itemStart = index + 1;
      index += 1;
      continue;
    }
    const isIdentifierBefore = /[A-Za-z0-9_]/u.test(masked[index - 1] ?? "");
    const isIdentifierAfter = /[A-Za-z0-9_]/u.test(masked[index + 2] ?? "");
    if (
      braceDepth === 0 &&
      masked.startsWith("fn", index) &&
      !isIdentifierBefore &&
      !isIdentifierAfter
    ) {
      const nameMatch = masked.slice(index + 2).match(
        /^\s+(?<name>[a-z_][a-z0-9_]*)(?=\s*(?:<|\())/u,
      );
      assert.ok(
        nameMatch?.groups?.name !== undefined,
        "root Rust fn tokens must use the reviewed ASCII declaration grammar",
      );
      const name = nameMatch.groups.name;
      const prefix = stripRustAttributes(masked.slice(itemStart, index));
      const visibility = [...prefix.matchAll(/\bpub(?<restriction>\s*\([^)]*\))?/gu)].at(-1);
      const end = rustFunctionEnd(masked, index);
      assert.equal(functions.has(name), false, `duplicate root Rust function ${name}`);
      functions.set(name, {
        public: visibility !== undefined && visibility.groups?.restriction === undefined,
        source: source.slice(itemStart, end),
        start: itemStart,
        end,
      });
      itemStart = end;
      index = end;
      continue;
    }
    index += 1;
  }
  assert.equal(braceDepth, 0, "Rust root item braces must balance");
  return functions;
};
const maskedRustProductionSource = (source) => {
  const masked = maskRustCommentsAndLiterals(source);
  const output = masked.split("");
  const testModules = [...masked.matchAll(
    /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+tests\s*\{/gu,
  )];
  assert.equal(testModules.length, 1, "the reviewed Rust source must have one cfg(test) module");
  const testModule = testModules[0];
  const start = testModule.index ?? -1;
  assert.ok(start >= 0, "the reviewed cfg(test) module must have a source position");
  let rootDepth = 0;
  for (let index = 0; index < start; index += 1) {
    if (masked[index] === "{") rootDepth += 1;
    if (masked[index] === "}") rootDepth -= 1;
  }
  assert.equal(rootDepth, 0, "the reviewed cfg(test) module must be a root item");
  const openingBrace = start + testModule[0].lastIndexOf("{");
  let depth = 1;
  let end = -1;
  for (let index = openingBrace + 1; index < masked.length; index += 1) {
    if (masked[index] === "{") depth += 1;
    if (masked[index] === "}") depth -= 1;
    if (depth === 0) {
      end = index + 1;
      break;
    }
  }
  assert.ok(end >= 0, "the reviewed cfg(test) module must terminate");
  for (let index = start; index < end; index += 1) {
    if (output[index] !== "\n" && output[index] !== "\r") output[index] = " ";
  }
  return output.join("");
};
const rustRootPublicItems = (source) => {
  const masked = maskRustCommentsAndLiterals(source);
  const items = [];
  let braceDepth = 0;
  for (let index = 0; index < masked.length; index += 1) {
    if (masked[index] === "{") {
      braceDepth += 1;
      continue;
    }
    if (masked[index] === "}") {
      braceDepth -= 1;
      assert.ok(braceDepth >= 0, "Rust public-item braces must remain balanced");
      continue;
    }
    if (
      braceDepth !== 0 ||
      !masked.startsWith("pub", index) ||
      /[A-Za-z0-9_]/u.test(masked[index - 1] ?? "") ||
      /[A-Za-z0-9_]/u.test(masked[index + 3] ?? "")
    ) {
      continue;
    }
    const tail = masked.slice(index + 3);
    if (/^\s*\(/u.test(tail)) continue;
    const functionMatch = tail.match(
      /^\s+(?:(?:const|async|unsafe|extern)\s+)*fn\s+(?<name>[a-z_][a-z0-9_]*)(?=\s*(?:<|\())/u,
    );
    if (functionMatch?.groups?.name !== undefined) {
      items.push({ kind: "fn", name: functionMatch.groups.name, source: "" });
      continue;
    }
    const typeMatch = tail.match(
      /^\s+(?<kind>struct|enum)\s+(?<name>[A-Za-z_][A-Za-z0-9_]*)(?=\s*(?:<|where\b|\{|\(|;))/u,
    );
    if (typeMatch?.groups?.kind !== undefined && typeMatch.groups.name !== undefined) {
      items.push({ kind: typeMatch.groups.kind, name: typeMatch.groups.name, source: "" });
      continue;
    }
    const constantMatch = tail.match(
      /^\s+const\s+(?<name>[A-Za-z_][A-Za-z0-9_]*)(?=\s*(?::|=))/u,
    );
    if (constantMatch?.groups?.name !== undefined) {
      items.push({ kind: "const", name: constantMatch.groups.name, source: "" });
      continue;
    }
    if (/^\s+use\b/u.test(tail)) {
      const end = masked.indexOf(";", index);
      assert.ok(end >= 0, "root public Rust use must terminate");
      items.push({ kind: "use", name: "", source: source.slice(index, end + 1) });
      continue;
    }
    assert.fail("unreviewed root public Rust item is forbidden");
  }
  assert.equal(braceDepth, 0, "Rust public-item braces must balance");
  return items;
};
const rustRootImplMethods = (source) => {
  const masked = maskRustCommentsAndLiterals(source);
  const implementations = [];
  let rootDepth = 0;
  for (let index = 0; index < masked.length; index += 1) {
    if (masked[index] === "{") {
      rootDepth += 1;
      continue;
    }
    if (masked[index] === "}") {
      rootDepth -= 1;
      assert.ok(rootDepth >= 0, "Rust impl root braces must remain balanced");
      continue;
    }
    if (
      rootDepth !== 0 ||
      !masked.startsWith("impl", index) ||
      /[A-Za-z0-9_]/u.test(masked[index - 1] ?? "") ||
      /[A-Za-z0-9_]/u.test(masked[index + 4] ?? "")
    ) {
      continue;
    }
    const openingBrace = masked.indexOf("{", index + 4);
    const declarationEnd = masked.indexOf(";", index + 4);
    assert.ok(
      openingBrace >= 0 && (declarationEnd < 0 || openingBrace < declarationEnd),
      "reviewed root Rust impls must have bodies",
    );
    const typeMatch = masked
      .slice(index, openingBrace)
      .match(/^impl\s+(?<name>[A-Za-z_][A-Za-z0-9_]*)\s*$/u);
    assert.ok(
      typeMatch?.groups?.name !== undefined,
      "unreviewed generic or trait root Rust impl is forbidden",
    );
    let closingBrace = -1;
    let memberDepth = 1;
    let memberStart = openingBrace + 1;
    const methods = [];
    for (let cursor = openingBrace + 1; cursor < masked.length; cursor += 1) {
      const character = masked[cursor];
      if (memberDepth === 1 && character === "!" && masked[cursor + 1] !== "=") {
        assert.fail("unreviewed Rust impl macros and includes are forbidden");
      }
      if (
        memberDepth === 1 &&
        masked.startsWith("pub", cursor) &&
        !/[A-Za-z0-9_]/u.test(masked[cursor - 1] ?? "") &&
        !/[A-Za-z0-9_]/u.test(masked[cursor + 3] ?? "")
      ) {
        const tail = masked.slice(cursor + 3);
        if (!/^\s*\(/u.test(tail)) {
          assert.match(
            tail,
            /^\s+(?:(?:const|async|unsafe|extern)\s+)*fn\s+[a-z_][a-z0-9_]*(?=\s*(?:<|\())/u,
            "unreviewed public Rust impl item is forbidden",
          );
        }
      }
      const isIdentifierBefore = /[A-Za-z0-9_]/u.test(masked[cursor - 1] ?? "");
      const isIdentifierAfter = /[A-Za-z0-9_]/u.test(masked[cursor + 2] ?? "");
      if (
        memberDepth === 1 &&
        masked.startsWith("fn", cursor) &&
        !isIdentifierBefore &&
        !isIdentifierAfter
      ) {
        const nameMatch = masked.slice(cursor + 2).match(
          /^\s+(?<name>[a-z_][a-z0-9_]*)(?=\s*(?:<|\())/u,
        );
        assert.ok(
          nameMatch?.groups?.name !== undefined,
          "Rust impl fn tokens must use the reviewed ASCII declaration grammar",
        );
        const prefix = stripRustAttributes(masked.slice(memberStart, cursor));
        const visibility = [...prefix.matchAll(/\bpub(?<restriction>\s*\([^)]*\))?/gu)].at(-1);
        const end = rustFunctionEnd(masked, cursor);
        methods.push({
          name: nameMatch.groups.name,
          public: visibility !== undefined && visibility.groups?.restriction === undefined,
          source: source.slice(memberStart, end),
        });
        memberStart = end;
        cursor = end - 1;
        continue;
      }
      if (character === "{") memberDepth += 1;
      if (character === "}") {
        memberDepth -= 1;
        if (memberDepth === 0) {
          closingBrace = cursor;
          break;
        }
        if (memberDepth === 1) memberStart = cursor + 1;
      }
      if (memberDepth === 1 && character === ";") memberStart = cursor + 1;
    }
    assert.ok(closingBrace >= 0, "reviewed root Rust impl body must terminate");
    implementations.push({ name: typeMatch.groups.name, methods });
    index = closingBrace;
  }
  assert.equal(rootDepth, 0, "Rust impl root braces must balance");
  return implementations;
};
const rustSignatureTokens = (source) => {
  const masked = maskRustCommentsAndLiterals(source);
  const openingBrace = masked.indexOf("{");
  assert.ok(openingBrace >= 0, "reviewed Rust function signatures must have bodies");
  const signature = stripRustAttributes(masked.slice(0, openingBrace));
  return [...signature.matchAll(/->|::|[A-Za-z_][A-Za-z0-9_]*|[&<>()\[\],:]/gu)]
    .map((match) => match[0])
    .join(" ");
};
const assertExactRustSignatures = (declarations, expected, label) => {
  const actual = Object.fromEntries(
    [...declarations]
      .filter(([, declaration]) => declaration.public)
      .map(([name, declaration]) => [name, rustSignatureTokens(declaration.source)]),
  );
  assert.deepEqual(
    actual,
    Object.fromEntries(
      Object.entries(expected).map(([name, signature]) => [name, rustSignatureTokens(signature)]),
    ),
    `${label} public function signatures must remain token-exact`,
  );
};
const rustCrateImplInventory = (sources) => {
  const inventory = [];
  for (const [path, source] of sources) {
    const production = maskedRustProductionSource(source);
    for (const match of production.matchAll(/\bimpl\b/gu)) {
      const start = match.index;
      assert.ok(start !== undefined);
      const lineEnd = production.indexOf("\n", start);
      const line = production
        .slice(start, lineEnd < 0 ? production.length : lineEnd)
        .replace(/\s+/gu, " ")
        .trim();
      if (path === "proto_contract.rs" && line === "impl FnOnce(Uuid) -> Result<T, DomainError>,") {
        inventory.push({ path, kind: "opaque-parameter", header: line });
        continue;
      }
      const openingBrace = production.indexOf("{", start);
      assert.ok(openingBrace >= 0, `production impl token in ${path} must have a reviewed body`);
      inventory.push({
        path,
        kind: "block",
        header: production.slice(start, openingBrace).replace(/\s+/gu, " ").trim(),
      });
    }
  }
  return inventory;
};
const rustRootReferences = (source, functionNames) => {
  const masked = maskRustCommentsAndLiterals(source);
  const references = new Set();
  for (const match of masked.matchAll(/\b(?<name>[a-z_][a-z0-9_]*)\b/gu)) {
    const name = match.groups?.name;
    if (name === undefined || !functionNames.has(name)) continue;
    references.add(name);
  }
  return [...references];
};
const assertV2WriterCapabilityGate = (input) => {
  const sources = input instanceof Map
    ? input
    : new Map(rustContractsSources).set("lib.rs", input);
  assert.deepEqual(
    [...sources.keys()],
    ["lib.rs", "proto_contract.rs"],
    "academic-contracts source inventory must remain explicit and complete",
  );
  const source = sources.get("lib.rs");
  const protoSource = sources.get("proto_contract.rs");
  assert.ok(source !== undefined && protoSource !== undefined);
  for (const [path, contractSource] of sources) {
    assert.doesNotMatch(
      maskedRustProductionSource(contractSource),
      /#\s*\[\s*path\s*=/u,
      `external Rust module paths are forbidden in contracts source ${path}`,
    );
  }
  const functions = rustRootFunctionBodies(source);
  const publicItems = rustRootPublicItems(source);
  const implementations = rustRootImplMethods(source);
  const protoFunctions = rustRootFunctionBodies(protoSource);
  const protoPublicItems = rustRootPublicItems(protoSource);
  const publicFunctions = [...functions]
    .filter(([, declaration]) => declaration.public)
    .map(([name]) => name)
    .toSorted();
  assert.deepEqual(
    publicFunctions,
    [
      "decode_canonical_claim_object",
      "decode_canonical_evidence_ids",
      "decode_canonical_evidence_locator",
      "decode_unsigned_batch",
      "encode_canonical_actor",
      "encode_canonical_claim_object",
      "encode_canonical_event_payload",
      "encode_canonical_evidence_ids",
      "encode_canonical_evidence_locator",
      "encode_unsigned_batch",
      "sign_batch",
      "verify_signed_batch",
    ],
    "academic-contracts root public functions must match the reviewed capability allowlist",
  );
  assert.deepEqual(
    publicItems.filter((item) => item.kind === "fn").map((item) => item.name).toSorted(),
    publicFunctions,
    "every public root function must be covered by structural declaration analysis",
  );
  const publicTypes = publicItems
    .filter((item) => item.kind === "struct" || item.kind === "enum")
    .map((item) => item.name)
    .toSorted();
  assert.deepEqual(
    publicTypes,
    ["ContractError", "DeviceAuthorization", "VerifiedBatch"],
    "academic-contracts root public types must match the reviewed capability allowlist",
  );
  const publicConstants = publicItems
    .filter((item) => item.kind === "const")
    .map((item) => item.name);
  assert.deepEqual(
    publicConstants,
    ["SIGNED_ENVELOPE_VERSION"],
    "academic-contracts root public constants must match the reviewed capability allowlist",
  );
  const publicUses = publicItems.filter((item) => item.kind === "use");
  assert.equal(publicUses.length, 1, "the reviewed root public use count must remain exact");
  const reexports = publicUses[0]?.source.match(
    /pub use proto_contract::\{(?<names>[\s\S]*?)\};/u,
  )?.groups?.names
    .split(",")
    .map((name) => name.trim())
    .filter(Boolean)
    .toSorted();
  assert.deepEqual(
    reexports,
    [
      "ProtoContractError",
      "decode_claim_relation_event_proto",
      "encode_claim_relation_event_proto",
    ].toSorted(),
    "academic-contracts Proto reexports must match the reviewed capability allowlist",
  );
  assert.deepEqual(
    protoPublicItems.map((item) => ({ kind: item.kind, name: item.name })),
    [
      { kind: "enum", name: "ProtoContractError" },
      { kind: "fn", name: "encode_claim_relation_event_proto" },
      { kind: "fn", name: "decode_claim_relation_event_proto" },
    ],
    "every child-module public item must match the reviewed capability allowlist",
  );
  assertExactRustSignatures(
    functions,
    {
      sign_batch: "pub fn sign_batch(batch: &UnsignedBatch, signing_key: &SigningKey,) -> Result<Vec<u8>, ContractError> {}",
      verify_signed_batch: "pub fn verify_signed_batch(envelope_bytes: &[u8], authorization: &DeviceAuthorization,) -> Result<VerifiedBatch, ContractError> {}",
      encode_unsigned_batch: "pub fn encode_unsigned_batch(batch: &UnsignedBatch) -> Result<Vec<u8>, ContractError> {}",
      decode_unsigned_batch: "pub fn decode_unsigned_batch(bytes: &[u8]) -> Result<UnsignedBatch, ContractError> {}",
      encode_canonical_actor: "pub fn encode_canonical_actor(actor: &Actor) -> Result<Vec<u8>, ContractError> {}",
      encode_canonical_event_payload: "pub fn encode_canonical_event_payload(event: &Event) -> Result<Vec<u8>, ContractError> {}",
      encode_canonical_claim_object: "pub fn encode_canonical_claim_object(object: &ClaimObject) -> Result<Vec<u8>, ContractError> {}",
      decode_canonical_claim_object: "pub fn decode_canonical_claim_object(bytes: &[u8]) -> Result<ClaimObject, ContractError> {}",
      encode_canonical_evidence_ids: "pub fn encode_canonical_evidence_ids(ids: &[EvidenceId]) -> Result<Vec<u8>, ContractError> {}",
      decode_canonical_evidence_ids: "pub fn decode_canonical_evidence_ids(bytes: &[u8]) -> Result<Vec<EvidenceId>, ContractError> {}",
      encode_canonical_evidence_locator: "pub fn encode_canonical_evidence_locator(locator: &EvidenceLocator,) -> Result<Vec<u8>, ContractError> {}",
      decode_canonical_evidence_locator: "pub fn decode_canonical_evidence_locator(bytes: &[u8]) -> Result<EvidenceLocator, ContractError> {}",
    },
    "academic-contracts root",
  );
  assertExactRustSignatures(
    protoFunctions,
    {
      encode_claim_relation_event_proto: "pub fn encode_claim_relation_event_proto(event: &Event) -> Result<Vec<u8>, ProtoContractError> {}",
      decode_claim_relation_event_proto: "pub fn decode_claim_relation_event_proto(bytes: &[u8]) -> Result<Event, ProtoContractError> {}",
    },
    "academic-contracts Proto module",
  );
  assert.deepEqual(
    rustCrateImplInventory(sources),
    [
      { path: "lib.rs", kind: "block", header: "impl VerifiedBatch" },
      { path: "lib.rs", kind: "block", header: "impl DeviceAuthorization" },
      {
        path: "proto_contract.rs",
        kind: "opaque-parameter",
        header: "impl FnOnce(Uuid) -> Result<T, DomainError>,",
      },
    ],
    "crate-wide production impl blocks must match the reviewed allowlist",
  );
  assert.deepEqual(
    implementations.map((implementation) => ({
      name: implementation.name,
      methods: implementation.methods.map((method) => method.name),
      publicMethods: implementation.methods
        .filter((method) => method.public)
        .map((method) => method.name),
    })),
    [
      {
        name: "VerifiedBatch",
        methods: [
          "batch",
          "public_key",
          "source_schema_version",
          "source_envelope",
          "source_payload",
          "signature_bytes",
          "payload_hash",
          "envelope_hash",
        ],
        publicMethods: [
          "batch",
          "public_key",
          "source_schema_version",
          "source_envelope",
          "source_payload",
          "signature_bytes",
          "payload_hash",
          "envelope_hash",
        ],
      },
      {
        name: "DeviceAuthorization",
        methods: ["new", "device_id", "user_id", "verifying_key"],
        publicMethods: ["new", "device_id", "user_id", "verifying_key"],
      },
    ],
    "root impl blocks and their public/private method surfaces must match the reviewed allowlist",
  );
  const methodSignatures = Object.fromEntries(
    implementations.flatMap((implementation) => implementation.methods
      .filter((method) => method.public)
      .map((method) => [
        `${implementation.name}::${method.name}`,
        rustSignatureTokens(method.source),
      ])),
  );
  const expectedMethodSignatures = {
    "VerifiedBatch::batch": "pub const fn batch(&self) -> &UnsignedBatch {}",
    "VerifiedBatch::public_key": "pub const fn public_key(&self) -> &VerifyingKey {}",
    "VerifiedBatch::source_schema_version": "pub const fn source_schema_version(&self) -> u16 {}",
    "VerifiedBatch::source_envelope": "pub fn source_envelope(&self) -> &[u8] {}",
    "VerifiedBatch::source_payload": "pub fn source_payload(&self) -> &[u8] {}",
    "VerifiedBatch::signature_bytes": "pub const fn signature_bytes(&self) -> &[u8] {}",
    "VerifiedBatch::payload_hash": "pub const fn payload_hash(&self) -> ContentDigest {}",
    "VerifiedBatch::envelope_hash": "pub const fn envelope_hash(&self) -> ContentDigest {}",
    "DeviceAuthorization::new": "pub const fn new(device_id: DeviceId, user_id: EntityId, verifying_key: VerifyingKey) -> Self {}",
    "DeviceAuthorization::device_id": "pub const fn device_id(&self) -> DeviceId {}",
    "DeviceAuthorization::user_id": "pub const fn user_id(&self) -> EntityId {}",
    "DeviceAuthorization::verifying_key": "pub const fn verifying_key(&self) -> &VerifyingKey {}",
  };
  assert.deepEqual(
    methodSignatures,
    Object.fromEntries(
      Object.entries(expectedMethodSignatures)
        .map(([name, signature]) => [name, rustSignatureTokens(signature)]),
    ),
    "every public inherent method signature must remain token-exact",
  );
  const writerSource = functions.get("encode_unsigned_batch")?.source ?? "";
  assert.match(
    writerSource,
    /let bytes = encode_cbor_value\(&json_to_cbor\(&json\)\?\)\?;\s*require_current_writer_payload\(&bytes\)\?;\s*Ok\(bytes\)/u,
    "the current writer must semantically validate the exact bytes it returns",
  );
  const writerGuard = functions.get("require_current_writer_payload")?.source ?? "";
  assert.match(
    writerGuard,
    /let json = decode_canonical_payload_json\(bytes\)\?;\s*let source_schema_version = read_schema_version\(&json\)\?;\s*if source_schema_version != EVENT_SCHEMA_VERSION_V3 \{\s*return Err\(DomainError::UnsupportedSchemaVersion\(source_schema_version\)\.into\(\)\);\s*\}/u,
    "the writer guard must decode returned bytes and require semantic schema v3",
  );
  assert.match(
    functions.get("sign_batch")?.source ?? "",
    /let payload = encode_unsigned_batch\(batch\)\?;/u,
    "the signed writer must obtain its payload from the guarded current writer",
  );
  // v1 and v2 are both read-only source versions now, so each owns a private
  // projection reachable only through the same source-equality capability.
  const legacyProjectionNames = [
    "encode_unsigned_batch_v1_projection",
    "encode_unsigned_batch_v2_projection",
  ];
  for (const name of legacyProjectionNames) {
    const projection = functions.get(name)?.source;
    assert.ok(projection, `the private ${name} verification projection must remain explicitly named`);
    assert.match(
      projection,
      new RegExp(`fn ${name}\\([\\s\\S]*_capability: LegacySourceEqualityCapability,`, "u"),
      "legacy projection access must require the private source-equality capability",
    );
    assert.match(
      functions.get("require_source_typed_equality")?.source ?? "",
      new RegExp(`${name}\\(batch, LegacySourceEqualityCapability\\)`, "u"),
      "only authenticated source equality may construct and consume the legacy capability",
    );
  }
  const projectionDeclarations = legacyProjectionNames.map((name) => functions.get(name));
  const equalityFunction = functions.get("require_source_typed_equality");
  assert.ok(projectionDeclarations.every((declaration) => declaration !== undefined));
  assert.ok(equalityFunction !== undefined);
  const countProjectionIdentifiers = (text, name) => [
    ...maskRustCommentsAndLiterals(text).matchAll(new RegExp(`\\b${name}\\b`, "gu")),
  ].length;
  const countCapabilityIdentifiers = (text) => [
    ...maskRustCommentsAndLiterals(text).matchAll(/\bLegacySourceEqualityCapability\b/gu),
  ].length;
  for (const [index, name] of legacyProjectionNames.entries()) {
    const declaration = projectionDeclarations[index];
    assert.equal(countProjectionIdentifiers(declaration.source, name), 1);
    assert.equal(countCapabilityIdentifiers(declaration.source), 1);
    assert.equal(countProjectionIdentifiers(equalityFunction.source, name), 1);
  }
  assert.equal(
    countCapabilityIdentifiers(equalityFunction.source),
    legacyProjectionNames.length,
    "source equality constructs the capability exactly once per legacy source version",
  );
  const productionReview = maskedRustProductionSource(source).split("");
  for (const declaration of [...projectionDeclarations, equalityFunction]) {
    for (let index = declaration.start; index < declaration.end; index += 1) {
      if (productionReview[index] !== "\n" && productionReview[index] !== "\r") {
        productionReview[index] = " ";
      }
    }
  }
  const productionWithoutReviewedFunctions = productionReview.join("");
  const capabilityDeclarations = [...productionWithoutReviewedFunctions.matchAll(
    /^struct LegacySourceEqualityCapability;[ \t]*$/gmu,
  )];
  assert.equal(capabilityDeclarations.length, 1, "the private legacy capability declaration is exact");
  const capabilityDeclaration = capabilityDeclarations[0];
  const capabilityStart = capabilityDeclaration.index ?? -1;
  assert.ok(capabilityStart >= 0);
  for (
    let index = capabilityStart;
    index < capabilityStart + capabilityDeclaration[0].length;
    index += 1
  ) {
    if (productionReview[index] !== "\n" && productionReview[index] !== "\r") {
      productionReview[index] = " ";
    }
  }
  const unreviewedProduction = productionReview.join("");
  assert.doesNotMatch(
    unreviewedProduction,
    /\bencode_unsigned_batch_v1_projection\b/u,
    "the legacy projection identifier is forbidden outside its declaration and authenticated equality",
  );
  assert.doesNotMatch(
    unreviewedProduction,
    /\bLegacySourceEqualityCapability\b/u,
    "the legacy capability identifier is forbidden outside reviewed declaration/equality sites",
  );
  for (const [path, childSource] of sources) {
    if (path === "lib.rs") continue;
    const childProduction = maskedRustProductionSource(childSource);
    assert.doesNotMatch(
      childProduction,
      /\bencode_unsigned_batch_v1_projection\b/u,
      `the legacy projection identifier is forbidden in child module ${path}`,
    );
    assert.doesNotMatch(
      childProduction,
      /\bLegacySourceEqualityCapability\b/u,
      `the legacy capability identifier is forbidden in child module ${path}`,
    );
  }
  const callGraph = new Map([...functions].map(([name, declaration]) => [
    name,
    rustRootReferences(declaration.source, new Set(functions.keys()))
      .filter((callee) => callee !== name),
  ]));
  const reachableFromWriters = new Set();
  const pending = ["sign_batch", "encode_unsigned_batch"];
  while (pending.length > 0) {
    const current = pending.pop();
    if (current === undefined || reachableFromWriters.has(current)) continue;
    reachableFromWriters.add(current);
    pending.push(...(callGraph.get(current) ?? []));
  }
  assert.equal(
    reachableFromWriters.has("encode_unsigned_batch_v1_projection"),
    false,
    "current writer data flow must never reach the legacy v1 projection capability",
  );
  for (const legacyImport of [
    "encode_unsigned_batch_v1_projection",
    "encode_legacy_projection",
  ]) {
    assert.ok(
      source.includes([
        "//! ```compile_fail",
        `//! use academic_contracts::${legacyImport};`,
        "//! ```",
      ].join("\n")),
      `downstream compile-fail evidence must pin private import ${legacyImport}`,
    );
  }
};
assertV2WriterCapabilityGate(rustContractsText);
const childModulePublicLegacyWriter = rustProtoContractText.replace(
  "#[cfg(test)]\nmod tests {",
  [
    "impl super::DeviceAuthorization {",
    "    pub fn encode_archived_batch(",
    "        &self,",
    "        batch: &academic_domain::UnsignedBatch,",
    "    ) -> Result<Vec<u8>, super::ContractError> {",
    "        super::encode_unsigned_batch_v1_projection(",
    "            batch,",
    "            super::LegacySourceEqualityCapability,",
    "        )",
    "    }",
    "}",
    "",
    "#[cfg(test)]",
    "mod tests {",
  ].join("\n"),
);
assert.notEqual(childModulePublicLegacyWriter, rustProtoContractText);
assert.throws(
  () => assertV2WriterCapabilityGate(
    new Map(rustContractsSources).set("proto_contract.rs", childModulePublicLegacyWriter),
  ),
  undefined,
  "a public inherent v1 writer declared in a child module must fail crate-wide review",
);
const publicSignatureConstnessDrift = rustContractsText.replace(
  "    pub const fn new(device_id: DeviceId, user_id: EntityId, verifying_key: VerifyingKey) -> Self {",
  "    pub fn new(device_id: DeviceId, user_id: EntityId, verifying_key: VerifyingKey) -> Self {",
);
assert.notEqual(publicSignatureConstnessDrift, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(publicSignatureConstnessDrift),
  /public inherent method signature must remain token-exact/u,
  "public API constness drift must fail signature-exact review",
);
const sealedSourceAccessorSignatureDrift = rustContractsText.replace(
  "    pub fn source_envelope(&self) -> &[u8] {",
  "    pub fn source_envelope(&self, normalized: bool) -> &[u8] {",
);
assert.notEqual(sealedSourceAccessorSignatureDrift, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(sealedSourceAccessorSignatureDrift),
  /public inherent method signature must remain token-exact/u,
  "sealed source accessor signature drift must fail signature-exact review",
);
const unrelatedCanonicalCapability = rustContractsText.replace(
  "\n#[cfg(test)]\nmod tests {",
  "\npub fn encode_unreviewed_value() -> Vec<u8> { Vec::new() }\n\n#[cfg(test)]\nmod tests {",
);
assert.notEqual(unrelatedCanonicalCapability, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(unrelatedCanonicalCapability),
  /public functions must match the reviewed capability allowlist/u,
  "an unrelated public canonical capability must fail the allowlist",
);
const neutralV1CloneReachedByWriter = rustContractsText
  .replace(
    "fn encode_unsigned_batch_v1_projection(",
    [
      "fn neutral_projection_clone(batch: &UnsignedBatch) -> Result<Vec<u8>, ContractError> {",
      "    batch.validate()?;",
      "    let mut json = serde_json::to_value(batch)?;",
      "    transform_decisions_for_v1(&mut json)?;",
      "    set_schema_version(&mut json, EVENT_SCHEMA_VERSION_V1)?;",
      "    encode_cbor_value(&json_to_cbor(&json)?)",
      "}",
      "",
      "fn encode_unsigned_batch_v1_projection(",
    ].join("\n"),
  )
  .replace(
    "    let bytes = encode_cbor_value(&json_to_cbor(&json)?)?;",
    "    let bytes = neutral_projection_clone(batch)?;",
  );
assert.notEqual(neutralV1CloneReachedByWriter, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(neutralV1CloneReachedByWriter),
  /current writer must semantically validate the exact bytes it returns/u,
  "a neutrally named v1 transform reached by the current writer must fail semantic review",
);
const additionalContractModule = new Map(rustContractsSources).set(
  "unreviewed.rs",
  "pub fn encode_archived_batch() {}\n#[cfg(test)]\nmod tests {}\n",
);
assert.throws(
  () => assertV2WriterCapabilityGate(additionalContractModule),
  /source inventory must remain explicit and complete/u,
  "an added crate source module must fail closed until its public surface is reviewed",
);
const renamedPublicLegacyWriter = rustContractsText
  .replaceAll("encode_unsigned_batch_v1_projection", "encode_legacy_projection")
  .replace("fn encode_legacy_projection(", "pub fn encode_legacy_projection(");
assert.notEqual(renamedPublicLegacyWriter, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(renamedPublicLegacyWriter),
  undefined,
  "a renamed public legacy projection must fail the public API allowlist",
);
const writerReachesLegacyProjection = rustContractsText.replace(
  "    batch.validate()?;\n    let json = serde_json::to_value(batch)?;",
  "    let _legacy = encode_unsigned_batch_v1_projection(batch, LegacySourceEqualityCapability)?;\n    batch.validate()?;\n    let json = serde_json::to_value(batch)?;",
);
assert.notEqual(writerReachesLegacyProjection, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(writerReachesLegacyProjection),
  undefined,
  "writer data flow reaching the legacy capability must fail contract verification",
);
const genericWriterBridge = rustContractsText
  .replace(
    "fn encode_unsigned_batch_v1_projection(",
    [
      "fn generic_legacy_bridge<T>(batch: &UnsignedBatch) -> Result<Vec<u8>, ContractError>",
      "where",
      "    T: Copy,",
      "{",
      "    let _marker = std::marker::PhantomData::<T>;",
      "    encode_unsigned_batch_v1_projection(batch, LegacySourceEqualityCapability)",
      "}",
      "",
      "fn encode_unsigned_batch_v1_projection(",
    ].join("\n"),
  )
  .replace(
    "    batch.validate()?;\n    let json = serde_json::to_value(batch)?;",
    "    let _legacy = generic_legacy_bridge::<u8>(batch)?;\n    batch.validate()?;\n    let json = serde_json::to_value(batch)?;",
  );
assert.notEqual(genericWriterBridge, rustContractsText);
assert.ok(
  rustRootFunctionBodies(genericWriterBridge).has("generic_legacy_bridge"),
  "the structural Rust declaration parser must discover private generic bridges",
);
assert.throws(
  () => assertV2WriterCapabilityGate(genericWriterBridge),
  undefined,
  "a current writer reaching the legacy projection through a generic turbofish bridge must fail",
);
const functionValueWriterBridge = rustContractsText
  .replace(
    "fn encode_unsigned_batch_v1_projection(",
    [
      "fn function_value_legacy_bridge(",
      "    batch: &UnsignedBatch,",
      ") -> Result<Vec<u8>, ContractError> {",
      "    let project = encode_unsigned_batch_v1_projection;",
      "    project(batch, LegacySourceEqualityCapability)",
      "}",
      "",
      "fn encode_unsigned_batch_v1_projection(",
    ].join("\n"),
  )
  .replace(
    "    batch.validate()?;\n    let json = serde_json::to_value(batch)?;",
    "    let _legacy = function_value_legacy_bridge(batch)?;\n    batch.validate()?;\n    let json = serde_json::to_value(batch)?;",
  );
assert.notEqual(functionValueWriterBridge, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(functionValueWriterBridge),
  undefined,
  "a writer reaching the legacy projection through a function value must fail",
);
const traitMethodWriterBridge = rustContractsText
  .replace(
    "fn encode_unsigned_batch_v1_projection(",
    [
      "trait LegacyBridge {",
      "    fn legacy_bridge(&self) -> Result<Vec<u8>, ContractError>;",
      "}",
      "",
      "impl LegacyBridge for UnsignedBatch {",
      "    fn legacy_bridge(&self) -> Result<Vec<u8>, ContractError> {",
      "        encode_unsigned_batch_v1_projection(self, LegacySourceEqualityCapability)",
      "    }",
      "}",
      "",
      "fn encode_unsigned_batch_v1_projection(",
    ].join("\n"),
  )
  .replace(
    "    batch.validate()?;\n    let json = serde_json::to_value(batch)?;",
    "    let _legacy = batch.legacy_bridge()?;\n    batch.validate()?;\n    let json = serde_json::to_value(batch)?;",
  );
assert.notEqual(traitMethodWriterBridge, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(traitMethodWriterBridge),
  undefined,
  "a writer reaching the legacy projection through a private trait method must fail",
);
const genericPublicLegacyWrapper = rustContractsText.replace(
  "fn encode_unsigned_batch_v1_projection(",
  [
    "#[inline]",
    "pub fn encode_archived<T>(",
    "    batch: &UnsignedBatch,",
    "    _marker: std::marker::PhantomData<T>,",
    ") -> Result<Vec<u8>, ContractError>",
    "where",
    "    T: Copy,",
    "{",
    "    encode_unsigned_batch_v1_projection(batch, LegacySourceEqualityCapability)",
    "}",
    "",
    "fn encode_unsigned_batch_v1_projection(",
  ].join("\n"),
);
assert.notEqual(genericPublicLegacyWrapper, rustContractsText);
assert.equal(
  rustRootFunctionBodies(genericPublicLegacyWrapper).get("encode_archived")?.public,
  true,
  "the structural Rust declaration parser must discover attributed generic qualified public wrappers",
);
assert.throws(
  () => assertV2WriterCapabilityGate(genericPublicLegacyWrapper),
  undefined,
  "an attributed generic public wrapper around the legacy projection must fail the official gate",
);
const commentedCfgTestGenericPublicWrapper = genericPublicLegacyWrapper.replace(
  "#[inline]\npub fn encode_archived<T>(",
  "// #[cfg(test)]\n#[inline]\npub fn encode_archived<T>(",
);
assert.notEqual(commentedCfgTestGenericPublicWrapper, genericPublicLegacyWrapper);
assert.equal(
  rustRootFunctionBodies(commentedCfgTestGenericPublicWrapper).get("encode_archived")?.public,
  true,
  "a cfg-test token inside a line comment must not truncate root declaration analysis",
);
assert.throws(
  () => assertV2WriterCapabilityGate(commentedCfgTestGenericPublicWrapper),
  undefined,
  "a commented cfg-test token must not hide a compiling public generic legacy wrapper",
);
for (const [name, mutation] of [
  [
    "raw identifier public wrapper",
    genericPublicLegacyWrapper.replace(
      "pub fn encode_archived<T>(",
      "pub fn r#encode_archived<T>(",
    ),
  ],
  [
    "Unicode identifier public wrapper",
    genericPublicLegacyWrapper.replace(
      "pub fn encode_archived<T>(",
      "pub fn 归档编码<T>(",
    ),
  ],
]) {
  assert.notEqual(mutation, genericPublicLegacyWrapper, `${name} mutation must alter the source`);
  assert.throws(
    () => assertV2WriterCapabilityGate(mutation),
    undefined,
    `${name} must fail closed instead of escaping the public function allowlist`,
  );
}
const unicodePublicType = rustContractsText.replaceAll("VerifiedBatch", "VerifiedBatché");
assert.notEqual(unicodePublicType, rustContractsText);
assert.throws(
  () => rustRootPublicItems(unicodePublicType),
  /unreviewed root public Rust item is forbidden/u,
  "an ASCII-prefix Unicode public type must not be tokenized as the reviewed ASCII type",
);
assert.throws(
  () => assertV2WriterCapabilityGate(unicodePublicType),
  undefined,
  "a compile-valid Unicode public type rename must fail the exact public-item allowlist",
);
const macroGeneratedLegacyWrapper = rustContractsText.replace(
  "fn encode_unsigned_batch_v1_projection(",
  [
    "macro_rules! expose_legacy_writer {",
    "    () => {",
    "        pub fn encode_archived(",
    "            batch: &UnsignedBatch,",
    "        ) -> Result<Vec<u8>, ContractError> {",
    "            encode_unsigned_batch_v1_projection(batch, LegacySourceEqualityCapability)",
    "        }",
    "    };",
    "}",
    "expose_legacy_writer!();",
    "",
    "fn encode_unsigned_batch_v1_projection(",
  ].join("\n"),
);
assert.notEqual(macroGeneratedLegacyWrapper, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(macroGeneratedLegacyWrapper),
  /unreviewed root Rust macros and includes are forbidden/u,
  "a compile-valid root macro-generated public writer must fail closed",
);
const rootIncludeMutation = rustContractsText.replace(
  "fn encode_unsigned_batch_v1_projection(",
  [
    "const _REVIEWED_SOURCE_BYTES: &[u8] = include_bytes!(\"lib.rs\");",
    "",
    "fn encode_unsigned_batch_v1_projection(",
  ].join("\n"),
);
assert.notEqual(rootIncludeMutation, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(rootIncludeMutation),
  /unreviewed root Rust macros and includes are forbidden/u,
  "a compile-valid unreviewed root include path must fail closed",
);
const publicModuleMutation = rustContractsText.replace(
  "fn encode_unsigned_batch_v1_projection(",
  "pub mod unreviewed_export {}\n\nfn encode_unsigned_batch_v1_projection(",
);
assert.notEqual(publicModuleMutation, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(publicModuleMutation),
  /unreviewed root public Rust item is forbidden/u,
  "an unreviewed public root module must fail closed",
);
const publicImplLegacyWrapper = rustContractsText.replace(
  "impl VerifiedBatch {",
  [
    "impl VerifiedBatch {",
    "    #[inline]",
    "    pub fn encode_archived<T>(",
    "        batch: &UnsignedBatch,",
    "        _marker: std::marker::PhantomData<T>,",
    "    ) -> Result<Vec<u8>, ContractError>",
    "    where",
    "        T: Copy,",
    "    {",
    "        encode_unsigned_batch_v1_projection(batch, LegacySourceEqualityCapability)",
    "    }",
  ].join("\n"),
);
assert.notEqual(publicImplLegacyWrapper, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(publicImplLegacyWrapper),
  undefined,
  "a compile-valid public generic legacy writer in a reviewed impl must fail the method allowlist",
);
const allowedMethodBodyLegacyCall = rustContractsText.replace(
  "    pub const fn batch(&self) -> &UnsignedBatch {\n        &self.batch",
  [
    "    pub fn batch(&self) -> &UnsignedBatch {",
    "        let _legacy = encode_unsigned_batch_v1_projection(",
    "            &self.batch,",
    "            LegacySourceEqualityCapability,",
    "        );",
    "        &self.batch",
  ].join("\n"),
);
assert.notEqual(allowedMethodBodyLegacyCall, rustContractsText);
assert.deepEqual(
  rustRootImplMethods(allowedMethodBodyLegacyCall).map((implementation) => ({
    name: implementation.name,
    methods: implementation.methods.map((method) => method.name),
  })),
  rustRootImplMethods(rustContractsText).map((implementation) => ({
    name: implementation.name,
    methods: implementation.methods.map((method) => method.name),
  })),
  "the allowed-method-body mutation must retain the reviewed impl/method names",
);
assert.throws(
  () => assertV2WriterCapabilityGate(allowedMethodBodyLegacyCall),
  undefined,
  "a compile-valid legacy projection call injected into an allowed method must fail exact crate review",
);
const extraPublicReexport = rustContractsText.replace(
  "pub use proto_contract::{",
  [
    "pub use academic_domain::UnsignedBatch as ArchivedBatch;",
    "",
    "pub use proto_contract::{",
  ].join("\n"),
);
assert.notEqual(extraPublicReexport, rustContractsText);
assert.throws(
  () => assertV2WriterCapabilityGate(extraPublicReexport),
  undefined,
  "an additional compile-valid public root re-export must fail the exact use allowlist",
);
assert.doesNotMatch(
  rustCoreText,
  /build_fixture_document_for_version|sign_batch_v1/u,
  "academic-core must not offer version-selectable or v1 fixture emission",
);
for (const [label, text] of [
  ["crates/cli/src/main.rs", rustCliText],
  ["crates/cli/src/commands/fixture.rs", rustCliFixtureText],
]) {
  assert.doesNotMatch(
    text.split(/\n#\[cfg\(test\)\]/u, 1)[0],
    /fixture_version|fixture-version/u,
    `the production-facing CLI must emit only the current v2 fixture (${label})`,
  );
}
const lockedCargoRegistryFetch = "cargo fetch --locked";
const lockfileCacheActionReference = "actions/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9";
// Hosted labels observed to schedule AND build clean. Each label carries one
// admission platform triple, so the set is closed: ubuntu-latest linux-x86_64,
// ubuntu-24.04-arm linux-aarch64, windows-latest windows-x86_64, windows-11-arm
// windows-aarch64, macos-latest macos-aarch64. macos-latest resolves to Apple
// Silicon; a macOS label that resolves to x86_64 does not carry the triple.
const hostedRustMatrixLabels = [
  "ubuntu-latest",
  "ubuntu-24.04-arm",
  "windows-latest",
  "windows-11-arm",
  "macos-latest",
];
// v1 and v2 are both read-only compatibility goldens; only v3 is emitted. The
// drift check covers the whole fixture directory rather than two named files,
// so a newly frozen golden cannot be added without also being held immutable.
const nativeFixtureCiCommands = [
  "cargo run --locked --quiet -p academic-cli -- fixture verify schemas/fixtures/signed-batch-v1.json",
  "cargo run --locked --quiet -p academic-cli -- fixture replay schemas/fixtures/signed-batch-v1.json",
  "cargo run --locked --quiet -p academic-cli -- fixture verify schemas/fixtures/signed-batch-v2.json",
  "cargo run --locked --quiet -p academic-cli -- fixture replay schemas/fixtures/signed-batch-v2.json",
  "cargo run --locked --quiet -p academic-cli -- fixture emit --output schemas/fixtures/signed-batch-v3.json",
  "git diff --exit-code -- schemas/fixtures/",
  "cargo run --locked --quiet -p academic-cli -- fixture verify schemas/fixtures/signed-batch-v3.json",
  "cargo run --locked --quiet -p academic-cli -- fixture replay schemas/fixtures/signed-batch-v3.json",
];
// The exit's platform claims are the Windows named-pipe endpoint and the Unix
// domain socket, so its matrix is exactly the two labels that carry one of
// those. macOS stays an open gate rather than an included label.
const phase1ExitMatrixLabels = ["ubuntu-latest", "windows-latest"];
// The fault lane is selected by naming each owning crate's non-default feature.
// `academic-daemon` forwards to `academic-core`, which forwards to the three
// crates that own failpoints, so the daemon feature alone would compile the
// lane; the full list is spelled out because clippy runs `--workspace` and each
// crate's own test targets need their own feature selected too.
const phase1FaultFeatureSelection = [
  "academic-core/phase1-fault-injection",
  "academic-daemon/phase1-fault-injection",
  "academic-portability/phase1-fault-injection",
  "academic-projections/phase1-fault-injection",
  "academic-test-support/phase1-fault-injection",
  "academic-vault/phase1-fault-injection",
].join(",");
const phase1ExitCiCommands = [
  `cargo clippy --workspace --all-targets --locked --features ${phase1FaultFeatureSelection} -- -D warnings`,
  "cargo test -p academic-daemon --test phase1_exit --locked --features phase1-fault-injection",
  // The exit corpus cannot reach BK03, so its NOT_RUN row points at the kill
  // matrices in the owner crates. Those suites compile to nothing under default
  // features, so the exit job has to run them itself or the pointer is not
  // evidence.
  "cargo test -p academic-portability -p academic-vault --test crash --locked --offline --features phase1-fault-injection",
  "node tools/phase1-exit.mjs --all-faults --format json",
];
const requireCiRecord = (value, label) => {
  assert.ok(
    typeof value === "object" && value !== null && !Array.isArray(value),
    `${label} must be a mapping`,
  );
  return value;
};
const requireCiSteps = (job, label) => {
  assert.ok(Array.isArray(job.steps), `${label}.steps must be a sequence`);
  return job.steps.map((step, index) => requireCiRecord(step, `${label}.steps[${index}]`));
};
const assertUnconditionalRequiredExecution = (value, label) => {
  assert.equal(
    Object.hasOwn(value, "if"),
    false,
    `${label} must not declare a condition`,
  );
  assert.equal(
    Object.hasOwn(value, "continue-on-error"),
    false,
    `${label} must not tolerate failure`,
  );
};
// t068 section 3.4's AEAD_CHUNKED_V2 lane. It is a non-default vault feature,
// so `cargo clippy --workspace` and `cargo test --workspace` never build it;
// without these two steps the lane would be linted and tested only on a
// developer's machine. Both run on every hosted Rust label, because the object
// format's durability boundary is per-platform.
const encryptedObjectCiCommands = [
  "cargo clippy -p academic-vault --all-targets --locked --features aead-objects,phase2-fault-injection -- -D warnings",
  "cargo test -p academic-vault --all-targets --locked --features aead-objects,phase2-fault-injection",
];
// t068 section 5's `P2-K5` rotation and retention lane, for the same reason:
// `rotation-engine` is non-default, so a workspace build never reaches the
// half of it that rewraps and shreds real objects. `phase2-fault-injection` is
// selected too because `KY03`-`KY05` and `RB01`-`RB02` are process kills whose
// failpoints exist only under it. Both run on every hosted Rust label, because
// the key-slot write and the recipient-set rename are per-platform.
const rotationEngineCiCommands = [
  "cargo clippy -p academic-retention --all-targets --locked --features rotation-engine,phase2-fault-injection -- -D warnings",
  "cargo test -p academic-retention --all-targets --locked --features rotation-engine,phase2-fault-injection",
];
// The encrypted store lane. It is the executor of `P2-K5`'s store-database
// rotation unit and the home of `EN01` ("kill mid store rekey; exactly one of
// the old and new keys opens the database"), which is the byte-level half of
// the rotation invariant. Before `P2-K5` no hosted job built this lane, so
// `EN01` was a pointer rather than evidence; this job runs it.
//
// Linux only, and that is a claim about the toolchain rather than about the
// lane: `openssl-src` needs a native Perl that the hosted Windows image does
// not carry, which t068 section 2.3-17 already records. Native Windows stays
// the README-documented local lane with its pinned interpreter.
const encryptedStoreCiCommands = [
  "cargo clippy -p academic-store --no-default-features --features sqlcipher-store --all-targets --locked -- -D warnings",
  "cargo test -p academic-store --no-default-features --features sqlcipher-store --locked",
  "cargo test -p academic-store --no-default-features --features sqlcipher-store --locked --test encrypted_profile encrypted::store_rekey_kill_leaves_exactly_one_working_key -- --exact",
];
const encryptedStoreMatrixLabels = ["ubuntu-latest"];
// The encrypted portability lane. It exists so the seam where `P2-K5`'s
// rotation and deletion meet `P2-K2`'s store and `P2-K4`'s backup and restore
// is executed evidence: `T111` found a named acceptance row there that
// imitated a restore and passed while the product restore applied no
// tombstone, and no hosted job built this lane at all. Linux only, for the
// same `openssl-src` toolchain reason as the store lane.
const encryptedPortabilityCiCommands = [
  "cargo clippy -p academic-portability --no-default-features --features encrypted-portability --all-targets --locked -- -D warnings",
  "cargo test -p academic-portability --no-default-features --features encrypted-portability --locked",
  "cargo test -p academic-portability --no-default-features --features encrypted-portability,phase2-fault-injection --locked --test encrypted_crash",
];
const encryptedPortabilityMatrixLabels = ["ubuntu-latest"];
const parseCiWorkflow = (ci) => parsePnpmLockYaml(ci, ".github/workflows/ci.yml");
const expectedCiWorkflow = {
  name: "ci",
  on: {
    pull_request: null,
    push: { branches: ["main"] },
    workflow_dispatch: null,
  },
  permissions: { contents: "read" },
  concurrency: {
    group: "ci-${{ github.workflow }}-${{ github.ref }}",
    "cancel-in-progress": true,
  },
  jobs: {
    "source-preflight": {
      name: "dependency-source-preflight",
      "runs-on": "ubuntu-latest",
      "timeout-minutes": 5,
      steps: [
        {
          name: "Checkout without persisted credentials",
          uses: "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
          with: { "persist-credentials": false },
        },
        {
          name: "Reject forbidden dependency sources before tool or dependency setup",
          run: "node tools/source-preflight.mjs",
        },
      ],
    },
    rust: {
      name: "rust-${{ matrix.os }}",
      needs: "source-preflight",
      "runs-on": "${{ matrix.os }}",
      "timeout-minutes": 20,
      strategy: {
        "fail-fast": false,
        matrix: { os: hostedRustMatrixLabels },
      },
      steps: [
        {
          name: "Checkout without persisted credentials",
          uses: "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
          with: { "persist-credentials": false },
        },
        {
          name: "Install pinned Rust toolchain",
          run: "rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy",
        },
        {
          name: "Restore the Cargo registry keyed on the committed Cargo lockfile",
          uses: lockfileCacheActionReference,
          with: {
            path: "~/.cargo/registry",
            key: "cargo-registry-rust-${{ matrix.os }}-${{ hashFiles('Cargo.lock') }}",
          },
        },
        {
          name: "Populate the Cargo registry from the committed lockfile",
          run: lockedCargoRegistryFetch,
        },
        {
          name: "Install pinned Node",
          uses: "actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020",
          with: { "node-version-file": ".nvmrc" },
        },
        { name: "Install pinned pnpm", run: "npm install --global pnpm@11.22.0" },
        { name: "Check formatting", run: "cargo fmt --all -- --check" },
        {
          name: "Lint all Rust targets",
          run: "cargo clippy --workspace --all-targets --locked -- -D warnings",
        },
        { name: "Test Rust workspace", run: "cargo test --workspace --locked" },
        {
          name: "Lint the encrypted object lane",
          run: encryptedObjectCiCommands[0],
        },
        {
          name: "Test the encrypted object lane",
          run: encryptedObjectCiCommands[1],
        },
        {
          name: "Lint the rotation and retention lane",
          run: rotationEngineCiCommands[0],
        },
        {
          name: "Test the rotation and retention lane",
          run: rotationEngineCiCommands[1],
        },
        {
          name: "Verify immutable v1 fixture and upcast",
          run: nativeFixtureCiCommands[0],
        },
        { name: "Replay immutable v1 fixture", run: nativeFixtureCiCommands[1] },
        {
          name: "Verify immutable v2 fixture and upcast",
          run: nativeFixtureCiCommands[2],
        },
        { name: "Replay immutable v2 fixture", run: nativeFixtureCiCommands[3] },
        { name: "Emit deterministic v3 fixture", run: nativeFixtureCiCommands[4] },
        { name: "Reject fixture byte drift", run: nativeFixtureCiCommands[5] },
        { name: "Verify deterministic v3 fixture", run: nativeFixtureCiCommands[6] },
        { name: "Replay deterministic v3 fixture", run: nativeFixtureCiCommands[7] },
      ],
    },
    // The Phase 1 crash, replay, and restore exit. Its matrix is a two-label
    // set rather than the full hosted Rust matrix: the exit's platform claims
    // are about the Windows named-pipe endpoint and the Unix domain socket, and
    // a label that carries neither adds no evidence. macOS remains an open gate
    // rather than a silently included one.
    "phase1-exit": {
      name: "phase1-exit-${{ matrix.os }}",
      needs: "source-preflight",
      "runs-on": "${{ matrix.os }}",
      "timeout-minutes": 45,
      strategy: {
        "fail-fast": false,
        matrix: { os: phase1ExitMatrixLabels },
      },
      steps: [
        {
          name: "Checkout without persisted credentials",
          uses: "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
          with: { "persist-credentials": false },
        },
        {
          name: "Install pinned Rust toolchain",
          run: "rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy",
        },
        {
          name: "Restore the Cargo registry keyed on the committed Cargo lockfile",
          uses: lockfileCacheActionReference,
          with: {
            path: "~/.cargo/registry",
            key: "cargo-registry-phase1-exit-${{ matrix.os }}-${{ hashFiles('Cargo.lock') }}",
          },
        },
        {
          name: "Populate the Cargo registry from the committed lockfile",
          run: lockedCargoRegistryFetch,
        },
        {
          name: "Install pinned Node",
          uses: "actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020",
          with: { "node-version-file": ".nvmrc" },
        },
        { name: "Install pinned pnpm", run: "npm install --global pnpm@11.22.0" },
        { name: "Lint the fault-injection lane", run: phase1ExitCiCommands[0] },
        { name: "Run the enumerated Phase 1 exit matrix", run: phase1ExitCiCommands[1] },
        {
          name: "Run the kill matrices the NOT_RUN rows are covered by",
          run: phase1ExitCiCommands[2],
        },
        { name: "Assemble the Phase 1 exit receipt", run: phase1ExitCiCommands[3] },
      ],
    },
    // The encrypted store lane, on Linux. It exists so `EN01` is executed
    // evidence rather than a citation: `P2-K5`'s rotation journal carries a
    // store-database unit whose executor is this lane's `PRAGMA rekey`, and a
    // covering suite no job runs is the defect `A3` found once already.
    "encrypted-store-lane": {
      name: "encrypted-store-lane-${{ matrix.os }}",
      needs: "source-preflight",
      "runs-on": "${{ matrix.os }}",
      "timeout-minutes": 45,
      strategy: {
        "fail-fast": false,
        matrix: { os: encryptedStoreMatrixLabels },
      },
      steps: [
        {
          name: "Checkout without persisted credentials",
          uses: "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
          with: { "persist-credentials": false },
        },
        {
          name: "Install pinned Rust toolchain",
          run: "rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy",
        },
        {
          name: "Restore the Cargo registry keyed on the committed Cargo lockfile",
          uses: lockfileCacheActionReference,
          with: {
            path: "~/.cargo/registry",
            key: "cargo-registry-encrypted-store-${{ matrix.os }}-${{ hashFiles('Cargo.lock') }}",
          },
        },
        {
          name: "Populate the Cargo registry from the committed lockfile",
          run: lockedCargoRegistryFetch,
        },
        {
          name: "Lint the encrypted store lane",
          run: encryptedStoreCiCommands[0],
        },
        {
          name: "Test the encrypted store lane",
          run: encryptedStoreCiCommands[1],
        },
        {
          name: "Run EN01, the store-rekey kill the rotation journal depends on",
          run: encryptedStoreCiCommands[2],
        },
      ],
    },
    "encrypted-portability-lane": {
      name: "encrypted-portability-lane-${{ matrix.os }}",
      needs: "source-preflight",
      "runs-on": "${{ matrix.os }}",
      "timeout-minutes": 45,
      strategy: {
        "fail-fast": false,
        matrix: { os: encryptedPortabilityMatrixLabels },
      },
      steps: [
        {
          name: "Checkout without persisted credentials",
          uses: "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
          with: { "persist-credentials": false },
        },
        {
          name: "Install pinned Rust toolchain",
          run: "rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy",
        },
        {
          name: "Restore the Cargo registry keyed on the committed Cargo lockfile",
          uses: lockfileCacheActionReference,
          with: {
            path: "~/.cargo/registry",
            key: "cargo-registry-encrypted-portability-${{ matrix.os }}-${{ hashFiles('Cargo.lock') }}",
          },
        },
        {
          name: "Populate the Cargo registry from the committed lockfile",
          run: lockedCargoRegistryFetch,
        },
        {
          name: "Lint the encrypted portability lane",
          run: encryptedPortabilityCiCommands[0],
        },
        {
          name: "Test the encrypted portability lane",
          run: encryptedPortabilityCiCommands[1],
        },
        {
          name: "Run the BK and RS kill rows under encryption",
          run: encryptedPortabilityCiCommands[2],
        },
      ],
    },
    contracts: {
      name: "pnpm-contracts",
      needs: "source-preflight",
      "runs-on": "ubuntu-latest",
      "timeout-minutes": 15,
      steps: [
        {
          name: "Checkout without persisted credentials",
          uses: "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
          with: { "persist-credentials": false },
        },
        {
          name: "Install pinned Rust toolchain",
          run: "rustup toolchain install 1.98.0 --profile minimal --component rustfmt --component clippy",
        },
        {
          name: "Restore the Cargo registry keyed on the committed Cargo lockfile",
          uses: lockfileCacheActionReference,
          with: {
            path: "~/.cargo/registry",
            key: "cargo-registry-contracts-ubuntu-latest-${{ hashFiles('Cargo.lock') }}",
          },
        },
        {
          name: "Populate the Cargo registry from the committed lockfile",
          run: lockedCargoRegistryFetch,
        },
        {
          name: "Install pinned Node",
          uses: "actions/setup-node@49933ea5288caeca8642d1e84afbd3f7d6820020",
          with: { "node-version-file": ".nvmrc" },
        },
        { name: "Install pinned pnpm", run: "npm install --global pnpm@11.22.0" },
        {
          name: "Restore the pnpm store keyed on the committed pnpm lockfile",
          uses: lockfileCacheActionReference,
          with: {
            path: "~/.pnpm-store",
            key: "pnpm-store-contracts-ubuntu-latest-${{ hashFiles('pnpm-lock.yaml') }}",
          },
        },
        {
          name: "Frozen dependency install",
          run: "pnpm install --frozen-lockfile --store-dir ~/.pnpm-store",
        },
        { name: "Lint", run: "pnpm lint" },
        { name: "Typecheck", run: "pnpm typecheck" },
        { name: "Test", run: "pnpm test" },
        { name: "Build", run: "pnpm build" },
        {
          name: "Verify schema, semantic parity, fixture, and Proto contracts",
          run: "pnpm verify:contracts",
        },
        {
          name: "Check structural dependency-source baseline and negative fixtures",
          run: "pnpm security",
        },
      ],
    },
  },
};
const assertExactCiExecutionPolicy = (ci) => {
  const parsed = parseCiWorkflow(ci);
  const ordinaryObjects = JSON.parse(JSON.stringify(parsed));
  assert.deepEqual(
    ordinaryObjects,
    expectedCiWorkflow,
    "CI workflow, job, and step inventory/values must match the reviewed execution policy",
  );
};
assertExactCiExecutionPolicy(ciText);
const assertNativeFixtureCiTopology = (ci) => {
  const workflow = requireCiRecord(parseCiWorkflow(ci), "CI workflow");
  const jobs = requireCiRecord(workflow.jobs, "CI jobs");
  const rustJob = requireCiRecord(jobs.rust, "CI job rust");
  assertUnconditionalRequiredExecution(rustJob, "CI job rust");
  assert.equal(
    rustJob["runs-on"],
    "${{ matrix.os }}",
    "the Rust native job must bind runs-on exactly to matrix.os",
  );
  const strategy = requireCiRecord(rustJob.strategy, "CI job rust.strategy");
  assert.deepEqual(
    Object.keys(strategy).toSorted(),
    ["fail-fast", "matrix"],
    "the Rust strategy must contain only the reviewed fail-fast and matrix keys",
  );
  const matrix = requireCiRecord(strategy.matrix, "CI job rust.strategy.matrix");
  assert.deepEqual(
    Object.keys(matrix),
    ["os"],
    "the Rust matrix must not add include, exclude, or unreviewed dimensions",
  );
  assert.deepEqual(
    matrix.os,
    hostedRustMatrixLabels,
    "the Rust native job must use the exact hosted runner labels that carry the admission platform triples",
  );
  const steps = requireCiSteps(rustJob, "CI job rust");
  for (const [index, step] of steps.entries()) {
    assertUnconditionalRequiredExecution(step, `CI job rust.steps[${index}]`);
  }
  const independentRuns = steps
    .map((step) => step.run)
    .filter((command) => typeof command === "string")
    .filter((command) => command.includes("fixture ") || command.startsWith("git diff --exit-code"));
  assert.deepEqual(
    independentRuns,
    nativeFixtureCiCommands,
    "every native fixture command must be an ordered independent CI step on Windows and Linux",
  );
};
assertNativeFixtureCiTopology(ciText);
const combinedNativeFixtureStep = ciText.replace(
  [
    "      - name: Verify immutable v1 fixture and upcast",
    `        run: ${nativeFixtureCiCommands[0]}`,
    "      - name: Replay immutable v1 fixture",
    `        run: ${nativeFixtureCiCommands[1]}`,
  ].join("\n"),
  [
    "      - name: Combined native fixture commands that can mask an intermediate failure",
    "        run: |",
    `          ${nativeFixtureCiCommands[0]}`,
    "          node -e \"process.exit(7)\"",
    `          ${nativeFixtureCiCommands[1]}`,
  ].join("\n"),
);
assert.notEqual(combinedNativeFixtureStep, ciText, "CI topology mutation must alter the workflow");
assert.throws(
  () => assertNativeFixtureCiTopology(combinedNativeFixtureStep),
  undefined,
  "a multiline native-fixture step with an intermediate native failure must be rejected",
);
for (const [name, mutation] of [
  [
    "fixed Ubuntu runner",
    ciText.replace("    runs-on: ${{ matrix.os }}", "    runs-on: ubuntu-latest"),
  ],
  [
    "broken runner binding",
    ciText.replace("    runs-on: ${{ matrix.os }}", "    runs-on: ${{ matrix.runner }}"),
  ],
  [
    "missing Windows matrix entry",
    ciText.replace(
      "        os: [ubuntu-latest, ubuntu-24.04-arm, windows-latest, windows-11-arm, macos-latest]",
      "        os: [ubuntu-latest]",
    ),
  ],
  [
    "missing Ubuntu matrix entry",
    ciText.replace(
      "        os: [ubuntu-latest, ubuntu-24.04-arm, windows-latest, windows-11-arm, macos-latest]",
      "        os: [windows-latest]",
    ),
  ],
  [
    "duplicate matrix entry",
    ciText.replace(
      "        os: [ubuntu-latest, ubuntu-24.04-arm, windows-latest, windows-11-arm, macos-latest]",
      "        os: [ubuntu-latest, ubuntu-24.04-arm, windows-latest, windows-11-arm, macos-latest, macos-latest]",
    ),
  ],
  [
    "extra matrix entry",
    ciText.replace(
      "        os: [ubuntu-latest, ubuntu-24.04-arm, windows-latest, windows-11-arm, macos-latest]",
      "        os: [ubuntu-latest, ubuntu-24.04-arm, windows-latest, windows-11-arm, macos-latest, macos-13]",
    ),
  ],
]) {
  assert.notEqual(mutation, ciText, `${name} mutation must alter the workflow`);
  assert.throws(
    () => assertNativeFixtureCiTopology(mutation),
    undefined,
    `${name} must fail the native fixture topology gate`,
  );
}
const assertSourcePreflightTopology = ({ bootstrap, preflightModules, ci }) => {
  for (const [moduleName, source] of preflightModules) {
    const externalImports = [
      ...source.matchAll(/\bfrom\s+["'](?<specifier>[^"']+)["']/gu),
      ...source.matchAll(/\bimport\s*(?:\(\s*)?["'](?<specifier>[^"']+)["']/gu),
    ]
      .map((match) => match.groups?.specifier)
      .filter((specifier) => specifier !== undefined)
      .filter((specifier) => !specifier.startsWith("node:") && !specifier.startsWith("./"));
    assert.deepEqual(
      externalImports,
      [],
      `source-preflight module ${moduleName} must have no installed dependencies`,
    );
  }
  const bootstrapGate = bootstrap.indexOf("await assertRepositorySourcePolicy()");
  assert.ok(bootstrapGate >= 0, "bootstrap must run the source preflight");
  for (const operation of ['["pnpm", ["install"', '["cargo", ["fetch"']) {
    assert.ok(
      bootstrapGate < bootstrap.indexOf(operation),
      `bootstrap source preflight must precede ${operation}`,
    );
  }
  const workflow = requireCiRecord(parseCiWorkflow(ci), "CI workflow");
  assertExactCiExecutionPolicy(ci);
  const jobs = requireCiRecord(workflow.jobs, "CI jobs");
  const sourcePreflight = requireCiRecord(jobs["source-preflight"], "CI job source-preflight");
  assertUnconditionalRequiredExecution(sourcePreflight, "CI job source-preflight");
  const sourcePreflightSteps = requireCiSteps(sourcePreflight, "CI job source-preflight");
  for (const [index, step] of sourcePreflightSteps.entries()) {
    assertUnconditionalRequiredExecution(step, `CI job source-preflight.steps[${index}]`);
  }
  assert.equal(
    sourcePreflightSteps.filter((step) => step.run === "node tools/source-preflight.mjs").length,
    1,
    "CI must execute the dependency-free source preflight exactly once",
  );
  for (const jobName of ["rust", "contracts"]) {
    const job = requireCiRecord(jobs[jobName], `CI job ${jobName}`);
    assertUnconditionalRequiredExecution(job, `CI job ${jobName}`);
    assert.equal(job.needs, "source-preflight", `CI executable job ${jobName} must depend on source-preflight`);
    const steps = requireCiSteps(job, `CI job ${jobName}`);
    for (const [index, step] of steps.entries()) {
      assertUnconditionalRequiredExecution(step, `CI job ${jobName}.steps[${index}]`);
    }
    const runs = steps.map((step) => step.run).filter((command) => typeof command === "string");
    assert.equal(
      runs.filter((command) => command === lockedCargoRegistryFetch).length,
      1,
      `CI job ${jobName} must fill the Cargo registry exactly once with an explicit locked fetch`,
    );
    assert.ok(
      runs
        .slice(0, runs.indexOf(lockedCargoRegistryFetch))
        .every((command) => !/^(?:cargo|pnpm) /u.test(command)),
      `CI job ${jobName} must fill the Cargo registry before any resolving Cargo or pnpm command`,
    );
    const cacheKeys = steps
      .filter((step) => step.uses === lockfileCacheActionReference)
      .map((step) => step.with?.key);
    assert.ok(cacheKeys.length > 0, `CI job ${jobName} must cache resolved dependency state`);
    for (const key of cacheKeys) {
      assert.match(
        String(key),
        /\$\{\{ hashFiles\('(?:Cargo\.lock|pnpm-lock\.yaml)'\) \}\}$/u,
        `CI job ${jobName} cache keys must be bound to a committed lockfile digest`,
      );
    }
  }
  assertNativeFixtureCiTopology(ci);
};
assertSourcePreflightTopology({
  bootstrap: bootstrapText,
  preflightModules: [
    ["source-preflight.mjs", sourcePreflightText],
    ["dependency-source-policy.mjs", dependencySourcePolicyText],
    ["cargo-lock-source-policy.mjs", cargoLockSourcePolicyText],
    ["restricted-yaml.mjs", restrictedYamlText],
  ],
  ci: ciText,
});
for (const [name, topology] of [
  [
    "bootstrap ordering",
    {
      bootstrap: bootstrapText.replace(
        "await assertRepositorySourcePolicy();",
        "// source preflight removed by mutation",
      ),
      preflightModules: [["source-preflight.mjs", sourcePreflightText]],
      ci: ciText,
    },
  ],
  [
    "Rust CI dependency",
    {
      bootstrap: bootstrapText,
      preflightModules: [["source-preflight.mjs", sourcePreflightText]],
      ci: ciText.replace("    needs: source-preflight\n", ""),
    },
  ],
  [
    "transitive installed dependency",
    {
      bootstrap: bootstrapText,
      preflightModules: [[
        "dependency-source-policy.mjs",
        `import YAML from "yaml";\n${dependencySourcePolicyText}`,
      ]],
      ci: ciText,
    },
  ],
]) {
  assert.throws(
    () => assertSourcePreflightTopology(topology),
    undefined,
    `${name} mutation must fail source-preflight topology verification`,
  );
}
const assertCompleteCiTopology = (ci) => assertSourcePreflightTopology({
  bootstrap: bootstrapText,
  preflightModules: [
    ["source-preflight.mjs", sourcePreflightText],
    ["dependency-source-policy.mjs", dependencySourcePolicyText],
    ["cargo-lock-source-policy.mjs", cargoLockSourcePolicyText],
    ["restricted-yaml.mjs", restrictedYamlText],
  ],
  ci,
});
for (const [name, mutation] of [
  [
    "source-preflight materialization before the gate",
    ciText.replace(
      "      - name: Reject forbidden dependency sources before tool or dependency setup",
      [
        "      - name: Unreviewed package materialization",
        "        run: npm install --global pnpm@11.22.0",
        "      - name: Reject forbidden dependency sources before tool or dependency setup",
      ].join("\n"),
    ),
  ],
  [
    "source-preflight step reordering",
    ciText.replace(
      [
        "      - name: Checkout without persisted credentials",
        "        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2",
        "        with:",
        "          persist-credentials: false",
        "      - name: Reject forbidden dependency sources before tool or dependency setup",
        "        run: node tools/source-preflight.mjs",
      ].join("\n"),
      [
        "      - name: Reject forbidden dependency sources before tool or dependency setup",
        "        run: node tools/source-preflight.mjs",
        "      - name: Checkout without persisted credentials",
        "        uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2",
        "        with:",
        "          persist-credentials: false",
      ].join("\n"),
    ),
  ],
  [
    "writable top-level contents permission",
    ciText.replace("  contents: read", "  contents: write"),
  ],
  [
    "floating checkout action reference",
    ciText.replace(
      "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
      "actions/checkout@main",
    ),
  ],
  [
    "checkout credential persistence",
    ciText.replace("          persist-credentials: false", "          persist-credentials: true"),
  ],
  [
    "missing source-preflight timeout",
    ciText.replace("    timeout-minutes: 5\n", ""),
  ],
  [
    "changed Rust timeout",
    ciText.replace("    timeout-minutes: 20", "    timeout-minutes: 21"),
  ],
  [
    "extra required job key",
    ciText.replace("  rust:\n    name:", "  rust:\n    env: {UNREVIEWED: true}\n    name:"),
  ],
  [
    "extra required step key",
    ciText.replace(
      "        run: cargo fmt --all -- --check",
      "        run: cargo fmt --all -- --check\n        working-directory: .",
    ),
  ],
  [
    "custom shell suppressing the reviewed run command",
    ciText.replace(
      "        run: cargo fmt --all -- --check",
      "        run: cargo fmt --all -- --check\n        shell: node -e \"process.exit(0)\" {0}",
    ),
  ],
  [
    "unreviewed run-step environment",
    ciText.replace(
      "        run: cargo fmt --all -- --check",
      "        run: cargo fmt --all -- --check\n        env: {UNREVIEWED: true}",
    ),
  ],
  [
    "flow-map explicit if key",
    ciText.replace(
      "      - name: Check formatting\n        run: cargo fmt --all -- --check",
      "      - {name: Check formatting, run: cargo fmt --all -- --check, ? if: false}",
    ),
  ],
  [
    "combined permission action credential and timeout drift",
    ciText
      .replace("  contents: read", "  contents: write")
      .replace(
        "actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683",
        "actions/checkout@main",
      )
      .replace("          persist-credentials: false", "          persist-credentials: true")
      .replace("    timeout-minutes: 5\n", ""),
  ],
  [
    "Windows matrix exclusion",
    ciText.replace(
      "        os: [ubuntu-latest, ubuntu-24.04-arm, windows-latest, windows-11-arm, macos-latest]",
      "        os: [ubuntu-latest, ubuntu-24.04-arm, windows-latest, windows-11-arm, macos-latest]\n        exclude:\n          - os: windows-latest",
    ),
  ],
  [
    "disabled Rust job",
    ciText.replace("  rust:\n    name:", "  rust:\n    if: false\n    name:"),
  ],
  [
    "failure-tolerant Rust job",
    ciText.replace("  rust:\n    name:", "  rust:\n    continue-on-error: true\n    name:"),
  ],
  [
    "failure-tolerant source-preflight step",
    ciText.replace(
      "        run: node tools/source-preflight.mjs",
      "        run: node tools/source-preflight.mjs\n        continue-on-error: true",
    ),
  ],
  [
    "failure-tolerant v3 verification step",
    ciText.replace(
      "      - name: Verify deterministic v3 fixture",
      "      - name: Verify deterministic v3 fixture\n        continue-on-error: true",
    ),
  ],
  [
    "disabled v3 replay step",
    ciText.replace(
      "      - name: Replay deterministic v3 fixture",
      "      - name: Replay deterministic v3 fixture\n        if: false",
    ),
  ],
  [
    "duplicate key in required Rust job",
    ciText.replace(
      "    runs-on: ${{ matrix.os }}",
      "    runs-on: ${{ matrix.os }}\n    runs-on: ubuntu-latest",
    ),
  ],
  [
    "duplicate key in required fixture step",
    ciText.replace(
      `        run: ${nativeFixtureCiCommands[4]}`,
      `        run: ${nativeFixtureCiCommands[4]}\n        run: ${nativeFixtureCiCommands[4]}`,
    ),
  ],
  [
    "contracts job without Cargo registry population",
    ciText.replace(
      [
        "          key: cargo-registry-contracts-ubuntu-latest-${{ hashFiles('Cargo.lock') }}",
        "      - name: Populate the Cargo registry from the committed lockfile",
        "        run: cargo fetch --locked",
      ].join("\n"),
      "          key: cargo-registry-contracts-ubuntu-latest-${{ hashFiles('Cargo.lock') }}",
    ),
  ],
  [
    "unlocked Cargo registry population",
    ciText.replaceAll("        run: cargo fetch --locked", "        run: cargo fetch"),
  ],
  [
    "Cargo cache key detached from the committed lockfile",
    ciText.replaceAll("${{ hashFiles('Cargo.lock') }}", "static"),
  ],
  [
    "pnpm cache key detached from the committed lockfile",
    ciText.replaceAll("${{ hashFiles('pnpm-lock.yaml') }}", "static"),
  ],
  [
    "duplicate required job identifier",
    ciText.replace(
      "\n  contracts:\n",
      "\n  rust:\n    runs-on: ubuntu-latest\n    steps: []\n\n  contracts:\n",
    ),
  ],
]) {
  assert.notEqual(mutation, ciText, `${name} mutation must alter the workflow`);
  assert.throws(
    () => assertCompleteCiTopology(mutation),
    undefined,
    `${name} must fail effective CI conformance verification`,
  );
}
const protoRoots = [protoV1Text, protoV2Text, protoV3Text].map((text) => {
  const root = protobuf.parse(text, { keepCase: true }).root;
  root.resolveAll();
  return root;
});
const assertPredictionProtoContract = ([v1Root, v2Root]) => {
  assert.ok(v1Root !== undefined && v2Root !== undefined);
  const v1Claim = v1Root.lookupType("academic.v1.Claim");
  const v2Claim = v2Root.lookupType("academic.v2.Claim");
  assert.equal(
    v1Claim.fields.prediction_metadata,
    undefined,
    "immutable v1 Claim must not acquire prediction metadata",
  );
  const predictionField = v2Claim.fields.prediction_metadata;
  assert.ok(predictionField, "current v2 Claim.prediction_metadata must exist");
  assert.equal(predictionField.id, 11);
  assert.equal(predictionField.resolvedType?.fullName, ".academic.v2.PredictionMetadata");

  const observationWindow = v2Root.lookupType("academic.v2.PredictionObservationWindow");
  assert.deepEqual(
    Object.fromEntries(Object.entries(observationWindow.fields).map(([name, field]) => [name, field.id])),
    { from: 1, to: 2 },
  );
  const metadata = v2Root.lookupType("academic.v2.PredictionMetadata");
  assert.deepEqual(
    Object.fromEntries(Object.entries(metadata.fields).map(([name, field]) => [name, field.id])),
    { version: 1, observation_window: 2, positive_sample_count: 3 },
  );

  const value = {
    confidence: 720,
    valid_time: {
      from: { unix_epoch_millis: 800 },
      to: { unix_epoch_millis: 1200 },
    },
    prediction_metadata: {
      version: 1,
      observation_window: {
        from: { unix_epoch_millis: 100 },
        to: { unix_epoch_millis: 700 },
      },
      positive_sample_count: 6,
    },
  };
  assert.equal(v2Claim.verify(value), null);
  const roundTrip = v2Claim.toObject(v2Claim.decode(v2Claim.encode(value).finish()), {
    longs: Number,
  });
  assert.equal(roundTrip.confidence, 720);
  assert.equal(roundTrip.prediction_metadata.version, 1);
  assert.equal(roundTrip.prediction_metadata.positive_sample_count, 6);
  assert.equal(
    roundTrip.prediction_metadata.observation_window.from.unix_epoch_millis,
    100,
  );
  assert.equal(roundTrip.valid_time.from.unix_epoch_millis, 800);
};
assertPredictionProtoContract(protoRoots);
const predictionTagMutation = protoV2Text.replace(
  "PredictionMetadata prediction_metadata = 11;",
  "PredictionMetadata prediction_metadata = 12;",
);
assert.notEqual(predictionTagMutation, protoV2Text);
const mutatedPredictionRoot = protobuf.parse(predictionTagMutation, { keepCase: true }).root;
mutatedPredictionRoot.resolveAll();
assert.throws(
  () => assertPredictionProtoContract([protoRoots[0], mutatedPredictionRoot]),
  undefined,
  "a current prediction metadata tag mutation must fail Proto verification",
);
const rustStructBody = (source, name) => {
  const body = source.match(new RegExp(`struct ${name} \\{(?<body>[\\s\\S]*?)\\n\\}`, "u"))?.groups?.body;
  assert.ok(body, `Rust wire message ${name} must exist`);
  return body;
};
const rustModuleEnumBody = (source, moduleName, enumName) => {
  const moduleBody = source.match(new RegExp(`mod ${moduleName} \\{(?<body>[\\s\\S]*?)\\n\\}`, "u"))?.groups?.body;
  assert.ok(moduleBody, `Rust wire module ${moduleName} must exist`);
  const enumBody = moduleBody.match(new RegExp(`enum ${enumName} \\{(?<body>[\\s\\S]*?)\\n    \\}`, "u"))?.groups?.body;
  assert.ok(enumBody, `Rust wire enum ${moduleName}::${enumName} must exist`);
  return enumBody;
};
const rustScalarFields = [
  ["ProtoUuidV7", "value", 1],
  ["ProtoSha256Digest", "value", 1],
  ["ProtoValidInterval", "from", 1],
  ["ProtoValidInterval", "to", 2],
  ["ProtoTimestampMillis", "unix_epoch_millis", 1],
  ["ProtoUserActor", "user_id", 1],
  ["ProtoDeterministicEngineActor", "name", 1],
  ["ProtoDeterministicEngineActor", "version", 2],
  ["ProtoModelRunActor", "run_id", 1],
  ["ProtoImporterActor", "name", 1],
  ["ProtoImporterActor", "version", 2],
  ["ProtoClaimRelation", "source_claim_id", 1],
  ["ProtoClaimRelation", "target_claim_id", 2],
  ["ProtoClaimRelation", "kind", 3],
  ["ProtoClaimRelation", "scope_id", 4],
  ["ProtoOriginEvent", "id", 1],
  ["ProtoOriginEvent", "origin_seq", 2],
  ["ProtoOriginEvent", "origin_observed_at", 3],
  ["ProtoOriginEvent", "domain_id", 4],
  ["ProtoOriginEvent", "actor", 5],
];
const actorWireFields = [
  ["User", "user", 1],
  ["DeterministicEngine", "deterministic_engine", 2],
  ["ModelRun", "model_run", 3],
  ["Importer", "importer", 4],
];
// Arms every declared Proto version carries. Tags 10..=15 are frozen.
const legacyPayloadWireFields = [
  ["ArtifactRegistered", "artifact_registered", 10],
  ["EvidenceRegistered", "evidence_registered", 11],
  ["ClaimAsserted", "claim_asserted", 12],
  ["DecisionRecorded", "decision_recorded", 13],
  ["ScopeRegistered", "scope_registered", 14],
  ["ClaimRelated", "claim_related", 15],
];
// Event schema v3 arms, declared only by academic.v3 and additive over the
// frozen legacy tags: [Rust oneof variant, Proto field, tag, message, parent].
const v3PayloadWireFields = [
  ["CurriculumVersionPublished", "curriculum_version_published", 16, "CurriculumVersionRegistration", undefined],
  ["CourseRevisionPublished", "course_revision_published", 17, "CourseRevisionRegistration", "curriculum_version_id"],
  ["OfferingObserved", "offering_observed", 18, "OfferingRegistration", "course_revision_id"],
  ["AttemptRecorded", "attempt_recorded", 19, "AttemptRegistration", "offering_id"],
  ["RequirementSetPublished", "requirement_set_published", 20, "RequirementSetRegistration", "curriculum_version_id"],
  ["AuditComputed", "audit_computed", 21, "AuditRegistration", "requirement_set_id"],
  ["CapturePermissionRecorded", "capture_permission_recorded", 22, "CapturePermissionRegistration", "offering_id"],
  ["LectureSessionRecorded", "lecture_session_recorded", 23, "LectureSessionRegistration", "offering_id"],
  ["TranscriptVersionAdded", "transcript_version_added", 24, "TranscriptVersionRegistration", "lecture_session_id"],
  ["LectureDocumentPublished", "lecture_document_published", 25, "LectureDocumentRegistration", "lecture_session_id"],
  ["SnapshotRegistered", "snapshot_registered", 26, "SnapshotRegistration", "repository_id"],
  ["FindingPublished", "finding_published", 27, "FindingRegistration", "snapshot_id"],
  ["ModelRunRecorded", "model_run_recorded", 28, "ModelRunRegistration", undefined],
  ["ProposalDisposed", "proposal_disposed", 29, "ProposalDispositionRegistration", "model_run_id"],
  ["EgressDecided", "egress_decided", 30, "EgressDecisionRegistration", undefined],
  ["ConsentRecorded", "consent_recorded", 31, "ConsentRegistration", undefined],
  ["EntityIdentityChanged", "entity_identity_changed", 32, "EntityIdentityChangeRegistration", "entity_id"],
  ["RetentionActionRecorded", "retention_action_recorded", 33, "RetentionActionRegistration", undefined],
];
const payloadWireFields = [...legacyPayloadWireFields, ...v3PayloadWireFields];
// Every registration message carries the identical frame, so one row states the
// whole v3 arm payload contract: 1 id, 2 parent where one exists, 3 domain_id,
// 4 scope_id, 5 source_digest, 6 valid_time.
const registrationMessageFields = (parent) => [
  ["id", 1],
  ...(parent === undefined ? [] : [[parent, 2]]),
  ["domain_id", 3],
  ["scope_id", 4],
  ["source_digest", 5],
  ["valid_time", 6],
];
// The same frame as hand-written Rust field rows, reviewed only against v3.
const v3RustScalarFields = v3PayloadWireFields.flatMap(([, , , messageName, parent]) =>
  registrationMessageFields(parent).map(([fieldName, tag]) => [
    `Proto${messageName}`,
    fieldName,
    tag,
  ]),
);
// The hand-written Prost mirror models the v3 superset so Prost applies
// protobuf's last-oneof-value rule to every tag any declared version emits. A
// v1 or v2 root therefore declares a subset of the arms Rust knows.
const declaredPayloadWireFields = (version) =>
  version === 3 ? payloadWireFields : legacyPayloadWireFields;
const relationKindValues = [
  ["Unspecified", "CLAIM_RELATION_KIND_UNSPECIFIED", 0],
  ["Supports", "CLAIM_RELATION_KIND_SUPPORTS", 1],
  ["Contradicts", "CLAIM_RELATION_KIND_CONTRADICTS", 2],
  ["Supersedes", "CLAIM_RELATION_KIND_SUPERSEDES", 3],
  ["Retracts", "CLAIM_RELATION_KIND_RETRACTS", 4],
  ["Duplicates", "CLAIM_RELATION_KIND_DUPLICATES", 5],
];
const relationKindNames = relationKindValues.slice(1).map(([rustName]) => rustName);
const parseRustProstField = (source, structName, fieldName) => {
  const body = rustStructBody(source, structName);
  const match = body.match(
    new RegExp(
      `#\\[prost\\((?<options>[^\\]]+)\\)\\]\\s*${fieldName}:\\s*(?<rustType>[^,\\n]+),`,
      "u",
    ),
  );
  assert.ok(match?.groups, `Rust wire field ${structName}.${fieldName} must exist`);
  const options = match.groups.options;
  const scalarKind = options.match(
    /(?:^|,\s*)(?<kind>bytes|double|float|fixed32|fixed64|int32|int64|message|sfixed32|sfixed64|sint32|sint64|string|uint32|uint64)\b/u,
  )?.groups?.kind;
  const enumeration = options.match(/\benumeration\s*=\s*"(?<name>[^"]+)"/u)?.groups?.name;
  const oneof = options.match(/\boneof\s*=\s*"(?<name>[^"]+)"/u)?.groups?.name;
  const bytes = options.match(/\bbytes\s*=\s*"(?<storage>[^"]+)"/u)?.groups?.storage;
  const tagText = options.match(/\btag\s*=\s*"(?<tag>\d+)"/u)?.groups?.tag;
  const tags = options.match(/\btags\s*=\s*"(?<tags>[\d, ]+)"/u)?.groups?.tags;
  return {
    kind: oneof === undefined ? enumeration === undefined ? scalarKind : "enumeration" : "oneof",
    typeParameter: oneof ?? enumeration ?? bytes,
    cardinality: /(?:^|,\s*)repeated(?:,|$)/u.test(options)
      ? "repeated"
      : /(?:^|,\s*)optional(?:,|$)/u.test(options)
        ? "optional"
        : "singular",
    tag: tagText === undefined ? undefined : Number(tagText),
    tags: tags?.split(",").map((tag) => Number(tag.trim())),
    rustType: match.groups.rustType.trim(),
  };
};
const rustProstFieldNames = (source, structName) => [
  ...rustStructBody(source, structName).matchAll(
    /#\[prost\([^\]]+\)\]\s*(?<fieldName>[A-Za-z_][A-Za-z0-9_]*):/gu,
  ),
].map((match) => match.groups.fieldName);
const expectedProstField = (field) => {
  const isMessage = field.resolvedType instanceof protobuf.Type;
  const isEnumeration = field.resolvedType instanceof protobuf.Enum;
  const kind = isMessage ? "message" : isEnumeration ? "enumeration" : field.type;
  const syntheticOptional =
    field.options?.proto3_optional === true && field.partOf?.name === `_${field.name}`;
  const cardinality = field.repeated
    ? "repeated"
    : field.partOf !== null && field.partOf !== undefined && !syntheticOptional
      ? "oneof"
      : isMessage || field.options?.proto3_optional === true
        ? "optional"
        : "singular";
  const scalarRustTypes = new Map([
    ["bytes", "Vec<u8>"],
    ["double", "f64"],
    ["float", "f32"],
    ["fixed32", "u32"],
    ["fixed64", "u64"],
    ["int32", "i32"],
    ["int64", "i64"],
    ["sfixed32", "i32"],
    ["sfixed64", "i64"],
    ["sint32", "i32"],
    ["sint64", "i64"],
    ["string", "String"],
    ["uint32", "u32"],
    ["uint64", "u64"],
  ]);
  const baseRustType = isMessage || isEnumeration
    ? isMessage ? `Proto${field.resolvedType.name}` : "i32"
    : scalarRustTypes.get(field.type);
  assert.ok(baseRustType, `unsupported Proto field type ${field.type}`);
  const rustType = cardinality === "optional"
    ? `Option<${baseRustType}>`
    : cardinality === "repeated"
      ? `Vec<${baseRustType}>`
      : baseRustType;
  return {
    kind,
    typeParameter: isEnumeration ? `Proto${field.resolvedType.name}` : kind === "bytes" ? "vec" : undefined,
    cardinality,
    tag: field.id,
    rustType,
    oneof: syntheticOptional ? undefined : field.partOf?.name,
  };
};
const parseRustOneofVariant = (source, moduleName, enumName, variantName) => {
  const body = rustModuleEnumBody(source, moduleName, enumName);
  const match = body.match(
    new RegExp(
      `#\\[prost\\((?<options>[^\\]]+)\\)\\]\\s*${variantName}\\((?<rustType>[^)]+)\\),`,
      "u",
    ),
  );
  assert.ok(match?.groups, `Rust oneof variant ${moduleName}::${variantName} must exist`);
  const tag = match.groups.options.match(/\btag\s*=\s*"(?<tag>\d+)"/u)?.groups?.tag;
  const kind = match.groups.options.match(
    /(?:^|,\s*)(?<kind>bytes|enumeration|message|string|sint64|uint64)\b/u,
  )?.groups?.kind;
  return {
    kind,
    tag: tag === undefined ? undefined : Number(tag),
    rustType: match.groups.rustType.trim(),
  };
};
const rustOneofVariantNames = (source, moduleName, enumName) => [
  ...rustModuleEnumBody(source, moduleName, enumName).matchAll(
    /#\[prost\([^\]]+\)\]\s*(?<variantName>[A-Za-z_][A-Za-z0-9_]*)\(/gu,
  ),
].map((match) => match.groups.variantName);
const rustRelationMappingLines = (source, functionName) => {
  const body = source.match(
    new RegExp(
      `const fn ${functionName}\\(value: [^)]+\\) -> [^{]+ \\{\\s*match value \\{(?<body>[\\s\\S]*?)\\n    \\}\\n\\}`,
      "u",
    ),
  )?.groups?.body;
  assert.ok(body, `Rust relation mapping ${functionName} must exist`);
  return body
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter((line) => line.length > 0);
};
const assertExactRustDomainRelationKind = (source) => {
  const body = source.match(
    /pub enum ClaimRelationKind \{(?<body>[\s\S]*?)\n\}/u,
  )?.groups?.body;
  assert.ok(body, "Rust domain ClaimRelationKind enum must exist");
  const actual = [...body.matchAll(/^\s*(\w+),\s*$/gmu)].map((match) => match[1]);
  assert.deepEqual(actual, relationKindNames, "Rust domain relation membership must be exact");
};
const assertRustRelationMappings = (source) => {
  assert.deepEqual(
    rustRelationMappingLines(source, "encode_relation_kind"),
    relationKindNames.map(
      (name) => `ClaimRelationKind::${name} => ProtoClaimRelationKind::${name},`,
    ),
    "every domain relation kind must encode to the identically named Proto discriminant",
  );
  assert.deepEqual(
    rustRelationMappingLines(source, "decode_relation_kind"),
    [
      ...relationKindNames.map(
        (name) =>
          `ProtoClaimRelationKind::${name} => Some(ClaimRelationKind::${name}),`,
      ),
      "ProtoClaimRelationKind::Unspecified => None,",
    ],
    "every Proto relation discriminant must decode to the identical domain kind",
  );
};
const actorVariantNames = actorWireFields.map(([name]) => name);
const assertRustActorMappings = (source) => {
  const functions = rustRootFunctionBodies(source);
  const encodeSource = functions.get("encode_actor")?.source ?? "";
  const decodeSource = functions.get("decode_actor")?.source ?? "";
  const encodedPairs = [...encodeSource.matchAll(
    /Actor::(?<domain>User|DeterministicEngine|ModelRun|Importer)\s*\{[^}]*\}\s*=>[\s\S]*?proto_actor::Kind::(?<wire>User|DeterministicEngine|ModelRun|Importer)\b/gu,
  )].map((match) => [match.groups.domain, match.groups.wire]);
  const decodedPairs = [...decodeSource.matchAll(
    /proto_actor::Kind::(?<wire>User|DeterministicEngine|ModelRun|Importer)\([^)]*\)\s*=>\s*Ok\(Actor::(?<domain>User|DeterministicEngine|ModelRun|Importer)\b/gu,
  )].map((match) => [match.groups.wire, match.groups.domain]);
  const identity = actorVariantNames.map((name) => [name, name]);
  assert.deepEqual(
    encodedPairs,
    identity,
    "every domain actor variant must encode to the independently matching Proto oneof arm",
  );
  assert.deepEqual(
    decodedPairs,
    identity,
    "every Proto actor oneof arm must decode to the independently matching domain variant",
  );
};
const assertRustWireContract = (source) => {
  const expectedFieldsByStruct = new Map();
  for (const [structName, fieldName] of [...rustScalarFields, ...v3RustScalarFields]) {
    const fields = expectedFieldsByStruct.get(structName) ?? [];
    fields.push(fieldName);
    expectedFieldsByStruct.set(structName, fields);
  }
  expectedFieldsByStruct.set("ProtoActor", ["kind"]);
  expectedFieldsByStruct.set("ProtoOriginEvent", [
    ...(expectedFieldsByStruct.get("ProtoOriginEvent") ?? []),
    "payload",
  ]);
  for (const [structName, expectedFields] of expectedFieldsByStruct) {
    assert.deepEqual(
      rustProstFieldNames(source, structName).sort(),
      expectedFields.toSorted(),
      `${structName} hand-written Prost field membership must be exact`,
    );
  }
  assert.deepEqual(
    rustOneofVariantNames(source, "proto_actor", "Kind"),
    actorWireFields.map(([variantName]) => variantName),
    "ProtoActor.kind hand-written oneof membership must be exact",
  );
  assert.deepEqual(
    rustOneofVariantNames(source, "proto_origin_event", "Payload"),
    payloadWireFields.map(([variantName]) => variantName),
    "ProtoOriginEvent.payload hand-written oneof membership must be exact",
  );
  for (const [structName, fieldName, tag] of [...rustScalarFields, ...v3RustScalarFields]) {
    const body = rustStructBody(source, structName);
    assert.match(
      body,
      new RegExp(`#\\[prost\\([^\\]]*tag = "${tag}"[^\\]]*\\)\\]\\s*${fieldName}:`, "u"),
      `${structName}.${fieldName} must retain tag ${tag}`,
    );
  }
  for (const [structName, oneofField, tags] of [
    ["ProtoActor", "kind", "1, 2, 3, 4"],
    ["ProtoOriginEvent", "payload", "10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33"],
  ]) {
    assert.match(
      rustStructBody(source, structName),
      // rustfmt breaks a long attribute across lines, so the review tolerates
      // whitespace between the reviewed tokens but not a change to any of them.
      new RegExp(
        `#\\[prost\\(\\s*oneof = "[^"]+",\\s*tags = "${tags}",?\\s*\\)\\]\\s*${oneofField}:`,
        "u",
      ),
      `${structName}.${oneofField} must enumerate every declared oneof tag`,
    );
  }
  for (const [moduleName, fields] of [
    ["proto_actor", actorWireFields],
    ["proto_origin_event", payloadWireFields],
  ]) {
    const body = rustModuleEnumBody(source, moduleName, moduleName === "proto_actor" ? "Kind" : "Payload");
    for (const [variantName, fieldName, tag] of fields) {
      assert.match(
        body,
        new RegExp(`#\\[prost\\(message, tag = "${tag}"\\)\\]\\s*${variantName}\\(`, "u"),
        `${moduleName}::${variantName} must retain tag ${tag}`,
      );
      for (const [versionIndex, root] of protoRoots.entries()) {
        const version = versionIndex + 1;
        const messageName = moduleName === "proto_actor" ? "Actor" : "OriginEvent";
        const oneofName = moduleName === "proto_actor" ? "kind" : "payload";
        const field = root.lookupType(`academic.v${version}.${messageName}`).fields[fieldName];
        if (
          moduleName === "proto_origin_event" &&
          !declaredPayloadWireFields(version).some(([, name]) => name === fieldName)
        ) {
          assert.equal(field, undefined, `v${version} must not declare ${fieldName}`);
          continue;
        }
        assert.ok(field, `v${version} ${messageName}.${fieldName} must exist`);
        const actual = parseRustOneofVariant(
          source,
          moduleName,
          moduleName === "proto_actor" ? "Kind" : "Payload",
          variantName,
        );
        assert.equal(field.partOf?.name, oneofName, `v${version} ${messageName}.${fieldName} oneof`);
        assert.equal(actual.kind, "message", `${moduleName}::${variantName} Prost type`);
        assert.equal(actual.tag, field.id, `${moduleName}::${variantName} tag`);
        assert.equal(
          actual.rustType,
          `Proto${field.resolvedType.name}`,
          `${moduleName}::${variantName} payload type`,
        );
      }
    }
  }
  for (const [versionIndex, root] of protoRoots.entries()) {
    const reviewedFields = versionIndex + 1 === 3
      ? [...rustScalarFields, ...v3RustScalarFields]
      : rustScalarFields;
    for (const [structName, fieldName] of reviewedFields) {
      const messageName = structName.slice("Proto".length);
      const protoField = root.lookupType(`academic.v${versionIndex + 1}.${messageName}`).fields[fieldName];
      assert.ok(protoField, `v${versionIndex + 1} ${messageName}.${fieldName} must exist`);
      const expected = expectedProstField(protoField);
      const actual = parseRustProstField(source, structName, fieldName);
      assert.deepEqual(
        actual,
        {
          kind: expected.kind,
          typeParameter: expected.typeParameter,
          cardinality: expected.cardinality,
          tag: expected.tag,
          tags: undefined,
          rustType: expected.rustType,
        },
        `v${versionIndex + 1} ${structName}.${fieldName} Prost type/cardinality/tag parity`,
      );
    }
    for (const [structName, fieldName, moduleName, enumName, protoMessage, oneofName] of [
      ["ProtoActor", "kind", "proto_actor", "Kind", "Actor", "kind"],
      ["ProtoOriginEvent", "payload", "proto_origin_event", "Payload", "OriginEvent", "payload"],
    ]) {
      const actual = parseRustProstField(source, structName, fieldName);
      const version = versionIndex + 1;
      const oneof = root.lookupType(`academic.v${version}.${protoMessage}`).oneofs[oneofName];
      const declaredTags = oneof.oneof.map((name) => root.lookupType(
        `academic.v${version}.${protoMessage}`,
      ).fields[name].id);
      // Rust enumerates every tag any declared version emits; each version's own
      // tag list must be exactly the prefix of that superset it declares, so no
      // emitted tag is ever dropped, reordered, or reused across the three.
      const supersetTags = protoMessage === "OriginEvent"
        ? payloadWireFields.map(([, , tag]) => tag)
        : declaredTags;
      assert.deepEqual(
        declaredTags,
        supersetTags.slice(0, declaredTags.length),
        `v${version} ${protoMessage}.${oneofName} tags must be a prefix of the emitted superset`,
      );
      assert.deepEqual(
        actual,
        {
          kind: "oneof",
          typeParameter: `${moduleName}::${enumName}`,
          cardinality: "singular",
          tag: undefined,
          tags: supersetTags,
          rustType: `Option<${moduleName}::${enumName}>`,
        },
        `v${version} ${structName}.${fieldName} oneof membership/type/tag parity`,
      );
    }
  }
  const relationEnum = source.match(/enum ProtoClaimRelationKind \{(?<body>[\s\S]*?)\n\}/u)?.groups?.body;
  assert.ok(relationEnum, "Rust relation enum must exist");
  const actualRelationEntries = [...relationEnum.matchAll(/^\s*(\w+)\s*=\s*(-?\d+),\s*$/gmu)]
    .map((match) => [match[1], Number(match[2])]);
  assert.deepEqual(
    actualRelationEntries,
    relationKindValues.map(([rustName, , value]) => [rustName, value]),
    "hand-written Rust Proto relation membership and discriminants must be exact",
  );
  assertRustRelationMappings(source);
  assertRustActorMappings(source);
};
assertExactRustDomainRelationKind(rustDomainText);
assertRustWireContract(rustProtoContractText);
const mutatedRustWire = rustProtoContractText.replace(
  '#[prost(message, tag = "15")]\n        ClaimRelated',
  '#[prost(message, tag = "16")]\n        ClaimRelated',
);
assert.throws(
  () => assertRustWireContract(mutatedRustWire),
  undefined,
  "a hand-written Rust wire-tag mutation must fail contract verification",
);
for (const [name, mutated] of [
  [
    "scalar wire type",
    rustProtoContractText.replace(
      '#[prost(sint64, tag = "1")]',
      '#[prost(int64, tag = "1")]',
    ),
  ],
  [
    "message cardinality",
    rustProtoContractText.replace(
      '#[prost(message, optional, tag = "1")]\n    user_id:',
      '#[prost(message, tag = "1")]\n    user_id:',
    ),
  ],
  [
    "Rust field payload type",
    rustProtoContractText.replace(
      'user_id: Option<ProtoUuidV7>,',
      'user_id: Option<ProtoTimestampMillis>,',
    ),
  ],
  [
    "oneof variant wire type",
    rustProtoContractText.replace(
      '#[prost(message, tag = "1")]\n        User(ProtoUserActor),',
      '#[prost(bytes, tag = "1")]\n        User(ProtoUserActor),',
    ),
  ],
  [
    "extra Rust field",
    rustProtoContractText.replace(
      'struct ProtoTimestampMillis {\n    #[prost(sint64, tag = "1")]\n    unix_epoch_millis: i64,',
      'struct ProtoTimestampMillis {\n    #[prost(sint64, tag = "1")]\n    unix_epoch_millis: i64,\n    #[prost(uint64, tag = "2")]\n    unexpected: u64,',
    ),
  ],
  [
    "extra oneof variant",
    rustProtoContractText.replace(
      '        Importer(ProtoImporterActor),',
      '        Importer(ProtoImporterActor),\n        #[prost(message, tag = "5")]\n        Unexpected(ProtoImporterActor),',
    ),
  ],
]) {
  assert.notEqual(mutated, rustProtoContractText, `${name} mutation must change Rust source`);
  assert.throws(
    () => assertRustWireContract(mutated),
    undefined,
    `a hand-written Rust ${name} mutation must fail contract verification`,
  );
}
const mutatedRustEncodeMapping = rustProtoContractText.replace(
  "ClaimRelationKind::Supports => ProtoClaimRelationKind::Supports,",
  "ClaimRelationKind::Supports => ProtoClaimRelationKind::Contradicts,",
);
assert.notEqual(mutatedRustEncodeMapping, rustProtoContractText);
assert.throws(
  () => assertRustWireContract(mutatedRustEncodeMapping),
  undefined,
  "a Rust relation encode-mapping mutation must fail contract verification",
);
const mutatedRustDecodeMapping = rustProtoContractText.replace(
  "ProtoClaimRelationKind::Supports => Some(ClaimRelationKind::Supports),",
  "ProtoClaimRelationKind::Supports => Some(ClaimRelationKind::Contradicts),",
);
assert.notEqual(mutatedRustDecodeMapping, rustProtoContractText);
assert.throws(
  () => assertRustWireContract(mutatedRustDecodeMapping),
  undefined,
  "a Rust relation decode-mapping mutation must fail contract verification",
);
const mutatedRustActorEncodeMapping = rustProtoContractText
  .replace(
    "Actor::User { user_id } => proto_actor::Kind::User(ProtoUserActor {",
    "Actor::User { user_id } => proto_actor::Kind::ModelRun(ProtoUserActor {",
  )
  .replace(
    "Actor::ModelRun { run_id } => proto_actor::Kind::ModelRun(ProtoModelRunActor {",
    "Actor::ModelRun { run_id } => proto_actor::Kind::User(ProtoModelRunActor {",
  );
assert.notEqual(mutatedRustActorEncodeMapping, rustProtoContractText);
assert.throws(
  () => assertRustWireContract(mutatedRustActorEncodeMapping),
  undefined,
  "a symmetric User/ModelRun actor encode-mapping swap must fail contract verification",
);
const mutatedRustActorDecodeMapping = rustProtoContractText
  .replace(
    "proto_actor::Kind::User(value) => Ok(Actor::User {",
    "proto_actor::Kind::User(value) => Ok(Actor::ModelRun {",
  )
  .replace(
    "proto_actor::Kind::ModelRun(value) => Ok(Actor::ModelRun {",
    "proto_actor::Kind::ModelRun(value) => Ok(Actor::User {",
  );
assert.notEqual(mutatedRustActorDecodeMapping, rustProtoContractText);
assert.throws(
  () => assertRustWireContract(mutatedRustActorDecodeMapping),
  undefined,
  "a symmetric User/ModelRun actor decode-mapping swap must fail contract verification",
);

const declaredMessageFields = [
  ["UuidV7", [["value", 1]]],
  ["Sha256Digest", [["value", 1]]],
  ["ValidInterval", [["from", 1], ["to", 2]]],
  ["TimestampMillis", [["unix_epoch_millis", 1]]],
  ["UserActor", [["user_id", 1]]],
  ["DeterministicEngineActor", [["name", 1], ["version", 2]]],
  ["ModelRunActor", [["run_id", 1]]],
  ["ImporterActor", [["name", 1], ["version", 2]]],
  ["Actor", actorWireFields.map(([, fieldName, tag]) => [fieldName, tag])],
  [
    "ClaimRelation",
    [["source_claim_id", 1], ["target_claim_id", 2], ["kind", 3], ["scope_id", 4]],
  ],
  [
    "OriginEvent",
    [
      ["id", 1],
      ["origin_seq", 2],
      ["origin_observed_at", 3],
      ["domain_id", 4],
      ["actor", 5],
      ...payloadWireFields.map(([, fieldName, tag]) => [fieldName, tag]),
    ],
  ],
];
const v3OnlyMessageFields = v3PayloadWireFields.map(([, , , messageName, parent]) => [
  messageName,
  registrationMessageFields(parent),
]);
for (const version of [1, 2, 3]) {
  const root = protoRoots[version - 1];
  assert.ok(root);
  const messagesForVersion = version === 3
    ? [...declaredMessageFields, ...v3OnlyMessageFields]
    : declaredMessageFields;
  for (const [messageName, fields] of messagesForVersion) {
    const declared = messageName === "OriginEvent"
      ? fields.filter(([fieldName]) =>
        !v3PayloadWireFields.some(([, v3Field]) => v3Field === fieldName) ||
        version === 3)
      : fields;
    const message = root.lookupType(`academic.v${version}.${messageName}`);
    assert.deepEqual(
      Object.keys(message.fields).sort(),
      declared.map(([fieldName]) => fieldName).sort(),
      `v${version} ${messageName} shape must match every hand-written Rust field`,
    );
    for (const [fieldName, tag] of declared) {
      assert.equal(message.fields[fieldName]?.id, tag, `v${version} ${messageName}.${fieldName}`);
    }
  }
  for (const [, , , messageName] of v3PayloadWireFields) {
    if (version === 3) {
      continue;
    }
    assert.throws(
      () => root.lookupType(`academic.v${version}.${messageName}`),
      undefined,
      `v${version} must not declare the v3 registration message ${messageName}`,
    );
  }
  const actor = root.lookupType(`academic.v${version}.Actor`);
  const originEvent = root.lookupType(`academic.v${version}.OriginEvent`);
  assert.deepEqual(actor.oneofs.kind.oneof, actorWireFields.map(([, fieldName]) => fieldName));
  assert.deepEqual(
    originEvent.oneofs.payload.oneof,
    declaredPayloadWireFields(version).map(([, fieldName]) => fieldName),
  );
  const relationKind = root.lookupEnum(`academic.v${version}.ClaimRelationKind`);
  for (const [, protoName, value] of relationKindValues) {
    assert.equal(relationKind.values[protoName], value, `v${version} ${protoName}`);
  }
  if (version === 2) {
    assert.deepEqual(
      Object.entries(relationKind.values).sort(([left], [right]) => left.localeCompare(right)),
      relationKindValues
        .map(([, protoName, value]) => [protoName, value])
        .sort(([left], [right]) => left.localeCompare(right)),
      "current v2 relation enum membership and discriminants must be exact",
    );
  }
}
const mutatedProtoV2RelationKind = protoV2Text.replace(
  "  CLAIM_RELATION_KIND_DUPLICATES = 5;",
  "  CLAIM_RELATION_KIND_DUPLICATES = 5;\n  CLAIM_RELATION_KIND_REPLACES = 6;",
);
assert.notEqual(mutatedProtoV2RelationKind, protoV2Text);
const mutatedProtoV2Root = protobuf.parse(mutatedProtoV2RelationKind, { keepCase: true }).root;
mutatedProtoV2Root.resolveAll();
assert.throws(
  () => {
    const relationKind = mutatedProtoV2Root.lookupEnum("academic.v2.ClaimRelationKind");
    assert.deepEqual(
      Object.entries(relationKind.values).sort(([left], [right]) => left.localeCompare(right)),
      relationKindValues
        .map(([, protoName, value]) => [protoName, value])
        .sort(([left], [right]) => left.localeCompare(right)),
    );
  },
  undefined,
  "an added current-v2 relation discriminant must fail contract verification",
);
const uuidBytes = (value) => Buffer.from(value.replaceAll("-", ""), "hex");
const actorWireGoldens = [
  {
    name: "User",
    arm: "user",
    value: { user: { user_id: { value: uuidBytes("01900000-0000-7000-8000-000000000020") } } },
    decoded: { user_id: { value: "AZAAAAAAcACAAAAAAAAAIA==" } },
    hex: "0a140a120a1001900000000070008000000000000020",
  },
  {
    name: "DeterministicEngine",
    arm: "deterministic_engine",
    value: { deterministic_engine: { name: "resolver", version: "1.2.3" } },
    decoded: { name: "resolver", version: "1.2.3" },
    hex: "12110a087265736f6c7665721205312e322e33",
  },
  {
    name: "ModelRun",
    arm: "model_run",
    value: { model_run: { run_id: { value: uuidBytes("01900000-0000-7000-8000-000000000021") } } },
    decoded: { run_id: { value: "AZAAAAAAcACAAAAAAAAAIQ==" } },
    hex: "1a140a120a1001900000000070008000000000000021",
  },
  {
    name: "Importer",
    arm: "importer",
    value: { importer: { name: "registrar", version: "2026.08" } },
    decoded: { name: "registrar", version: "2026.08" },
    hex: "22140a097265676973747261721207323032362e3038",
  },
];
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
const relationWireGolden = "0a120a100190000000007000800000000000000c100c1a0308e00122120a10019000000000700080000000000000012a2522230a1a73796e7468657469632e6f6666696369616c2e666978747572651205312e302e307a3e0a120a100190000000007000800000000000020612120a1001900000000070008000000000000205180322120a1001900000000070008000000000000007";
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
  const actorType = protoRoot.lookupType(`academic.v${version}.Actor`);
  for (const golden of actorWireGoldens) {
    assert.equal(actorType.verify(golden.value), null, `v${version} ${golden.name} actor shape`);
    const actorBytes = Buffer.from(actorType.encode(golden.value).finish());
    assert.equal(
      actorBytes.toString("hex"),
      golden.hex,
      `v${version} protobuf.js ${golden.name} bytes must match the independent Rust golden`,
    );
    const actorRoundTrip = actorType.toObject(actorType.decode(actorBytes), {
      bytes: String,
      oneofs: true,
    });
    assert.equal(actorRoundTrip.kind, golden.arm, `v${version} ${golden.name} selected oneof arm`);
    assert.deepEqual(
      actorRoundTrip[golden.arm],
      golden.decoded,
      `v${version} ${golden.name} decoded fields`,
    );
  }
  const originEventType = protoRoot.lookupType(`academic.v${version}.OriginEvent`);
  assert.equal(originEventType.verify(protoRelationEvent), null);
  const relationWire = Buffer.from(originEventType.encode(protoRelationEvent).finish());
  assert.equal(
    relationWire.toString("hex"),
    relationWireGolden,
    `v${version} protobuf.js relation bytes must match the Rust/Proto golden`,
  );
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
assert.match(
  protoV2Text,
  /message PredictionMetadata\s*\{\s*uint32 version = 1;[\s\S]*PredictionObservationWindow observation_window = 2;[\s\S]*uint32 positive_sample_count = 3;/u,
);
assert.match(protoV2Text, /PredictionMetadata prediction_metadata = 11;/u);
assert.doesNotMatch(protoV2Text, /actor_kind/u);
assert.doesNotMatch(protoV2Text, /optional UuidV7 scope_id/u);
assert.doesNotMatch(protoV2Text, /plaintext_digest|keyed_vault_locator|confidence_permille/u);
assert.match(protoV2Text, /bytes deterministic_payload_cbor = 2;/u);
assert.doesNotMatch(fixtureV1Text, /https?:\/\//u);
assert.doesNotMatch(fixtureV2Text, /https?:\/\//u);

// ---------------------------------------------------------------------------
// Event schema v3: tag discipline, three-version codegen drift, and independent
// cross-runtime agreement on every new arm. The three assertions below carry the
// acceptance-evidence names for P2-C1 so an audit can find them by name.
// ---------------------------------------------------------------------------

/// `v3_arms_use_unreused_tags`
///
/// Tags 10..=15 stay bound to their v1/v2 arms across all three declared
/// versions, 6..=9 stay reserved in every version, and the eighteen v3 arms
/// occupy 16..=33 with no tag emitted by an earlier version reused or moved.
const v3_arms_use_unreused_tags = (roots) => {
  const emitted = new Map();
  for (const [versionIndex, root] of roots.entries()) {
    const version = versionIndex + 1;
    const originEvent = root.lookupType(`academic.v${version}.OriginEvent`);
    for (const fieldName of originEvent.oneofs.payload.oneof) {
      const tag = originEvent.fields[fieldName].id;
      const previous = emitted.get(tag);
      assert.ok(
        previous === undefined || previous === fieldName,
        `Proto tag ${tag} is ${previous} in an earlier version and ${fieldName} in v${version}`,
      );
      emitted.set(tag, fieldName);
    }
    for (const reserved of [6, 7, 8, 9]) {
      assert.ok(
        !Object.values(originEvent.fields).some((field) => field.id === reserved),
        `v${version} OriginEvent must keep tag ${reserved} reserved`,
      );
    }
  }
  assert.deepEqual(
    [...emitted.entries()].toSorted(([left], [right]) => left - right),
    payloadWireFields.map(([, fieldName, tag]) => [tag, fieldName]),
    "the union of every declared version's arms is exactly the emitted tag table",
  );
  const v3Tags = v3PayloadWireFields.map(([, , tag]) => tag);
  assert.deepEqual(v3Tags, Array.from({ length: 18 }, (_, index) => 16 + index));
  assert.equal(new Set(v3Tags).size, 18, "no v3 arm may share a tag with another");
  for (const [, , legacyTag] of legacyPayloadWireFields) {
    assert.ok(!v3Tags.includes(legacyTag), `v3 must not reuse tag ${legacyTag}`);
  }
};
v3_arms_use_unreused_tags(protoRoots);

/// `proto_codegen_has_no_drift_v3`
///
/// The hand-written Prost mirror and all three declared schemas are reviewed
/// together, and a tag mutation on either side of the v3 boundary fails.
const proto_codegen_has_no_drift_v3 = () => {
  assert.match(protoV3Text, /package academic\.v3;/u);
  assert.equal(protoRoots.length, 3, "the drift gate reviews three Proto versions");
  assertRustWireContract(rustProtoContractText);

  const rustTagMutation = rustProtoContractText.replace(
    '#[prost(message, tag = "16")]\n        CurriculumVersionPublished',
    '#[prost(message, tag = "34")]\n        CurriculumVersionPublished',
  );
  assert.notEqual(rustTagMutation, rustProtoContractText);
  assert.throws(
    () => assertRustWireContract(rustTagMutation),
    undefined,
    "a hand-written v3 arm tag mutation must fail contract verification",
  );

  const rustFieldMutation = rustProtoContractText.replace(
    '#[prost(message, optional, tag = "5")]\n    source_digest: Option<ProtoSha256Digest>,\n    #[prost(message, optional, tag = "6")]\n    valid_time: Option<ProtoValidInterval>,\n}\n\n#[derive(Clone, PartialEq, Message)]\nstruct ProtoCourseRevisionRegistration',
    '#[prost(message, optional, tag = "7")]\n    source_digest: Option<ProtoSha256Digest>,\n    #[prost(message, optional, tag = "6")]\n    valid_time: Option<ProtoValidInterval>,\n}\n\n#[derive(Clone, PartialEq, Message)]\nstruct ProtoCourseRevisionRegistration',
  );
  assert.notEqual(rustFieldMutation, rustProtoContractText);
  assert.throws(
    () => assertRustWireContract(rustFieldMutation),
    undefined,
    "a v3 registration field tag mutation must fail contract verification",
  );

  const protoTagMutation = protoV3Text.replace(
    "CurriculumVersionRegistration curriculum_version_published = 16;",
    "CurriculumVersionRegistration curriculum_version_published = 34;",
  );
  assert.notEqual(protoTagMutation, protoV3Text);
  const mutatedRoot = protobuf.parse(protoTagMutation, { keepCase: true }).root;
  mutatedRoot.resolveAll();
  assert.throws(
    () => v3_arms_use_unreused_tags([protoRoots[0], protoRoots[1], mutatedRoot]),
    undefined,
    "a declared v3 arm tag mutation must fail Proto verification",
  );

  const reusedLegacyTag = protoV3Text.replace(
    "CurriculumVersionRegistration curriculum_version_published = 16;",
    "CurriculumVersionRegistration curriculum_version_published = 15;",
  );
  assert.notEqual(reusedLegacyTag, protoV3Text);
  assert.throws(
    () => {
      const reusedRoot = protobuf.parse(reusedLegacyTag, { keepCase: true }).root;
      reusedRoot.resolveAll();
      v3_arms_use_unreused_tags([protoRoots[0], protoRoots[1], reusedRoot]);
    },
    undefined,
    "reusing a frozen v1/v2 tag in v3 must fail Proto verification",
  );
};
proto_codegen_has_no_drift_v3();

/// `rust_and_protobufjs_agree_on_every_v3_arm`
///
/// Every golden in the Rust wire test is recomputed here from the declared
/// `academic.v3` schema by protobuf.js. Neither runtime can move without the
/// other, and each arm is proved to select its own tag and round-trip.
const rust_and_protobufjs_agree_on_every_v3_arm = () => {
  const goldenBody = rustProtoContractText.match(
    /fn v3_arm_goldens\(\)[\s\S]*?\n    \}\n/u,
  )?.[0];
  assert.ok(goldenBody, "the Rust v3 arm golden table must remain explicitly named");
  const rustGoldens = [...goldenBody.matchAll(
    /\n {12}\(\n {16}(?<tag>\d+),\n {16}"(?<field>[a-z_]+)",[\s\S]*?\n {16}"(?<hex>[0-9a-f]+)",\n {12}\),/gu,
  )].map((match) => [Number(match.groups.tag), match.groups.field, match.groups.hex]);
  assert.equal(rustGoldens.length, 18, "Rust must publish one golden per v3 arm");

  const v3Root = protoRoots[2];
  assert.ok(v3Root);
  const originEvent = v3Root.lookupType("academic.v3.OriginEvent");
  const registrationBase = {
    id: { value: uuidBytes("01900000-0000-7000-8000-00000000000c") },
    origin_seq: 12,
    origin_observed_at: { unix_epoch_millis: 112 },
    domain_id: { value: uuidBytes("01900000-0000-7000-8000-000000000001") },
    actor: { importer: { name: "synthetic.official.fixture", version: "1.0.0" } },
  };
  const registrationRecord = (parent, withDigest) => ({
    id: { value: uuidBytes("01900000-0000-7000-8000-000000000410") },
    ...(parent === undefined
      ? {}
      : { [parent]: { value: uuidBytes("01900000-0000-7000-8000-000000000411") } }),
    domain_id: { value: uuidBytes("01900000-0000-7000-8000-000000000001") },
    scope_id: { value: uuidBytes("01900000-0000-7000-8000-000000000007") },
    ...(withDigest
      ? {
        source_digest: {
          value: Buffer.from(
            "0aa68a055c7e14b3b3aa6730ea4e4135a3d3365c8f75249d44c73a0dbb5b8134",
            "hex",
          ),
        },
      }
      : {}),
    valid_time: { from: { unix_epoch_millis: 100 } },
  });

  for (const [index, [, fieldName, tag, , parent]] of v3PayloadWireFields.entries()) {
    const [rustTag, rustField, rustHex] = rustGoldens[index];
    assert.equal(rustTag, tag, `Rust golden ${index} must carry tag ${tag}`);
    assert.equal(rustField, fieldName, `Rust golden ${index} must name ${fieldName}`);

    const value = { ...registrationBase, [fieldName]: registrationRecord(parent, true) };
    assert.equal(originEvent.verify(value), null, `${fieldName} shape`);
    const bytes = Buffer.from(originEvent.encode(value).finish());
    assert.equal(
      bytes.toString("hex"),
      rustHex,
      `protobuf.js ${fieldName} bytes must match the independent Rust golden`,
    );
    const roundTrip = originEvent.toObject(originEvent.decode(bytes), {
      bytes: String,
      longs: Number,
      oneofs: true,
    });
    assert.equal(roundTrip.payload, fieldName, `${fieldName} selected oneof arm`);
    assert.equal(
      roundTrip[fieldName].id.value,
      Buffer.from("01900000000070008000000000000410", "hex").toString("base64"),
      `${fieldName} decoded aggregate identity`,
    );

    // The last known arm still wins, so a v3 arm cannot be shadowed by a legacy
    // one appearing after it and vice versa.
    const emptyRelation = Buffer.from([0x7a, 0x00]);
    assert.equal(
      originEvent.toObject(
        originEvent.decode(Buffer.concat([emptyRelation, bytes])),
        { oneofs: true },
      ).payload,
      fieldName,
      `${fieldName} after claim_related must override it`,
    );
    assert.equal(
      originEvent.toObject(
        originEvent.decode(Buffer.concat([bytes, emptyRelation])),
        { oneofs: true },
      ).payload,
      "claim_related",
      `claim_related after ${fieldName} must override it`,
    );
  }

  // The optional provenance digest is absence, not an empty digest.
  const bareHex = rustProtoContractText.match(
    /fn t093_v3_source_digest_round_trips_present_and_absent\(\)[\s\S]*?hex::encode\(&encoded\),\s*"(?<hex>[0-9a-f]+)"\s*[,)]/u,
  )?.groups?.hex;
  assert.ok(bareHex, "the Rust absent-source-digest golden must remain named");
  const bareValue = {
    ...registrationBase,
    curriculum_version_published: registrationRecord(undefined, false),
  };
  assert.equal(
    Buffer.from(originEvent.encode(bareValue).finish()).toString("hex"),
    bareHex,
    "protobuf.js must reproduce the absent-source-digest golden",
  );
  assert.equal(
    originEvent.toObject(
      originEvent.decode(Buffer.from(bareHex, "hex")),
      { oneofs: true },
    ).curriculum_version_published.source_digest,
    undefined,
    "an absent source digest must decode as absent",
  );
};
rust_and_protobufjs_agree_on_every_v3_arm();

// The predicate registry is one contract in two files: the JSON source of truth
// and the Rust constants rendered from it. Both are pinned to the §7.1 node
// hierarchy and the §7.2 edge table of the canonical design document, whose
// digest is already asserted above, so a specification edit that moves an edge
// fails here before it can reach a caller.
const predicate_registry_matches_its_source_and_the_specification = async () => {
  const registry = JSON.parse(await readFile(REGISTRY_PATH, "utf8"));
  const generated = await readFile(GENERATED_PATH, "utf8");
  assert.equal(
    generated,
    renderRustModule(registry),
    "the generated predicate constants must be a fresh render of the registry file",
  );

  const specText = canonicalSpecBytes.toString("utf8");
  const section = (heading, next) =>
    specText.slice(specText.indexOf(heading), specText.indexOf(next));

  const screaming = (name) =>
    name.replaceAll(/(?<lower>[a-z])(?<upper>[A-Z])/gu, "$<lower>_$<upper>").toUpperCase();
  const nodeTypes = section("### 7.1", "### 7.2")
    .split("\n")
    .filter((line) => line.includes("─ ") && line.includes(": "))
    .flatMap((line) => line.slice(line.indexOf(": ") + 2).split(", "))
    .map((name) => screaming(name.trim()));
  assert.deepEqual(
    registry.node_types,
    nodeTypes,
    "registry node types must be the §7.1 hierarchy leaves in specification order",
  );

  const edges = section("### 7.2", "### 7.3")
    .split("\n")
    .filter((line) => line.startsWith("| `"))
    .map((line) => line.split("|").map((cell) => cell.trim()));
  assert.equal(edges.length, 20, "§7.2 must still fix exactly twenty edges");
  assert.equal(registry.predicates.length, edges.length);

  for (const [index, entry] of registry.predicates.entries()) {
    const [, name, direction, meaning] = edges[index];
    assert.equal(`\`${entry.name}\``, name, "registry order must follow the §7.2 table");
    assert.equal(entry.spec_direction, direction, `${entry.name} must quote its direction cell`);
    assert.equal(entry.spec_meaning, meaning, `${entry.name} must quote its meaning cell`);
    assert.equal(entry.predicate_id, predicateId(entry.name));
    assert.ok(
      entry.subject_types.length > 0 && entry.object_types.length > 0,
      `${entry.name} must declare both ends`,
    );
    for (const node of [...entry.subject_types, ...entry.object_types]) {
      assert.ok(registry.node_types.includes(node), `${entry.name} uses unknown node type ${node}`);
    }
    assert.equal(
      entry.prerequisite,
      entry.strengths.length > 0,
      `${entry.name} must carry a strength exactly when it is a prerequisite edge`,
    );
    assert.ok(entry.inverse_label.length > 0, `${entry.name} must name its inverse reading`);
  }

  // An inverse is a view. No registry name is another entry's inverse label, so
  // there is no reverse predicate to store a duplicate row under.
  const names = new Set(registry.predicates.map((entry) => entry.name));
  for (const entry of registry.predicates) {
    assert.ok(
      !names.has(entry.inverse_label.toUpperCase().replaceAll(" ", "_")),
      `${entry.name} declares an inverse label that is itself a predicate`,
    );
  }

  // A single-source HARD prerequisite is rejected by the registry, not by a
  // caller's own rule.
  const requires = registry.predicates.find((entry) => entry.name === "REQUIRES");
  const hard = requires.minimum_evidence.by_strength.find((row) => row.strength === "HARD");
  assert.ok(hard, "REQUIRES must override its evidence rule at HARD");
  assert.ok(
    hard.rule.independent_sources >= 2,
    "a HARD REQUIRES edge must demand more than one independent source",
  );
  assert.ok(
    !requires.strengths.includes("HELPFUL"),
    "REQUIRES is a hard/near-hard dependency, never a preference",
  );
  assert.ok(
    !registry.predicates.find((entry) => entry.name === "BUILDS_ON").strengths.includes("HARD"),
    "BUILDS_ON must stay distinguishable from REQUIRES",
  );
  assert.deepEqual(
    registry.predicates.find((entry) => entry.name === "RELATED_TO").strengths,
    [],
    "RELATED_TO must not be usable as a prerequisite",
  );

  assert.ok(
    registry.open_gates.includes("GATE-38-022"),
    "the base taxonomy mix must stay a visibly open gate",
  );
};
await predicate_registry_matches_its_source_and_the_specification();

// The engine registry is the same two-file contract as the predicate registry:
// a JSON source of truth and Rust constants rendered from it, both pinned to
// the canonical design document whose digest is asserted above. §28 fixes what
// each engine is, so a specification edit that renames, drops, or adds an
// engine fails here rather than reaching a caller.
//
// The registry is the §28 table and nothing else. t068 §3.9 calls it a
// "thirteen-engine registry"; §28 tabulates twelve, and the thirteenth t068
// implies is the property sentence under the table, which names no inputs, no
// outputs, and no invariant. The comparison below is against the table, so the
// registry follows the specification rather than t068's count.
const engine_registry_matches_its_source_and_the_specification = async () => {
  const registry = JSON.parse(await readFile(ENGINE_REGISTRY_PATH, "utf8"));
  const generated = await readFile(ENGINE_GENERATED_PATH, "utf8");
  assert.equal(
    generated,
    renderEngineModule(registry),
    "the generated engine constants must be a fresh render of the registry file",
  );

  const specText = canonicalSpecBytes.toString("utf8");
  const section = specText.slice(
    specText.indexOf("## 28. Deterministic Engines"),
    specText.indexOf("## 29. Data Ingestion"),
  );
  const rows = section
    .split("\n")
    .filter((line) => line.startsWith("| ") && !line.startsWith("| Engine "))
    .filter((line) => !line.startsWith("|---"))
    .map((line) =>
      line
        .split("|")
        .map((cell) => cell.trim())
        .slice(1, -1),
    );

  // Enumerated, not counted: a registered engine the table does not name and a
  // tabulated engine the registry drops are the same mismatch.
  assert.deepEqual(
    registry.engines.map((entry) => entry.name),
    rows.map((row) => specName(row[0])),
    "the registered engines are not the §28 table",
  );

  // Each entry quotes its own row.
  for (const [index, row] of rows.entries()) {
    const entry = registry.engines[index];
    assert.deepEqual(
      entry.spec_row,
      { engine: row[0], inputs: row[1], outputs: row[2], invariant: row[3] },
      `${entry.name} must quote its §28 row verbatim`,
    );
    assert.equal(
      entry.spec_sentence,
      undefined,
      `${entry.name} must be registered from a table row, never from prose`,
    );
  }

  const harnessDirs = new Set();
  const engineIds = new Set();
  for (const [index, entry] of registry.engines.entries()) {
    assert.equal(entry.engine_id, engineId(entry.name));
    assert.equal(entry.requirement_id, `REQ-28-${String(index + 1).padStart(3, "0")}`);
    assert.equal(entry.since_registry_version, registry.registry_version);
    assert.equal(entry.harness_dir, entry.name.toLowerCase());
    assert.ok(!engineIds.has(entry.engine_id), `${entry.name} reuses an engine id`);
    assert.ok(!harnessDirs.has(entry.harness_dir), `${entry.name} reuses a harness directory`);
    engineIds.add(entry.engine_id);
    harnessDirs.add(entry.harness_dir);
    assert.ok(
      ["PLANNED", "IMPLEMENTED"].includes(entry.lifecycle),
      `${entry.name} has an unknown lifecycle`,
    );
  }

  // The high-impact four §3.9 names, one engine per path and no more. Egress is
  // decided by the permission broker: it is the only registered engine whose
  // output governs whether data may leave the device.
  assert.deepEqual(
    registry.engines
      .filter((entry) => entry.high_impact_path !== null)
      .map((entry) => [entry.name, entry.high_impact_path]),
    [
      ["GPA", "GPA"],
      ["GRADUATION_AUDIT", "GRADUATION"],
      ["PERMISSION_BROKER", "EGRESS"],
      ["RETENTION_DELETION", "DELETION"],
    ],
    "the high-impact four must stay GPA, graduation, deletion, and egress",
  );
  assert.deepEqual(
    registry.high_impact_paths.toSorted(),
    ["DELETION", "EGRESS", "GPA", "GRADUATION"],
  );
  assert.deepEqual(registry.adverse_paths, ["UNKNOWN", "CONFLICT", "PARTIAL_FAILURE"]);
  assert.deepEqual(registry.artifact_classes, [
    "GOLDEN_FIXTURES",
    "PROPERTY_TESTS",
    "VERSION_COMPAT_FIXTURES",
    "EXPLANATION_SNAPSHOT",
  ]);

  // Nothing unregistered hides under the harness root, and no planned engine
  // has quietly acquired artifacts there.
  const planned = new Set(
    registry.engines
      .filter((entry) => entry.lifecycle === "PLANNED")
      .map((entry) => entry.harness_dir),
  );
  const present = await readdir(registry.harness_root).catch(() => []);
  for (const name of present) {
    assert.ok(harnessDirs.has(name), `${name} is under the harness root and is not an engine`);
    assert.ok(!planned.has(name), `${name} is PLANNED and has harness artifacts`);
  }
};
await engine_registry_matches_its_source_and_the_specification();

console.log(
  "Immutable v1 and v2 contracts, the §7.1/§7.2 predicate registry and the §28 engine registry with their generated constants, strict synthetic fixture ingress, Phase 1 manifest policy, crate-wide semantic v3-only writers, event schema v3 arm and tag discipline across three Proto versions, RFC-variant UUIDv7 parity, effective native CI execution, Rust/Proto wire descriptors, and source-preflight topology verified.",
);
