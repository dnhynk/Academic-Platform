# Durable detail workspace seam

The native `desktop_request_v1` command reads a host-selected synthetic profile
and appends signed relation dispositions through the daemon's single writer.
It does not select a filesystem path, accept a corpus, choose an actor, or accept
arbitrary events from the webview. An empty profile returns an empty corpus.
The normal service does not seed data or select a hard-coded product fixture.

## Source and authority boundary

`academic-core::details::CorpusRecord` is a version-1 **imported snapshot** of the
closed detail DTO, entity aliases, canonical relation-claim IDs and optional
lecture media artifact IDs. It is stored as a v4 `ClaimAsserted` with predicate
`detail.workspace.corpus.v1`. Each displayed relation has a preceding accepted
`detail.workspace.relation.v1` claim in the same domain/scope and an actual
owning entity. Both claims require an Importer actor, `DIRECT_OBSERVATION` and
`CODE_OBSERVED`. Relation JSON must equal the signed snapshot; its displayed
source bytes must match the first evidence excerpt digest. Evidence and
artifact closure are verified by canonical replay and vault read-back.

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
links. Invalid existing markers fail closed. Reopen is stable; canonical
backup/restore and fresh same-path profiles get a new marker. Relocation changes
the profile ID. A filesystem clone containing local metadata at the identical
path is not an authenticated recovery procedure.

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

## Verification and dependency ownership

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
