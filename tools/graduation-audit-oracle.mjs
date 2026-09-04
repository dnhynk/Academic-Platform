#!/usr/bin/env node
// An independent oracle for the `P2-U3` graduation-audit fixtures.
//
// WHY THIS EXISTS, AND WHY IT IS IN A DIFFERENT LANGUAGE
//
// A proof tree checked against a tree the same engine produced proves only that
// the engine is deterministic, which is a different claim. It is a particularly
// easy mistake to make here, because a proof tree is large and comparing two of
// them *looks* like thorough evidence. So the expected statuses and measures
// come from here: a second transcription of the transcript, a second
// transcription of the grade table and the repeat ceiling, a second
// transcription of the rules, and a second arithmetic, written against the
// specification and against `docs/contracts/gpa-and-attempts.md` rather than
// against `crates/audit`.
//
// Four things are deliberately independent of the Rust implementation:
//
//   1. **The transcript.** The nine rows below were typed from section 10 and
//      from the GPA contract's table, not generated from
//      `academic_record::corpus`. If that corpus changes a grade, a credit, a
//      term or an origin, this file still says the old answer and the Rust
//      comparison fails.
//   2. **The credit admission.** Rust reaches it through `RecordViews`'s
//      `CreditContribution`. This decides it from the row: a replaced repeat, an
//      `F`, a `W` and a not-settled registration earn nothing; an `S` and a
//      recognized exchange grade earn their credits.
//   3. **The numeric representation.** Rust carries a coefficient and a scale
//      and compares by cross-multiplication. This carries fixed-point BigInt
//      units -- credits whole, grade points in tenths, quality points in tenths
//      -- and compares by cross-multiplication in those units.
//   4. **The rule evaluation.** Rust folds `academic-requirement`'s per-rule
//      verdicts. This re-states each rule's condition directly.
//
// The cumulative reading is `33.9` over `12`, which the GPA contract states
// independently; the floor is `2.0`. `339 * 10 >= 20 * 12 * 10` is `3390 >=
// 2400`, so the grade-point rule passes with room, and the row that actually
// separates implementations is the *repeat ceiling*: without it the repeated
// `A+` contributes `4.3` and the weighted total is `34.8` rather than `33.9`.
//
// Usage:
//   node tools/graduation-audit-oracle.mjs            # print the expected block
//   node tools/graduation-audit-oracle.mjs --write    # write the expected file
//   node tools/graduation-audit-oracle.mjs --check    # exit non-zero if it differs

import { readFile, writeFile } from "node:fs/promises";
import { argv, exit } from "node:process";

const OUTPUT = "testdata/engines/graduation_audit/oracle.expected";

// ---------------------------------------------------------------------------
// The grade table and the repeat ceiling, transcribed from section 10
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

// "2015학년도 1학기부터 재수강 성적은 A0를 상한으로 한다." The ceiling applies
// to the recognized attempt of a repeat group taken from 2015 spring onward.
const REPEAT_CEILING_FROM = { year: 2015, session: 1 };
const REPEAT_CEILING_GRADE = "A0";

// Sessions in the order the academic year runs, so a term compares as a number.
const SESSIONS = { SPRING: 1, SUMMER: 2, FALL: 3, WINTER: 4 };

function termOrder(term) {
  const [year, session] = term.split("_");
  return Number(year) * 10 + SESSIONS[session];
}

const CEILING_FROM_ORDER =
  REPEAT_CEILING_FROM.year * 10 + REPEAT_CEILING_FROM.session;

// ---------------------------------------------------------------------------
// The transcript, transcribed by hand
// ---------------------------------------------------------------------------
//
// `earns` is this file's own reading of whether the row earns credit toward a
// requirement, and `inAverage` is its own reading of whether the row reaches the
// grade-point denominator. Both are decided from the row rather than read back
// out of anything.
const TRANSCRIPT = [
  {
    id: 1,
    course: "M1522.000100",
    term: "2014_SPRING",
    grade: "C0",
    credits: 3n,
    repeat: "REPLACED",
    origin: "INTERNAL",
    settled: true,
  },
  {
    id: 2,
    course: "M1522.000100",
    term: "2015_SPRING",
    grade: "A+",
    credits: 3n,
    repeat: "REPEAT",
    origin: "INTERNAL",
    settled: true,
  },
  {
    id: 3,
    course: "4190.101",
    term: "2014_SPRING",
    grade: "B+",
    credits: 3n,
    repeat: "ORIGINAL",
    origin: "INTERNAL",
    settled: true,
  },
  {
    id: 4,
    course: "L0442.000200",
    term: "2014_SPRING",
    grade: "S",
    credits: 2n,
    repeat: "ORIGINAL",
    origin: "INTERNAL",
    settled: true,
  },
  {
    id: 5,
    course: "4190.210",
    term: "2014_FALL",
    grade: "F",
    credits: 3n,
    repeat: "ORIGINAL",
    origin: "INTERNAL",
    settled: true,
  },
  {
    id: 6,
    course: "4190.310",
    term: "2014_FALL",
    grade: "W",
    credits: 3n,
    repeat: "ORIGINAL",
    origin: "INTERNAL",
    settled: true,
  },
  {
    id: 7,
    course: "326.212",
    term: "2015_SPRING",
    grade: "A0",
    credits: 3n,
    repeat: "ORIGINAL",
    origin: "INTERNAL",
    settled: true,
  },
  {
    id: 8,
    course: "X0001.000100",
    term: "2015_FALL",
    grade: "B0",
    credits: 3n,
    repeat: "ORIGINAL",
    origin: "EXCHANGE",
    recognition: "RECOGNIZED",
    settled: true,
  },
  {
    id: 9,
    course: "4190.408",
    term: "2026_FALL",
    grade: null,
    credits: 3n,
    repeat: "ORIGINAL",
    origin: "INTERNAL",
    settled: false,
  },
];

