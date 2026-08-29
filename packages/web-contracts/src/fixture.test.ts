import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  decodePortableFixtureJsonBytes,
  parseArtifactDescriptorJson,
  parseFixtureDocument,
  parseFixtureDocumentJson,
} from "./index.js";

const fixtureV1Url = new URL("../../../schemas/fixtures/signed-batch-v1.json", import.meta.url);
const fixtureV2Url = new URL("../../../schemas/fixtures/signed-batch-v2.json", import.meta.url);
const fixtureV3Url = new URL("../../../schemas/fixtures/signed-batch-v3.json", import.meta.url);
const immutableV1Sha256 = "287f7dea8fd24c3c6eb205c3f1e2873f6afdf7d6532fe7be4fccfb44a0b7e163";
const immutableV2Sha256 = "f94dfcf7e3e376e54b5514ceb3016b0b7d97d17366562f7ac4a16286d3aa367d";
const artifactCorpusUrl = new URL(
  "../../../schemas/fixtures/artifact-descriptor-parity-v1.json",
  import.meta.url,
);
const fixtureRawCorpusUrl = new URL(
  "../../../schemas/fixtures/signed-batch-raw-parity-v1.json",
  import.meta.url,
);
const fixtureIntegerCorpusUrl = new URL(
  "../../../schemas/fixtures/signed-batch-integer-lexeme-parity-v1.json",
  import.meta.url,
);
const fixtureByteCorpusUrl = new URL(
  "../../../schemas/fixtures/signed-batch-byte-parity-v1.json",
  import.meta.url,
);
const predictionMetadataCorpusUrl = new URL(
  "../../../schemas/fixtures/prediction-metadata-parity-v1.json",
  import.meta.url,
);

function replaceBytesOnce(
  source: Uint8Array,
  needleUtf8: string,
  replacementHex: string,
  label: string,
): Buffer {
  assert.match(replacementHex, /^(?:[0-9a-f]{2})+$/u, `${label}: canonical replacement hex`);
  const sourceBuffer = Buffer.from(source);
  const needle = Buffer.from(needleUtf8, "utf8");
  const index = sourceBuffer.indexOf(needle);
  assert.ok(index >= 0, `${label}: byte needle must exist`);
  assert.equal(sourceBuffer.indexOf(needle, index + needle.length), -1, `${label}: byte needle must be unique`);
  return Buffer.concat([
    sourceBuffer.subarray(0, index),
    Buffer.from(replacementHex, "hex"),
    sourceBuffer.subarray(index + needle.length),
  ]);
}

void test("the immutable v1 and v2 fixtures and the current v3 fixture are synthetic and structurally valid", async () => {
  const [v1Bytes, v2Bytes, v3Bytes] = await Promise.all([
    readFile(fixtureV1Url),
    readFile(fixtureV2Url),
    readFile(fixtureV3Url),
  ]);
  assert.equal(createHash("sha256").update(v1Bytes).digest("hex"), immutableV1Sha256);
  assert.equal(createHash("sha256").update(v2Bytes).digest("hex"), immutableV2Sha256);

  for (const [version, bytes] of [[1, v1Bytes], [2, v2Bytes], [3, v3Bytes]] as const) {
    const fixture = parseFixtureDocumentJson(bytes);

    assert.equal(fixture.fixture_version, version);
    assert.equal(fixture.data_class, "SYNTHETIC_ONLY");
    assert.equal(fixture.network_egress, "NONE");
    assert.equal(fixture.expected_replay.mastery, "PRACTICED");
    assert.equal(fixture.expected_replay.freshness, "STALE");
    assert.equal(
      fixture.expected_replay.accepted_events,
      version === 1 ? 13 : version === 2 ? 14 : 32,
    );
    if (version === 1) {
      assert.equal(fixture.expected_replay.prediction_claims, undefined);
    } else {
      const disclosures = fixture.expected_replay.prediction_claims;
      assert.ok(disclosures !== undefined);
      assert.equal(disclosures.length, 1);
      const disclosure = disclosures[0];
      assert.ok(disclosure !== undefined);
      assert.equal(disclosure.confidence, 720);
      assert.equal(disclosure.prediction_metadata.version, 1);
      assert.equal(disclosure.prediction_metadata.positive_sample_count, 6);
      assert.deepEqual(disclosure.prediction_metadata.observation_window, {
        from: 100,
        to: 700,
      });
      assert.deepEqual(disclosure.valid_time, { from: 800, to: 1200 });
      assert.notDeepEqual(
        disclosure.prediction_metadata.observation_window,
        disclosure.valid_time,
      );
    }
  }
});

