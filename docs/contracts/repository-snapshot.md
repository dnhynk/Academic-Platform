# Repository snapshot contract

`academic-repository` is the `P2-R1` boundary. It turns one of section 17.1's
eight inputs into section 17.2's `RepositorySnapshot`, with section 17.3's
permission and secret gate running before anything is inventoried or indexed.

It reads no `.git` directory, runs no version-control command, and opens no
socket. What it consumes is a `WorkingTreeFacts` its caller supplies and the
bytes under a root; what it produces is a frozen value.

## The eight inputs and the four snapshot types are different vocabularies

Section 17.1 names eight things a user can point this system at. Section 17.2's
`sourceType` has four values. They answer different questions — *how the
repository was named* against *what the frozen snapshot is of* — and
`RepositorySource` and `SnapshotType` keep them apart.

`resolve_snapshot_type` is the one derivation between them, and it has one call
site. Two inputs fix the answer because they name something that is not a commit
tree at all:

| Input | Snapshot type |
|---|---|
| `ARCHIVE` | `ARCHIVE` |
| `SPEC_ONLY` | `SPEC_ONLY` |
| the other six | `DIRTY_WORKTREE` if the tree differs from HEAD, else `GIT_COMMIT` |

The last row is the contract. **A dirty working tree resolves to
`DIRTY_WORKTREE` however it was named**, so a request that says `COMMIT` over a
tree with a modified or untracked file produces a dirty snapshot, not a commit
one. `dirty_worktree_is_not_head` walks all eight inputs over dirty facts and
requires none of them to produce `GIT_COMMIT`; migration `0012`'s
`guard_repository_snapshot_dirty_shape` refuses the same shape in a row that
arrived from somewhere else.

## The gate runs first, and that is three separate things

Section 17.3 draws `permission + secret gate` above `inventory and immutable
snapshot` above `syntax/semantic indexing`. Three independent mechanisms hold
that order, because each is blind to a different way of losing it.

**By type.** `AdmittedPaths` and `Inventory` have crate-private constructors.
`SnapshotStages` is public, so a caller may substitute or wrap a stage, but an
implementation written in another crate cannot *return* either value without
calling this crate's stage that produces it — `LocalStages::inventory`, which
takes an `&AdmittedPaths`, which only the gate produces. An admission also
carries the digest of the request it was decided for, and the inventory refuses
one that names a different request, so an admission cannot be carried from one
capture into another.

**By count.** `crates/repository/tests/repository_scans.rs` pins `capture` as
whole text and counts the call sites of every stage, of `scan_secrets`, and of
`AdmittedPaths::admit`, over every file of the package, with the one file each
may be called from. A count of one is what says there is no second path; the
caller column is what says the one path did not move into a new module. That is
the `P2-RF10`/`P2-RF11` lesson applied: a pin on a body says nothing about
whether the body runs or about whether a second body exists beside it.

**By observation.** `secret_gate_precedes_indexer` is the execution plan's
call-count spy. It wraps the real `LocalStages` and records each stage as it is
entered, for every one of the eight inputs. Two claims come out of it:

- on a clean source the sequence is exactly gate, inventory, freeze, index, each
  once;
- **on a source the gate blocks, the indexer's count is zero.**

The second is the one that fails if the scan is moved behind the indexer. That
variant compiles, spells nothing forbidden, and leaves the stage sequence
looking identical — and the count of what ran on a blocked repository is what
separates it from the correct order.

## What the gate applies

Section 29.6 names four things a local analyzer applies before an analyzer sees
a path: file allow/deny rules, `.gitignore`, user exclusions, and a secret scan.
Section 32.4 splits the last into a file-level policy and a content scan.

`PathPolicy::classify` is the first three plus 32.4's point-1 file defaults, in
that order, and it is pinned whole. An excluded path is never opened:
`analyzer_never_sees_an_excluded_path` compares the set of paths the inventory
actually read against the set the policy admits, computed independently of the
walk, and exercises all four `ExclusionReason` arms as a whole set.

The `.gitignore` parser models blank lines, comments, anchors, directory
prefixes and leading-`*` suffixes. **It refuses a negation (`!`) rather than
ignoring it**, because a rule mis-read as an exclusion hides a file from the
analyzer and one mis-read the other way shows it a file the user meant to hide.

`scan_secrets` is the content half: five detectors for the five things section
32.4 point 2 names — known key format, cloud credential, connection string,
token pattern, entropy — each mapping to one of section 3.5's closed
`ReasonCode`s rather than to a vocabulary of its own.

