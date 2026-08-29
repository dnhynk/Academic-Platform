// A type that carries key material or decrypted plaintext must not derive
// `Debug`.
//
// This regressed three times. `OpenedHeader` shipped deriving `Debug` over a
// raw DEK and a plaintext digest; `EncryptedDomainKeyring` shipped deriving it
// over per-domain KEKs; `EncryptedObjectReader` shipped deriving it over a
// buffer holding up to one chunk -- a mebibyte by default -- of decrypted
// artifact plaintext. In each case `format!("{value:?}")` in a log line, a
// panic message, or an audit row would have printed the bytes, which is what
// ADR-005 "Zeroization and exposure boundary" forbids.
//
// `missing_debug_implementations = "deny"` is what makes the regression easy:
// the lint demands a `Debug`, and the one-line way to satisfy it is the derive
// that leaks. So the rule is checked mechanically rather than by review, in two
// halves that fail for different reasons:
//
//   1. A registry of the types already known to carry secrets. Each must still
//      exist, must not derive `Debug` or `Display`, and must have a
//      hand-written `Debug`. This is what fails if someone adds a derive back.
//   2. A discovery net over every other type, for the ones nobody has listed
//      yet: a type that derives `Debug` and owns a raw byte buffer under a
//      field name from the key-material vocabulary. Bytes that are genuinely
//      public are named below with the reason they are public, so the net
//      documents its own exceptions instead of hiding them.
//
// Scope is `crates/*/src`, the product surface ADR-005 governs. Test-only
// helper types are not scanned.

import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const CRATES_ROOT = join(REPOSITORY_ROOT, "crates");

/** Types known to carry key material or decrypted plaintext, and why. */
const SECRET_BEARING_TYPES = new Map([
  ["OpenedHeader", "the raw per-object DEK and the plaintext digest"],
  ["EncryptedObjectReader", "a buffer of decrypted artifact plaintext"],
  ["RecoveredSecret", "the secret an operating-system broker returned"],
  ["BackupMasterKey", "the 32-byte backup root"],
  ["DomainKeyring", "raw domain key bytes"],
  ["EncryptedDomainKeyring", "per-domain KEKs and locator keys"],
]);

/**
 * Field names that mean key material or plaintext when they hold raw bytes.
 * A name is only a signal; the exceptions below carry the judgement.
 */
const SECRET_FIELD_NAMES =
  /^_?(dek|kek|key|keys|key_bytes|secret|secrets|plaintext|plain|digest|seed|chunk|hex|raw|passphrase|password|opened)$/;

/** Field types that hold bytes transparently, so a derived `Debug` prints them. */
const RAW_BYTE_TYPES =
  /^(Vec\s*<\s*u8\s*>|\[\s*u8\s*;[^\]]*\]|String|Zeroizing\s*<[\s\S]*>|Box\s*<\s*\[\s*u8\s*\]\s*>)$/;

/**
 * Fields the net matches on name and type whose bytes are not secret. Each
 * entry states why, because the reason is the whole content of the exception.
 */
const PUBLIC_BYTES = new Map([
  [
    "Qualifier.key",
    "a qualifier name from the predicate registry's closed schema, not a cryptographic key",
  ],
  [
    "RegistryError.key",
    "the qualifier name a rejected assertion used, reported so the caller can fix it",
  ],
  [
    "KeyMaterialState.digest",
    "SHA-256 over the recipient set's canonical CBOR, which ADR-005 puts on disk holding no key byte",
  ],
  [
    "StreamingPrefix.digest",
    "SHA-256 of the object header's cleartext prefix P0, which is on disk in the clear",
  ],
]);

const DERIVE_PATTERN = /#\s*\[\s*derive\s*\(([\s\S]*?)\)\s*\]/g;
const DEFINITION_PATTERN =
  /^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(struct|enum|union)\s+([A-Za-z_][A-Za-z0-9_]*)/;
const NAMED_FIELD_PATTERN =
  /^\s*(?:pub(?:\s*\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*([^,\n]+),/gm;

async function rustSourcesUnder(directory) {
  const found = [];
  let entries;
  try {
    entries = await readdir(directory, { withFileTypes: true });
  } catch {
    return found;
  }
  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      found.push(...(await rustSourcesUnder(path)));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      found.push(path);
    }
  }
  return found;
}