/// Whether a row earns credit toward a requirement.
///
/// A replaced repeat does not (a later attempt displaced it). An `F`, a `W` and
/// a not-settled registration earn nothing. Everything else earns its credits,
/// including an `S` and a recognized exchange grade.
function earnsCredit(row) {
  if (!row.settled) return false;
  if (row.repeat === "REPLACED") return false;
  if (row.grade === "F" || row.grade === "W" || row.grade === "U") return false;
  if (row.grade === null) return false;
  if (row.origin !== "INTERNAL") return row.recognition === "RECOGNIZED";
  return true;
}

/// Whether a row reaches the grade-point denominator.
///
/// An `S` does not (no grade point). An `F` does, which is what stops a failure
/// from raising an average. An external grade after 2004 does not. A replaced
/// repeat does not, and a not-settled row does not.
function inAverage(row) {
  if (!row.settled) return false;
  if (row.repeat === "REPLACED") return false;
  if (row.grade === null) return false;
  if (!(row.grade in POINTS_TENTHS)) return false;
  if (row.origin !== "INTERNAL") return false;
  return true;
}

/// The grade the average uses, after the repeat ceiling.
function effectiveGrade(row) {
  if (row.repeat !== "REPEAT") return row.grade;
  if (termOrder(row.term) < CEILING_FROM_ORDER) return row.grade;
  return POINTS_TENTHS[row.grade] > POINTS_TENTHS[REPEAT_CEILING_GRADE]
    ? REPEAT_CEILING_GRADE
    : row.grade;
}

// ---------------------------------------------------------------------------
// The curriculum facts, transcribed by hand
// ---------------------------------------------------------------------------

const COURSES = {
  "M1522.000100": {
    id: "repeated",
    categories: ["ALL_RECOGNIZED", "CSE_MAJOR"],
    language: "UNVERIFIED",
  },
  "4190.101": {
    id: "data_structures",
    categories: ["ALL_RECOGNIZED", "CSE_MAJOR"],
    language: "FOREIGN",
  },
  "L0442.000200": {
    id: "satisfactory",
    categories: ["ALL_RECOGNIZED"],
    language: "UNVERIFIED",
  },
  "4190.210": {
    id: "failed",
    categories: ["ALL_RECOGNIZED", "CSE_MAJOR"],
    language: "UNVERIFIED",
  },
  "326.212": {
    id: "computing_overview",
    categories: ["ALL_RECOGNIZED"],
    language: "FOREIGN",
  },
  "X0001.000100": {
    id: "exchange",
    categories: ["ALL_RECOGNIZED", "EXTERNAL_RECOGNIZED"],
    language: "UNVERIFIED",
  },
  "4190.310": {
    id: "withdrawn",
    categories: ["ALL_RECOGNIZED", "CSE_MAJOR"],
    language: "UNVERIFIED",
  },
  "4190.408": {
    id: "registered",
    categories: ["ALL_RECOGNIZED", "CSE_MAJOR"],
    language: "UNVERIFIED",
  },
};

// ---------------------------------------------------------------------------
// The rules, transcribed by hand
// ---------------------------------------------------------------------------

const GPA_FLOOR_TENTHS = 20n; // 2.0

/// Every course that earned credit, by durable course identity.
function earnedCourses() {
  const earned = new Set();
  for (const row of TRANSCRIPT) {
    if (earnsCredit(row)) earned.add(COURSES[row.course].id);
  }
  return earned;
}

function creditsIn(category) {
  let total = 0n;
  for (const row of TRANSCRIPT) {
    if (!earnsCredit(row)) continue;
    if (!COURSES[row.course].categories.includes(category)) continue;
    total += row.credits;
  }
  return total;
}

function foreignLectureCount() {
  let count = 0;
  for (const row of TRANSCRIPT) {
    if (!earnsCredit(row)) continue;
    if (COURSES[row.course].language === "FOREIGN") count += 1;
  }
  return count;
}

