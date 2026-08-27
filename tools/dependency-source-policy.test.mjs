import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";

import { assertCargoLockSourcePolicy } from "./cargo-lock-source-policy.mjs";
import { assertPnpmLockSourcePolicy } from "./dependency-source-policy.mjs";
import { parsePnpmLockYaml } from "./restricted-yaml.mjs";

const corpora = [
  {
    directory: "tools/fixtures/cargo-source-policy",
    extension: ".toml",
    assertPolicy: assertCargoLockSourcePolicy,
  },
  {
    directory: "tools/fixtures/pnpm-source-policy",
    extension: ".yaml",
    assertPolicy: assertPnpmLockSourcePolicy,
  },
];

let fixtureCount = 0;
for (const corpus of corpora) {
  const fixtureNames = (await readdir(corpus.directory))
    .filter((name) => name.endsWith(corpus.extension))
    .sort();
  assert.ok(fixtureNames.some((name) => name.startsWith("allow-")));
  assert.ok(fixtureNames.some((name) => name.startsWith("reject-")));
  fixtureCount += fixtureNames.length;

  for (const fixtureName of fixtureNames) {
    const fixtureText = await readFile(`${corpus.directory}/${fixtureName}`, "utf8");
    if (fixtureName.startsWith("allow-")) {
      assert.doesNotThrow(
        () => corpus.assertPolicy(fixtureText, fixtureName),
        `${fixtureName} must remain allowed`,
      );
    } else if (fixtureName.startsWith("reject-")) {
      assert.throws(
        () => corpus.assertPolicy(fixtureText, fixtureName),
        undefined,
        `${fixtureName} must be rejected`,
      );
    } else {
      assert.fail(`source policy fixture needs allow-/reject- expectation prefix: ${fixtureName}`);
    }
  }
}

const explicitFlowVariation = await readFile(
  "tools/fixtures/pnpm-source-policy/reject-yaml-explicit-flow-key.yaml",
  "utf8",
);
assert.throws(
  () => parsePnpmLockYaml(explicitFlowVariation, "explicit-flow-variation"),
  /explicit mapping keys are outside the lockfile profile/u,
  "a compact flow explicit key must reject before nested variation/binary decoding",
);
for (const [name, source] of [
  [
    "nested block sequence explicit key",
    "lockfileVersion: '9.0'\nprobe:\n  - ? resolution: {type: binary}\n",
  ],
  [
    "nested flow sequence explicit key",
    "lockfileVersion: '9.0'\nprobe: [{? resolution: {type: binary}}]\n",
  ],
]) {
  assert.throws(
    () => parsePnpmLockYaml(source, name),
    /explicit mapping keys are outside the lockfile profile/u,
    `${name} must reject before key decoding`,
  );
}
const questionKeyControls = parsePnpmLockYaml(
  [
    "lockfileVersion: '9.0'",
    "ordinary: {resolution: registry}",
    "quoted: {'? resolution': literal}",
    "plain: {?resolution: literal}",
    "",
  ].join("\n"),
  "question-key-controls",
);
assert.equal(questionKeyControls.ordinary.resolution, "registry");
assert.equal(questionKeyControls.quoted["? resolution"], "literal");
assert.equal(questionKeyControls.plain["?resolution"], "literal");

const scalarKindCorpus = JSON.parse(
  await readFile("tools/fixtures/pnpm-yaml-scalar-kind-conformance-v1.json", "utf8"),
);
assert.equal(scalarKindCorpus.schema_version, 1);
const dependencyEntry = (position, scalar) => {
  if (position === "direct") return scalar;
  if (position === "specifier") {
    return `{specifier: ${scalar}, version: '1.0.0'}`;
  }
  if (position === "version") {
    return `{specifier: '1.0.0', version: ${scalar}}`;
  }
  assert.fail(`unsupported scalar-kind position ${position}`);
};
const scalarFixture = (style, position, scalar) => {
  const entry = dependencyEntry(position, scalar);
  if (style === "flow") {
    return [
      "lockfileVersion: '9.0'",
      `importers: {'.': {dependencies: {probe: ${entry}}}}`,
      "",
    ].join("\n");
  }
  if (style === "block") {
    if (position === "direct") {
      return [
        "lockfileVersion: '9.0'",
        "importers:",
        "  .:",
        "    dependencies:",
        `      probe: ${entry}`,
        "",
      ].join("\n");
    }
    const field = position === "specifier" ? "specifier" : "version";
    const otherField = field === "specifier" ? "version" : "specifier";
    return [
      "lockfileVersion: '9.0'",
      "importers:",
      "  .:",
      "    dependencies:",
      "      probe:",
      `        ${field}: ${scalar}`,
      `        ${otherField}: '1.0.0'`,
      "",
    ].join("\n");
  }
  assert.fail(`unsupported scalar-kind style ${style}`);
};
const parsedProbeValue = (lock, position) => {
  const entry = lock.importers["."].dependencies.probe;
  return position === "direct" ? entry : entry[position];
};

let scalarFixtureCount = 0;
for (const style of scalarKindCorpus.styles) {
  for (const position of scalarKindCorpus.positions) {
    for (const scalar of scalarKindCorpus.scalars) {
      const label = `${style} ${position} ${scalar.name}`;
      const plainFixture = scalarFixture(style, position, scalar.plain);
      const parsedPlain = parsePnpmLockYaml(plainFixture, label);
      const plainValue = parsedProbeValue(parsedPlain, position);
      if (scalar.kind === "null") {
        assert.equal(plainValue, null, `${label}: null kind must be preserved`);
      } else if (scalar.kind === "date") {
        assert.ok(plainValue instanceof Date, `${label}: timestamp kind must be preserved`);
        assert.equal(plainValue.toISOString(), scalar.iso, `${label}: timestamp value must match pnpm`);
      } else if (scalar.kind === "number") {
        assert.equal(typeof plainValue, "number", `${label}: numeric kind must be preserved`);
        assert.equal(plainValue, scalar.value, `${label}: numeric value must match pnpm`);
      } else {
        assert.equal(typeof plainValue, scalar.kind, `${label}: scalar kind must be preserved`);
      }
      assert.throws(
        () => assertPnpmLockSourcePolicy(plainFixture, label),
        undefined,
        `${label}: unquoted non-string dependency source must reject`,
      );

      const quotedFixture = scalarFixture(style, position, JSON.stringify(scalar.plain));
      const parsedQuoted = parsePnpmLockYaml(quotedFixture, `${label} quoted`);
      assert.equal(
        parsedProbeValue(parsedQuoted, position),
        scalar.plain,
        `${label}: explicitly quoted lookalike must remain a string`,
      );
      assert.doesNotThrow(
        () => assertPnpmLockSourcePolicy(quotedFixture, `${label} quoted`),
        `${label}: quoted source lookalike must remain allowed`,
      );
      scalarFixtureCount += 2;
    }
  }
}

console.log(
  `Cargo/pnpm structural source-policy corpora passed (${fixtureCount} files + ${scalarFixtureCount} scalar-kind cases).`,
);
