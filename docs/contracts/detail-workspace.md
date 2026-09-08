# Durable detail workspace seam

The native `desktop_request_v1` command reads a host-selected synthetic profile
and appends signed relation dispositions through the daemon's single writer.
It does not select a filesystem path, accept a corpus, choose an actor, or accept
arbitrary events from the webview. An empty profile returns an empty corpus.
The normal service does not seed data or select a hard-coded product fixture.

## Source and authority boundary

`academic-core::details::CorpusRecord` is a versioned **imported snapshot** of the
closed detail DTO, entity aliases, canonical relation-claim IDs and optional
lecture media artifact IDs. It is stored as a v4 `ClaimAsserted` with predicate
`detail.workspace.corpus.v1`. Each displayed relation has a preceding accepted
`detail.workspace.relation.v1` claim in the same domain/scope and signed
owning-entity membership. Both claims require an Importer actor, `DIRECT_OBSERVATION` and
`CODE_OBSERVED`. Relation JSON must equal the signed snapshot; its displayed
source bytes must match the first evidence excerpt digest. Evidence and
artifact closure are verified by canonical replay and vault read-back.

Corpus and relation eligibility uses the canonical resolver at the requested
known/valid coordinates with the `ImplementationObservation` policy. Authorized
retractions and supersessions, expiry, disputed state and equal-rank conflicts
cannot be bypassed by a signed snapshot. A relation that is no longer eligible
is omitted from every occurrence in the selected corpus and from current
disposition overlays. Its original canonical ID and signed disposition remain
available to receipt lookup and historical reads.

Version 1 binds a relation claim's subject to an owning entity. Distinct values
in that owner/scope/predicate slot are canonical conflicts; the compatibility
reader cannot reinterpret their old subjects or grant current writes. Version 2
retains the same JSON fields and predicate but assigns one subject per relation.
It hashes the canonical JSON tuple `("academic.details.relation-subject.v2",
domain_id, scope_id, workspace_entity_id, relation_alias)` and derives its UUIDv7
entity with the existing `derived_id` function's `entity` label. The reader
recomputes that subject and validates all owners through signed corpus membership
and the complete entity map. Shared owners never choose the subject. The explicit
fixture importer now identifies itself as version 2; the resolver-aware projector
identifies itself as `academic.details.v2`. Existing signed v1 records and
disposition IDs are retained unchanged.

Titles, status/confidence labels, source locator labels and detail-specific
text are imported metadata. A displayed `CONFIRMED` relation label is not a
canonical `UserDecision` or proof of `USER_CONFIRMED` authority. Likewise this
slice does not recompute knowledge state, repository freshness, question
resolution, transcript alignment, or source locator semantics from domain
engines. It verifies that the selected signed import and its evidence agree.

**Residual:** no ordinary domain-to-detail producer currently translates the
existing lecture/question/knowledge/repository engine outputs into these
predicates. The only current producer is the explicit synthetic fixture tool.
Two different imported profiles prove generic profile selection and persistence,
not full real-domain integration or real-data admission.

## Read and disposition protocol

The Proto envelope adds request tag 16 and response tag 17, each containing
one `canonical_json` bytes field. Existing tags, protocol major/minor 1.0,
and historical signed v1-v4 bytes remain unchanged. Three independently
negotiated capabilities end in `details-read.v1`, `details-decide.v1` and
`details-audio.v1` under `learning-platform.local`.

The Rust DTO in `crates/rpc/src/details.rs` is authoritative. JSON is strict
UTF-8, closed, duplicate-free and canonically re-encoded; maximum size is
1 MiB, nesting depth 24, list length 4096, object fields 128, string bytes
65536, and numeric values nonnegative JavaScript-safe integers. Native commands
are flat; daemon request frames wrap the decision/audio fields in a typed
`decision`/`audio` object. The selector is
`{view:"detail_workspace",known_at_accept_seq:null,valid_at_ms:null}` by default.
Historical reads apply both coordinates; writes require both optional values
to be null. Source digest covers accepted signed batch hashes known at the
chosen watermark. A watermark inside a batch includes that original batch hash.
Replay currently refuses histories above 4096 batches or 32 MiB of original
signed envelopes; incremental projection beyond these bounds is future work.
The store and prospective disposition path use one fixed count/byte budget.
A new disposition envelope that would exceed either ceiling returns
`HISTORY_BUDGET_EXCEEDED` before persistence; revision, history and receipts do
not change. Matching prior requests are looked up before this new-write check.

Decisions carry `relation_id`, `action` (`reject` or `undo`), `expected_revision`,
`expected_profile_id`, selector, 16-byte request/client IDs and a 32-byte
idempotency key. The request digest binds every field and a versioned domain
separator. The host resolves aliases and supplies the signer, actor, scope,
entity and original evidence. `detail.workspace.disposition.v1` records a
`USER_EXPLICIT`/`USER_CONFIRMED` user claim containing the complete request,
canonical relation claim ID and the exact rejection sequence undone. Undo
removes that detail overlay; it never confirms or replaces an underlying claim.

