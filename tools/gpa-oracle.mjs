#!/usr/bin/env node
// An independent oracle for the `P2-U4` grade-point average fixtures.
//
// WHY THIS EXISTS, AND WHY IT IS IN A DIFFERENT LANGUAGE
//
// `gpa_formula_fixture` is worthless if its expected values are produced by the
// implementation it checks. Running `RecordViews` twice and comparing proves
// only that the function is deterministic, which is a different claim. So the
// expected values come from here: a second transcription of the corpus, a
// second transcription of the grade table, and a second arithmetic, written
// against the specification rather than against `crates/record`.
//
// Three things are deliberately independent of the Rust implementation:
//
//   1. **The corpus.** The nine rows below were typed from the same
//      specification the Rust builder was, not generated from it. If
//      `corpus::baseline_history` changes a grade, a credit, a term, or an
//      origin, this file still says the old answer and the Rust test fails.
//   2. **The numeric representation.** Rust carries a coefficient and a scale
//      and rescales to a common one. This carries fixed-point BigInt units —
//      credits in tenths, grade points in tenths, quality points in hundredths
//      — and never rescales at all. The two agree only if both are right.
//   3. **The rounding.** Rust divides two `Decimal`s. This computes
//      `numerator * 10^scale / (denominator * 10)` in integers and rounds half
//      away from zero by doubling the remainder. A shared rounding bug would
//      have to be made twice, in two shapes.
//
// The cumulative case is `3390 / 1200` hundredths-over-tenths, which is exactly
// `2.825` — a tie at the second digit. `f64` cannot hold `2.825`: the nearest
// double is below it, so a floating-point implementation publishes `2.82` and
// this file says `2.83`. That row is the float detector.
//
// Usage:
//   node tools/gpa-oracle.mjs            # print the expected block
//   node tools/gpa-oracle.mjs --write    # write testdata/engines/gpa/oracle.expected
//   node tools/gpa-oracle.mjs --check    # exit non-zero if the committed file differs

import { readFile, writeFile } from "node:fs/promises";
import { argv, exit } from "node:process";

const OUTPUT = "testdata/engines/gpa/oracle.expected";

// ---------------------------------------------------------------------------
// The grade table, transcribed from section 10
// ---------------------------------------------------------------------------
//
// "SNU 공식 표는 A+ 4.3, A0 4.0, …, D- 0.7, F 0이며 S/U 교과목은 평점 계산에서
// 제외한다." Points are in tenths so every value is an integer.
const POINTS_TENTHS = {
  "A+": 43n,
  A0: 40n,
  "A-": 37n,
  "B+": 33n,
  B0: 30n,
  "B-": 27n,
  "C+": 23n,
  C0: 20n,
  "C-": 17n,
  "D+": 13n,
  D0: 10n,
  "D-": 7n,
  F: 0n,
};
// Outside the average. `S` earns its credits, the rest do not.
const EARNS_WITHOUT_POINTS = new Set(["S"]);
const NO_POINTS = new Set(["S", "U", "W", "I"]);

// ---------------------------------------------------------------------------
// The corpus, transcribed by hand
// ---------------------------------------------------------------------------

const SEMESTER_ORDER = { SPRING: 1, SUMMER: 2, FALL: 3, WINTER: 4 };

function termOrder(term) {
  const [year, semester] = term.split("_");
  const rank = SEMESTER_ORDER[semester];
  if (!rank) throw new Error(`unknown semester in ${term}`);
  return Number(year) * 10 + rank;
}

// credits are tenths: 30 means 3.0 credits.
const ATTEMPTS = [
  { id: 1, course: "M1522.000100", term: "2014_SPRING", status: "COMPLETED", origin: "INTERNAL", grade: "C0", attempted: 30n, earned: 30n, repeat: "ORIGINAL", recognition: "UNDECIDED" },
  { id: 2, course: "M1522.000100", term: "2015_SPRING", status: "COMPLETED", origin: "INTERNAL", grade: "A+", attempted: 30n, earned: 30n, repeat: "REPEAT", recognition: "UNDECIDED" },
  { id: 3, course: "4190.101", term: "2014_SPRING", status: "COMPLETED", origin: "INTERNAL", grade: "B+", attempted: 30n, earned: 30n, repeat: "NOT_APPLICABLE", recognition: "UNDECIDED" },
  { id: 4, course: "L0442.000200", term: "2014_SPRING", status: "COMPLETED", origin: "INTERNAL", grade: "S", attempted: 20n, earned: 20n, repeat: "NOT_APPLICABLE", recognition: "UNDECIDED" },
  { id: 5, course: "4190.210", term: "2014_FALL", status: "COMPLETED", origin: "INTERNAL", grade: "F", attempted: 30n, earned: 0n, repeat: "NOT_APPLICABLE", recognition: "UNDECIDED" },
  { id: 6, course: "4190.310", term: "2014_FALL", status: "WITHDRAWN", origin: "INTERNAL", grade: "W", attempted: 30n, earned: 0n, repeat: "NOT_APPLICABLE", recognition: "UNDECIDED" },
  { id: 7, course: "326.212", term: "2015_SPRING", status: "COMPLETED", origin: "INTERNAL", grade: "A0", attempted: 30n, earned: 30n, repeat: "NOT_APPLICABLE", recognition: "UNDECIDED" },
  { id: 8, course: "X0001.000100", term: "2015_FALL", status: "RECOGNIZED", origin: "EXCHANGE", grade: "B0", attempted: 30n, earned: 30n, repeat: "NOT_APPLICABLE", recognition: "RECOGNIZED" },
  { id: 9, course: "4190.408", term: "2026_FALL", status: "REGISTERED", origin: "INTERNAL", grade: null, attempted: 30n, earned: 0n, repeat: "NOT_APPLICABLE", recognition: "UNDECIDED" },
];

