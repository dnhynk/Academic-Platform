#!/usr/bin/env node
// An independent oracle for the `P2-U5` offering-forecast fixtures.
//
// WHY THIS EXISTS, AND WHY IT IS IN A DIFFERENT LANGUAGE
//
// A forecast checked against numbers the forecaster produced proves only that
// the forecaster is deterministic, which is a different claim -- and it is a
// particularly easy mistake here, because a probability looks like a
// measurement whichever side of the comparison it came from. So every expected
// value below is derived here: a second transcription of the corpus, a second
// transcription of the rule set, a second transcription of the calibration
// curve, and a second arithmetic.
//
// Five things are deliberately independent of the Rust implementation:
//
//   1. **The histories.** The ten cases below were typed from
//      `docs/contracts/offering-forecast.md` and from section 8.3 of the design
//      document, not generated from `academic_offering::corpus`. If that corpus
//      moves a term, a flag or an instructor, this file still says the old
//      answer and the Rust comparison fails.
//   2. **The feature arithmetic.** Rust folds seven `FeatureSignal`s through a
//      shared accumulator. This restates each family's rule directly from the
//      rule-set text and sums them in one expression.
//   3. **The calibration curve.** Rust reaches it through `P2-M1`'s
//      `CalibrationRegistry`. This is the seven-point curve written out and
//      searched by hand.
//   4. **The standing decision.** Rust decides it in `standing::resolve` over
//      typed values. This compares two integers.
//   5. **The metrics.** Rust accumulates a `u64` numerator over
//      `EvaluationEntry` values. This sums squared integer errors over a
//      hand-written outcome table.
//
// The row that separates implementations is `offering_gap`. `gap_two` and
// `every_other_spring` have the same seasonal rate (500 permille), the same
// window depth (4), the same single instructor and no notices; they differ only
// in *where in the window* the two offered terms sit. A forecaster that read
// the last N terms as a majority vote -- which section 8.3 forbids -- would
// give them the same answer. This oracle says 480 raw / 330 permille /
// UNCERTAIN for one and 700 raw / 780 permille / HISTORICALLY_LIKELY for the
// other.
//
// Usage:
//   node tools/offering-forecast-oracle.mjs            # print the expected block
//   node tools/offering-forecast-oracle.mjs --write    # write the expected file
//   node tools/offering-forecast-oracle.mjs --check    # exit non-zero if it differs

import { readFile, writeFile } from "node:fs/promises";
import { argv, exit } from "node:process";

const OUTPUT = "testdata/offering-forecast/oracle.expected";

// ---------------------------------------------------------------------------
// The rule set, transcribed from docs/contracts/offering-forecast.md
// ---------------------------------------------------------------------------

const BASE = 500;
const CLAMP_LOW = 0;
const CLAMP_HIGH = 1000;

/** Recorded criteria. Synthetic and user-confirmed; no source states either. */
const LIKELY_FLOOR_PERMILLE = 600;
const MINIMUM_WINDOW_TERMS = 3;

/** The calibration curve: raw units at or below `upper` read as `permille`. */
const CALIBRATION = [
  { upper: 199, permille: 20 },
  { upper: 399, permille: 150 },
  { upper: 499, permille: 330 },
  { upper: 599, permille: 480 },
  { upper: 699, permille: 620 },
  { upper: 799, permille: 780 },
  { upper: 1000, permille: 910 },
];

// ---------------------------------------------------------------------------
// The corpus, transcribed by hand
// ---------------------------------------------------------------------------
//
// Each history is a list of same-semester readings, **oldest first**, because
// every family that reads position reads it in that direction. `o` is offered,
// `x` is read and empty. `who` names the instructor set of an offered term and
// `special` is section 8.3's 불규칙 특강 flag.

const A = "Instructor A";
const B = "Instructor B";
const C = "Instructor C";
const H = "Instructor H";

/** Six springs, every one offered, one stable instructor. */
const EVERY_SPRING = [
  { o: true, who: A },
  { o: true, who: A },
  { o: true, who: A },
  { o: true, who: A },
  { o: true, who: A },
  { o: true, who: A },
];

/** The same history asked for autumn: six autumn terms, none offered. */
const EVERY_SPRING_AUTUMN_VIEW = [
  { o: false },
  { o: false },
  { o: false },
  { o: false },
  { o: false },
  { o: false },
];

