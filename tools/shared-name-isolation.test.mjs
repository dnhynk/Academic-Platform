// Every name this repository puts in a namespace it does not own is enumerated.
//
// A *shared name* is one that resolves to the same object for two processes on
// one machine: a path under the home directory or the temporary directory, an
// environment variable the machine set rather than this run, an AppContainer
// profile. A test that grabs one with a fixed string works alone and destroys
// another process's state when two of them run at once, and the damage is silent
// in both directions -- the process that was destroyed reports a failure with no
// cause, and the one that destroyed it reports success.
//
// This repository has had that defect three times, twice in one file. The
// `P2-G4` acceptance suite wrote its home canary to
// `<home>/.academic-worker-g4-<label>` and removed it on `Drop`; two lanes
// running it at once deleted each other's canary and one reported 1 passed and 7
// failed. The same suite's Windows backend asks for one fixed AppContainer
// profile name on every launch; two concurrent asks for an absent profile tear
// its directory down, and every `CreateProcessW` issued into the container while
// it was absent failed with `ERROR_FILE_NOT_FOUND` -- about one run in ten.
// `rotation_gate.rs` built `%TEMP%/academic-rotation-gate-recipients` and
// removed it at both ends of the test.
//
// So the claim is executed. What follows are five whole sets, each compared with
// a committed table in both directions. A list of forbidden spellings would be
// the wrong shape and this repository has measured why: it refuses the edits
// somebody thought of and admits every edit spelled differently. A whole set
// refuses an addition it has never seen, whatever it is called, and the price is
// that a legitimate addition needs a row. A row is the intended outcome, not a
// hole.
//
// The width of the claim is exactly what the five sets check and no wider. This
// file does not decide whether a name is *safe*. It decides that no name reaches
// a shared namespace without a row saying somebody looked.

import assert from "node:assert/strict";
import { readdir, readFile } from "node:fs/promises";
import { dirname, join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const REPOSITORY_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));

/** Trees that can name something outside this process. */
const SEARCH_ROOTS = ["crates"];

/** Directories that hold no reviewed source. */
const SKIP_DIRECTORIES = new Set(["node_modules", "target", "dist"]);

// ---------------------------------------------------------------------------
// Reading Rust well enough to tell code from text.
// ---------------------------------------------------------------------------

/**
 * Splits a file into code and string literals, in one pass.
 *
 * A scan file quotes the shapes it refuses, so `absolute_paths("let _ =
 * std::env::var(k);")` spells an environment read that no process ever makes.
 * Counting text as code there reports a hole in the file whose whole job is to
 * close one, and the three files that do it are among the most careful in this
 * repository. Comments are blanked rather than deleted so every offset still
 * points at the same character it did.
 *
 * The three things that are not a string and look like one are handled: a line
 * comment, a block comment, and a char literal holding a quote. A lifetime --
 * `&'static str` -- is a lone apostrophe and is left alone, which is why a char
 * literal is recognised only when its closing quote is where one can be.
 */
