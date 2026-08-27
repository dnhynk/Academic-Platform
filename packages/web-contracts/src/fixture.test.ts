import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  parseArtifactDescriptorJson,
  parseFixtureDocument,
  parseFixtureDocumentJson,
} from "./index.js";

const fixtureV1Url = new URL("../../../schemas/fixtures/signed-batch-v1.json", import.meta.url);
const fixtureV2Url = new URL("../../../schemas/fixtures/signed-batch-v2.json", import.meta.url);
const immutableV1Sha256 = "287f7dea8fd24c3c6eb205c3f1e2873f6afdf7d6532fe7be4fccfb44a0b7e163";
const artifactCorpusUrl = new URL(
  "../../../schemas/fixtures/artifact-descriptor-parity-v1.json",
  import.meta.url,
);
const fixtureRawCorpusUrl = new URL(
  "../../../schemas/fixtures/signed-batch-raw-parity-v1.json",
  import.meta.url,
);

void test("the immutable v1 and current v2 fixtures are synthetic and structurally valid", async () => {
  const [v1Bytes, v2Text] = await Promise.all([readFile(fixtureV1Url), readFile(fixtureV2Url, "utf8")]);
  assert.equal(createHash("sha256").update(v1Bytes).digest("hex"), immutableV1Sha256);

  for (const [version, text] of [[1, v1Bytes.toString("utf8")], [2, v2Text]] as const) {
    const fixture = parseFixtureDocumentJson(text);

    assert.equal(fixture.fixture_version, version);
    assert.equal(fixture.data_class, "SYNTHETIC_ONLY");
    assert.equal(fixture.network_egress, "NONE");
    assert.equal(fixture.expected_replay.mastery, "PRACTICED");
    assert.equal(fixture.expected_replay.freshness, "STALE");
    assert.equal(fixture.expected_replay.accepted_events, 13);
  }
});

void test("shared raw v1/v2 fixture corpus rejects duplicates and surrogates before semantics", async () => {
  const [corpusText, fixtureV1Text, fixtureV2Text] = await Promise.all([
    readFile(fixtureRawCorpusUrl, "utf8"),
    readFile(fixtureV1Url, "utf8"),
    readFile(fixtureV2Url, "utf8"),
  ]);
  const corpus = JSON.parse(corpusText) as {
    readonly schema_version: number;
    readonly cases: readonly {
      readonly name: string;
      readonly fixture: 1 | 2;
      readonly valid: boolean;
      readonly replacements: readonly {
        readonly needle: string;
        readonly replacement: string;
      }[];
    }[];
  };
  assert.equal(corpus.schema_version, 1);
  assert.ok(corpus.cases.some((entry) => entry.name.includes("duplicate")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("surrogate")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("decimal")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("exponent")));
  for (const entry of corpus.cases) {
    let raw = entry.fixture === 1 ? fixtureV1Text : fixtureV2Text;
    for (const replacement of entry.replacements) {
      const next = raw.replace(replacement.needle, replacement.replacement);
      assert.notEqual(next, raw, `${entry.name}: replacement must mutate the fixture`);
      raw = next;
    }
    if (entry.valid) {
      const parsed = parseFixtureDocumentJson(raw);
      assert.equal(parsed.fixture_version, entry.fixture, entry.name);
    } else {
      assert.throws(() => parseFixtureDocumentJson(raw), TypeError, entry.name);
    }
  }
});

void test("const, minimum, uniqueness, and additional-property violations fail closed", async () => {
  const text = await readFile(fixtureV2Url, "utf8");
  const fixture: unknown = JSON.parse(text) as unknown;
  assert.equal(typeof fixture, "object");
  const mutations: readonly ((value: Record<string, unknown>) => void)[] = [
    (value) => { value.name = ""; },
    (value) => { value.data_class = "PERSONAL"; },
    (value) => { value.unexpected = true; },
    (value) => {
      const contract = value.contract as Record<string, unknown>;
      contract.payload = "wrong";
    },
    (value) => {
      value.fixture_version = 1;
    },
    (value) => {
      const replay = value.expected_replay as Record<string, unknown>;
      replay.accepted_events = 0;
    },
    (value) => {
      const replay = value.expected_replay as Record<string, unknown>;
      const ids = replay.mastery_active_claim_ids as string[];
      const first = ids[0];
      if (first !== undefined) {
        ids.push(first);
      }
    },
  ];
  for (const mutate of mutations) {
    const candidate = structuredClone(fixture) as Record<string, unknown>;
    mutate(candidate);
    assert.throws(() => parseFixtureDocument(candidate), TypeError);
  }
});

void test("raw artifact validation rejects unknown fields at every descriptor boundary", async () => {
  const corpus = JSON.parse(await readFile(artifactCorpusUrl, "utf8")) as {
    readonly base: unknown;
    readonly cases: readonly {
      readonly name: string;
      readonly mutations: readonly {
        readonly op: string;
        readonly path: string;
        readonly value: unknown;
      }[];
    }[];
  };
  const unknownPropertyCases = corpus.cases.filter((entry) =>
    entry.name.startsWith("unknown field"),
  );
  assert.equal(unknownPropertyCases.length, 6);
  for (const entry of unknownPropertyCases) {
    const mutation = entry.mutations[0];
    if (mutation === undefined) {
      assert.fail(`${entry.name} must contain one unknown-property mutation`);
    }
    const candidate = structuredClone(corpus.base) as Record<string, unknown>;
    const components = mutation.path.split("/").slice(1);
    let target: unknown = candidate;
    for (const component of components.slice(0, -1)) {
      if (typeof target !== "object" || target === null) {
        assert.fail(`${entry.name}: invalid mutation path`);
      }
      target = (target as Record<string, unknown>)[component];
    }
    const finalComponent = components.at(-1);
    if (typeof target !== "object" || target === null || finalComponent === undefined) {
      assert.fail(`${entry.name}: invalid mutation target`);
    }
    (target as Record<string, unknown>)[finalComponent] = structuredClone(mutation.value);
    assert.throws(() => parseArtifactDescriptorJson(JSON.stringify(candidate)), TypeError);
  }
});

void test("raw artifact validation rejects duplicate keys and non-scalar strings", async () => {
  const corpus = JSON.parse(await readFile(artifactCorpusUrl, "utf8")) as {
    readonly raw_json_cases: readonly {
      readonly name: string;
      readonly raw_json: string;
      readonly valid: boolean;
    }[];
  };
  assert.ok(corpus.raw_json_cases.some((entry) => entry.name.includes("duplicate")));
  assert.ok(corpus.raw_json_cases.some((entry) => entry.name.includes("surrogate")));
  for (const entry of corpus.raw_json_cases) {
    if (entry.valid) {
      assert.doesNotThrow(() => parseArtifactDescriptorJson(entry.raw_json), entry.name);
    } else {
      assert.throws(() => parseArtifactDescriptorJson(entry.raw_json), TypeError, entry.name);
    }
  }
});