void test("shared raw and exact-integer fixture corpora reject before semantics", async () => {
  const [corpusText, integerCorpusText, fixtureV1Text, fixtureV2Text] = await Promise.all([
    readFile(fixtureRawCorpusUrl, "utf8"),
    readFile(fixtureIntegerCorpusUrl, "utf8"),
    readFile(fixtureV1Url),
    readFile(fixtureV2Url),
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
  const integerCorpus = JSON.parse(integerCorpusText) as typeof corpus;
  assert.equal(corpus.schema_version, 1);
  assert.equal(integerCorpus.schema_version, 1);
  assert.ok(corpus.cases.some((entry) => entry.name.includes("duplicate")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("surrogate")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("decimal")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("exponent")));
  assert.ok(integerCorpus.cases.some((entry) => entry.name.includes("high-precision integral")));
  assert.ok(integerCorpus.cases.some((entry) => entry.name.includes("high-precision fractional")));
  assert.ok(integerCorpus.cases.some((entry) => entry.name.includes("prediction confidence")));
  assert.ok(integerCorpus.cases.some((entry) => entry.name.includes("prediction metadata version")));
  assert.ok(integerCorpus.cases.some((entry) => entry.name.includes("prediction sample count")));
  assert.ok(integerCorpus.cases.some((entry) => entry.name.includes("prediction observation")));
  assert.ok(integerCorpus.cases.some((entry) => entry.name.includes("prediction applicability")));
  for (const entry of [...corpus.cases, ...integerCorpus.cases]) {
    let raw = decodePortableFixtureJsonBytes(entry.fixture === 1 ? fixtureV1Text : fixtureV2Text);
    for (const replacement of entry.replacements) {
      const next = raw.replace(replacement.needle, replacement.replacement);
      assert.notEqual(next, raw, `${entry.name}: replacement must mutate the fixture`);
      raw = next;
    }
    if (entry.valid) {
      const parsed = parseFixtureDocumentJson(Buffer.from(raw, "utf8"));
      assert.equal(parsed.fixture_version, entry.fixture, entry.name);
    } else {
      assert.throws(() => parseFixtureDocumentJson(Buffer.from(raw, "utf8")), TypeError, entry.name);
    }
  }
});

void test("shared byte corpus rejects malformed UTF-8 before JSON and semantics", async () => {
  const [corpusText, fixtureV1Bytes, fixtureV2Bytes] = await Promise.all([
    readFile(fixtureByteCorpusUrl, "utf8"),
    readFile(fixtureV1Url),
    readFile(fixtureV2Url),
  ]);
  const corpus = JSON.parse(corpusText) as {
    readonly schema_version: number;
    readonly cases: readonly {
      readonly name: string;
      readonly fixture: 1 | 2;
      readonly valid: boolean;
      readonly replacements: readonly {
        readonly needle_utf8: string;
        readonly replacement_hex: string;
      }[];
    }[];
  };
  assert.equal(corpus.schema_version, 1);
  assert.ok(corpus.cases.some((entry) => entry.name.includes("ff byte")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("truncated")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("overlong")));
  assert.ok(corpus.cases.some((entry) => entry.name.includes("surrogate")));
  for (const entry of corpus.cases) {
    let bytes: Uint8Array = Buffer.from(entry.fixture === 1 ? fixtureV1Bytes : fixtureV2Bytes);
    for (const replacement of entry.replacements) {
      bytes = replaceBytesOnce(
        bytes,
        replacement.needle_utf8,
        replacement.replacement_hex,
        entry.name,
      );
    }
    if (entry.valid) {
      assert.doesNotThrow(() => decodePortableFixtureJsonBytes(bytes), entry.name);
      assert.equal(parseFixtureDocumentJson(bytes).fixture_version, entry.fixture, entry.name);
    } else {
      assert.throws(() => decodePortableFixtureJsonBytes(bytes), TypeError, entry.name);
      assert.throws(() => parseFixtureDocumentJson(bytes), TypeError, entry.name);
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

void test("v2 prediction disclosure is required, bounded, versioned, and semantically validated", async () => {
  const text = await readFile(fixtureV2Url, "utf8");
  const fixture = JSON.parse(text) as Record<string, unknown>;
  const mutateDisclosure = (
    candidate: Record<string, unknown>,
    mutate: (disclosure: Record<string, unknown>) => void,
  ): void => {
    const replay = candidate.expected_replay as Record<string, unknown>;
    const disclosures = replay.prediction_claims as Record<string, unknown>[];
    const disclosure = disclosures[0];
    assert.ok(disclosure !== undefined);
    mutate(disclosure);
  };
  const mutations: readonly ((candidate: Record<string, unknown>) => void)[] = [
    (candidate) => {
      delete (candidate.expected_replay as Record<string, unknown>).prediction_claims;
    },
    (candidate) => {
      (candidate.expected_replay as Record<string, unknown>).prediction_claims = [];
    },
    (candidate) => {
      mutateDisclosure(candidate, (disclosure) => {
        disclosure.confidence = 1001;
      });
    },
    (candidate) => {
      mutateDisclosure(candidate, (disclosure) => {
        const metadata = disclosure.prediction_metadata as Record<string, unknown>;
        metadata.version = 2;
      });
    },
    (candidate) => {
      mutateDisclosure(candidate, (disclosure) => {
        const metadata = disclosure.prediction_metadata as Record<string, unknown>;
        metadata.positive_sample_count = 0;
      });
    },
    (candidate) => {
      mutateDisclosure(candidate, (disclosure) => {
        const metadata = disclosure.prediction_metadata as Record<string, unknown>;
        const window = metadata.observation_window as Record<string, unknown>;
        window.to = window.from;
      });
    },
    (candidate) => {
      mutateDisclosure(candidate, (disclosure) => {
        const validTime = disclosure.valid_time as Record<string, unknown>;
        validTime.to = validTime.from;
      });
    },
    (candidate) => {
      mutateDisclosure(candidate, (disclosure) => {
        const metadata = disclosure.prediction_metadata as Record<string, unknown>;
        metadata.unexpected = true;
      });
    },
  ];
  for (const mutate of mutations) {
    const candidate = structuredClone(fixture);
    mutate(candidate);
    assert.throws(() => parseFixtureDocument(candidate), TypeError);
  }

  const boundary = structuredClone(fixture);
  mutateDisclosure(boundary, (disclosure) => {
    disclosure.confidence = 0;
    const metadata = disclosure.prediction_metadata as Record<string, unknown>;
    metadata.positive_sample_count = 4_294_967_295;
    metadata.observation_window = { from: 0, to: 9_007_199_254_740_991 };
    disclosure.valid_time = { from: 0, to: null };
  });
  assert.doesNotThrow(() => parseFixtureDocument(boundary));

  const v1 = JSON.parse(await readFile(fixtureV1Url, "utf8")) as Record<string, unknown>;
  (v1.expected_replay as Record<string, unknown>).prediction_claims = [];
  assert.throws(() => parseFixtureDocument(v1), TypeError);
});

void test("shared prediction metadata corpus matches TypeScript semantics", async () => {
  const [fixtureText, corpusText] = await Promise.all([
    readFile(fixtureV2Url, "utf8"),
    readFile(predictionMetadataCorpusUrl, "utf8"),
  ]);
  const fixture = JSON.parse(fixtureText) as Record<string, unknown>;
  const corpus = JSON.parse(corpusText) as {
    readonly schema_version: number;
    readonly cases: readonly {
      readonly name: string;
      readonly schema_valid: boolean;
      readonly semantic_valid: boolean;
      readonly disclosure: unknown;
    }[];
  };
  assert.equal(corpus.schema_version, 1);
  for (const entry of corpus.cases) {
    const candidate = structuredClone(fixture);
    (candidate.expected_replay as Record<string, unknown>).prediction_claims = [
      structuredClone(entry.disclosure),
    ];
    if (entry.semantic_valid) {
      assert.doesNotThrow(() => parseFixtureDocument(candidate), entry.name);
    } else {
      assert.throws(() => parseFixtureDocument(candidate), TypeError, entry.name);
    }
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