const CASES = [
  {
    case: "every_spring",
    course: "M9001.000100",
    terms: EVERY_SPRING,
    lifecycle: "ESTABLISHED",
    notices: [],
    realized: "OFFERED",
  },
  {
    case: "spring_only_asked_for_autumn",
    course: "M9001.001000",
    terms: EVERY_SPRING_AUTUMN_VIEW,
    lifecycle: "ESTABLISHED",
    notices: [],
    realized: "NOT_OFFERED",
  },
  {
    case: "sparse",
    course: "M9001.000200",
    terms: [
      { o: true, who: B },
      { o: true, who: B },
    ],
    lifecycle: "ESTABLISHED",
    notices: [],
    realized: "OFFERED",
  },
  {
    case: "irregular_only",
    course: "M9001.000300",
    terms: [
      { o: true, who: C, special: true },
      { o: true, who: C, special: true },
      { o: false },
      { o: true, who: C, special: true },
    ],
    lifecycle: "ESTABLISHED",
    notices: [],
    realized: "NOT_OFFERED",
  },
  {
    case: "instructor_volatile",
    course: "M9001.000400",
    terms: [
      { o: true, who: "Instructor G" },
      { o: true, who: "Instructor F" },
      { o: true, who: "Instructor E" },
      { o: true, who: "Instructor D" },
    ],
    lifecycle: "ESTABLISHED",
    notices: [],
    realized: "OFFERED",
  },
  {
    case: "never_observed",
    course: "M9001.000500",
    terms: [{ o: false }, { o: false }, { o: false }, { o: false }],
    lifecycle: "ESTABLISHED",
    notices: [],
    realized: "NOT_OFFERED",
  },
  {
    case: "gap_two",
    course: "M9001.000600",
    terms: [
      { o: true, who: H },
      { o: true, who: H },
      { o: false },
      { o: false },
    ],
    lifecycle: "ESTABLISHED",
    notices: [],
    realized: "NOT_OFFERED",
  },
  {
    case: "every_other_spring",
    course: "M9001.000700",
    terms: [
      { o: false },
      { o: false },
      { o: true, who: H },
      { o: true, who: H },
    ],
    lifecycle: "ESTABLISHED",
    notices: [],
    realized: "OFFERED",
  },
  {
    case: "retired",
    course: "M9001.000800",
    terms: EVERY_SPRING,
    lifecycle: "RETIRED_AT_OR_BEFORE_TARGET",
    notices: [],
    realized: "NOT_OFFERED",
  },
  {
    case: "suspended_notice",
    course: "M9001.000900",
    terms: [
      { o: false },
      { o: false },
      { o: true, who: H },
      { o: true, who: H },
    ],
    lifecycle: "ESTABLISHED",
    notices: ["OFFERING_SUSPENDED"],
    realized: null,
  },
];

// ---------------------------------------------------------------------------
// The seven families
// ---------------------------------------------------------------------------

function seasonality(terms) {
  if (terms.length === 0) {
    return { value: 0, contribution: 0 };
  }
  const positive = terms.filter((term) => term.o).length;
  const value = Math.trunc((positive * 1000) / terms.length);
  return { value, contribution: Math.trunc(((value - 500) * 2) / 5) };
}

function lifecycleStatus(lifecycle) {
  switch (lifecycle) {
    case "UNKNOWN":
      return { value: 0, contribution: 0 };
    case "ESTABLISHED":
      return { value: 1, contribution: 0 };
    case "NEW_STARTED":
      return { value: 2, contribution: 60 };
    case "NEW_NOT_YET":
      return { value: 3, contribution: -500 };
    case "SUNSET_AFTER_TARGET":
      return { value: 4, contribution: -40 };
    case "RETIRED_AT_OR_BEFORE_TARGET":
      return { value: 5, contribution: -500 };
    default:
      throw new Error(`unknown lifecycle ${lifecycle}`);
  }
}

function instructorChange(terms) {
  const sets = [];
  for (const term of terms) {
    if (!term.o) {
      continue;
    }
    if (!sets.includes(term.who)) {
      sets.push(term.who);
    }
  }
  const value = sets.length;
  const contribution = value === 0 ? 0 : value === 1 ? 60 : value === 2 ? -60 : -120;
  return { value, contribution };
}

function recentNotices(notices) {
  let value = 0;
  for (const notice of notices) {
    if (notice === "OFFERING_ANNOUNCED") {
      value += 80;
    } else if (notice === "OFFERING_SUSPENDED") {
      value -= 200;
    } else if (notice === "CURRICULUM_CHANGE") {
      value -= 60;
    } else {
      throw new Error(`unknown notice ${notice}`);
    }
  }
  return { value, contribution: Math.min(300, Math.max(-300, value)) };
}

function offeringGap(terms) {
  let value = 0;
  for (let index = terms.length - 1; index >= 0; index -= 1) {
    const term = terms[index];
    if (term === undefined || term.o) {
      break;
    }
    value += 1;
  }
  const contribution = value === 0 ? 60 : value === 1 ? -60 : value === 2 ? -160 : -260;
  return { value, contribution };
}

function irregularSpecial(terms) {
  const offered = terms.filter((term) => term.o).length;
  const value = terms.filter((term) => term.o && term.special === true).length;
  const contribution = value === 0 ? 0 : value === offered ? -200 : -100;
  return { value, contribution };
}

function historyWindow(terms) {
  const value = terms.length;
  const contribution = value <= 1 ? -150 : value === 2 ? -40 : value === 3 ? 30 : 80;
  return { value, contribution };
}

