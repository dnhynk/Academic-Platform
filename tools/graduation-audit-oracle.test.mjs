// The independent graduation-audit oracle is committed and re-derivable.
//
// `testdata/engines/graduation_audit/oracle.expected` is what
// `the_baseline_tree_agrees_with_an_independent_oracle` compares the Rust proof
// tree against. It is only worth comparing against if it came from somewhere
// else, so two things have to hold and this asserts both:
//
//   1. the committed file is exactly what a fresh oracle render produces, so it
//      cannot be hand-edited into agreement with a broken engine;
//   2. the oracle names statuses and measures `crates/audit` does not produce,
//      so a change on the Rust side moves one side of the comparison and not
//      the other.
//
// The second is not assertable from here -- it is a property of where the
// values came from, and `tools/graduation-audit-oracle.mjs` is a separate
// transcription of the transcript, the grade table, the repeat ceiling and the
// rules for exactly that reason. What is assertable is that the file is
// non-empty, carries the rows the Rust test reads, and re-renders identically.

import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import test from "node:test";

test("graduation_audit_oracle_is_committed_and_re_derivable", async () => {
  const committed = await readFile(
    "testdata/engines/graduation_audit/oracle.expected",
    "utf8",
  );
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

  // The rows the Rust comparison reads. A renamed key would make that test fail
  // loudly rather than skip a check, but naming them here says which rows the
  // Rust side depends on.
  for (const key of [
    "status.total_credits",
    "measure.total_credits",
    "status.cse_major_total",
    "measure.cse_major_total",
    "status.required_course_set",
    "operand.required_course_set.op.001",
    "operand.seminar_choice.op.000",
    "status.seminar_choice",
    "status.foreign_language_lectures",
    "status.overall_gpa",
    "status.major_exclusive",
    "status.equivalency_shared",
    "gpa.weighted_points_tenths",
    "gpa.denominator_credits",
    "root.status",
    "verdict",
    "outcome",
    "earned_credits",
  ]) {
    assert.ok(rows.has(key), `the oracle no longer carries ${key}`);
  }

  // The repeat ceiling is the row that separates implementations: without it
  // the repeated `A+` contributes 4.3 rather than 4.0 and the weighted total is
  // 348 tenths rather than 339. This asserts the oracle itself applied it.
  assert.equal(rows.get("gpa.weighted_points_tenths"), "339");
  assert.notEqual(
    rows.get("gpa.weighted_points_tenths"),
    "348",
    "the oracle did not apply the 2015 repeat ceiling, so it detects nothing",
  );

  // Earned credits and the grade-point denominator are different quantities on
  // this corpus -- 14 against 12 -- which is what says the oracle kept the two
  // apart rather than computing one and printing it twice.
  assert.equal(rows.get("earned_credits"), "14");
  assert.equal(rows.get("gpa.denominator_credits"), "12");
  assert.notEqual(rows.get("earned_credits"), rows.get("gpa.denominator_credits"));

  const rerender = spawnSync(
    process.execPath,
    ["tools/graduation-audit-oracle.mjs", "--check"],
    { encoding: "utf8" },
  );
  assert.equal(
    rerender.status,
    0,
    `the committed oracle does not match a fresh render: ${rerender.stdout}${rerender.stderr}`,
  );
});
