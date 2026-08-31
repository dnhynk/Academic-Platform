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
// that leaks. So the rule is checked mechanically rather than by review, in
// five halves that fail for different reasons:
//
//   1. A registry of the types already known to carry secrets. Each must still
//      exist, must not derive `Debug` or `Display`, and must have a
//      hand-written `Debug`. This is what fails if someone adds a derive back.
//   2. The registry is itself checked against the source: a type that holds
//      secret bytes in a field of its own and hand-writes a redacting `Debug`
//      must be in it. `T114` found that *deleting* a registration was silent —
//      every other check iterates the registry, so a type removed from it
//      simply stopped being covered.
//   3. What a hand-written `Debug` prints. `T114` injected an impl that
//      redacted every field but one; the registry said the impl existed and
//      nothing read it. A raw byte field may reach the formatter only through
//      a length.
//   4. A discovery net over every other type, for the ones nobody has listed
//      yet: a type that derives `Debug` and owns a raw byte buffer under a
//      field name from the key-material vocabulary. Bytes that are genuinely
//      public are named below with the reason they are public, so the net
//      documents its own exceptions instead of hiding them.
//   5. The same net for tuple structs and tuple enum variants, which have no
//      field name at all. `T114` found `RecoveredSecret`, `BackupMasterKey`,
//      and a variant `Dek([u8; 32])` invisible to a guard that read only named
//      fields.
//
// The shapes the net reads are the ones `T114`'s injection matrix reached:
// `&'a [u8]`, `Option<Vec<u8>>`, a path-qualified `zeroize::Zeroizing<...>`,
// a `cfg_attr`-wrapped derive, a single-line struct body, and a field type
// whose buffer is behind a comma inside its generic arguments.
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
  /^_?(dek|kek|key|keys|key_bytes|key_material|material|secret|secrets|secret_bytes|plaintext|plaintext_bytes|plain|digest|seed|chunk|chunk_bytes|hex|raw|passphrase|password|phrase|mnemonic|entropy|opened|vmk|skey|master)$/;

/**
 * Field types that hold bytes transparently, so a derived `Debug` prints them.
 *
 * Matched against a *normalized* spelling: `normalizeFieldType` strips a
 * borrow, an `Option`/`Box`/`Zeroizing` wrapper, and any path qualification, so
 * `&'a [u8]`, `Option<Vec<u8>>`, and `zeroize::Zeroizing<Vec<u8>>` reach it as
 * the byte buffer each one is. `T114` found all three silent.
 */
const RAW_BYTE_TYPES =
  /^(Vec\s*<\s*u8\s*>|\[\s*u8\s*;[^\]]*\]|\[\s*u8\s*\]|String|str)$/;

/**
 * The subset of {@link RAW_BYTE_TYPES} a *tuple* position carries alone.
 *
 * A named field is judged by its name and its type together, and the name is
 * what tells `Qualifier.key: String` from a key. A tuple position has no name,
 * so the type has to carry the whole signal: a byte buffer does, and `String`
 * and `str` do not — every error type in this workspace reports through one and
 * no key in `P2-K1`'s schedule is text. `T116` found `AuditLeak(Vec<u8>)`
 * silent.
 */
const RAW_BYTE_PAYLOAD_TYPES =
  /^(Vec\s*<\s*u8\s*>|\[\s*u8\s*;[^\]]*\]|\[\s*u8\s*\])$/;

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
    "QualifierSchema.key",
    "the qualifier name a predicate schema declares, which is the registry's public vocabulary",
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

/**
 * Tuple newtypes whose whole payload is a public identifier or digest.
 *
 * The payload check below cannot read a field name, so every `[u8; N]` newtype
 * reaches it and each one needs a judgement recorded here. Each entry states
 * where those bytes already are in the clear; a new newtype is refused until
 * someone writes that sentence, which is the point.
 */