function lex(source) {
  const code = [...source];
  const spans = [];
  let index = 0;
  const blank = (from, to) => {
    for (let at = from; at < to && at < code.length; at += 1) {
      if (code[at] !== "\n") {
        code[at] = " ";
      }
    }
  };
  while (index < source.length) {
    const character = source[index];
    const next = source[index + 1];
    if (character === "/" && next === "/") {
      let end = source.indexOf("\n", index);
      end = end === -1 ? source.length : end;
      blank(index, end);
      index = end;
      continue;
    }
    if (character === "/" && next === "*") {
      let depth = 1;
      let at = index + 2;
      while (at < source.length && depth > 0) {
        if (source[at] === "/" && source[at + 1] === "*") {
          depth += 1;
          at += 2;
        } else if (source[at] === "*" && source[at + 1] === "/") {
          depth -= 1;
          at += 2;
        } else {
          at += 1;
        }
      }
      blank(index, at);
      index = at;
      continue;
    }
    if (character === "'") {
      // `'a'` and `'\n'` are char literals; `'static` is a lifetime.
      const close = next === "\\" ? source.indexOf("'", index + 2) : index + 2;
      if (source[close] === "'" && close - index <= 5) {
        index = close + 1;
        continue;
      }
      index += 1;
      continue;
    }
    if (character === "r" && !/[A-Za-z0-9_]/u.test(source[index - 1] ?? "")) {
      const opener = /^r(#*)"/u.exec(source.slice(index, index + 40));
      if (opener) {
        const terminator = `"${opener[1]}`;
        const close = source.indexOf(terminator, index + opener[0].length);
        const end = close === -1 ? source.length : close + terminator.length;
        spans.push([index, end]);
        index = end;
        continue;
      }
    }
    if (character === '"') {
      let at = index + 1;
      while (at < source.length) {
        if (source[at] === "\\") {
          at += 2;
          continue;
        }
        if (source[at] === '"') {
          break;
        }
        at += 1;
      }
      spans.push([index, at + 1]);
      index = at + 1;
      continue;
    }
    index += 1;
  }
  return { code: code.join(""), spans };
}

/**
 * A view with the whitespace Rust allows inside a path removed, and a map back.
 *
 * `::std :: env :: temp_dir()` is one path and `std::env::temp_dir()` is the same
 * one. Deleting *all* whitespace is wrong in the direction that matters -- it
 * joins unrelated tokens, so `Formatter and core::str` becomes one word and a
 * real key disappears -- so only the whitespace on either side of a `::` goes.
 * `policy-source-scans.md` records the two vacuous passes that taught this.
 *
 * The map is not decoration. Padding the removed whitespace back with spaces
 * keeps every offset pointing at the same character and *defeats the collapse*:
 * `std ::  env` still has a gap between `std::` and `env`, so a path spelled with
 * interior whitespace reads as two paths and matches neither. `N-I1` injected
 * exactly that spelling and this scan passed it. So the text really is collapsed,
 * and `map[i]` is the offset in the original that character came from, which is
 * what the string-literal check needs.
 */
function collapsePaths(source) {
  const characters = [];
  const map = [];
  let last = 0;
  const copy = (from, to) => {
    for (let at = from; at < to; at += 1) {
      characters.push(source[at]);
      map.push(at);
    }
  };
  for (const match of source.matchAll(/\s*::\s*/gu)) {
    copy(last, match.index);
    characters.push(":", ":");
    map.push(match.index, match.index);
    last = match.index + match[0].length;
  }
  copy(last, source.length);
  return { code: characters.join(""), map };
}

/**
 * Whether an offset in the collapsed view falls inside a string literal.
 *
 * The spans are measured on the original text, so the offset is mapped back
 * before it is compared. A collapsed offset compared against original spans is
 * an off-by-however-much-whitespace-a-file-holds, which grows with the file.
 */
function insideString(file, at) {
  const original = file.map[at] ?? at;
  return file.spans.some(([start, end]) => original >= start && original < end);
}

/**
 * Reads one balanced `(...)` starting at the opening parenthesis.
 *
 * A `join` argument is a `format!` with its own parentheses and its own string
 * literals, so counting to the first `)` reads a fragment and pins a fragment.
 */
function balancedCall(text, open) {
  let depth = 0;
  let inString = false;
  for (let index = open; index < text.length; index += 1) {
    const character = text[index];
    if (inString) {
      if (character === "\\") {
        index += 1;
      } else if (character === '"') {
        inString = false;
      }
      continue;
    }
    if (character === '"') {
      inString = true;
    } else if (character === "(") {
      depth += 1;
    } else if (character === ")") {
      depth -= 1;
      if (depth === 0) {
        return text.slice(open, index + 1);
      }
    }
  }
  return null;
}

/** The name of the `fn` a byte offset falls inside, or null at file scope. */
function enclosingFunctions(code) {
  const functions = [];
  for (const match of code.matchAll(/(?<!\w)fn\s+([a-z_][a-z0-9_]*)\s*[(<]/gu)) {
    functions.push({ name: match[1], at: match.index });
  }
  return (offset) => {
    let found = null;
    for (const entry of functions) {
      if (entry.at > offset) {
        break;
      }
      found = entry.name;
    }
    return found;
  };
}

/** `const NAME: &str = "VALUE";`, so an identifier can be resolved. */
function stringConstants(source) {
  const constants = new Map();
  const pattern =
    /(?:const|static)\s+([A-Z][A-Z0-9_]*)\s*:\s*&(?:'static\s+)?str\s*=\s*"([^"\n]*)"/gu;
  for (const match of source.matchAll(pattern)) {
    constants.set(match[1], match[2]);
  }
  return constants;
}

/** `for x in ["A", "B"]`, so a loop variable can be resolved. */
function loopLiteralArrays(source) {
  const arrays = new Map();
  for (const match of source.matchAll(/for\s+([a-z_][a-z0-9_]*)\s+in\s*\[([^\]]*)\]/gu)) {
    const literals = [...match[2].matchAll(/"([^"\n]*)"/gu)].map((entry) => entry[1]);
    if (literals.length > 0) {
      arrays.set(match[1], literals);
    }
  }
  return arrays;
}

/**
 * The names a `var`/`var_os` call reads, or `null` when it cannot be resolved.
 *
 * `null` is a failure and not a skip. An unresolvable argument is exactly the
 * shape a bypass takes, so it stops the test with the file and the text that
 * could not be read rather than passing quietly.
 */
function resolveArgument(argument, constants, arrays) {
  const trimmed = argument.trim();
  const literal = /^"([^"\n]*)"$/u.exec(trimmed);
  if (literal) {
    return [literal[1]];
  }
  const identifier = /^&?\s*(?:[A-Za-z_][A-Za-z0-9_]*::)*([A-Za-z_][A-Za-z0-9_]*)$/u.exec(trimmed);
  if (identifier) {
    const name = identifier[1];
    if (arrays.has(name)) {
      return arrays.get(name);
    }
    if (constants.has(name)) {
      return [constants.get(name)];
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Set 1: every way this repository reaches `std::env`.
// ---------------------------------------------------------------------------

/**
 * This set is the seed for everything below: an item that hands back a directory
 * the machine owns makes every name built on it a shared name, and an item that
 * reads a variable makes whatever the machine put there this repository's input.
 * A new way to reach outside the process -- `env::home_dir`, say -- is a new item
 * here before it is anything else, which is why this is the first set.
 */
const ENV_VOCABULARY = new Map([
  ["env::args", "PROCESS_LOCAL: this process's own argument vector."],
  ["env::args_os", "PROCESS_LOCAL: the same vector, unlossy."],
  ["env::current_exe", "PROCESS_LOCAL: the path of this running image."],
  ["env::split_paths", "PROCESS_LOCAL: splits a value it is handed."],
  [
    "env::temp_dir",
    "AMBIENT_ROOT: a directory the machine owns. Every name built on it is a shared name, so each appears in SHARED_NAME_SITES.",
  ],
  [
    "env::var",
    "AMBIENT_READ: the value is whatever this machine put there. Each name read appears in ENVIRONMENT_NAMES.",
  ],
  ["env::var_os", "AMBIENT_READ: the same, unlossy."],
  [
    "env::vars",
    "AMBIENT_READ: the whole block at once. It is copied into a child's environment and no name is selected out of it, so it names nothing.",
  ],
]);

// ---------------------------------------------------------------------------
// Set 2: every environment variable read that this repository did not write.
// ---------------------------------------------------------------------------

/**
 * A variable this repository *sets* on a child it spawns is this run's own
 * protocol: the parent chose the value, no other process sees it, and it is not a
 * shared name. Those are recognised rather than listed, by collecting every name
 * handed to a `.env(...)` anywhere in the tree. What is left is read from the
 * ambient machine, and each one gets a row.
 */
const ENVIRONMENT_NAMES = new Map([
  [
    "ACADEMIC_CRYPTO_TEST_FAULT_ACTION",
    "A fault hook's action, read and never set here: an operator sets it to hold a killed child instead of aborting it. Ambient by definition, and it changes nothing unless somebody sets it.",
  ],
  ["ACADEMIC_RETENTION_TEST_FAULT_ACTION", "The same hook in `academic-retention`."],
  ["ACADEMIC_TRANSCRIPT_TEST_FAULT_ACTION", "The same hook in `academic-transcript`."],
  ["ACADEMIC_VAULT_TEST_FAULT_ACTION", "The same hook in `academic-vault`."],
  [
    "CARGO",
    "The cargo running this test, so a nested build uses the same toolchain. Read only; naming it changes nothing on the machine.",
  ],
  [
    "CARGO_MANIFEST_DIR",
    "Read by `crates/rpc/build.rs`, where cargo is what set it. It names this crate's own directory.",
  ],
  [
    "HOME",
    "The Unix home directory: a directory shared by every process of this user, so a name built under it is a shared name.",
  ],
  ["USERPROFILE", "The Windows home directory, for the same reason."],
  [
    "LOCALAPPDATA",
    "Where Windows keeps AppContainer profiles. The two sandbox backends put their profile lock beside the profile, which is the one location every lane of this user resolves identically whatever each set TEMP to.",
  ],
  ["PATH", "Read to compose a child's environment. Not a name this repository writes."],
  ["PATHEXT", "The same."],
  [
    "XDG_RUNTIME_DIR",
    "The per-user runtime directory a Unix daemon binds its socket under. Per-user and per-boot, and the socket path below it carries the profile hash.",
  ],
  [
    "DROPBOX_PATH",
    "A sync root the store refuses to place a profile inside. Read to find where the machine put it; nothing is written to it.",
  ],
  ["NEXTCLOUD_PATH", "The same."],
  ["OWNCLOUD_PATH", "The same."],
  ["SYNCTHING_ROOT", "The same."],
  ["OneDrive", "The same, on Windows."],
  ["OneDriveCommercial", "The same."],
  ["OneDriveConsumer", "The same."],
]);

/**
 * Reads whose argument is a parameter, so the name is the caller's to choose.
 *
 * `required_path(variable)` reads whatever it is handed, and following that to
 * the literals needs the data flow this scan deliberately does not do. The *site*
 * is enumerated instead: a new one fails, and its row names who supplies it. That
 * is narrower than resolving the name and it is what can be checked here.
 */
const INDIRECT_READS = new Map([
  [
    "crates/core/tests/projection_format.rs::required_env_os",
    "Called with the crash-child protocol variables this file also sets on the child.",
  ],
  [
    "crates/core/tests/projection_generation.rs::required_env_os",
    "The same, for the projection fault child.",
  ],
  [
    "crates/daemon/tests/phase1_exit.rs::required_path",
    "Called with the `P2-X1` child protocol variables this file sets on the child.",
  ],
  [
    "crates/worker/probes/worker_probe.rs::canary_read",
    "Called with the two canary variables the launcher sets on the contained process.",
  ],
]);

// ---------------------------------------------------------------------------
// Set 3: every function that hands an ambient root to its callers.
// ---------------------------------------------------------------------------

/**
 * A root reaches a `join` far from where it was read, so the set of *roots* has
 * to be closed under wrapping before the set of *names* means anything. A
 * function that reads a root and returns something else keeps it; one that
 * returns a path hands it on, and only those need a row.
 */
const ROOT_PRODUCERS = new Map([
  [
    "crates/cli/src/client.rs::default_runtime_root",
    "XDG_RUNTIME_DIR, or the temporary directory.",
  ],
  ["crates/cli/tests/cli.rs::temporary_base", "The temporary directory, canonicalised on Unix."],
  [
    "crates/cli/tests/cli.rs::runtime_base",
    "`/tmp`, short enough to leave room for a Unix socket path.",
  ],
  ["crates/core/src/service.rs::temporary_base", "The temporary directory."],
  ["crates/core/tests/support/mod.rs::temporary_base", "The temporary directory."],
  ["crates/daemon/tests/support/mod.rs::temporary_base", "The temporary directory."],
  ["crates/daemon/tests/support/mod.rs::runtime_base", "`/tmp`, for the endpoint budget."],
  ["crates/daemon/tests/phase1_exit.rs::temporary_base", "The temporary directory."],
  ["crates/daemon/tests/phase1_exit.rs::runtime_base", "`/tmp`, for the endpoint budget."],
  [
    "crates/daemon/tests/phase1_exit.rs::default_build_lane",
    "The CARGO_TARGET_DIR for the nested default-feature build. It is deliberately one shared directory; SHARED_NAME_SITES says why and when that stops being acceptable.",
  ],
  [
    "crates/daemon/tests/phase1_exit.rs::default_feature_daemon_binary",
    "The binary inside that shared lane, so its caller reads a path below a directory the machine owns.",
  ],
  ["crates/cli/src/main.rs::resolve_runtime", "The runtime root, from the client's default."],
  ["crates/portability/tests/support/mod.rs::temporary_base", "The temporary directory."],
  ["crates/portability/tests/encrypted_support/mod.rs::temporary_base", "The temporary directory."],
  [
    "crates/store/src/connection.rs::migrated_database",
    "A pair of paths under the temporary directory.",
  ],
  ["crates/store/src/entity_registry_tests.rs::temporary_base", "The temporary directory."],
  ["crates/store/src/platform/unix.rs::native_sync_roots", "The cloud-sync roots this host names."],
  ["crates/store/src/platform/windows.rs::native_sync_roots", "The same, from the OneDrive variables."],
  ["crates/store/tests/acceptance.rs::temporary_base", "The temporary directory."],
  ["crates/store/tests/bitemporal.rs::temporary_base", "The temporary directory."],
  ["crates/store/tests/encrypted_profile.rs::temporary_base", "The temporary directory."],
  ["crates/store/tests/outbox.rs::temporary_base", "The temporary directory."],
  ["crates/store/tests/profile_policy.rs::temporary_base", "The temporary directory."],
  ["crates/store/tests/sql_policy.rs::temporary_base", "The temporary directory."],
  ["crates/store-platform/tests/native.rs::temporary_base", "The temporary directory."],
  ["crates/test-support/src/synthetic_artifacts.rs::temporary_base", "The temporary directory."],
  [
    "crates/worker/tests/containment.rs::home_directory",
    "The home directory, read from USERPROFILE or HOME. The canary has to be under the real home directory, because reaching that directory is what the sandbox is proved not to do.",
  ],
  [
    "crates/worker/src/sandbox/windows.rs::profile_lock_path",
    "LOCALAPPDATA or USERPROFILE, for the file whose exclusive open serialises AppContainer profile creation.",
  ],
  [
    "crates/capture-gate/src/native/windows.rs::profile_lock_path",
    "The same, for the capture device layer's profile.",
  ],
]);

/** The variables that name a directory the machine owns. */
const ROOT_VARIABLES = new Set([
  "HOME",
  "USERPROFILE",
  "LOCALAPPDATA",
  "XDG_RUNTIME_DIR",
  "DROPBOX_PATH",
  "NEXTCLOUD_PATH",
  "OWNCLOUD_PATH",
  "SYNCTHING_ROOT",
  "OneDrive",
  "OneDriveCommercial",
  "OneDriveConsumer",
]);

// ---------------------------------------------------------------------------
// Set 4: every name built directly on an ambient root.
// ---------------------------------------------------------------------------

/**
 * What makes a name this process's own.
 *
 * A vocabulary rather than a pattern, because "does this name separate two
 * processes" has a small closed answer and a regular expression over it would
 * accept `id` inside `identity`. `P2-X7`'s nine-value sweep is the same shape.
 */
const DISCRIMINATORS = new Set([
  "process::id()",
  "thread::current().id()",
  "unique_suffix()",
  "nanos",
  "nonce",
  "sequence",
  "counter",
  "stamp",
]);

/**
 * The verdict on each name is whether it can collide with another process's.
 * `UNIQUE` means it carries something no other process spells the same way, and
 * the discriminator check below holds it to that. `SHARED` means it does not,
 * with the reason that is acceptable and when it stops being so.
 *
 * What this set covers is a name built on a producer *call*, or on a local or
 * parameter that holds the root itself. It does not follow a root through a
 * constructor -- the two `PathBuf::from(value).join(PROFILE_LOCK_FILE)` sites in
 * the sandbox backends are reached that way, and they are enumerated as
 * `profile_lock_path` in ROOT_PRODUCERS and written down in the worker sandbox
 * contract instead. That is the width of this set, and it is stated here rather
 * than implied.
 */
const SHARED_NAME_SITES = new Map([
  [
    'crates/core/src/service.rs :: temporary_base()?.join(format!("academic-s2-ipc02-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/core/tests/support/mod.rs :: temporary_base()?.join(format!("academic-projections-{}-{}-{sequence}",sanitize(label),std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/daemon/tests/phase1_exit.rs :: temporary_base()?.join(format!("academic-x1-{label}-{stamp}"))',
    "UNIQUE: `stamp` is the process id and a counter.",
  ],
  [
    'crates/daemon/tests/phase1_exit.rs :: runtime_base()?.join(format!("ax1-{stamp}"))',
    "UNIQUE: the same `stamp`.",
  ],
  [
    'crates/daemon/tests/phase1_exit.rs :: temporary_base()?.join("academic-x1-default-features")',
    "SHARED, deliberately: a CARGO_TARGET_DIR for the nested default-feature build, shared so the build is cached across runs instead of repeated. Cargo takes its own exclusive lock on a target directory, so two processes serialise rather than corrupt each other. It becomes a problem when two processes on one machine run this test with the same TEMP and one is killed mid-build, which has happened in this repository once.",
  ],
  [
    'crates/daemon/tests/phase1_exit.rs :: lane.join("debug")',
    "SHARED, and inside the shared lane above: cargo puts a debug profile's output there and every build with these features produces the same bytes. It is read, never removed.",
  ],
  [
    'crates/policy/tests/process_isolation.rs :: std::env::temp_dir().join(format!("academic-policy-canary-{}-{nonce}",std::process::id()))',
    "UNIQUE: process id and a nonce.",
  ],
  [
    'crates/policy/tests/process_isolation.rs :: std::env::temp_dir().join(format!("academic-policy-retention-{}-{nonce}",std::process::id()))',
    "UNIQUE: process id and a nonce.",
  ],
  [
    'crates/portability/tests/support/mod.rs :: temporary_base()?.join(format!("acad-b1-{label}-{}-{nanos}-{sequence}",std::process::id()))',
    "UNIQUE: process id, wall clock and a counter, reserved with `create_dir`.",
  ],
  [
    'crates/portability/tests/encrypted_support/mod.rs :: temporary_base()?.join(format!("acad-k4-{label}-{}-{nanos}-{sequence}",std::process::id()))',
    "UNIQUE: the same shape.",
  ],
  [
    'crates/recovery/tests/recovery_admission.rs :: std::env::temp_dir().join(format!("acad-k4-{label}-{}-{nanos}-{sequence}",std::process::id()))',
    "UNIQUE: the same shape.",
  ],
  [
    'crates/retention/tests/retention.rs :: std::env::temp_dir().join(format!("academic-retention-{label}-{}-{:?}",std::process::id(),std::thread::current().id()))',
    "UNIQUE: process id and thread id.",
  ],
  [
    'crates/retention/tests/rotation_gate.rs :: std::env::temp_dir().join(format!("academic-rotation-gate-recipients-{}",std::process::id()))',
    "UNIQUE: the process id, and it needs one -- the next line removes whatever is at this path, so a fixed name deleted another lane's journal mid-test.",
  ],
  [
    'crates/retention/tests/rotation_support/mod.rs :: std::env::temp_dir().join(format!("academic-retention-{label}-{}-{}",std::process::id(),unique_suffix()))',
    "UNIQUE: process id, wall clock and a counter.",
  ],
  [
    'crates/retention/tests/tombstone.rs :: std::env::temp_dir().join(format!("academic-retention-tombstone-{label}-{}",std::process::id()))',
    "UNIQUE: process id.",
  ],
  [
    'crates/store/src/aggregate_closure_tests.rs :: std::env::temp_dir().join(format!("academic-store-0004-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/src/aggregate_timeline_tests.rs :: std::env::temp_dir().join(format!("academic-store-timeline-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/src/connection.rs :: std::env::temp_dir().join(format!("academic-store-connection-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/src/curriculum_tests.rs :: std::env::temp_dir().join(format!("academic-store-0014-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/src/entity_registry_tests.rs :: temporary_base()?.join(format!("academic-store-c3-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/src/model_run_closure_tests.rs :: std::env::temp_dir().join(format!("academic-store-0007-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/src/proposal_closure_tests.rs :: std::env::temp_dir().join(format!("academic-store-0009-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/src/repository_snapshot_tests.rs :: std::env::temp_dir().join(format!("academic-store-0012-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/src/requirement_tests.rs :: std::env::temp_dir().join(format!("academic-store-0015-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/acceptance.rs :: temporary_base()?.join(format!("academic-s2-acceptance-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/api_boundary.rs :: std::env::temp_dir().join(format!("academic-store-api-boundary-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/bitemporal.rs :: temporary_base()?.join(format!("academic-s2-bitemporal-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/encrypted_profile.rs :: temporary_base()?.join(format!("academic-store-k2-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/migration.rs :: std::env::temp_dir().join(format!("academic-store-migration-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/outbox.rs :: temporary_base()?.join(format!("academic-s2-outbox-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/profile_policy.rs :: temporary_base()?.join(format!("academic-store-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/sql_policy.rs :: temporary_base()?.join(format!("academic-store-sql-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store/tests/sqlcipher_spike.rs :: std::env::temp_dir().join(format!("academic-sqlcipher-e1-{label}-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/store-platform/tests/native.rs :: temporary_base()?.join(format!("academic-store-platform-{label}-{}-{counter}",process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/test-support/src/synthetic_artifacts.rs :: base.join(format!("academic-vault-{label}-{}-{nanos}-{sequence}",std::process::id()))',
    "UNIQUE: process id, wall clock and a counter.",
  ],
  [
    'crates/transcript/tests/support/mod.rs :: std::env::temp_dir().join(format!("academic-transcript-{label}-{}-{nanos}-{sequence}",std::process::id()))',
    "UNIQUE: process id, wall clock and a counter.",
  ],
  [
    'crates/vault/src/platform/windows.rs :: std::env::temp_dir().join(format!("academic-vault-windows-{}-{sequence}",std::process::id()))',
    "UNIQUE: process id and a counter.",
  ],
  [
    'crates/worker/tests/containment.rs :: home.join(format!(".academic-worker-g4-{label}-{}-{nanos}-{sequence}",std::process::id()))',
    "UNIQUE: process id, wall clock and a counter, reserved with `create_dir`. This is the name that had none. `Drop` removes the directory, so two lanes deleted each other's canary and the survivor measured ERROR_PATH_NOT_FOUND where the backend owed ERROR_ACCESS_DENIED.",
  ],
]);

/**
 * Every `<producer>.join(<name>)`, and every `<local or parameter holding the
 * root>.join(<name>)`.
 */
function sharedNameSites(file, producerNames) {
  const code = file.code;
  const inside = enclosingFunctions(code);
  const producer = `(?:(?:std::)?env::temp_dir|${[...producerNames].join("|")})`;
  const producerCall = new RegExp(`${producer}\\s*\\(\\s*\\)`, "u");

  /** name -> the functions in which that name holds the root itself. */
  const roots = new Map();
  const note = (name, where) => {
    if (!roots.has(name)) {
      roots.set(name, new Set());
    }
    roots.get(name).add(where);
  };

  // A local holds the root when the producer call *begins* its initialiser and
  // the initialiser adds no component of its own. `let root =
  // temporary_base()?.join(..)` is already a directory this process made, and
  // every path below it is inside that one, so counting those would report a
  // shared name for `root.join("store.sqlite3")`, which is not one.
  for (const match of code.matchAll(/let\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*=\s*([^;]+);/gu)) {
    const value = match[2].replace(/\s+/gu, "");
    const head = producerCall.exec(value);
    if (head && head.index === 0 && !value.includes(".join(")) {
      note(match[1], inside(match.index) ?? "<file scope>");
    }
  }

  // A root handed to a function is still a root inside it. The canary this file
  // was written for is built in `reserve_canary_directory(&home_directory()?,
  // label)`, one call away from the read, and a scan that stopped at locals would
  // report no site in the one place a site is known to have been. The parameter
  // counts only inside the function it belongs to, because `root` and `path` name
  // derived directories elsewhere in the same files. One level, not data flow: a
  // root passed on twice is out of reach, and this says so rather than implying
  // otherwise.
  for (const call of code.matchAll(/(?<!\w)([a-z_][a-z0-9_]*)\s*\(/gu)) {
    const argumentList = balancedCall(code, call.index + call[0].length - 1);
    if (argumentList === null || !producerCall.test(argumentList)) {
      continue;
    }
    const position = argumentList
      .slice(1, -1)
      .split(",")
      .findIndex((argument) => producerCall.test(argument));
    const signature = new RegExp(`(?<!\\w)fn\\s+${call[1]}\\s*\\(([^)]*)\\)`, "u").exec(code);
    const parameter = signature?.[1]?.split(",")[position]?.split(":")[0]?.trim();
    if (parameter && /^[a-z_][a-z0-9_]*$/u.test(parameter)) {
      note(parameter, call[1]);
    }
  }

  const receiver = new RegExp(
    `(${producer}\\s*\\(\\s*\\)\\??|(?:[A-Za-z_][A-Za-z0-9_]*::)*[A-Za-z_][A-Za-z0-9_]*(?:\\([^()]*\\))?)\\s*\\.join\\s*\\(`,
    "gu",
  );
  const sites = [];
  for (const match of code.matchAll(receiver)) {
    if (insideString(file, match.index)) {
      continue;
    }
    const head = match[1].replace(/\s+/gu, "");
    if (!producerCall.test(head)) {
      const where = inside(match.index) ?? "<file scope>";
      if (!/^[a-z_][a-z0-9_]*$/u.test(head) || !roots.get(head)?.has(where)) {
        continue;
      }
    }
    const argument = balancedCall(code, match.index + match[0].length - 1);
    if (argument === null) {
      continue;
    }
    sites.push({ receiver: head, argument: argument.slice(1, -1).replace(/\s+/gu, "") });
  }
  return sites;
}

// ---------------------------------------------------------------------------
// Set 5: the AppContainer profile, whose creation has to stay serialised.
// ---------------------------------------------------------------------------

/**
 * A pin on the profile *name* would say nothing: the defect was never the name's
 * spelling, it was two callers reaching the creation call at once. So what is
 * pinned is the sequence that reaches it -- the first two statements of every
 * function that calls it -- which is what `rotation_gate.rs` does for its seven
 * gated entry points, and what `policy-source-scans.md` records as the repair for
 * a pin whose caller was left unconstrained.
 */
const PROFILE_GATE =
  "let_serialised=PROFILE_CREATION.lock().unwrap_or_else(PoisonError::into_inner);let_machine_wide=ProfileLock::acquire();";

const PROFILE_GATE_SITES = new Map([
  ["crates/worker/src/sandbox/windows.rs::container_sid", "The worker sandbox's AppContainer."],
  [
    "crates/capture-gate/src/native/windows.rs::container_sid",
    "The capture device layer's AppContainer.",
  ],
]);

// ---------------------------------------------------------------------------
// The corpus.
// ---------------------------------------------------------------------------

/** Every reviewed source file under the search roots. */
async function searchableFiles() {
  const found = [];
  const pending = SEARCH_ROOTS.map((root) => join(REPOSITORY_ROOT, root));
  while (pending.length > 0) {
    const directory = pending.pop();
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!SKIP_DIRECTORIES.has(entry.name)) {
          pending.push(join(directory, entry.name));
        }
      } else if (entry.isFile() && entry.name.endsWith(".rs")) {
        found.push(join(directory, entry.name));
      }
    }
  }
  return found.sort();
}

/** The repository-relative path, spelled with forward slashes on every host. */
function relativePath(path) {
  return relative(REPOSITORY_ROOT, path).split(sep).join("/");
}

const FILES = await Promise.all(
  (await searchableFiles()).map(async (path) => {
    const source = await readFile(path, "utf8");
    const { spans } = lex(source);
    const { code, map } = collapsePaths(lex(source).code);
    return { path: relativePath(path), source, code, map, spans };
  }),
);

/**
 * Every `const NAME: &str` in the scanned trees, and the same per crate.
 *
 * Four crates declare `FAULT_ACTION_VARIABLE` with four different values, so one
 * flat map answers three of those four reads with another crate's string. A read
 * resolves inside its own crate first and falls back to the repository, which is
 * how a probe reaches the constant its launcher declares.
 */
const ALL_CONSTANTS = new Map(FILES.flatMap((file) => [...stringConstants(file.source)]));
const CRATE_CONSTANTS = new Map();
for (const file of FILES) {
  const crate = file.path.split("/").slice(0, 2).join("/");
  if (!CRATE_CONSTANTS.has(crate)) {
    CRATE_CONSTANTS.set(crate, new Map());
  }
  const scoped = CRATE_CONSTANTS.get(crate);
  for (const [name, value] of stringConstants(file.source)) {
    scoped.set(name, value);
  }
}

/** The constants a file resolves against: its own crate's, then the repository's. */
function constantsFor(file) {
  const crate = file.path.split("/").slice(0, 2).join("/");
  return new Map([...ALL_CONSTANTS, ...(CRATE_CONSTANTS.get(crate) ?? new Map())]);
}

/** `env::var(NAME)` and `env::var_os(NAME)`, with the argument text captured. */
const ENV_READ = /(?<!\w)(?:std::)?env::var(?:_os)?\s*\(([^),]*)\)/gu;