const FAMILIES = [
  ["seasonality", (entry) => seasonality(entry.terms)],
  ["lifecycle_status", (entry) => lifecycleStatus(entry.lifecycle)],
  ["instructor_change", (entry) => instructorChange(entry.terms)],
  ["recent_notices", (entry) => recentNotices(entry.notices)],
  ["offering_gap", (entry) => offeringGap(entry.terms)],
  ["irregular_special", (entry) => irregularSpecial(entry.terms)],
  ["history_window", (entry) => historyWindow(entry.terms)],
];

// ---------------------------------------------------------------------------
// The forecast
// ---------------------------------------------------------------------------

function calibrate(rawUnits) {
  for (const bin of CALIBRATION) {
    if (rawUnits <= bin.upper) {
      return bin.permille;
    }
  }
  return null;
}

function evaluate(entry) {
  const signals = FAMILIES.map(([name, read]) => [name, read(entry)]);
  const total = signals.reduce((sum, [, signal]) => sum + signal.contribution, BASE);
  const rawUnits = Math.min(CLAMP_HIGH, Math.max(CLAMP_LOW, total));

  const positive = entry.terms.filter((term) => term.o).length;
  const offered = entry.terms.filter((term) => term.o);
  const distinctInstructors = instructorChange(entry.terms).value;

  let abstention = null;
  if (positive === 0) {
    abstention = "NEVER_OBSERVED";
  } else if (entry.terms.length < MINIMUM_WINDOW_TERMS) {
    abstention = "WINDOW_BELOW_RECORDED_MINIMUM";
  } else if (offered.length > 0 && offered.every((term) => term.special === true)) {
    abstention = "IRREGULAR_ONLY";
  } else if (offered.length >= 3 && distinctInstructors === offered.length) {
    abstention = "INSTRUCTOR_VOLATILE";
  }

  const permille = abstention === null ? calibrate(rawUnits) : null;
  const standing =
    abstention !== null
      ? "UNCERTAIN"
      : permille !== null && permille >= LIKELY_FLOOR_PERMILLE
        ? "HISTORICALLY_LIKELY"
        : "UNCERTAIN";

  return { signals, rawUnits, abstention, permille, standing, positive };
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

function main() {
  const lines = [];
  const scored = [];

  for (const entry of CASES) {
    const result = evaluate(entry);
    lines.push(`case.${entry.case}.course=${entry.course}`);
    for (const [name, signal] of result.signals) {
      lines.push(`case.${entry.case}.value.${name}=${signal.value}`);
      lines.push(`case.${entry.case}.contribution.${name}=${signal.contribution}`);
    }
    lines.push(`case.${entry.case}.window.seasonal_terms=${entry.terms.length}`);
    lines.push(`case.${entry.case}.window.positive_samples=${result.positive}`);
    lines.push(`case.${entry.case}.raw_units=${result.rawUnits}`);
    lines.push(
      `case.${entry.case}.calibrated_permille=${result.permille === null ? "ABSTAINED" : result.permille}`,
    );
    lines.push(
      `case.${entry.case}.abstention=${result.abstention === null ? "NONE" : result.abstention}`,
    );
    lines.push(`case.${entry.case}.standing=${result.standing}`);
    if (result.abstention === null && result.permille !== null) {
      scored.push({ entry, permille: result.permille });
    }
  }

  // Per-term evaluation over the whole corpus.
  const total = CASES.length;
  const abstained = CASES.length - scored.length;
  let brierNumerator = 0;
  let resolved = 0;
  const missing = [];
  for (const { entry, permille } of scored) {
    if (entry.realized === null) {
      missing.push(entry.course);
      continue;
    }
    const outcome = entry.realized === "OFFERED" ? 1000 : 0;
    const error = permille - outcome;
    brierNumerator += error * error;
    resolved += 1;
  }

  lines.push(`metrics.total=${total}`);
  lines.push(`metrics.scored=${scored.length}`);
  lines.push(`metrics.abstained=${abstained}`);
  lines.push(`metrics.resolved=${resolved}`);
  lines.push(`metrics.abstention_permille=${Math.trunc((abstained * 1000) / total)}`);
  lines.push(`metrics.coverage_permille=${Math.trunc((resolved * 1000) / total)}`);
  lines.push(`metrics.brier_numerator=${brierNumerator}`);
  lines.push(`metrics.brier_denominator=${resolved}`);
  lines.push(
    `metrics.brier_per_million_floor=${resolved === 0 ? "NONE" : Math.trunc(brierNumerator / resolved)}`,
  );
  lines.push(`metrics.missing_outcomes=${missing.join(",")}`);

  return `${lines.join("\n")}\n`;
}

const rendered = main();

if (argv.includes("--write")) {
  await writeFile(OUTPUT, rendered, "utf8");
  console.log(`wrote ${OUTPUT}`);
} else if (argv.includes("--check")) {
  let committed = "";
  try {
    committed = await readFile(OUTPUT, "utf8");
  } catch {
    console.error(`${OUTPUT} is missing`);
    exit(1);
  }
  if (committed !== rendered) {
    console.error(`${OUTPUT} differs from a fresh render`);
    exit(1);
  }
} else {
  process.stdout.write(rendered);
}
