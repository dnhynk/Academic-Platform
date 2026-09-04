// The independent offering-forecast oracle is committed and re-derivable.
//
// `testdata/offering-forecast/oracle.expected` is what
// `the_corpus_agrees_with_an_independent_oracle` and `term_forecast_metrics`
// compare the Rust engine against. It is only worth comparing against if it
// came from somewhere else, so two things have to hold and this asserts both:
//
//   1. the committed file is exactly what a fresh oracle render produces, so it
//      cannot be hand-edited into agreement with a broken forecaster;
//   2. the oracle carries the rows the Rust tests read, and its own numbers
//      separate implementations rather than agreeing with anything.
//
// The second is not fully assertable from here -- it is a property of where the
// values came from, and `tools/offering-forecast-oracle.mjs` is a separate
// transcription of the histories, the rule set and the calibration curve for
// exactly that reason. What is assertable is the row that would catch a
// majority-vote forecaster: `gap_two` and `every_other_spring` have the same
// seasonal rate, window depth, instructor set and notices, and a vote over the
// last N terms would answer them identically. This requires the oracle to give
// them different answers.

import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import test from "node:test";

test("offering_forecast_oracle_is_committed_and_re_derivable", async () => {
  const committed = await readFile("testdata/offering-forecast/oracle.expected", "utf8");
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

  // The rows the Rust comparison reads. A renamed key makes that test fail
  // loudly rather than skip a check; naming them here says which rows the Rust
  // side depends on.
  for (const key of [
    "case.every_spring.raw_units",
    "case.every_spring.calibrated_permille",
    "case.every_spring.standing",
    "case.never_observed.abstention",
    "case.gap_two.raw_units",
    "case.every_other_spring.raw_units",
    "metrics.total",
    "metrics.scored",
    "metrics.abstained",
    "metrics.resolved",
    "metrics.abstention_permille",
    "metrics.coverage_permille",
    "metrics.brier_numerator",
    "metrics.brier_denominator",
    "metrics.brier_per_million_floor",
    "metrics.missing_outcomes",
  ]) {
    assert.ok(rows.has(key), `the oracle no longer carries ${key}`);
  }

  // Every case carries a value and a contribution for all seven families, so a
  // family dropped from the oracle fails here rather than being skipped by the
  // Rust loop.
  const FAMILIES = [
    "seasonality",
    "lifecycle_status",
    "instructor_change",
    "recent_notices",
    "offering_gap",
    "irregular_special",
    "history_window",
  ];
  const cases = [...rows.keys()]
    .filter((key) => key.endsWith(".raw_units"))
    .map((key) => key.slice("case.".length, -".raw_units".length));
  assert.ok(cases.length >= 10, `the oracle renders only ${cases.length} cases`);
  for (const name of cases) {
    for (const family of FAMILIES) {
      assert.ok(rows.has(`case.${name}.value.${family}`), `${name} has no ${family} value`);
      assert.ok(
        rows.has(`case.${name}.contribution.${family}`),
        `${name} has no ${family} contribution`,
      );
    }
  }

  // The row that separates a seasonal forecaster from a majority vote. The two
  // cases agree on the seasonal rate and disagree on the answer.
  assert.equal(rows.get("case.gap_two.value.seasonality"), "500");
  assert.equal(rows.get("case.every_other_spring.value.seasonality"), "500");
  assert.equal(rows.get("case.gap_two.window.seasonal_terms"), "4");
  assert.equal(rows.get("case.every_other_spring.window.seasonal_terms"), "4");
  assert.notEqual(rows.get("case.gap_two.standing"), rows.get("case.every_other_spring.standing"));

  // And the seasonal window itself: one history read for two semesters is two
  // different questions.
  assert.equal(rows.get("case.every_spring.standing"), "HISTORICALLY_LIKELY");
  assert.equal(rows.get("case.spring_only_asked_for_autumn.standing"), "UNCERTAIN");
  assert.equal(rows.get("case.spring_only_asked_for_autumn.abstention"), "NEVER_OBSERVED");

  // Coverage and abstention are different quantities on this corpus -- 400
  // against 500 -- which is what says the oracle kept the two apart rather than
  // computing one and printing it twice.
  assert.equal(rows.get("metrics.coverage_permille"), "400");
  assert.equal(rows.get("metrics.abstention_permille"), "500");
  assert.notEqual(rows.get("metrics.coverage_permille"), rows.get("metrics.abstention_permille"));

  // A Brier score over four resolved forecasts, computed in exact integers.
  assert.equal(rows.get("metrics.brier_denominator"), "4");
  assert.equal(rows.get("metrics.brier_numerator"), "274300");
  assert.equal(rows.get("metrics.missing_outcomes"), "M9001.000900");

  const rerender = spawnSync(process.execPath, ["tools/offering-forecast-oracle.mjs", "--check"], {
    encoding: "utf8",
  });
  assert.equal(
    rerender.status,
    0,
    `the committed oracle does not match a fresh render: ${rerender.stdout}${rerender.stderr}`,
  );
});