The S2 acceptance service atomically persists the signed batch, revision,
request identity and receipt. Matching retries reuse the original envelope and
receipt even after restart or later writes; altered payload reuse is rejected.
New stale writes, absent relations, duplicate rejects, empty undo and historical
writes are refused. A dry canonical replay checks the bounded resulting response
before a new write. An accepted reply includes the original receipt ID,
`decision_sequence`, a `receipt_decision` derived from the original verified
disposition envelope, echoed identity/digest and a current verified snapshot.
The independent receipt contains sequence, relation alias, canonical relation
claim ID, action, undo target and actor. Native confirmation checks this original
decision even if its relation has since expired, disappeared or been replaced.
Current visible decisions remain scoped to the current canonical relation;
the receipt never reapplies an old overlay to a replacement with the same alias.
Transport loss yields unavailable, never invented acceptance.

The profile ID incorporates the validated host location, schema metadata and a
synced 32-byte `detail-incarnation.v1` marker. The marker is nonsecret local
correlation metadata, created without overwrite and opened without following
links. Unix opens the retained descriptor with `NONBLOCK` before regular-file
and 32-byte validation, then reads at most 33 bytes. Invalid existing markers
fail closed. Reopen is stable; canonical
backup/restore and fresh same-path profiles get a new marker. Relocation changes
the profile ID. A filesystem clone containing local metadata at the identical
path is not an authenticated recovery procedure.

Doctor, deterministic export, backup and empty-target restore derive synthetic
domain closure from signed history verified by the build's independent trust
anchor. Keys and predicate policies come from the host's closed synthetic
configuration. Backup-supplied signing keys never authorize a restore. Restored
projections rebuild for the verified domains; an imported source without a
materialized sidecar still reports its actual projection lag in deep doctor.
The round trip preserves original signed envelopes, source artifacts, media
bytes and dispositions, while issuing a new local incarnation.
Restore derives its material from the staged database after integrity, metadata
and signed-replay checks against caller-owned authorizations. The compatible
`restore_profile` entry point and the new material-factory path share the same
object closure, rebuild and atomic publication. Neither opens the published
backup database; a material-factory refusal leaves it unchanged and publishes
no destination.

## Bounded media and explicit fixture tool

`details_audio` takes lecture ID, expected profile/revision, selector, offset
and length (1..4096). It resolves only media registered before the signed corpus
and returns exact verified WAV bytes with SHA-256, total length and offset.
It exposes no paths, URL, arbitrary vault object read, HTTP service or blob
capability. Source files are opened through retained vault evidence and verified
before and after each bounded read. The current ceiling is **4 MiB per WAV**.
Full-lecture codec/streaming support remains a concrete X4 follow-up. Policy-
staged egress preview is also absent; displayed source bytes must not be called
the minimized/substituted egress artifact.

The non-default `synthetic-detail-fixtures` feature builds
`academic-detail-fixture <new-profile> <synthetic-corpus.json> [<lecture-id> <synthetic.wav>]`.
It validates a supplied corpus, seals exact source bytes, signs import claims,
and uses ordinary durable acceptance in a new disposable profile. It has no
compiled product corpus, fixed relation allowlist or ordinary daemon seed path.
The running daemon still uses independently configured synthetic trust and
locator keys; this work does not admit production keys or real data.

The native launcher permits `--session <session.meta> --smoke` together. When
paired with the integrated X4 UI smoke, that smoke appends a real rejection and
undo through the supplied native client. Run it only against a disposable seeded
synthetic profile. The UI integration owns the corresponding update to the
desktop-shell smoke documentation and actual DOM smoke implementation.

## Version 3 normal-domain read scaffold

The independent `learning-platform.local.details-domain-read.v3` capability
adds the closed `details_domain_read_v3` operation to the existing bounded JSON
frames and `desktop_request_v1` invoke. Its context requires both domain and
scope; its selector binds known and valid coordinates; its query selects one
complete surface index or one typed subject. No producer payload, actor, source
path or canonical write operation crosses this command. Imported v1 source
history, the corrected `academic.details.v2` projection and original decision
receipt retry semantics keep their existing wire shapes. V3 replies have no
imported receipt fields. A UI can distinguish the imported source namespace from
`domain_projection` without equating their identities.

The store-owned `domain_history_snapshot` reads original envelopes, replica
revision, selected coordinates, source-outbox binding and aggregate timeline in
one deferred transaction. Core authenticates each envelope with the
host-selected independent authorization, replays complete-batch closure and
checks original local acceptance ranges and the outbox source digest. Aggregate
frame rows are compared with the authenticated event registrations. Resolution
uses the canonical scoped resolver at those coordinates. The emitted authority
is `CANONICAL_SNAPSHOT`; no materialized-graph generation or configuration
preimage is invented. The returned profile identity includes the existing
incarnation check, while actor, origin order/time, acceptance sequence and valid
time stay separate.