// ---------------------------------------------------------------------------
// The five sets.
// ---------------------------------------------------------------------------

test("the scan reads something", () => {
  assert.ok(
    FILES.length > 200,
    `only ${FILES.length} files were scanned, so every assertion below is vacuous`,
  );
  const lexed = FILES.filter((file) => file.spans.length > 0).length;
  assert.ok(
    lexed > 200,
    `only ${lexed} files hold a string literal, so the lexer is not reading Rust`,
  );
});

test("every way this repository reaches std::env is enumerated", () => {
  const found = new Set();
  for (const file of FILES) {
    for (const match of file.code.matchAll(/(?<!\w)(?:std::)?env::([a-z_]+)/gu)) {
      if (!insideString(file, match.index)) {
        found.add(`env::${match[1]}`);
      }
    }
  }
  const unlisted = [...found].filter((item) => !ENV_VOCABULARY.has(item)).sort();
  assert.deepEqual(
    unlisted,
    [],
    `${unlisted.join(", ")} reaches std::env and ENV_VOCABULARY does not name it. ` +
      "Add a row saying what it reaches; an item that hands back a machine-owned " +
      "root also needs a ROOT_PRODUCERS row for the function that returns it.",
  );
  const stale = [...ENV_VOCABULARY.keys()].filter((item) => !found.has(item)).sort();
  assert.deepEqual(stale, [], `${stale.join(", ")} is listed and no longer spelled anywhere.`);
});

