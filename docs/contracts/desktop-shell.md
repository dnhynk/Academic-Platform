# Desktop shell contract

`P2-X1` fixes four things: the route manifest, the typed local-core command
allowlist, the boundary that keeps this surface away from the database and the
keys, and the rule that an optimistic update is not canonical until the core
returns a receipt. It also commits the Tauri capability and CSP snapshot that
`P2-A2` and its re-audit could not diff, because there was nothing to diff.

## What this is not evidence for

**No Tauri runtime is linked and no window opens.** `crates/desktop` depends on
`academic-rpc` and `thiserror` and on nothing else. The snapshot under
`crates/desktop/` is committed configuration, machine-checked against the
formats Tauri itself reads; it is not a running application, and no test here
claims otherwise.

The measurement that decided it is in
`docs/security/dependency-admission-phase2-x1.json`. `cargo metadata` on
`tauri 2.11.5` resolves **388 new packages into the default product closure**,
344 with `default-features = false`, and 160 for `tauri-utils` alone. All three
closures contain `http`; the first two also contain `http-body`, `hyper`,
`hyper-util`, `reqwest` and `tower-http`. Those six are exactly what
`phase1_default_features_have_no_product_network` forbids in the workspace
default product graph, at every feature setting. Linking the runtime is
therefore a separate decision with its own dependency admission, and this task
does not make it.

What the snapshot *is* evidence for is its own content, and that is what a later
audit can diff.

## Route manifest

`packages/ui/src/routes.ts` is the manifest and `P2-X1` owns it. Every entry
carries the section 25.1 label it answers for.

`route_manifest_matches_ia_exactly` parses section 25.1's drawn tree out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares the two as
sets in both directions, then compares the parent of each label and the reading
order. No count appears anywhere in the comparison: both sides are enumerated.

**One tree line is one route.** Two of the specification's lines name a pair —
`Course Catalog & Course Detail` and `Concepts / CS Map` — and neither is split,
because splitting on punctuation would make the comparison depend on how a label
is spelled. A route that also addresses one entity carries a detail parameter
instead, and contributes two destinations: its index form and its detail form.
`every_destination_opens` opens both.

The view registry in `packages/ui/src/views.ts` is written out route by route
rather than derived from the manifest. A derived registry would make
`every_destination_opens` vacuous — every route would have a view because every
route was a route. Written out, the two enumerations are independent and are
compared in both directions.

A view is a structure, not pixels: a title, a breadcrumb reaching the root, at
least one section, the right-hand evidence drawer, and the backlinks of any
bound entity. Each section names the task that fills it with product content, so
a reader of a rendered view cannot mistake an empty frame for a finished
surface. `P2-X2` through `P2-X7` own that content.

## Command palette, backlinks, evidence drawer

Section 25.1 requires Course, Concept, Project and Question to be reachable from
any screen by command palette and by backlink, and requires the evidence drawer
for the selected entity to persist rather than costing a tab.

- `palette_reaches_four_entity_types_from_every_route` enumerates the whole
  `destination × entity kind` product and requires each cell to yield at least
  one command whose target is the route the manifest says opens that kind, whose
  entity the corpus holds, and which actually opens.
- `backlinks_resolve_for_four_entity_types` walks every entity in the corpus,
  compares the backlink set against one derived in the test from the relation
  table, and requires each backlink to open the referring entity's own detail
  form and to traverse back.
- `evidence_drawer_persists_across_views` enumerates every ordered pair of
  destinations and requires the selection to survive, and separately requires
  that navigating with nothing pinned invents no selection.

The drawer is shell state carried by `navigate`, not view state a view may keep.

The corpus in `packages/ui/src/entities.ts` is synthetic and built in process,
as `CONTRIBUTING.md` requires. Nothing here reads a profile, a database or a
network, and this surface has no way to reach any of them.

## The typed local-core command allowlist

`academic_desktop::DesktopCommand` is a closed enum with no constructor from a
string: no `TryFrom<&str>`, no `FromStr`, no variant carrying a free-form
capability identifier. `tests/compile_fail/desktop_command_is_not_built_from_a_string.rs`
is what says so, and it fails with a committed diagnostic rather than merely
failing.

The allowlist is compared against `academic_rpc`'s own tables rather than
restating them:

- the capability set the enum yields equals `PHASE1_CAPABILITY_IDS`;
- each write variant's capability equals `expected_capability_for_command` of the
  wire command it builds;
- the read and write halves partition into `READ_ONLY_CAPABILITY_IDS` and
  `WRITE_CAPABILITY_IDS` exactly.

A capability list restated here would drift from the daemon's silently. One
compared against the daemon's cannot.

`SyntheticFixtureId` is closed for the same reason: the surface cannot ask the
core to ingest a path, a URL, or anything a user typed. Its one identifier is
compared against `academic-core`'s `PHASE1_SYNTHETIC_FIXTURE_ID` as *text*, by
`desktop_names_only_the_core_fixture_allowlist`, because the desktop must have
no dependency edge to `academic-core`.

## The desktop opens no database and holds no key

ADR-001's surface table forbids this surface the database, provider and root
keys, and unrestricted filesystem or network authority.
`desktop_cannot_open_the_database_or_read_keys` in
`tools/phase1-scaffold-policy.test.mjs` judges it three ways, following
`only_egress_crate_has_a_socket`, because each is blind to a different bypass.

**Graph.** The declared workspace closure of every edge kind — normal, build and
dev — is exactly `academic-admission`, `academic-contracts`, `academic-domain`
and `academic-rpc`, compared whole. The resolved closure is checked against ten
workspace crates that own the database or a key.