// programme -> course -> category
const CLASSIFICATION = {
  cse: {
    "M1522.000100": "MAJOR_REQUIRED",
    "4190.101": "MAJOR_ELECTIVE",
    "L0442.000200": "GENERAL_ELECTIVE",
    "4190.210": "MAJOR_ELECTIVE",
    "4190.310": "MAJOR_ELECTIVE",
  },
  stat: {
    "4190.101": "FREE_ELECTIVE",
    "326.212": "MAJOR_REQUIRED",
  },
};
const MAJOR_CATEGORIES = new Set(["MAJOR_REQUIRED", "MAJOR_ELECTIVE"]);

const SETTLED = new Set(["COMPLETED", "WITHDRAWN", "TRANSFERRED", "RECOGNIZED"]);

// Effective-dated rows. `ceiling: null` means the row states no ceiling.
const BASELINE_REPEAT_ROWS = [
  { from: "2000_SPRING", ceiling: null, recognition: "LATEST" },
  { from: "2015_SPRING", ceiling: "A0", recognition: "LATEST" },
];
const EXTERNAL_ROWS = [{ from: "2004_SPRING", excludedFromAverage: true }];

function repeatRowAt(rows, term) {
  const order = termOrder(term);
  let found = null;
  for (const row of rows) if (termOrder(row.from) <= order) found = row;
  return found;
}

function externalRowAt(term) {
  const order = termOrder(term);
  let found = null;
  for (const row of EXTERNAL_ROWS) if (termOrder(row.from) <= order) found = row;
  return found;
}

// ---------------------------------------------------------------------------
// The arithmetic
// ---------------------------------------------------------------------------

/// Rounds `top / bottom` half away from zero. Both are non-negative BigInts.
function divideHalfAwayFromZero(top, bottom) {
  const quotient = top / bottom;
  const remainder = top % bottom;
  return remainder * 2n >= bottom ? quotient + 1n : quotient;
}

/// Renders a BigInt coefficient at `scale` digits the way the repository's
/// canonical decimal spelling does: fixed point, trailing zeros removed.
function render(coefficient, scale) {
  const negative = coefficient < 0n;
  let digits = (negative ? -coefficient : coefficient).toString();
  if (scale > 0) {
    while (digits.length <= scale) digits = `0${digits}`;
    digits = `${digits.slice(0, digits.length - scale)}.${digits.slice(digits.length - scale)}`;
    digits = digits.replace(/0+$/u, "").replace(/\.$/u, "");
  }
  if (digits === "") digits = "0";
  return negative && digits !== "0" ? `-${digits}` : digits;
}

/// Computes every disposition under one repeat-row set.
function dispose(repeatRows) {
  const settled = ATTEMPTS.filter((attempt) => SETTLED.has(attempt.status));

  // Repeat groups: a course with more than one settled attempt, at least one
  // of which is marked as a repeat.
  const byCourse = new Map();
  for (const attempt of settled) {
    if (!byCourse.has(attempt.course)) byCourse.set(attempt.course, []);
    byCourse.get(attempt.course).push(attempt);
  }
  const displaced = new Set();
  const capped = new Map();
  const undecided = new Set();
  for (const [, group] of byCourse) {
    const isRepeatGroup =
      group.length > 1 && group.some((a) => a.repeat === "REPEAT" || a.repeat === "REPLACED");
    if (!isRepeatGroup) continue;
    group.sort((a, b) => termOrder(a.term) - termOrder(b.term) || a.id - b.id);
    const latest = group[group.length - 1];
    const row = repeatRowAt(repeatRows, latest.term);
    if (!row || row.recognition !== "LATEST") {
      for (const a of group) undecided.add(a.id);
      continue;
    }
    for (const a of group) if (a.id !== latest.id) displaced.add(a.id);
    if (row.ceiling && POINTS_TENTHS[latest.grade] > POINTS_TENTHS[row.ceiling]) {
      capped.set(latest.id, row.ceiling);
    }
  }

  return ATTEMPTS.map((attempt) => {
    const base = { id: attempt.id, term: attempt.term, course: attempt.course };
    if (!SETTLED.has(attempt.status)) return { ...base, kind: "excluded", earned: 0n };
    if (undecided.has(attempt.id)) return { ...base, kind: "unknown", earned: 0n };
    if (displaced.has(attempt.id)) return { ...base, kind: "excluded", earned: 0n };
    if (attempt.grade === null || attempt.grade === "I") return { ...base, kind: "unknown", earned: 0n };

    if (attempt.origin !== "INTERNAL") {
      const row = externalRowAt(attempt.term);
      if (!row) return { ...base, kind: "unknown", earned: 0n };
      const earned = attempt.recognition === "RECOGNIZED" ? attempt.earned : 0n;
      if (attempt.recognition === "UNDECIDED") return { ...base, kind: "unknown", earned: 0n };
      if (row.excludedFromAverage) return { ...base, kind: "excluded", earned };
      // Not reached by this corpus; kept so the branch is not a lie.
      const points = POINTS_TENTHS[attempt.grade];
      return { ...base, kind: "included", qualityPoints: attempt.attempted * points, denominator: attempt.attempted, earned };
    }

    if (NO_POINTS.has(attempt.grade)) {
      const earned = EARNS_WITHOUT_POINTS.has(attempt.grade) ? attempt.earned : 0n;
      return { ...base, kind: "excluded", earned };
    }

    const effective = capped.get(attempt.id) ?? attempt.grade;
    const points = POINTS_TENTHS[effective];
    return {
      ...base,
      kind: "included",
      qualityPoints: attempt.attempted * points,
      denominator: attempt.attempted,
      earned: attempt.grade === "F" ? 0n : attempt.earned,
    };
  });
}

