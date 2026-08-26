import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";

import Ajv2020 from "ajv/dist/2020.js";
import protobuf from "protobufjs";

const [
  fixtureV1Bytes,
  fixtureV2Text,
  fixtureSchemaV1Bytes,
  fixtureSchemaV2Text,
  artifactSchemaText,
  artifactCorpusText,
  protoV1Bytes,
  protoV2Text,
  canonicalSpecBytes,
  rustProtoContractText,
  rustDomainText,
  rustContractsText,
  rustCoreText,
  rustCliText,
  bootstrapText,
  sourcePreflightText,
  dependencySourcePolicyText,
  cargoLockSourcePolicyText,
  restrictedYamlText,
  ciText,
] = await Promise.all([
  readFile("schemas/fixtures/signed-batch-v1.json"),
  readFile("schemas/fixtures/signed-batch-v2.json", "utf8"),
  readFile("schemas/jsonschema/signed-batch-fixture-v1.schema.json"),
  readFile("schemas/jsonschema/signed-batch-fixture-v2.schema.json", "utf8"),
  readFile("schemas/jsonschema/artifact-descriptor-v1.schema.json", "utf8"),
  readFile("schemas/fixtures/artifact-descriptor-parity-v1.json", "utf8"),
  readFile("schemas/proto/academic/v1/ledger.proto"),
  readFile("schemas/proto/academic/v2/ledger.proto", "utf8"),
  readFile("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
  readFile("crates/contracts/src/proto_contract.rs", "utf8"),
  readFile("crates/domain/src/lib.rs", "utf8"),
  readFile("crates/contracts/src/lib.rs", "utf8"),
  readFile("crates/core/src/lib.rs", "utf8"),
  readFile("crates/cli/src/main.rs", "utf8"),
  readFile("tools/bootstrap.mjs", "utf8"),
  readFile("tools/source-preflight.mjs", "utf8"),
  readFile("tools/dependency-source-policy.mjs", "utf8"),
  readFile("tools/cargo-lock-source-policy.mjs", "utf8"),
  readFile("tools/restricted-yaml.mjs", "utf8"),
  readFile(".github/workflows/ci.yml", "utf8"),
]);

const fixtureV1Text = fixtureV1Bytes.toString("utf8");
const fixtureSchemaV1Text = fixtureSchemaV1Bytes.toString("utf8");
const protoV1Text = protoV1Bytes.toString("utf8");
const fixtureV1 = JSON.parse(fixtureV1Text);
const fixtureV2 = JSON.parse(fixtureV2Text);
const fixtureSchemaV1 = JSON.parse(fixtureSchemaV1Text);
const fixtureSchemaV2 = JSON.parse(fixtureSchemaV2Text);
const artifactSchema = JSON.parse(artifactSchemaText);
const artifactCorpus = JSON.parse(artifactCorpusText);
const {
  assertArtifactDescriptorSemantics,
  assertCanonicalArtifactJsonNumberTokens,
  parseArtifactDescriptorJson,
  parseFixtureDocument,
} = await import("../packages/web-contracts/dist/index.js");

const ajv = new Ajv2020({ allErrors: true, strict: true });
const validateFixtureSchemaV1 = ajv.compile(fixtureSchemaV1);
const validateFixtureSchemaV2 = ajv.compile(fixtureSchemaV2);
const validateArtifactSchema = ajv.compile(artifactSchema);
const sha256Upper = (bytes) => createHash("sha256").update(bytes).digest("hex").toUpperCase();
const assertImmutableV1Bytes = (bytes, expected, label) => {
  assert.equal(sha256Upper(bytes), expected, `${label} bytes are immutable`);
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
for (const [field, token, valid] of [
  ["fixture_version", "2.0", true],
  ["fixture_version", "2e0", true],
  ["event_schema_version", "2.0", true],
  ["event_schema_version", "2e0", true],
  ["fixture_version", "2.5", false],
  ["event_schema_version", "65536", false],
]) {
  const rawFixture = fixtureV2Text.replace(`"${field}": 2`, `"${field}": ${token}`);
  assert.notEqual(rawFixture, fixtureV2Text, `${field} fixture lexeme probe must mutate source`);
  let schemaValid = true;
  let typescriptValid = true;
  try {
    const parsed = JSON.parse(rawFixture);
    schemaValid = validateFixtureSchemaV2(parsed);
    parseFixtureDocument(parsed);
  } catch {
    typescriptValid = false;
  }
  assert.equal(schemaValid, valid, `schema fixture integer-lexeme parity: ${field}=${token}`);
  assert.equal(
    typescriptValid,
    valid,
    `TypeScript fixture integer-lexeme parity: ${field}=${token}`,
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
assert.doesNotMatch(
  rustContractsText,
  /pub\s+fn\s+\w*v1\w*|sign_batch_v1/u,
  "academic-contracts must expose no general v1 encoder or signer",
);
assert.doesNotMatch(
  rustCoreText,
  /build_fixture_document_for_version|sign_batch_v1/u,
  "academic-core must not offer version-selectable or v1 fixture emission",
);
assert.doesNotMatch(
  rustCliText.split(/\n#\[cfg\(test\)\]/u, 1)[0],
  /fixture_version|fixture-version/u,
  "the production-facing CLI must emit only the current v2 fixture",
);
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
  assert.match(
    ci,
    /jobs:\s*\n\s{2}source-preflight:[\s\S]*?run: node tools\/source-preflight\.mjs/u,
    "CI must begin with a dependency-free source-preflight job",
  );
  for (const job of ["rust", "contracts"]) {
    const marker = `\n  ${job}:\n`;
    const start = ci.indexOf(marker);
    assert.ok(start >= 0, `CI executable job ${job} must exist`);
    const remainder = ci.slice(start + marker.length);
    const nextJob = remainder.search(/\n  [a-z0-9-]+:\n/iu);
    const jobBody = nextJob < 0 ? remainder : remainder.slice(0, nextJob);
    assert.match(
      jobBody,
      /^    needs: source-preflight$/mu,
      `CI executable job ${job} must depend on source-preflight`,
    );
  }
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
const protoRoots = [protoV1Text, protoV2Text].map((text) => {
  const root = protobuf.parse(text, { keepCase: true }).root;
  root.resolveAll();
  return root;
});
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
const payloadWireFields = [
  ["ArtifactRegistered", "artifact_registered", 10],
  ["EvidenceRegistered", "evidence_registered", 11],
  ["ClaimAsserted", "claim_asserted", 12],
  ["DecisionRecorded", "decision_recorded", 13],
  ["ScopeRegistered", "scope_registered", 14],
  ["ClaimRelated", "claim_related", 15],
];
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
  const cardinality = field.repeated
    ? "repeated"
    : field.partOf !== null && field.partOf !== undefined
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
    oneof: field.partOf?.name,
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
const assertRustWireContract = (source) => {
  const expectedFieldsByStruct = new Map();
  for (const [structName, fieldName] of rustScalarFields) {
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
  for (const [structName, fieldName, tag] of rustScalarFields) {
    const body = rustStructBody(source, structName);
    assert.match(
      body,
      new RegExp(`#\\[prost\\([^\\]]*tag = "${tag}"[^\\]]*\\)\\]\\s*${fieldName}:`, "u"),
      `${structName}.${fieldName} must retain tag ${tag}`,
    );
  }
  for (const [structName, oneofField, tags] of [
    ["ProtoActor", "kind", "1, 2, 3, 4"],
    ["ProtoOriginEvent", "payload", "10, 11, 12, 13, 14, 15"],
  ]) {
    assert.match(
      rustStructBody(source, structName),
      new RegExp(`#\\[prost\\(oneof = "[^"]+", tags = "${tags}"\\)\\]\\s*${oneofField}:`, "u"),
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
        const messageName = moduleName === "proto_actor" ? "Actor" : "OriginEvent";
        const oneofName = moduleName === "proto_actor" ? "kind" : "payload";
        const field = root.lookupType(`academic.v${versionIndex + 1}.${messageName}`).fields[fieldName];
        assert.ok(field, `v${versionIndex + 1} ${messageName}.${fieldName} must exist`);
        const actual = parseRustOneofVariant(
          source,
          moduleName,
          moduleName === "proto_actor" ? "Kind" : "Payload",
          variantName,
        );
        assert.equal(field.partOf?.name, oneofName, `v${versionIndex + 1} ${messageName}.${fieldName} oneof`);
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
    for (const [structName, fieldName] of rustScalarFields) {
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
      const oneof = root.lookupType(`academic.v${versionIndex + 1}.${protoMessage}`).oneofs[oneofName];
      const expectedTags = oneof.oneof.map((name) => root.lookupType(
        `academic.v${versionIndex + 1}.${protoMessage}`,
      ).fields[name].id);
      assert.deepEqual(
        actual,
        {
          kind: "oneof",
          typeParameter: `${moduleName}::${enumName}`,
          cardinality: "singular",
          tag: undefined,
          tags: expectedTags,
          rustType: `Option<${moduleName}::${enumName}>`,
        },
        `v${versionIndex + 1} ${structName}.${fieldName} oneof membership/type/tag parity`,
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

const declaredMessageFields = [
  ["UuidV7", [["value", 1]]],
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
for (const version of [1, 2]) {
  const root = protoRoots[version - 1];
  assert.ok(root);
  for (const [messageName, fields] of declaredMessageFields) {
    const message = root.lookupType(`academic.v${version}.${messageName}`);
    assert.deepEqual(
      Object.keys(message.fields).sort(),
      fields.map(([fieldName]) => fieldName).sort(),
      `v${version} ${messageName} shape must match every hand-written Rust field`,
    );
    for (const [fieldName, tag] of fields) {
      assert.equal(message.fields[fieldName]?.id, tag, `v${version} ${messageName}.${fieldName}`);
    }
  }
  const actor = root.lookupType(`academic.v${version}.Actor`);
  const originEvent = root.lookupType(`academic.v${version}.OriginEvent`);
  assert.deepEqual(actor.oneofs.kind.oneof, actorWireFields.map(([, fieldName]) => fieldName));
  assert.deepEqual(
    originEvent.oneofs.payload.oneof,
    payloadWireFields.map(([, fieldName]) => fieldName),
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
assert.doesNotMatch(protoV2Text, /actor_kind/u);
assert.doesNotMatch(protoV2Text, /optional UuidV7 scope_id/u);
assert.doesNotMatch(protoV2Text, /plaintext_digest|keyed_vault_locator|confidence_permille/u);
assert.match(protoV2Text, /bytes deterministic_payload_cbor = 2;/u);
assert.doesNotMatch(fixtureV1Text, /https?:\/\//u);
assert.doesNotMatch(fixtureV2Text, /https?:\/\//u);

console.log(
  "Immutable v1 contracts, source-aware signed decoding, v2-only writers, portable raw artifact JSON, fixture integer lexemes, exact Rust/Proto wire descriptors, and source-preflight topology verified.",
);
