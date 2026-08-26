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
  assert.equal(fixture.expected_replay.accepted_events, 12);
});

void test("unknown or unsafe fixture policy values fail closed", () => {
  assert.throws(
    () =>
      parseFixtureDocument({
        fixture_version: 1,
        data_class: "PERSONAL",
        network_egress: "ALLOWED",
      }),
    /synthetic and offline/u,
  );
});
