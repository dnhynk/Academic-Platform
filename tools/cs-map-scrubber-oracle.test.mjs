// The committed scrubber expectation is what the oracle renders today, and the
// oracle is doing work.
//
// `crates/cs-map/tests/cs_map.rs` compares the Rust scrubber against
// `testdata/cs-map/scrubber.expected`. That comparison is only worth anything
// while the file is what `tools/cs-map-scrubber-oracle.mjs` produces, so this
// re-renders it and requires a byte-identical result.
//
// The two controls below are what make the file more than a snapshot of
// whatever the oracle happened to say. Both are about the pair of bitemporal
// coordinates `P2-C6` fixed: a reader that admitted an event on the acceptance
// sequence alone, or on the valid instant alone, would agree with the fixture at
// most readings and disagree at these two.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const ORACLE = join("tools", "cs-map-scrubber-oracle.mjs");
const EXPECTED = join("testdata", "cs-map", "scrubber.expected");

/** The committed file, as a `key -> value` map. */
async function committed() {
  const text = await readFile(join(REPOSITORY_ROOT, EXPECTED), "utf8");
  const rows = new Map();
  for (const line of text.split("\n")) {
    if (line.trim() === "") {
      continue;
    }
    const at = line.indexOf("=");
    assert.ok(at > 0, `oracle line has no '=': ${line}`);
    rows.set(line.slice(0, at), line.slice(at + 1));
  }
  return rows;
}

test("cs_map_scrubber_expectation_is_a_fresh_render", () => {
  const check = spawnSync(process.execPath, [ORACLE, "--check"], {
    cwd: REPOSITORY_ROOT,
    encoding: "utf8",
  });
  assert.equal(
    check.status,
    0,
    `${EXPECTED} differs from a fresh render:\n${check.stdout}${check.stderr}`,
  );
});

test("cs_map_scrubber_expectation_reads_both_bitemporal_axes", async () => {
  const rows = await committed();

  // Two lines per reading, and the readings are the ones the Rust suite asks
  // for. A file that lost half its lines would still satisfy a spot check.
  assert.equal(rows.size, 16, `expected sixteen rows, found ${rows.size}`);

  // Control one: `concept.logging` becomes visible at acceptance 20 and valid
  // 5000. Reading `30/3000` is past its acceptance and behind its valid
  // instant, so a reader that admitted on acceptance alone would show it and
  // this file says it is not there. Reading `25/5000` is behind its acceptance
  // and at its valid instant, and it *is* there — so the two readings differ,
  // and they differ in the direction only a two-axis reader produces.
  const behindValid = rows.get("visible@30/3000").split(",").filter(Boolean);
  const pastValid = rows.get("visible@25/5000").split(",").filter(Boolean);
  const onlyInPastValid = pastValid.filter((id) => !behindValid.includes(id));
  assert.ok(
    onlyInPastValid.length > 0,
    "the valid-at axis changes nothing between 30/3000 and 25/5000",
  );
  const onlyInBehindValid = behindValid.filter((id) => !pastValid.includes(id));
  assert.ok(
    onlyInBehindValid.length > 0,
    "the known-at axis changes nothing between 30/3000 and 25/5000",
  );

  // Control two: the earliest reading is before every event, so it holds
  // nothing. A projection that leaked a later event into it would show here.
  assert.equal(rows.get("visible@5/500"), "");
  assert.equal(rows.get("entered@5/500"), "");

  // Every visible node carries a reason, and the reason vocabulary is closed.
  const named = new Set([
    "EVIDENCE_CHANGE",
    "ONTOLOGY_CHANGE",
    "ANALYZER_UPGRADE",
    "OFFICIAL_SOURCE_CORRECTION",
    "USER_SCOPE_CHANGE",
  ]);
  const seen = new Set();
  for (const [key, value] of rows) {
    if (!key.startsWith("entered@")) {
      continue;
    }
    const visible = rows.get(key.replace("entered@", "visible@")).split(",").filter(Boolean);
    const entries = value.split(",").filter(Boolean);
    assert.equal(entries.length, visible.length, `${key} and its visible row disagree in length`);
    for (const entry of entries) {
      const [id, reason] = entry.split(":");
      assert.ok(visible.includes(id), `${key} names ${id}, which is not visible`);
      assert.ok(named.has(reason), `${key} carries an unnamed reason ${reason}`);
      seen.add(reason);
    }
  }
  assert.deepEqual([...seen].sort(), [...named].sort(), "a reason is declared and never rendered");
});