const PUBLIC_TUPLE_BYTES = new Map([
  [
    "ProfileId",
    "the profile identity every key derivation is salted with and every profile path spells",
  ],
  [
    "DomainId",
    "the domain identity written in the clear at a fixed offset of every object header",
  ],
  [
    "ContentDigest",
    "SHA-256 over plaintext or canonical semantic bytes, which the signed artifact_descriptor row and a backup manifest's plaintext_sha256 both carry in the clear",
  ],
  [
    "VaultLocator",
    "the domain-keyed HMAC written in the clear at a fixed header offset and spelled into the object's own path",
  ],
  [
    "KeyGeneration",
    "the public generation name from ADR-005: SHA-256 of an HKDF output, readable on a locked profile and structurally not usable as a key",
  ],
  ["RotationId", "the rotation identity the journal records in the clear"],
  ["ActionId", "one retention action's identity, which the store's retention_action row carries"],
  [
    "BackupSetId",
    "the backup set identity, which is the HKDF salt written into the backup's own manifest",
  ],
  ["GenerationId", "an opaque identifier for one disposable projection generation"],
  ["OpaqueId", "an RPC wire identifier carried with no narrowing"],
  ["IdempotencyKey", "an RPC retry key, which the caller supplies and the wire carries"],
  [
    "SessionNonce",
    "the P1 handshake nonce, whose hex `capability_id` and `as_hex` publish into the owner-only runtime metadata the same user reads; it is not ADR-005 key material and what confines it is that directory",
  ],
]);

// `#[cfg_attr(<cfg>, derive(Debug))]` derives exactly as `#[derive(Debug)]`
// does whenever the cfg holds. `T114` found the guard silent on it.
const DERIVE_PATTERN =
  /#\s*\[\s*(?:cfg_attr\s*\([\s\S]*?,\s*)?derive\s*\(([\s\S]*?)\)\s*\)?\s*\]/g;
const DEFINITION_PATTERN =
  /^\s*(?:pub(?:\s*\([^)]*\))?\s+)?(struct|enum|union)\s+([A-Za-z_][A-Za-z0-9_]*)/;
// Anchored on the separator rather than on a line start: `struct X { key:
// Vec<u8>, }` written on one line is the same declaration as the rustfmt
// spelling, and `T114` found the guard reading only the second.
const NAMED_FIELD_PATTERN =
  /(?:^|[{,])\s*(?:pub(?:\s*\([^)]*\))?\s+)?([a-z_][a-z0-9_]*)\s*:\s*([^\n]+)/gm;

/**
 * Trims a captured declaration to the type itself.
 *
 * A field type is captured to the end of its line, because stopping at the
 * first comma reads `BTreeMap<DomainId, Vec<u8>>` as `BTreeMap<DomainId` and
 * loses the buffer -- which is how `T114`'s `DomainKeyring` injection stayed
 * silent. The trailing separator is removed here, at the first comma that is
 * not inside a bracket.
 */
function trimDeclaredType(text) {
  let depth = 0;
  for (let cursor = 0; cursor < text.length; cursor += 1) {
    const character = text[cursor];
    if (character === "<" || character === "(" || character === "[") {
      depth += 1;
    } else if (character === ">" || character === ")" || character === "]") {
      depth -= 1;
      if (depth < 0) {
        return text.slice(0, cursor);
      }
    } else if (depth === 0 && character === ",") {
      return text.slice(0, cursor);
    }
  }
  return text;
}

/** Payload positions of a tuple struct, which has no field name at all. */
const TUPLE_FIELD_PATTERN = /(?:^|[(,])\s*(?:pub(?:\s*\([^)]*\))?\s+)?([^,()]+)/g;

/**
 * Enum variants, spelled `Type::Variant`, whose byte payload is public.
 *
 * The check that reaches these payloads does not read the variant's name, so
 * each entry must state why its bytes may be printed rather than be excused by
 * being called something bland.
 */
const PUBLIC_TUPLE_VARIANT_BYTES = new Map([
  [
    "AcceptancePublicKey::Provisioned",
    "the user's Ed25519 public half compiled as receipt-verification authority; the private half is offline and absent",
  ],
]);

