import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { parseFixtureDocument } from "./index.js";

const fixtureUrl = new URL("../../../schemas/fixtures/signed-batch-v1.json", import.meta.url);

void test("the cross-language fixture is synthetic and structurally valid", async () => {
  const text = await readFile(fixtureUrl, "utf8");
  const input: unknown = JSON.parse(text) as unknown;
  const fixture = parseFixtureDocument(input);

  assert.equal(fixture.fixture_version, 1);
  assert.equal(fixture.data_class, "SYNTHETIC_ONLY");
  assert.equal(fixture.network_egress, "NONE");
  assert.equal(fixture.expected_replay.mastery, "PRACTICED");
  assert.equal(fixture.expected_replay.freshness, "STALE");
  assert.equal(fixture.expected_replay.accepted_events, 13);
});

void test("const, minimum, uniqueness, and additional-property violations fail closed", async () => {
  const text = await readFile(fixtureUrl, "utf8");
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