test("every environment variable read is resolved, and every ambient one is enumerated", () => {
  const unresolved = [];
  const indirect = new Set();
  const read = new Map();
  const written = new Set();
  for (const file of FILES) {
    const inside = enclosingFunctions(file.code);
    const constants = constantsFor(file);
    const arrays = loopLiteralArrays(file.code);
    for (const match of file.code.matchAll(/\.env\s*\(\s*([^,\n]+)\s*,/gu)) {
      for (const name of resolveArgument(match[1], constants, arrays) ?? []) {
        written.add(name);
      }
    }
    for (const match of file.code.matchAll(ENV_READ)) {
      if (insideString(file, match.index)) {
        continue;
      }
      const site = `${file.path}::${inside(match.index) ?? "<file scope>"}`;
      const names = resolveArgument(match[1], constants, arrays);
      if (names === null) {
        if (INDIRECT_READS.has(site)) {
          indirect.add(site);
        } else {
          unresolved.push(`${site}: env::var(${match[1].trim()})`);
        }
        continue;
      }
      for (const name of names) {
        if (!read.has(name)) {
          read.set(name, file.path);
        }
      }
    }
  }
  assert.deepEqual(
    unresolved.sort(),
    [],
    `${unresolved.join("; ")} reads a variable whose name this scan cannot resolve. ` +
      "Spell it as a literal, a `const NAME: &str`, or a literal array in a `for`; " +
      "or, when the name is the caller's to choose, give the site an INDIRECT_READS " +
      "row naming who supplies it.",
  );
  const staleIndirect = [...INDIRECT_READS.keys()].filter((site) => !indirect.has(site)).sort();
  assert.deepEqual(
    staleIndirect,
    [],
    `${staleIndirect.join(", ")} is listed as an indirect read and no longer is one.`,
  );

  const ambient = [...read.keys()].filter((name) => !written.has(name)).sort();
  const unlisted = ambient.filter((name) => !ENVIRONMENT_NAMES.has(name));
  assert.deepEqual(
    unlisted,
    [],
    `${unlisted.map((name) => `${name} (${read.get(name)})`).join(", ")} is read from the ` +
      "ambient environment and ENVIRONMENT_NAMES does not name it. A variable this " +
      "repository sets on a child needs no row; one the machine set does.",
  );
  const stale = [...ENVIRONMENT_NAMES.keys()].filter((name) => !read.has(name)).sort();
  assert.deepEqual(stale, [], `${stale.join(", ")} is listed and no longer read anywhere.`);
});

test("every function that hands out an ambient root is enumerated", () => {
  const sites = [];
  for (const file of FILES) {
    const inside = enclosingFunctions(file.code);
    const constants = constantsFor(file);
    const arrays = loopLiteralArrays(file.code);
    const reaches = [];
    for (const match of file.code.matchAll(/(?<!\w)(?:std::)?env::temp_dir\s*\(/gu)) {
      if (!insideString(file, match.index)) {
        reaches.push(match.index);
      }
    }
    for (const match of file.code.matchAll(ENV_READ)) {
      if (insideString(file, match.index)) {
        continue;
      }
      const names = resolveArgument(match[1], constants, arrays) ?? [];
      if (names.some((name) => ROOT_VARIABLES.has(name))) {
        reaches.push(match.index);
      }
    }
    for (const at of reaches) {
      const name = inside(at);
      if (name === null) {
        sites.push(`${file.path}::<file scope>`);
        continue;
      }
      // A function that reads a root and returns something else keeps it to
      // itself; only one that returns a path hands it to a caller. `fn new(..) ->
      // Result<Self>` builds its own name here, and that name is what the next
      // test checks instead.
      const signature = new RegExp(`(?<!\\w)fn\\s+${name}\\s*\\([^{]*?\\)\\s*(->[^{]*)?\\{`, "u");
      if (/Path/u.test(signature.exec(file.code)?.[1] ?? "")) {
        sites.push(`${file.path}::${name}`);
      }
    }
  }
  // A wrapper that calls a producer hands the same root on. `runtime_base` is
  // `fs::canonicalize("/tmp").or_else(|_| temporary_base())`: it reads no
  // variable of its own, and a set that stopped at the direct reads would miss
  // the root that `ax1-{stamp}` is built on. Iterating to a fixed point is what
  // makes the set closed under wrapping, which is the property Set 4 needs.
  const declared = new Set(
    [...ROOT_PRODUCERS.keys()].map((site) => site.slice(site.lastIndexOf("::") + 2)),
  );
  for (const file of FILES) {
    const inside = enclosingFunctions(file.code);
    for (const name of declared) {
      const call = new RegExp(`(?<!\\w)${name}\\s*\\(\\s*\\)`, "gu");
      for (const match of file.code.matchAll(call)) {
        if (insideString(file, match.index)) {
          continue;
        }
        const holder = inside(match.index);
        if (holder === null || holder === name) {
          continue;
        }
        const signature = new RegExp(
          `(?<!\\w)fn\\s+${holder}\\s*\\([^{]*?\\)\\s*(->[^{]*)?\\{`,
          "u",
        );
        if (/Path/u.test(signature.exec(file.code)?.[1] ?? "")) {
          sites.push(`${file.path}::${holder}`);
        }
      }
    }
  }

  const found = [...new Set(sites)].sort();
  const unlisted = found.filter((site) => !ROOT_PRODUCERS.has(site));
  assert.deepEqual(
    unlisted,
    [],
    `${unlisted.join(", ")} hands a directory the machine owns to its callers and ` +
      "ROOT_PRODUCERS does not name it. Every name built on that root has to be " +
      "reviewable, which means the function handing the root out is named here first.",
  );
  const stale = [...ROOT_PRODUCERS.keys()].filter((site) => !found.includes(site)).sort();
  assert.deepEqual(stale, [], `${stale.join(", ")} is listed and hands out no ambient root.`);
});

test("every name built on an ambient root is enumerated and separates two processes", () => {
  const producerNames = new Set(
    [...ROOT_PRODUCERS.keys()].map((site) => site.slice(site.lastIndexOf("::") + 2)),
  );
  const found = new Map();
  for (const file of FILES) {
    const bindings = new Map();
    for (const match of file.code.matchAll(/let\s+(?:mut\s+)?([a-z_][a-z0-9_]*)\s*=\s*([^;]+);/gu)) {
      if (!bindings.has(match[1])) {
        bindings.set(match[1], match[2].replace(/\s+/gu, ""));
      }
    }
    for (const site of sharedNameSites(file, producerNames)) {
      // One level of substitution, so a name assembled in two steps -- `let stamp
      // = format!("{}-{sequence}", process::id())` and then `format!("...{stamp}")`
      // -- is read with what it interpolates rather than without it.
      let expanded = site.argument;
      for (const [name, value] of bindings) {
        if (expanded.includes(`{${name}}`)) {
          expanded += value;
        }
      }
      found.set(`${file.path} :: ${site.receiver}.join(${site.argument})`, expanded);
    }
  }

  const unlisted = [...found.keys()].filter((key) => !SHARED_NAME_SITES.has(key)).sort();
  assert.deepEqual(
    unlisted,
    [],
    `${unlisted.join("\n  ")}\nbuilds a name on a directory the machine owns and ` +
      "SHARED_NAME_SITES does not name it. Give it a row: UNIQUE with what separates " +
      "it from another process's name, or SHARED with why that is acceptable.",
  );
  const stale = [...SHARED_NAME_SITES.keys()].filter((key) => !found.has(key)).sort();
  assert.deepEqual(stale, [], `${stale.join("\n  ")}\nis listed and no longer built anywhere.`);

  const undiscriminated = [];
  for (const [key, expanded] of found) {
    if ((SHARED_NAME_SITES.get(key) ?? "").startsWith("SHARED")) {
      continue;
    }
    if (![...DISCRIMINATORS].some((token) => expanded.includes(token))) {
      undiscriminated.push(key);
    }
  }
  assert.deepEqual(
    undiscriminated,
    [],
    `${undiscriminated.join("\n  ")}\nis recorded UNIQUE and spells none of ` +
      `${[...DISCRIMINATORS].join(", ")}, so two processes build the same name.`,
  );
});

test("AppContainer profile creation is serialised at every call site", () => {
  const found = new Map();
  for (const file of FILES) {
    const inside = enclosingFunctions(file.code);
    for (const match of file.code.matchAll(/(?<!\w)CreateAppContainerProfile\s*\(/gu)) {
      if (insideString(file, match.index)) {
        continue;
      }
      const name = inside(match.index) ?? "<file scope>";
      found.set(`${file.path}::${name}`, { code: file.code, fn: name });
    }
  }
  const unlisted = [...found.keys()].filter((site) => !PROFILE_GATE_SITES.has(site)).sort();
  assert.deepEqual(
    unlisted,
    [],
    `${unlisted.join(", ")} creates an AppContainer profile and PROFILE_GATE_SITES does ` +
      "not name it. Two creators of one absent profile name tear its directory down, and " +
      "every CreateProcessW issued into the container meanwhile fails with " +
      "ERROR_FILE_NOT_FOUND.",
  );
  const stale = [...PROFILE_GATE_SITES.keys()].filter((site) => !found.has(site)).sort();
  assert.deepEqual(stale, [], `${stale.join(", ")} is listed and creates no profile.`);

  const ungated = [];
  for (const [site, entry] of found) {
    const opening = new RegExp(
      `(?<!\\w)fn\\s+${entry.fn}\\s*\\([^{]*?\\)\\s*(?:->[^{]*)?\\{`,
      "u",
    ).exec(entry.code);
    if (opening === null) {
      ungated.push(`${site} (its signature could not be read)`);
      continue;
    }
    const body = entry.code.slice(opening.index + opening[0].length).replace(/\s+/gu, "");
    if (!body.startsWith(PROFILE_GATE)) {
      ungated.push(site);
    }
  }
  assert.deepEqual(
    ungated,
    [],
    `${ungated.join(", ")} does not open with the two guards that keep two creators of the ` +
      `profile apart. Its first two statements have to be exactly:\n  ${PROFILE_GATE}`,
  );
});