/** Enum variants written as `Dek([u8; 32])`, whose name is the only signal. */
const TUPLE_VARIANT_PATTERN = /(?:^|[,{])\s*([A-Z][A-Za-z0-9_]*)\s*\(([^()]*)\)/gm;

/**
 * Reduces a field type to the buffer it holds.
 *
 * A borrow, a path qualification, and an `Option`, `Box`, `Rc`, `Arc`,
 * `Zeroizing`, or `Cow` wrapper all print their contents through a derived
 * `Debug`, so none of them makes a byte buffer safe. `T114` found each one
 * silent in turn.
 */
function normalizeFieldType(text) {
  let current = text.trim().replace(/\s+/g, " ");
  for (let round = 0; round < 8; round += 1) {
    const before = current;
    current = current.replace(/^&\s*(?:'[A-Za-z_][A-Za-z0-9_]*\s*)?(?:mut\s+)?/, "");
    current = current.replace(/^(?:[A-Za-z_][A-Za-z0-9_]*\s*::\s*)+/, "");
    const wrapped = /^(Option|Box|Rc|Arc|Zeroizing|Cow)\s*<([\s\S]*)>$/.exec(current);
    if (wrapped !== null) {
      current = wrapped[2].trim().replace(/^'[A-Za-z_][A-Za-z0-9_]*\s*,\s*/, "");
    }
    if (current === before) {
      return current;
    }
  }
  return current;
}

/** Reports whether a field type prints raw bytes through a derived `Debug`. */
function holdsRawBytes(text) {
  return RAW_BYTE_TYPES.test(normalizeFieldType(text));
}

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
    return "";
  }
  return matched(text, opener, "{", "}");
}

/** Returns the parenthesised body of a tuple struct starting at `index`. */
function tupleBodyAt(lines, index) {
  const text = lines.slice(index, index + 40).join("\n");
  const opener = text.search(/[{(;]/);
  if (opener === -1 || text[opener] !== "(") {
    return "";
  }
  return matched(text, opener, "(", ")");
}

function matched(text, opener, open, close) {
  let depth = 0;
  for (let cursor = opener; cursor < text.length; cursor += 1) {
    if (text[cursor] === open) {
      depth += 1;
    } else if (text[cursor] === close) {
      depth -= 1;
      if (depth === 0) {
        return text.slice(opener + 1, cursor);
      }
    }
  }
  return text.slice(opener + 1);
}

/**
 * Returns the body of every hand-written `Debug` *and* `Display` impl, by type.
 *
 * Both, because `Display` prints too. The registry test reads derives only, and
 * `handWrittenDebug` recorded a `Display` impl as satisfying the "has a
 * hand-written Debug" requirement while nothing ever read what it printed —
 * `T118` injected a `Display` on the registered `OpenedHeader` that wrote
 * `self.dek` and every test passed. A type with both impls has them
 * concatenated here: each one is scanned, and neither may reach the bytes.
 */
function handWrittenFormatterBodies(contents) {
  const bodies = new Map();
  const pattern =
    /\bimpl\s+(?:core::fmt::|std::fmt::|fmt::)?(?:Debug|Display)\s+for\s+([A-Za-z_][A-Za-z0-9_]*)/g;
  for (const match of contents.matchAll(pattern)) {
    const opener = contents.indexOf("{", match.index + match[0].length);
    if (opener === -1) {
      continue;
    }
    const body = matched(contents, opener, "{", "}");
    const existing = bodies.get(match[1]);
    bodies.set(match[1], existing === undefined ? body : `${existing}\n${body}`);
  }
  return bodies;
}

/**
 * Names a formatter body binds by destructuring `self`, mapped to the field
 * each one stands for.
 *
 * `let Self { dek } = self;` and `let Self { dek: bytes, .. } = self;` both put
 * a secret field behind a bare identifier, and a net that greps `self.dek`
 * reads neither. `T118` injected the first and nothing failed.
 */
const DESTRUCTURE_PATTERN =
  /let\s+(?:Self|[A-Z][A-Za-z0-9_]*)\s*\{([^}]*)\}\s*=\s*(?:\*?self|&\s*\*?self)\b\s*;?/g;

function destructuredBindings(body) {
  const bindings = new Map();
  for (const match of body.matchAll(DESTRUCTURE_PATTERN)) {
    for (const part of match[1].split(",")) {
      const trimmed = part.trim();
      if (trimmed === "" || trimmed === "..") {
        continue;
      }
      const [field, alias] = trimmed.split(":").map((piece) => piece.trim());
      if (field === "") {
        continue;
      }
      bindings.set(alias === undefined || alias === "" ? field : alias, field);
    }
  }
  return bindings;
}

/** Methods a formatter body may call on `self` without printing bytes. */
const FORMATTER_SAFE_METHODS = /^(len|is_empty|count|capacity)$/;


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
  const debugBodies = new Map();
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
    for (const [name, body] of handWrittenFormatterBodies(contents)) {
      const existing = debugBodies.get(name);
      debugBodies.set(name, existing === undefined ? body : `${existing}\n${body}`);
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
        // A tuple struct has no named field for the net to read, so its
        // payload positions are collected separately; `T114` found a tuple
        // secret invisible to a guard that only read named fields.
        tuple: tupleBodyAt(lines, index).replace(/\/\/.*$/gm, ""),
      });
      definitions.set(match[2], sites);
    });
  }
  return { definitions, handWrittenDebug, debugBodies, macroKeyTypes, keysSource };
}

