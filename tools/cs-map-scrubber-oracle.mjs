#!/usr/bin/env node
// An independent oracle for the `P2-X5` timeline scrubber.
//
// WHY THIS EXISTS, AND WHY IT IS IN A DIFFERENT LANGUAGE
//
// A scrubber checked against the sets the scrubber produced proves only that
// the scrubber is deterministic. That is a particularly easy mistake for a time
// projection, because a node either is or is not in a set and both sides look
// equally like an answer. So every expected value below is derived here: a
// second transcription of the event table, a second transcription of the
// readings, a second derivation of the identities, and a different algorithm.
//
// Four things are deliberately independent of the Rust implementation:
//
//   1. **The events.** The twelve rows below were typed from
//      `docs/contracts/cs-map-atlas.md` and from section 26.5 of the design
//      document, not read out of `crates/cs-map/tests/support/mod.rs`. If a row
//      moves there, this file still says the old answer and the Rust comparison
//      fails.
//   2. **The identities.** Rust builds them through `academic_domain`'s
//      `ContentDigest::sha256` and `EntityId::try_from_uuid`. This does the same
//      reshaping with `node:crypto` and formats the UUID by hand, so the two
//      agree only if both implement the same rule.
//   3. **The algorithm.** Rust folds the admitted events in order through a
//      running visible set. This groups the admitted events *by subject*, sorts
//      each subject's own events, and asks what the last one did — a
//      per-subject question rather than a running one. Both answers coincide
//      exactly when the fold is order-correct on both axes.
//   4. **The admission rule.** Rust filters with `>` on two struct fields. This
//      restates section 26.5's rule from the contract: an event counts when its
//      acceptance sequence is at or below the reading's **and** its valid
//      instant is at or below the reading's. A reader that let one axis stand in
//      for the other would agree with this file at exactly the readings where
//      the two axes happen to move together, and the table below is written so
//      that four of the eight readings are not among them.
//
// The rows that separate implementations are the ones whose two coordinates
// disagree. `concept.logging` appears at acceptance 20 and valid 5000; a
// scrubber that admitted on acceptance alone would show it at reading
// `30/3000`, and this oracle says it is not there. `concept.ordering` appears at
// acceptance 40 and valid 9000; a scrubber that admitted on valid time alone
// would show it at reading `50/5000`, and this oracle says it is not there.
//
// Usage:
//   node tools/cs-map-scrubber-oracle.mjs            # print the expected block
//   node tools/cs-map-scrubber-oracle.mjs --write    # write the expected file
//   node tools/cs-map-scrubber-oracle.mjs --check    # exit non-zero if it differs

import { createHash } from "node:crypto";
import { readFile, writeFile } from "node:fs/promises";
import { argv, exit } from "node:process";

const OUTPUT = "testdata/cs-map/scrubber.expected";

// ---------------------------------------------------------------------------
// Identity, derived here rather than imported
// ---------------------------------------------------------------------------

/**
 * The UUIDv7-shaped identity of a fixture tag.
 *
 * SHA-256 of the tag, first sixteen bytes, version nibble forced to 7 and
 * variant bits forced to RFC 4122, formatted 8-4-4-4-12 in lowercase hex. That
 * is the rule `crates/cs-map/tests/support/mod.rs` implements through the
 * domain crate's helpers; nothing is shared between the two.
 */
function identity(tag) {
  const digest = createHash("sha256").update(Buffer.from(tag, "utf8")).digest();
  const bytes = Uint8Array.prototype.slice.call(digest, 0, 16);
  bytes[6] = (bytes[6] & 0x0f) | 0x70;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return [
    hex.slice(0, 8),
    hex.slice(8, 12),
    hex.slice(12, 16),
    hex.slice(16, 20),
    hex.slice(20, 32),
  ].join("-");
}

// ---------------------------------------------------------------------------
// The event table, transcribed
// ---------------------------------------------------------------------------

/** `[acceptance sequence, valid instant, subject tag, appears, transition]`. */
const EVENTS = [
  [10, 1000, "concept.transaction", true, "EVIDENCE_CHANGE"],
  [10, 1000, "concept.isolation", true, "EVIDENCE_CHANGE"],
  [20, 2000, "concept.locking", true, "EVIDENCE_CHANGE"],
  [20, 5000, "concept.logging", true, "USER_SCOPE_CHANGE"],
  [30, 3000, "concept.isolation", false, "ONTOLOGY_CHANGE"],
  [30, 3000, "sense.transaction.db", true, "ONTOLOGY_CHANGE"],
  [40, 4000, "concept.serializability", true, "ANALYZER_UPGRADE"],
  [40, 9000, "concept.ordering", true, "ANALYZER_UPGRADE"],
  [50, 5000, "concept.locking", false, "OFFICIAL_SOURCE_CORRECTION"],
  [50, 5000, "code.txn-manager", true, "OFFICIAL_SOURCE_CORRECTION"],
  [60, 6000, "concept.isolation", true, "EVIDENCE_CHANGE"],
  [70, 7000, "concept.logging", false, "USER_SCOPE_CHANGE"],
];

/** `[acceptance sequence, valid instant]` for each scrubber position read. */
const READINGS = [
  [5, 500],
  [10, 1000],
  [20, 2000],
  [25, 5000],
  [30, 3000],
  [40, 9000],
  [50, 5000],
  [70, 7000],
];

// ---------------------------------------------------------------------------
// The projection, computed per subject
// ---------------------------------------------------------------------------

/**
 * What the map holds at one reading.
 *
 * Groups the admitted events by subject and asks each group what its last event
 * did. `Appears` sorts before `Disappears` at identical coordinates, which is
 * the tie-break `docs/cs-map-atlas.md` states; no row in the table above ties,
 * so the rule is recorded rather than exercised.
 */
function project(knownAt, validAt) {
  const bySubject = new Map();
  for (const [seq, valid, tag, appears, transition] of EVENTS) {
    if (seq > knownAt || valid > validAt) {
      continue;
    }
    if (!bySubject.has(tag)) {
      bySubject.set(tag, []);
    }
    bySubject.get(tag).push({ seq, valid, appears, transition });
  }

  const visible = [];
  for (const [tag, events] of bySubject) {
    events.sort((left, right) => {
      if (left.seq !== right.seq) {
        return left.seq - right.seq;
      }
      if (left.valid !== right.valid) {
        return left.valid - right.valid;
      }
      return Number(right.appears) - Number(left.appears);
    });
    const last = events[events.length - 1];
    if (last.appears) {
      visible.push({ id: identity(tag), transition: last.transition });
    }
  }
  visible.sort((left, right) => (left.id < right.id ? -1 : left.id > right.id ? 1 : 0));
  return visible;
}

function render() {
  const lines = [];
  for (const [knownAt, validAt] of READINGS) {
    const visible = project(knownAt, validAt);
    lines.push(`visible@${knownAt}/${validAt}=${visible.map((row) => row.id).join(",")}`);
    lines.push(
      `entered@${knownAt}/${validAt}=${visible
        .map((row) => `${row.id}:${row.transition}`)
        .join(",")}`,
    );
  }
  return `${lines.join("\n")}\n`;
}

const rendered = render();

if (argv.includes("--write")) {
  await writeFile(OUTPUT, rendered, "utf8");
  console.log(`wrote ${OUTPUT}`);
} else if (argv.includes("--check")) {
  const committed = await readFile(OUTPUT, "utf8");
  if (committed !== rendered) {
    console.error(`${OUTPUT} differs from a fresh render of the oracle`);
    exit(1);
  }
  console.log(`${OUTPUT} matches the oracle`);
} else {
  process.stdout.write(rendered);
}