**The entropy detector counts distinct characters rather than computing Shannon
entropy.** The textbook formula needs `log2`, whose last bits are not guaranteed
identical between two targets, and a snapshot whose `secretScanResult` differed
between Windows and Linux for a file near the threshold would be a worse defect
than the sharper detector is worth. The function is named for what it counts.

A file the scanner cannot read as bounded text is `ContentVerdict::Opaque`: it
is manifested by digest and **not ingested**. Section 32.4's point 5 is
fail-closed about what such a file may be used *for* — no external transmission
— and not a reason to refuse a snapshot; since its bytes reach no value this
crate hands on, there is nothing for a later stage to transmit.

## A secret file's digest requires a recorded decision

Section 17.2 says a secret file's *hash* has its disclosure scope reviewed. The
default here is that there is no review and therefore no digest.

`SecretFinding` has one constructor, it takes no bytes, and it writes
`blob_digest: None`. The only other writer of that field is
`SecretFinding::disclose`, which takes a `DisclosureDecision` by value. The
whole `impl SecretFinding` block is pinned, both assignments are counted across
the package, and the count of functions taking a `DisclosureDecision` by value
is one — so a second door into the same field fails rather than passing.

`DisclosureDecision::record` refuses an empty decision identifier, actor or
reason. Migration `0012` is the second layer: `repository_secret_finding`'s
`blob_digest` and `disclosure_decision_id` are present together or absent
together, enforced by `guard_repository_secret_finding_disclosure`, and the
decision identifier is a foreign key into `repository_hash_disclosure_decision`
— so a digest cannot name a decision nobody recorded.

A blocked capture produces no snapshot, so a finding is keyed on the request
digest rather than on a `snapshot_id`. The rows that matter most come from
captures that never produced a snapshot at all.

## The snapshot does not change when the source does

Every field of `RepositorySnapshot` is owned and filled at freeze time. The type
holds no path, no handle, no closure and no borrow of the tree: rereading a
field cannot consult the filesystem, because the field is the answer rather than
a way to get one.

`the_snapshot_hands_back_owned_data_and_nothing_else` compares the whole set of
the type's method signatures against a pinned list, in both directions, and
requires none to take `&mut self` and no field to be public.
`snapshot_is_immutable_after_source_change` rewrites a file, adds one and
removes one, then compares the frozen value field by field — and captures the
same directory again to show the tree really did change, so the first comparison
is not vacuous.

**This is a claim about the value, not about the bytes on disk.** The operating
system is what decides whether a file can be written; `worker-sandbox.md` is
where a measured claim about the operating system lives.

## What "the analyzer cannot mutate the source or open a socket" claims

Three levels, and they are not the same claim.

**This crate.** `analyze` takes a frozen `RepositorySnapshot` by reference and
returns a value; there is no path in its argument to write through.
`the_crate_touches_the_filesystem_only_to_read_it` compares the whole set of
`fs::` names the product code spells — `read`, `read_dir`, `symlink_metadata` —
and the whole set of its `use` items against pinned inventories, in both
directions. A mutation reached without spelling `fs::` needs an import, and an
import appears in the second set, so between them there is no route to the
filesystem that spells no listed name and adds no listed import.

**The process class.** `ProcessClass::RepositoryAnalyzer` holds
`ReadArtifactRange`, `AnalyzeRepository` and `CreateClaim` and nothing else.
`analyzer_cannot_mutate_source_or_open_a_socket` mints all three through
`P2-G1`'s broker and observes `OpenOutboundSocket`, `WriteStagedArtifact` and
`ReadKeyMaterial` each refused with exactly one `DENY` audit row carrying
`NO_GRANT`. Asserting both halves is what keeps a matrix that refused everything
from passing.

**The operating system.** Not this crate's claim. `P2-G4` measured what a kernel
refuses a sandboxed process, on which platform, with which error number, and
[the worker sandbox contract](worker-sandbox.md) is where that lives. This
contract cites it rather than repeating it at a strength nothing here executes.

The crate spells no socket construct, and `only_egress_crate_has_a_socket` in
`tools/phase1-scaffold-policy.test.mjs` reads that as the absence of a
`SOCKET_ALLOWANCE` entry and as a link closure holding nothing that can open one.

## Everything a repository holds is untrusted