const { definitions, handWrittenDebug, debugBodies, macroKeyTypes, keysSource } =
  await scan();

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
          !holdsRawBytes(trimDeclaredType(fieldType))
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
          `${site.location}: ${site.kind} ${site.name} derives Debug over ${qualified}: ${trimDeclaredType(fieldType).trim()}. Write the impl by hand and redact, or record in PUBLIC_BYTES why these bytes are public.`,
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


/** Reports whether a type text holds a raw byte buffer anywhere inside it. */
function containsRawByteBuffer(text) {
  const normalized = normalizeFieldType(text);
  return (
    RAW_BYTE_TYPES.test(normalized) ||
    /(Vec\s*<\s*u8\s*>|\[\s*u8\s*(;[^\]]*)?\])/.test(normalized)
  );
}

/** Type-name tokens a field's declared type mentions. */
function typeTokens(text) {
  return normalizeFieldType(text).match(/[A-Za-z_][A-Za-z0-9_]*/g) ?? [];
}

/** Every field a definition declares, as `{ name, type }`. */
function declaredFields(site) {
  const fields = [...site.body.matchAll(NAMED_FIELD_PATTERN)].map((match) => ({
    name: match[1],
    type: trimDeclaredType(match[2]),
  }));
  for (const position of site.tuple.matchAll(TUPLE_FIELD_PATTERN)) {
    const type = position[1].trim();
    if (type !== "") {
      fields.push({ name: null, type });
    }
  }
  return fields;
}

/**
 * Types that carry key material or plaintext, computed from the source.
 *
 * A field carries it when its declared type *is* one of the `secret_key!` key
 * types, or when it holds a raw byte buffer under a name from the vocabulary,
 * or -- for a tuple position, which has no name -- when it holds one at all.
 * A type holding such a type is one too, iterated to a fixed point so a two-hop
 * holder is reached: `EncryptedDomainKeyring` holds `DomainObjectKeys` holds
 * `DomainKek`.
 *
 * Propagation runs over *declared field types* only. Reading the whole body
 * would make every type that merely mentions a key type in a method secret,
 * which is how a first attempt at this flagged `AcceptanceService`.
 */
function secretBearingTypeNames() {
  const direct = new Set();
  const bearing = new Set(macroKeyTypes);
  for (const [name, sites] of definitions) {
    for (const site of sites) {
      for (const field of declaredFields(site)) {
        const isKeyType = macroKeyTypes.has(normalizeFieldType(field.type));
        const named =
          field.name !== null &&
          SECRET_FIELD_NAMES.test(field.name) &&
          containsRawByteBuffer(field.type);
        const positional = field.name === null && containsRawByteBuffer(field.type);
        if (isKeyType || named || positional) {
          bearing.add(name);
          direct.add(name);
        }
      }
    }
  }
  for (let round = 0; round < 8; round += 1) {
    let grew = false;
    for (const [name, sites] of definitions) {
      if (bearing.has(name)) {
        continue;
      }
      const reaches = sites
        .flatMap(declaredFields)
        .flatMap((field) => typeTokens(field.type))
        .some((token) => token !== name && bearing.has(token));
      if (reaches) {
        bearing.add(name);
        grew = true;
      }
    }
    if (!grew) {
      break;
    }
  }
  return { direct, bearing };
}


const { direct: DIRECT_SECRET_BEARING, bearing: SECRET_BEARING } =
  secretBearingTypeNames();

