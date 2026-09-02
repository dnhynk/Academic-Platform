// The independent GPA oracle is committed and re-derivable.
//
// `testdata/engines/gpa/oracle.expected` is what `gpa_formula_fixture` compares
// the Rust implementation against. It is only worth comparing against if it
// came from somewhere else, so two things have to hold and this asserts both:
//
//   1. the committed file is exactly what a fresh oracle render produces, so it
//      cannot be hand-edited into agreement with a broken engine;
//   2. the oracle names values `crates/record` does not produce, so a change on
//      the Rust side moves one side of the comparison and not the other.
//
// The second is not assertable from here — it is a property of where the
// numbers came from, and `tools/gpa-oracle.mjs` is a separate transcription of
// the corpus in a separate arithmetic for exactly that reason. What is
// assertable is that the file is non-empty, carries the rows the fixture reads,
// and re-renders identically.

import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import test from "node:test";

test("gpa_oracle_is_committed_and_re_derivable", async () => {
  const committed = await readFile("testdata/engines/gpa/oracle.expected", "utf8");
  assert.ok(committed.length > 0, "the committed oracle block is empty");

  const rows = new Map(
    committed
      .split("\n")
      .filter((line) => line.length > 0)
      .map((line) => {
        const index = line.indexOf("=");
        assert.notEqual(index, -1, `oracle line has no '=': ${line}`);
        return [line.slice(0, index), line.slice(index + 1)];
      }),
  );

  // The rows `gpa_formula_fixture` and `repeat_ceiling_effective_date` read. A
  // renamed key would make those tests fail loudly rather than skip a check,
  // but naming them here says which rows the Rust side depends on.
  for (const key of [
    "cumulative.gpa.scale2",
    "cumulative.gpa.scale3",
    "cumulative.quality_points",
    "cumulative.denominator_credits",
    "cumulative.earned_credits",
    "major.cse.gpa",
    "major.stat.gpa",
    "ceiling_from.2016_SPRING.cumulative.gpa",
    "ceiling_from.2014_SPRING.cumulative.gpa",
  ]) {
    assert.ok(rows.has(key), `the oracle no longer carries ${key}`);
  }

  // The tie the corpus is built to land on: 33.9 / 12 is exactly 2.825.
  // Half away from zero publishes 2.83; the nearest f64 to 2.825 is below it,
  // so a floating-point implementation publishes 2.82. This asserts the oracle
  // itself did not drift onto the float answer.
  assert.equal(rows.get("cumulative.gpa.scale2"), "2.83");
  assert.equal(rows.get("cumulative.gpa.scale3"), "2.825");
  assert.notEqual(
    (33.9 / 12).toFixed(2),
    rows.get("cumulative.gpa.scale2"),
    "the float answer and the exact answer must differ, or this corpus detects nothing",
  );

  const rerender = spawnSync(process.execPath, ["tools/gpa-oracle.mjs", "--check"], {
    encoding: "utf8",
  });
  assert.equal(
    rerender.status,
    0,
    `the committed oracle does not match a fresh render: ${rerender.stdout}${rerender.stderr}`,
  );
});
