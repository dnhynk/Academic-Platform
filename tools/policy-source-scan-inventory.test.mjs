// `docs/contracts/policy-source-scans.md` names every file in this repository
// that reads another file's Rust source text.
//
// The page says "this page enumerates every one of them", and it has now been
// wrong twice. `P2-G2` found it missing the egress rows. `T141` found it
// missing three more, and one of those -- `crates/crypto/tests/key_hierarchy.rs`
// -- was the weaker half of a contract whose other half had already been
// repaired, so nobody comparing the two ever saw them side by side. A sentence
// nothing executes is a sentence that decays, and the decay is invisible
// exactly where the page is supposed to help.
//
// So the claim is executed. The page cannot know what a scan *means*, so this
// does not try: it finds every file that names a Rust source path in a position
// where it is read, and requires the page to name that file. That is narrower
// than "every policy source scan" and it is what can be checked mechanically --
// the page's own sentence is written to that width, and no wider.
//
// A file this catches that is not a policy source scan is listed anyway, in the
// page's "not a source-text scan" rows, with what it does instead. That is the
// intended outcome for a false positive: the page gets a row, not a hole.

import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));
const INVENTORY = "docs/contracts/policy-source-scans.md";

/** Trees that can hold a scan. Everything else is generated or vendored. */
const SEARCH_ROOTS = ["crates", "tools", "packages"];

/** Extensions a scan can be written in. */
const SEARCH_EXTENSIONS = [".rs", ".mjs", ".ts"];

/** Directories that hold no reviewed source. */
const SKIP_DIRECTORIES = new Set(["node_modules", "target", "dist"]);

/**
 * A `#[path = "..."]` names a file the compiler pulls in, not a file the code
 * reads. `crates/vault/tests/encrypted_objects.rs` includes two shared test
 * modules that way and reads no source at all, so the attribute is removed
 * before the markers below are applied.
 */
const PATH_ATTRIBUTE = /#\[\s*path\s*=\s*"[^"]*"\s*\]/gu;

/**
 * The positions a `.rs` path can appear in that mean the file reads source.
 *
 * Each is a shape this repository actually uses. `include_str!` is the fixed
 * three-path form; a literal argument to a read call and a `join(...)` are the
 * `CARGO_MANIFEST_DIR`-rooted forms; an extension comparison is what every
 * recursive walk filters on; and a `const` or a table entry holding a path is
 * how `rotation_gate.rs` and the two contract registries name their targets.
 */
const MARKERS = [
  ["include_str", /include_str!\(\s*"[^"]+\.rs"/u],
  ["read-literal", /(?:read_to_string|fs::read|readFile|readFileSync)\([^\n]*\.rs"/u],
  ["join-literal", /join\(\s*"[^"\n]*\.rs"\s*\)/u],
  ["ext-filter", /(?:==|!=)\s*"rs"|endsWith\("\.rs"\)/u],
  ["const-path", /=\s*"[^"\n]*\.rs"\s*;/u],
  ["table-path", /\(\s*"[^"\n]*\.rs"\s*,/u],
];

/** Every reviewed source file under the search roots. */
async function searchableFiles() {
  const found = [];
  const pending = SEARCH_ROOTS.map((root) => join(REPOSITORY_ROOT, root));
  while (pending.length > 0) {
    const directory = pending.pop();
    let entries;
    try {
      entries = await readdir(directory, { withFileTypes: true });
    } catch {
      continue;
    }
    for (const entry of entries) {
      const path = join(directory, entry.name);
      if (entry.isDirectory()) {
        if (!SKIP_DIRECTORIES.has(entry.name)) {
          pending.push(path);
        }
      } else if (SEARCH_EXTENSIONS.some((extension) => entry.name.endsWith(extension))) {
        found.push(path);
      }
    }
  }
  return found.toSorted();
}

/** The marker that says `text` reads Rust source text, or `null`. */
function sourceReadMarker(text) {
  const withoutPathAttributes = text.replaceAll(PATH_ATTRIBUTE, "");
  for (const [name, pattern] of MARKERS) {
    if (pattern.test(withoutPathAttributes)) {
      return name;
    }
  }
  return null;
}

/** Absolute paths of the modules `text` pulls in with `#[path = "..."]`. */
function includedModules(path, text) {
  return [...text.matchAll(/#\[\s*path\s*=\s*"([^"]*)"\s*\]/gu)].map(([, target]) =>
    resolve(dirname(path), target),
  );
}

test("the markers are not vacuous", () => {
  // A marker list that matched nothing would make the assertion below pass over
  // an empty set, which is the empty-scan shape the inventory page is about.
  const samples = {
    include_str: 'let s = include_str!("../src/lib.rs");',
    "read-literal": 'let s = read_to_string(root.join("a"))?; readFile("crates/x/src/a.rs");',
    "join-literal": 'let p = root.join("rotation.rs");',
    "ext-filter": 'if extension == "rs" { }',
    "const-path": 'const GATE_SOURCE: &str = "src/rotation.rs";',
    "table-path": 'const SITES: [(&str, &str); 1] = [("src/engine.rs", "pub fn begin(")];',
  };
  for (const [name, pattern] of MARKERS) {
    assert.equal(pattern.test(samples[name]), true, `${name} matches nothing`);
  }
  assert.equal(sourceReadMarker('#[path = "../../test-support/src/shared.rs"]\nmod shared;'), null);
  assert.equal(sourceReadMarker('LogicalPath::parse("src/domain.rs")'), null);
});

test("every file that reads Rust source text is named in the inventory", async () => {
  const page = await readFile(join(REPOSITORY_ROOT, INVENTORY), "utf8");
  const texts = new Map();
  for (const path of await searchableFiles()) {
    texts.set(path, await readFile(path, "utf8"));
  }

  const markers = new Map();
  for (const [path, text] of texts) {
    const marker = sourceReadMarker(text);
    if (marker !== null) {
      markers.set(path, marker);
    }
  }

  // A scan that delegates its walk to a shared module reads source through that
  // module and names no path of its own. `KY06`'s two halves both do exactly
  // that, and the crypto half is the scan `T141` found missing from this page —
  // so the delegation is followed rather than left as the hole it would be.
  for (const [path, text] of texts) {
    if (markers.has(path)) {
      continue;
    }
    if (includedModules(path, text).some((target) => markers.has(target))) {
      markers.set(path, "included-module");
    }
  }

  const readers = [...markers].map(([path, marker]) => [
    relative(REPOSITORY_ROOT, path).split("\\").join("/"),
    marker,
  ]);
  readers.sort(([left], [right]) => (left < right ? -1 : left > right ? 1 : 0));

  // The floor is what fails if the walk above stops finding files: a scan
  // deleted is a row the page should lose deliberately, and a walk that returns
  // nothing would otherwise satisfy every assertion made over its result.
  assert.equal(
    readers.length >= 20,
    true,
    `the walk found only ${readers.length} files that read Rust source text`,
  );

  const missing = readers
    .filter(([relativePath]) => !page.includes(relativePath))
    .map(([relativePath, marker]) => `${relativePath} (${marker})`);
  assert.deepEqual(
    missing,
    [],
    `${INVENTORY} does not name every file that reads this repository's Rust source text`,
  );
});