**Link.** The resolved shipping closure is pinned entire, so a dependency added
anywhere below the surface is a review of the whole new closure. On top of that
it is intersected with thirteen database-capable crates and seventeen
key-custody crates, and holds none of either. Notably it holds **no SQLite
driver of any kind**.

`ed25519-dalek`, `sha2`, `hmac` and `zeroize` *are* in the closure, through
`academic-admission`'s receipt signature verification and `academic-domain`'s
digests. Verifying a signature over public evidence is not key custody, and the
guard says so by naming those four as deliberately absent from the custody list
rather than by excusing them.

**Source.** A closed world over path roots: every identifier the crate writes a
`::` after must be one of twenty-five reviewed roots, read on paths rather than
on imports, so a fully qualified `rusqlite::Connection::open` is refused even
though it spells no `use`. The allowlist is compared in both directions, so a
dead entry fails. The walk reads the whole package rather than `src`, has a
floor under it, and requires every `mod` and `#[path]` target to be a file it
read.

`only_egress_crate_has_a_socket` records this crate's socket-capable link
closure — `libc`, `mio`, `rustix`, `socket2`, `tokio`, `windows-sys`, all through
`academic-rpc`, which needs them for the named pipe and Unix-domain socket the
daemon listens on. The crate spells no socket construct, which is why its
`SOCKET_ALLOWANCE` entry is absent rather than empty.

## An optimistic update is not canonical before a receipt

ADR-001: "A UI optimistic update is not canonical until the core returns an
immutable object/event ID and local acceptance receipt."

`academic_desktop::Optimistic<T>` enforces it by having no exit. There is no
`value`, `get` or `into_inner`; no `Deref`, `AsRef` or `Borrow`; no
`From<Optimistic<T>> for T`; no caller-closure `map`; no `Serialize`; and a
`Debug` that redacts. `Optimistic::confirm` consumes the wrapper, compares all
four fields the core binds a receipt to — request id, client instance id,
idempotency key, request digest — and returns `Canonical<T>` only when every one
matches. Taking `self` by value means a refused receipt leaves no wrapper behind
for a second attempt.

`packages/ui/src/optimistic.ts` is the same contract for shell state that never
crosses into Rust. There the seal is a module-scoped `WeakMap`: the wrapper
carries a tag and the submitted request and nothing else, so there is no
property to read, no spread that recovers the value, and no `JSON.stringify`
that emits it. A structurally identical forgery confirms to nothing.

**This is the same kind of seal as `academic_scenario::Proposed<T>` and is
deliberately not that type.** `Proposed<T>` has no promotion at all, because a
projection becomes canonical only through a user decision recorded as its own
event; adding one would weaken a contract `P2-X1` does not own. An optimistic
update has exactly one promotion, and it is a receipt. The overlap is real and
is recorded here rather than resolved by making one type serve both rules.

## Capability and CSP snapshot

`crates/desktop/tauri.conf.json` and `crates/desktop/capabilities/desktop.json`
are the snapshot. `capability_snapshot_has_no_wildcard` in
`packages/ui/src/capability-snapshot.test.ts` checks three things that fail for
different reasons on purpose:

1. **Whole-file pins.** All four files — the two snapshot documents and the two
   vendored Tauri schemas — are pinned by the SHA-256 of their whole bytes.
2. **The format Tauri reads.** Both documents validate against
   `schemas/tauri/config-2.11.5.schema.json`, which is Tauri's own published
   schema, and `schemas/tauri/capability-2.9.3.schema.json`, generated from
   `tauri_utils::acl::capability::Capability`. Negative controls show the schema
   is doing work — and one shows it is *not* the wildcard guard, because it
   accepts a `$HOME/**` asset-protocol scope quite happily.
3. **`scanSnapshot`.** A closed world over reviewed strings, keys and values in
   separate sets, plus exact comparisons on the fields that carry authority: the
   permission list, the asset-protocol scope, every CSP directive, the window
   labels, the absence of `remote`, and an empty `plugins`.

`WILDCARD_FORMS` enumerates ten shapes — glob stars, base-directory variables,
insecure schemes, scheme wildcards, scheme-less hosts, brace expansion, path
traversal. **It explains; it does not decide.** A deny list of shapes is broken
by the shape that is not on it. The closed world is what refuses a fullwidth
asterisk, a bare drive root, a protocol-relative source and a `data:` scheme,
none of which any enumerated form matches. The injection matrix for all of this
is in [policy source scans](policy-source-scans.md).

**Filesystem, HTTP and shell authority in Tauri v2 arrive through the
`tauri-plugin-fs`, `tauri-plugin-http` and `tauri-plugin-shell` crates.** The
snapshot declares `"plugins": {}` and `crates/desktop` declares no plugin
dependency of any kind, and the dependency guard above is what keeps that true.

The CSP is written as a directive map rather than one string, so each directive
is compared on its own. It grants `'self'` to the fetch directives, `ipc:` to
`connect-src`, and `'none'` to everything else. When the runtime is linked, the
Windows custom-protocol hosts may have to be added to `connect-src`; that is a
snapshot change, and the pin is what will force it through review.

## What stays open

- The Tauri runtime binding, with the 388-package admission it implies.
- Every §25.2–§25.13 surface's content. `P2-X1` fixes the frame, not the
  contents.
- `production_data_allowed` is still `false` and ADR-002 is still unaccepted.
  Nothing in this task changes either.