async function productSources() {
  const crates = await readdir(CRATES_ROOT, { withFileTypes: true });
  const sources = [];
  for (const crate of crates) {
    if (crate.isDirectory()) {
      sources.push(...(await rustSourcesUnder(join(CRATES_ROOT, crate.name, "src"))));
    }
  }
  return sources.sort();
}

/** Returns the attribute block immediately above `index`, if any. */
function attributesAbove(lines, index) {
  const collected = [];
  for (let cursor = index - 1; cursor >= 0; cursor -= 1) {
    const trimmed = lines[cursor].trim();
    if (trimmed === "") {
      break;
    }
    // Doc comments sit between a derive and its definition often enough that
    // stopping at one would miss the derive; a line that is neither a comment
    // nor part of an attribute ends the block.
    const isAttributePart =
      trimmed.startsWith("//") ||
      trimmed.startsWith("#") ||
      trimmed.startsWith("]") ||
      trimmed.endsWith(",") ||
      trimmed.endsWith("(");
    if (!isAttributePart) {
      break;
    }
    collected.unshift(lines[cursor]);
  }
  return collected.join("\n");
}

/** Returns the brace-matched body of the definition starting at `index`. */
function bodyAt(lines, index) {
  const text = lines.slice(index, index + 400).join("\n");
  const opener = text.search(/[{(;]/);
  if (opener === -1 || text[opener] !== "{") {
    // A tuple struct or a unit struct has no named field for the net to read;
    // the registry above is what covers those.
    return "";
  }
  let depth = 0;
  for (let cursor = opener; cursor < text.length; cursor += 1) {
    if (text[cursor] === "{") {
      depth += 1;
    } else if (text[cursor] === "}") {
      depth -= 1;
      if (depth === 0) {
        return text.slice(opener + 1, cursor);
      }
    }
  }
  return text.slice(opener + 1);
}

function derivedTraits(attributeBlock) {
  const traits = new Set();
  for (const match of attributeBlock.matchAll(DERIVE_PATTERN)) {
    for (const name of match[1].split(",")) {
      const trimmed = name.trim().replace(/^.*::/, "");
      if (trimmed !== "") {
        traits.add(trimmed);
      }
    }
  }
  return traits;
}

async function scan() {
  const definitions = new Map();
  const handWrittenDebug = new Set();
  const macroKeyTypes = new Set();
  let keysSource = "";
  for (const path of await productSources()) {
    const contents = await readFile(path, "utf8");
    const lines = contents.split(/\r?\n/);
    const location = relative(REPOSITORY_ROOT, path).split("\\").join("/");
    if (location.endsWith("crates/crypto/src/keys.rs")) {
      keysSource = contents;
    }
    for (const match of contents.matchAll(
      /\bsecret_key!\s*\(\s*\n?\s*([A-Za-z_][A-Za-z0-9_]*)\s*,/g,
    )) {
      macroKeyTypes.add(match[1]);
    }
    for (const match of contents.matchAll(
      /\bimpl\s+(?:core::fmt::|std::fmt::|fmt::)?(?:Debug|Display)\s+for\s+([A-Za-z_][A-Za-z0-9_]*)/g,
    )) {
      handWrittenDebug.add(match[1]);
    }
    lines.forEach((line, index) => {
      const match = DEFINITION_PATTERN.exec(line);
      if (match === null) {
        return;
      }
      const sites = definitions.get(match[2]) ?? [];
      sites.push({
        name: match[2],
        kind: match[1],
        location: `${location}:${index + 1}`,
        derives: derivedTraits(attributesAbove(lines, index)),
        // Comments are stripped so prose naming a type is not read as a field.
        body: bodyAt(lines, index).replace(/\/\/.*$/gm, ""),
      });
      definitions.set(match[2], sites);
    });
  }
  return { definitions, handWrittenDebug, macroKeyTypes, keysSource };
}

const { definitions, handWrittenDebug, macroKeyTypes, keysSource } = await scan();

test("the secret_key! macro still declares the ADR-005 key types and redacts them", () => {
  assert.ok(
    macroKeyTypes.size >= 11,
    `expected at least the eleven ADR-005 key types from secret_key!, found ${macroKeyTypes.size}: ${[...macroKeyTypes].sort().join(", ")}`,
  );
  const macroBody = keysSource.slice(
    keysSource.indexOf("macro_rules! secret_key"),
    keysSource.indexOf("secret_key!("),
  );
  assert.ok(
    macroBody.length > 0,
    "the secret_key! macro body was not found in crates/crypto/src/keys.rs",
  );
  assert.ok(
    !/#\s*\[\s*derive\s*\([^)]*\bDebug\b/.test(macroBody),
    "secret_key! must not derive Debug: every key type it declares would print its bytes",
  );
  assert.ok(
    /impl\s+fmt::Debug\s+for\s+\$name/.test(macroBody),
    "secret_key! must hand-write the redacting Debug its key types rely on",
  );
});

test("every secret_key! type is named in the zeroize-on-drop enumeration", () => {
  // The enumeration is written out by name rather than counted, so it has to be
  // extended when a key type is added. `RehearsalKey` was the eleventh type and
  // was missing from it; nothing failed, because nothing checked. This does.
  const enumeration = keysSource.slice(
    keysSource.indexOf("fn every_key_type_is_zeroize_on_drop"),
  );
  const body = enumeration.slice(0, enumeration.indexOf("\n    }"));
  const unlisted = [...macroKeyTypes].filter(
    (name) => !body.includes(`assert_zeroize_on_drop::<${name}>()`),
  );
  assert.deepEqual(
    unlisted.sort(),
    [],
    `secret_key! declares these key types and every_key_type_is_zeroize_on_drop does not name them: ${unlisted.join(", ")}`,
  );
});

test("every registered secret-bearing type still exists", () => {
  const missing = [...SECRET_BEARING_TYPES.keys()].filter(
    (name) => !definitions.has(name),
  );
  assert.deepEqual(
    missing,
    [],
    `renamed or removed, so this guard silently stopped covering them: ${missing.join(", ")}`,
  );
});

test("no registered secret-bearing type derives Debug or Display", () => {
  const leaks = [];
  for (const [name, carries] of SECRET_BEARING_TYPES) {
    for (const site of definitions.get(name) ?? []) {
      for (const trait of ["Debug", "Display"]) {
        if (site.derives.has(trait)) {
          leaks.push(
            `${site.location}: ${site.kind} ${name} derives ${trait} over ${carries}; write the impl by hand and redact`,
          );
        }
      }
    }
  }
  assert.deepEqual(leaks.sort(), [], leaks.join("\n"));
});

test("every registered secret-bearing type has a hand-written redacting Debug", () => {
  const undebuggable = [...SECRET_BEARING_TYPES.keys()].filter(
    (name) => !handWrittenDebug.has(name),
  );
  assert.deepEqual(
    undebuggable.sort(),
    [],
    `these carry secrets and have no hand-written Debug, so any added derive would leak: ${undebuggable.join(", ")}`,
  );
});

test("no unregistered type derives Debug over a raw key or plaintext buffer", () => {
  const leaks = [];
  const exercised = new Set();
  for (const sites of definitions.values()) {
    for (const site of sites) {
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        const [, fieldName, fieldType] = field;
        if (
          !SECRET_FIELD_NAMES.test(fieldName) ||
          !RAW_BYTE_TYPES.test(fieldType.trim())
        ) {
          continue;
        }
        const qualified = `${site.name}.${fieldName}`;
        if (PUBLIC_BYTES.has(qualified)) {
          exercised.add(qualified);
          continue;
        }
        if (!site.derives.has("Debug") && !site.derives.has("Display")) {
          continue;
        }
        leaks.push(
          `${site.location}: ${site.kind} ${site.name} derives Debug over ${qualified}: ${fieldType.trim()}. Write the impl by hand and redact, or record in PUBLIC_BYTES why these bytes are public.`,
        );
      }
    }
  }
  assert.deepEqual(leaks.sort(), [], leaks.join("\n"));

  // An exception that no longer matches anything is stale and hides nothing,
  // so it is removed rather than left to look like coverage.
  const stale = [...PUBLIC_BYTES.keys()].filter((entry) => !exercised.has(entry));
  assert.deepEqual(
    stale.sort(),
    [],
    `these PUBLIC_BYTES exceptions match no field any more and must be deleted: ${stale.join(", ")}`,
  );
});