What survives a read is an `Untrusted<IngestedDocument>` from `P2-G5`, held in
that crate's `SourceIndex`. This crate keeps no other copy of a file's text: the
manifest holds a digest, a length and a language, and the bytes exist only
inside the closure that read them and inside the sealed wrapper.

Three of `P2-G5`'s six `SourceKind` arms apply to a repository. **No seventh arm
is added** — that would be a change to that crate's closed enum and to every
`match` over it — so a source file is tagged `CODE_COMMENT`, which is the arm
`P2-G5` fixed for bytes that came out of source code, and prose is `README`.

`SourceId` accepts `[A-Za-z0-9._-]` up to 64 bytes and a path holds neither, so
the identifier is derived from the path's digest rather than taken from it. It
is a function of the path alone, so two captures of one tree produce the same
identifiers.

## GitHub access

Sections 29.6 and 32.4 fix three properties, and each is a separate thing to
test.

**Repo-scoped.** `TokenScope` names exactly one `GitHubRepository` and
`TokenScope::covers` is exact equality. There is no wildcard, no owner scope,
and no "all repositories" arm.

**Read-only.** `TokenPermission` has three variants and no write variant.
`TokenPermission::access` is a total function over the enum returning
`Access::Read`, so a write permission added later has no arm to return and the
crate stops compiling. The whole `impl TokenPermission` block is pinned.

**Expiring.** `TokenLifetime::new` refuses an interval that is empty, backwards,
or longer than `MAX_TOKEN_LIFETIME_MILLIS` (one hour). Validity is half-open, so
the expiry instant is already outside.

`FineGrainedToken::authorize` checks the three in a fixed order with a distinct
error for each, so a refusal says which property failed rather than reporting a
denial a test could satisfy for the wrong reason.

The material rests in the operating-system credential store.
`CredentialStore` is generic over `P2-K1`'s `DeviceKeystore`, so the reviewed
native broker and the in-memory test double are the same two things every other
crate in this repository uses. `CredentialStore::borrow` **checks the expiry
before it asks the broker**, so an unusable token's material never leaves it.
The token's own `Debug` is hand-written and prints a length; the field is named
`secret`, which is a name `tools/secret-debug-policy.test.mjs` already holds, so
a derived `Debug` is refused by the existing net rather than by a rule this task
invented.

**No implementation of `GitHubRepositoryReader` ships**, the way
`academic-egress-boundary` ships no transport. Every test supplies its own
in-memory reader over synthetic bytes. What a real implementation would need is
an outbound socket, and that belongs to `ProcessClass::EgressProxy`.

## Migration `0012`

`P2-R1` owns the `SNAPSHOT_REGISTERED` aggregate, and migration `0004`'s rule is
that each aggregate owner adds its own typed columns later. `0012` is that
migration: six tables, five guards, and an append-only trigger pair on each.

It is `0012` rather than `0010` or `0011` because those two numbers were
reserved for `P2-L2` and `P2-U6`, which were in flight on the same `main`.
`0008` stays unclaimed for the reason migration `0009` gives. A migration number
decides the order and nothing else rests on it: what the admission fingerprint
fixes is the resulting object set.

The six tables are in `CANONICAL_TABLES`, so the trigger pair in the migration is
the first enforcement layer and the SQLite authorizer's blanket denial of `DROP`
and `ALTER` is the second — the condition
`authorizer_covers_every_canonical_table` enforces.

## What this contract does not claim

- **Nothing here is ADR-002 acceptance.** The default lane remains
  `storage_encryption=NONE`, `production_data_allowed=false`,
  `adr_002_accepted=false`, the acceptance public key is unprovisioned, and the
  committed candidate receipt carries two of five platform rows.
- **No real repository, token or credential store was used.** Every fixture is
  synthetic and built in-process; the keystore and the GitHub reader are
  in-memory doubles. No network call is made and none can be: no implementation
  of the reader trait ships.
- **The `.gitignore` parser is not git's.** It models the shapes this
  repository's own ignore files use and refuses a negation rather than guessing
  at it. A richer matcher is a change to `PathRule` and to nothing else.
- **This crate reads no object database.** Tracked, untracked and dirty state
  come from the `WorkingTreeFacts` its caller supplies. What it concludes from
  them is checked; what it cannot check is whether the caller reported the tree
  correctly.
- **`analyze` is a seam, not static analysis.** `P2-R2` owns AST, symbol,
  call-flow, schema, config and IaC indexing and the five-tier evidence ladder.
  What this crate fixes is the read-only argument type that analysis receives.
- **`§38` is neither opened nor closed by this task.**