V3 accepts a known watermark of zero or an exact completed batch end. An
interior-batch selector returns `SELECTOR_UNAVAILABLE`; it never rounds to a
batch boundary or falls back to the current view. The lower store accessor
continues to return full original envelopes at exact interior coordinates,
while this v3 restriction ensures every emitted provenance event can be cited
at or before the selected watermark.

The default schema-one profile cannot admit aggregate registrations. This
scaffold therefore returns `PROJECTION_UNAVAILABLE` for a normal-domain surface
on that profile, and unknown contexts return `CONTEXT_UNAVAILABLE`. Positive
registration-command acceptance depends on the separately reviewed encrypted
synthetic service and registration policy. An in-memory projection or a DTO
round trip does not close that dependency. This change grants no admission,
migration, fingerprint, producer or ordinary-domain write capability.

For a supported authenticated aggregate snapshot, the implemented projection
availability is deliberately narrow:

| Surface or field | Read implementation | Remaining source dependency |
| --- | --- | --- |
| Lecture index/detail identity | Actual `LectureSessionId`, `OfferingId` parent, event/acceptance/validity provenance | Accepted registration service; no `EntityId` cast |
| Lecture transcript/document identity | Actual matching registered version/document IDs | Body schema and producer; registration-only content stays unavailable |
| Lecture document/coverage/captures/review/audio | Explicit typed unavailability | Validated bodies, alignment, coverage witnesses and original-media reader |
| Concept identity/title | Actual identity-change anchor and scoped resolved kind/label claims; exact label text | Accepted identity registrations; ambiguous kind refuses the complete index, ambiguous label is `AMBIGUOUS_IN_SCOPE` |
| Concept `USED_IN` | Concept-to-concept canonical claim, verified endpoint kinds, registry evidence rules and scoped resolution; complete query witness | Accepted endpoint registrations; unsupported endpoint mappings remain unavailable |
| Concept state/freshness/strong evidence and other groups | `PRODUCER_NOT_CONNECTED` | Accepted typed knowledge results and each group's supported relation/history adapter |
| Question and project surfaces | `PROJECTION_UNAVAILABLE` | Canonical subject mapping plus accepted typed body/association producer |
| Evidence locator and provenance | Exact accepted evidence/artifact IDs, representation index, role/strength and extractor metadata | No label is promoted into domain authority |
| Evidence excerpt | Exact whole-artifact UTF-8 reading aid after bounded retained-vault digest/read-back verification | Missing bytes: `SOURCE_BODY_UNAVAILABLE`; unsupported mapping: `UNSUPPORTED_FIELD`; wrong digest: read refusal |
| Ordinary relation action | `CANONICAL_RELATION_WRITE_ADAPTER_MISSING` | Separately reviewed canonical decision adapter |

The wire preserves all six question statuses, all document-node kinds and
preservation transforms, typed confidence/freshness, original ID newtypes and
separate snapshot/repository/optional branch/optional commit/dirty identities.
Declaring those fields does not assert the corresponding body producer exists.
Existing ordinary user decisions may be read with their original claim target
and accepted-event provenance; imported disposition claims cannot supply them.

Every available field carries nonempty acyclic provenance. Empty supported
queries require a complete scoped query witness. Indices refuse above 256
entries, provenance above 512 entries and field references above 32; excerpts
have a 262144-byte total ceiling under the existing 1 MiB frame, depth 24,
4096-item list, 128-field object and 65536-byte string limits. Numeric JSON
coordinates must be JavaScript-safe; signed/unsigned full-width coordinates
use canonical decimal strings. Bounds refuse instead of truncating or rounding.

The new RPC structs reuse the existing dependencies. The source-policy inventory
adds only the closed read DTO roots, redacted excerpt/document/transcript content
and the isolated test path. No scanner rule or discovery floor is weakened.
Full X4 remains open for ordinary producer coverage, canonical relation
disposition, original media, exact staged policy preview and integrated native
UI verification.

## Existing imported verification and dependency ownership

Core tests cover empty/different profiles, exact history and restart retries,
revision/idempotency/profile guards, reject/undo targets, profile restoration
and bounded media. Daemon tests execute concurrent revision races and restart
over local IPC. RPC and optional native tests check closed requests and receipt
correlation; historical fixture byte checks remain required.

RPC now uses the already admitted workspace `serde` 1.0.229 (derive/default
std) and `serde_json` 1.0.151 (default std), both MIT OR Apache-2.0, for the
closed canonical detail boundary. There is no new package/version, network
stack or signing authority. Core owns DTO source interpretation; RPC owns
bounded transport decoding. The daemon's dev-only core feature enables the
synthetic producer for IPC tests. Existing dependency security and source gates
remain required; these edges are not a new A5 acceptance or admission claim.
