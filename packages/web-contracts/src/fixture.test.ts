import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { parseArtifactDescriptorJson, parseFixtureDocument } from "./index.js";

const fixtureV1Url = new URL("../../../schemas/fixtures/signed-batch-v1.json", import.meta.url);
const fixtureV2Url = new URL("../../../schemas/fixtures/signed-batch-v2.json", import.meta.url);
const immutableV1Sha256 = "287f7dea8fd24c3c6eb205c3f1e2873f6afdf7d6532fe7be4fccfb44a0b7e163";
const artifactCorpusUrl = new URL(
  "../../../schemas/fixtures/artifact-descriptor-parity-v1.json",
  import.meta.url,
);

void test("the immutable v1 and current v2 fixtures are synthetic and structurally valid", async () => {
  const [v1Bytes, v2Text] = await Promise.all([readFile(fixtureV1Url), readFile(fixtureV2Url, "utf8")]);
  assert.equal(createHash("sha256").update(v1Bytes).digest("hex"), immutableV1Sha256);

  for (const [version, text] of [[1, v1Bytes.toString("utf8")], [2, v2Text]] as const) {
    const input: unknown = JSON.parse(text) as unknown;
    const fixture = parseFixtureDocument(input);

    assert.equal(fixture.fixture_version, version);
    assert.equal(fixture.data_class, "SYNTHETIC_ONLY");
    assert.equal(fixture.network_egress, "NONE");
    assert.equal(fixture.expected_replay.mastery, "PRACTICED");
    assert.equal(fixture.expected_replay.freshness, "STALE");
    assert.equal(fixture.expected_replay.accepted_events, 13);
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

void test("raw artifact validation rejects unknown fields in every evidence locator", async () => {
  const corpus = JSON.parse(await readFile(artifactCorpusUrl, "utf8")) as {
    readonly base: unknown;
    readonly cases: readonly {
      readonly name: string;
      readonly mutations: readonly { readonly value: unknown }[];
    }[];
  };
  const unknownLocatorCases = corpus.cases.filter((entry) =>
    entry.name.startsWith("unknown field in "),
  );
  assert.equal(unknownLocatorCases.length, 4);
  for (const entry of unknownLocatorCases) {
    const mutation = entry.mutations[0];
    const candidate = structuredClone(corpus.base) as {
      evidence_representations: { locator: unknown }[];
    };
    const representation = candidate.evidence_representations[0];
    if (mutation === undefined || representation === undefined) {
      assert.fail(`${entry.name} must replace one evidence locator`);
    }
    representation.locator = structuredClone(mutation.value);
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