test("a type whose Debug is hand-written over secret bytes is registered", () => {
  // Deleting a registration was silent: the remaining tests all iterate the
  // registry, so a type removed from it stopped being covered and nothing
  // failed. The registry is now checked against the source. A type that carries
  // secret bytes in a field of its own -- and whose author wrote `Debug` by
  // hand rather than deriving it -- is exactly the shape the registry is for,
  // so it must be in it. A type that only reaches a secret through another
  // type is not required here: the inner type's own redacting `Debug` is what
  // a derive on the outer one would print, and a type holding a secret with no
  // `Debug` at all cannot be derived over without a compile error.
  const unregistered = [];
  for (const [name, body] of debugBodies) {
    if (SECRET_BEARING_TYPES.has(name) || !DIRECT_SECRET_BEARING.has(name)) {
      continue;
    }
    if (!/<redacted>|finish_non_exhaustive/.test(body)) {
      continue;
    }
    unregistered.push(name);
  }
  assert.deepEqual(
    unregistered.sort(),
    [],
    `these hand-write a redacting Debug over secret bytes and are not in SECRET_BEARING_TYPES, so removing that impl would be silent: ${unregistered.join(", ")}`,
  );
});

test("no hand-written Debug prints a secret field it was written to hide", () => {
  // The registry says a type has a hand-written `Debug`; it did not say what
  // that `Debug` prints. `T114` injected an impl that redacted every field but
  // one and neither this guard nor the Rust unit test noticed. A raw byte field
  // may reach the formatter only through a length.
  //
  // It runs over every type that holds secret bytes in a field of its own, not
  // only the registered ones. The registration test above reads an impl only
  // when it contains `<redacted>` or `finish_non_exhaustive`, so an impl that
  // does not redact at all was in neither net: `T116` injected an unregistered
  // type whose hand-written `Debug` printed `self.dek` and nothing failed.
  const leaks = [];
  const handWritten = new Set([...SECRET_BEARING_TYPES.keys(), ...DIRECT_SECRET_BEARING]);
  for (const name of handWritten) {
    const body = debugBodies.get(name);
    if (body === undefined) {
      continue;
    }
    const rawFields = new Set();
    for (const site of definitions.get(name) ?? []) {
      for (const field of site.body.matchAll(NAMED_FIELD_PATTERN)) {
        const declared = trimDeclaredType(field[2]);
        if (PUBLIC_BYTES.has(`${name}.${field[1]}`)) {
          continue;
        }
        if (holdsRawBytes(declared) || SECRET_BEARING.has(normalizeFieldType(declared))) {
          rawFields.add(field[1]);
        }
      }
    }
    for (const field of rawFields) {
      const uses = body.matchAll(
        new RegExp(`self\\s*\\.\\s*${field}\\b([^,)]*)`, "g"),
      );
      for (const use of uses) {
        if (/^\s*\.\s*(len|is_empty|count|capacity)\s*\(/.test(use[1])) {
          continue;
        }
        leaks.push(
          `${name}: its hand-written Debug reaches self.${field} without reducing it to a length, so the bytes it was written to hide are printed`,
        );
      }
    }

    // The same field reached through a name the body bound by destructuring.
    // The binding statements are removed first, so what is left is uses of the
    // name and nothing else: `{dek:?}` in a format string is a use and the
    // `let Self { dek } = self;` that introduced it is not.
    const afterBinding = body.replace(DESTRUCTURE_PATTERN, " ");
    for (const [binding, field] of destructuredBindings(body)) {
      if (!rawFields.has(field)) {
        continue;
      }
      const uses = afterBinding.matchAll(
        new RegExp(`(?<![.\\w])${binding}\\b([^,)]*)`, "gu"),
      );
      for (const use of uses) {
        if (/^\s*\.\s*(len|is_empty|count|capacity)\s*\(/.test(use[1])) {
          continue;
        }
        leaks.push(
          `${name}: its hand-written Debug destructures self and reaches ${field} as ${binding}, so the bytes it was written to hide are printed`,
        );
      }
    }

    // A method call on `self` is the other way out. A redacting formatter has
    // no reason to make one, and one that does hands the decision to code this
    // net never reads: `T118` injected `self.render()` returning
    // `hex::encode(self.dek)` and nothing failed. Reducing a buffer to a length
    // is the exception, and it is the only one.
    if (rawFields.size > 0) {
      for (const call of body.matchAll(/self\s*\.\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(/g)) {
        if (FORMATTER_SAFE_METHODS.test(call[1])) {
          continue;
        }
        leaks.push(
          `${name}: its hand-written Debug calls self.${call[1]}(), so what reaches the formatter is decided somewhere this guard cannot read`,
        );
      }
    }
  }
  assert.deepEqual(leaks.sort(), [], leaks.join("\n"));
});

test("every public tuple payload exception still names a type that has one", () => {
  // The named exceptions are checked the same way. An entry left behind after
  // its type changed shape is a judgement about something that no longer
  // exists, and the next reader would take it for a live one.
  const stale = [];
  for (const name of PUBLIC_TUPLE_BYTES.keys()) {
    const sites = definitions.get(name) ?? [];
    const bears = sites.some((site) =>
      [...site.tuple.matchAll(TUPLE_FIELD_PATTERN)].some((position) =>
        RAW_BYTE_PAYLOAD_TYPES.test(normalizeFieldType(position[1].trim())),
      ),
    );
    if (!bears) {
      stale.push(name);
    }
  }
  assert.deepEqual(
    stale.sort(),
    [],
    `these are excepted as public tuple bytes but no longer declare a byte payload: ${stale.join(", ")}`,
  );
});

test("no unregistered tuple type derives Debug over a secret payload", () => {
  // A tuple struct and a tuple enum variant have no field name, so the named
  // net never saw them. `T114` found `RecoveredSecret`, `BackupMasterKey`, and
  // an enum variant `Dek([u8; 32])` all silent, and `T116` found a plain
  // `AuditLeak(Vec<u8>)` silent after them: the payload check read only
  // `Zeroizing` and the `secret_key!` types, so a tuple that simply held the
  // bytes was not a shape it looked for.
  const leaks = [];
  for (const [name, sites] of definitions) {
    for (const site of sites) {
      if (!site.derives.has("Debug") && !site.derives.has("Display")) {
        continue;
      }
      if (site.tuple !== "") {
        for (const position of site.tuple.matchAll(TUPLE_FIELD_PATTERN)) {
          const payload = position[1].trim();
          if (payload === "") {
            continue;
          }
          const zeroizing = /(^|::)Zeroizing\s*</.test(payload);
          const rawBytes =
            RAW_BYTE_PAYLOAD_TYPES.test(normalizeFieldType(payload)) &&
            !PUBLIC_TUPLE_BYTES.has(name);
          if (zeroizing || rawBytes || macroKeyTypes.has(normalizeFieldType(payload))) {
            leaks.push(
              `${site.location}: ${site.kind} ${name} derives Debug over the tuple payload ${payload}`,
            );
          }
        }
      }
      if (site.kind === "enum") {
        for (const variant of site.body.matchAll(TUPLE_VARIANT_PATTERN)) {
          const [, variantName, payload] = variant;
          const snake = variantName
            .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
            .toLowerCase();
          const named = SECRET_FIELD_NAMES.test(snake);
          for (const position of payload.matchAll(TUPLE_FIELD_PATTERN)) {
            const inner = position[1].trim();
            if (inner === "") {
              continue;
            }
            const normalized = normalizeFieldType(inner);
            // A variant name is one signal and the payload type is the other.
            // Requiring both is what made `Payload(Vec<u8>)` and
            // `Buffer([u8; 32])` silent while `Dek([u8; 32])` was caught —
            // `T118` injected exactly that. A raw byte payload is enough on its
            // own, the same way it is for a tuple struct, and no enum in this
            // workspace declares one that is public.
            const secretByName =
              named && (holdsRawBytes(inner) || macroKeyTypes.has(normalized));
            const secretByPayload =
              (RAW_BYTE_PAYLOAD_TYPES.test(normalized) ||
                macroKeyTypes.has(normalized) ||
                /(^|::)Zeroizing\s*</.test(inner)) &&
              !PUBLIC_TUPLE_VARIANT_BYTES.has(`${name}::${variantName}`);
            if (secretByName || secretByPayload) {
              leaks.push(
                `${site.location}: enum ${name} derives Debug over variant ${variantName}(${inner})`,
              );
            }
          }
        }
      }
    }
  }
  assert.deepEqual(leaks.sort(), [], leaks.join("\n"));
});