/// Folds a disposition subset into a published average at `scale` digits.
function average(dispositions, scale) {
  if (dispositions.some((d) => d.kind === "unknown")) return "UNKNOWN";
  const included = dispositions.filter((d) => d.kind === "included");
  if (included.length === 0) return "NO_GRADED_ATTEMPTS";
  // qualityPoints are hundredths, denominator is tenths.
  const numerator = included.reduce((total, d) => total + d.qualityPoints, 0n);
  const denominator = included.reduce((total, d) => total + d.denominator, 0n);
  if (denominator === 0n) return "NO_GRADED_ATTEMPTS";
  // (num/100) / (den/10) = num / (den * 10); shift by 10^scale before dividing.
  const coefficient = divideHalfAwayFromZero(numerator * 10n ** BigInt(scale), denominator * 10n);
  return render(coefficient, scale);
}

function majorFor(program) {
  const table = CLASSIFICATION[program] ?? {};
  return (disposition) => MAJOR_CATEGORIES.has(table[disposition.course]);
}

function block() {
  const lines = [];
  const baseline = dispose(BASELINE_REPEAT_ROWS);

  lines.push(`cumulative.gpa.scale2=${average(baseline, 2)}`);
  lines.push(`cumulative.gpa.scale3=${average(baseline, 3)}`);

  const numerator = baseline
    .filter((d) => d.kind === "included")
    .reduce((total, d) => total + d.qualityPoints, 0n);
  const denominator = baseline
    .filter((d) => d.kind === "included")
    .reduce((total, d) => total + d.denominator, 0n);
  const earned = baseline.reduce((total, d) => total + d.earned, 0n);
  lines.push(`cumulative.quality_points=${render(numerator, 2)}`);
  lines.push(`cumulative.denominator_credits=${render(denominator, 1)}`);
  lines.push(`cumulative.earned_credits=${render(earned, 1)}`);
  lines.push(
    `cumulative.included_attempts=${baseline
      .filter((d) => d.kind === "included")
      .map((d) => d.id)
      .join(",")}`,
  );

  const terms = [...new Set(ATTEMPTS.map((a) => a.term))].sort(
    (a, b) => termOrder(a) - termOrder(b),
  );
  for (const term of terms) {
    lines.push(`term.${term}.gpa=${average(baseline.filter((d) => d.term === term), 2)}`);
  }

  for (const program of Object.keys(CLASSIFICATION).sort()) {
    lines.push(`major.${program}.gpa=${average(baseline.filter(majorFor(program)), 2)}`);
  }

  for (const moved of ["2016_SPRING", "2014_SPRING"]) {
    const rows = [
      { from: "2000_SPRING", ceiling: null, recognition: "LATEST" },
      { from: moved, ceiling: "A0", recognition: "LATEST" },
    ];
    lines.push(`ceiling_from.${moved}.cumulative.gpa=${average(dispose(rows), 2)}`);
  }

  return `${lines.join("\n")}\n`;
}

const rendered = block();
if (argv.includes("--write")) {
  await writeFile(OUTPUT, rendered, "utf8");
  process.stdout.write(`wrote ${OUTPUT}\n`);
} else if (argv.includes("--check")) {
  const committed = await readFile(OUTPUT, "utf8");
  if (committed !== rendered) {
    process.stderr.write(`${OUTPUT} does not match a fresh oracle render\n`);
    exit(1);
  }
  process.stdout.write(`${OUTPUT} matches\n`);
} else {
  process.stdout.write(rendered);
}