function gradePoint() {
  let weightedTenths = 0n;
  let denominator = 0n;
  for (const row of TRANSCRIPT) {
    if (!inAverage(row)) continue;
    weightedTenths += POINTS_TENTHS[effectiveGrade(row)] * row.credits;
    denominator += row.credits;
  }
  return { weightedTenths, denominator };
}

function shortfall(attained, required) {
  return attained >= required ? "SATISFIED" : "NEEDS";
}

function main() {
  const earned = earnedCourses();
  const lines = [];

  const total = creditsIn("ALL_RECOGNIZED");
  lines.push(`status.total_credits=${shortfall(total, 130n)}`);
  lines.push(`measure.total_credits=${total}/130`);

  const major = creditsIn("CSE_MAJOR");
  lines.push(`status.cse_major_total=${shortfall(major, 63n)}`);
  lines.push(`measure.cse_major_total=${major}/63`);

  // `required_course_set`: data structures directly, algorithms never
  // attempted, discrete maths only through the equivalency the same set
  // publishes -- computing overview presented for it.
  const operands = [
    earned.has("data_structures"),
    earned.has("algorithms"),
    earned.has("computing_overview"),
  ];
  const satisfiedOperands = operands.filter(Boolean).length;
  lines.push(
    `status.required_course_set=${
      satisfiedOperands === operands.length ? "SATISFIED" : "NOT_SATISFIED"
    }`,
  );
  lines.push(`measure.required_course_set=${satisfiedOperands}/${operands.length}`);
  lines.push(`operand.required_course_set.op.000=${operands[0] ? "SATISFIED" : "NOT_SATISFIED"}`);
  lines.push(`operand.required_course_set.op.001=${operands[1] ? "SATISFIED" : "NOT_SATISFIED"}`);
  lines.push(`operand.required_course_set.op.002=${operands[2] ? "SATISFIED" : "NOT_SATISFIED"}`);
  lines.push(`equivalency.required_course_set.op.002=equivalency_shared`);

  // `seminar_choice`: one of the seminar or the computing overview.
  const seminar = [earned.has("cse_seminar"), earned.has("computing_overview")];
  const seminarCount = seminar.filter(Boolean).length;
  lines.push(`status.seminar_choice=${shortfall(BigInt(seminarCount), 1n)}`);
  lines.push(`measure.seminar_choice=${seminarCount}/1`);
  lines.push(`operand.seminar_choice.op.000=${seminar[0] ? "SATISFIED" : "NOT_SATISFIED"}`);
  lines.push(`operand.seminar_choice.op.001=${seminar[1] ? "SATISFIED" : "NOT_SATISFIED"}`);

  const foreign = foreignLectureCount();
  lines.push(`status.foreign_language_lectures=${shortfall(BigInt(foreign), 3n)}`);
  lines.push(`measure.foreign_language_lectures=${foreign}/3`);

  const { weightedTenths, denominator } = gradePoint();
  lines.push(`gpa.weighted_points_tenths=${weightedTenths}`);
  lines.push(`gpa.denominator_credits=${denominator}`);
  // `weighted / denominator >= floor`, without dividing.
  const meets = weightedTenths * 10n >= GPA_FLOOR_TENTHS * denominator;
  lines.push(`status.overall_gpa=${meets ? "SATISFIED" : "NEEDS"}`);

  // `major_exclusive`: at most one of data structures and the withdrawn course
  // may be recognized.
  const members = ["data_structures", "withdrawn"].filter((course) => earned.has(course));
  lines.push(`status.major_exclusive=${members.length <= 1 ? "SATISFIED" : "CONFLICT"}`);
  lines.push(`measure.major_exclusive=${members.length}/1`);

  // `equivalency_shared`: the presented course was taken and the relation is
  // live at the instant the audit is anchored to.
  lines.push(
    `status.equivalency_shared=${
      earned.has("computing_overview") ? "SATISFIED" : "NOT_SATISFIED"
    }`,
  );

  // The root folds the leaves: conflict, then unknown, then not-satisfied, then
  // needs, then satisfied.
  const leafStatuses = lines
    .filter((line) => line.startsWith("status.") || line.startsWith("operand."))
    .map((line) => line.split("=")[1]);
  const root = leafStatuses.includes("CONFLICT")
    ? "CONFLICT"
    : leafStatuses.includes("UNKNOWN")
      ? "UNKNOWN"
      : leafStatuses.includes("NOT_SATISFIED")
        ? "NOT_SATISFIED"
        : leafStatuses.includes("NEEDS")
          ? "NEEDS"
          : "SATISFIED";
  lines.push(`root.status=${root}`);

  // Every rule is placed and none is unknown, no conflict case is open, and the
  // source is inside the recorded freshness criterion, so all three of section
  // 11.4's gates hold and the determination is 졸업 불가.
  lines.push(`verdict=DETERMINATE`);
  lines.push(`outcome=${root === "SATISFIED" ? "POSSIBLE" : "NOT_POSSIBLE"}`);
  lines.push(`earned_credits=${total}`);

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
