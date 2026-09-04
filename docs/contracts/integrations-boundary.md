# Integrations boundary contract

`academic-integrations` is the `P2-P3` boundary. It is every place this product
touches something outside itself, and section 33's rule is the same at all of
them: the outside is a mapping and a seam, never the record.

It opens no socket, reads no clock, touches no file and spawns no process. What
it consumes is values its caller supplies; what it produces is values, and one
of them — a staged assistant payload — is produced by `P2-G2` rather than here.

## Section 33's table is the vocabulary, and it is not counted

`ConnectorKind` is that table's first column, in its order.
`every_section_33_row_is_a_connector_kind` parses the table back out of
`PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md` and compares it with
`ConnectorKind::ALL` in both directions. Nothing in the crate asserts how many
rows there are: a row added to the design document without a variant fails, and
a variant with no row fails too.

Every other closed vocabulary in this crate is held the same way, and by one
scan rather than by a rule per enum. `every_all_array_is_the_enum_it_names`
walks the whole product source, finds every `pub enum` that declares a
`pub const ALL`, and compares that array's entries against the variant list read
out of the enum body — so an arm added without an entry fails rather than
passing every walk that iterates the array. The set of types declaring an `ALL`
is pinned beside it, so removing one to escape the comparison is an extra key
rather than a silent exemption.

That scan found its own defect on its first run: the `ALL` reader searched
forward from an `impl` header to the end of the file, so `ConnectorError` — which
declares no `ALL` — reported `HttpMethod`'s and the scan failed on a
disagreement it had invented. The reader is now bounded to the `impl` block, and
`the_helpers_are_not_vacuous` holds a two-type sample the unbounded version gets
wrong.

## The core opens when every connector is down

Three independent things hold it, because each is blind to a different way of
losing it.

**By graph.** This crate has no product edge to `academic-ledger` or to any
crate that owns one. A connector failure has nothing to fail *through*, because
the core read path does not run through this package at all.
`workspace_dependency_direction_is_acyclic` is where that edge set is frozen.

**By text.** `IntegrationSurface::read_core` is pinned as whole text, and the
whole set of identifiers its body spells is compared against a four-name set.
`fleet` is not among them. A health check reached through any other spelling
still has to name something that is not on that list.

**By count.** `core_graph_opens_with_every_connector_down` builds a real
`academic-ledger` `LedgerState` — a signed batch, verified and accepted through
that crate's own path — and reads every `CoreView` twice: once with
`ConnectorRegistry::all_down` and once with `all_up`. The two byte sequences must
be identical, the connector fleet's call count must be zero across both, and the
core's own read count must equal the number of views. The registry's
`unreachable()` set is compared with `ConnectorKind::ALL`, so *every* connector
is down rather than a sampled one, and the fleet is then asked one question
directly and must both answer and count it — a fleet that recorded nothing would
otherwise satisfy the zero.

`academic-ledger` is a **dev** edge for exactly this reason. Making it a product
edge would put the ledger inside the closure of the crate the claim says is
irrelevant to it.

## The GitHub connector is read-only, and that is four whole sets

Not a list of forbidden verbs. `github_connector_is_read_only_and_scoped`
compares four whole sets, and a write would have to defeat all of them:

1. **The operations.** `GitHubOperation::ALL` is walked, and
   `GitHubOperation::method` is a total `match` returning `HttpMethod`, whose
   only variant is `Get`. That is `P2-R1`'s `TokenPermission::access` shape: an
   operation added without an arm is a compile error, and a `Post` variant added
   to `HttpMethod` only matters if some arm returns it — which the walk sees.
2. **The request.** `ReadRequest`'s whole `(name, type)` field set is compared
   against a pinned inventory. There is no body field, and one added is an extra
   key rather than a name somebody had to spot.
3. **The connector's surface.** The whole set of `GitHubConnector`'s public
   methods is pinned. A second request builder is an added line whatever it is
   called.
4. **The seams.** The whole set of `trait` declarations in this crate, each with
   its whole method set, is compared against a three-entry list — `CoreGraph`,
   `ConnectorFleet`, `IdeWorkspace`. None of them sends anything, so a write
   reached through a seam needs a method that appears here.

The scoped half is the same walk: every operation's path is built from *this
connector's* repository rather than from an argument, and each of the six is
required to sit under `/repos/{owner}/{name}`. `P2-R1`'s three credential
properties are then exercised for every operation rather than for one:
an expired token, a token scoped elsewhere, and a token missing the operation's
permission each produce their own distinct error, and the operation whose
permission the narrow token *does* carry is required to succeed — so the
refusals are attributable to the property rather than to a check that refuses
everything.

A webhook is the inbound half. `WebhookDelivery::accept` produces a
`P2-G5` `Untrusted<IngestedDocument>` and no `ReadRequest`. **No seventh
`SourceKind` arm is added**: a push body is `README` and an issue or
pull-request body is `ISSUE`, which are the arms `P2-G5` already fixed.

## A private blob's second grant is a second row

`BlobVisibility::required_grants` is a total function: one grant for public
bytes, two for private ones. The second grant is not a flag. It is a second
`P2-G1` row, minted from its own complete request tuple with its own
`purpose_id`, and **consumed through `PermissionBroker::execute`** before any
byte reaches `P2-G2`'s transmit.

`PrivateBlobEgress::bind_disclosure` is the first statement of the one public
transmit, for the reason `bind_grant` is the first statement of both of `P2-G2`'s
paths. It refuses three things in order:

| Presented | Outcome |
| --- | --- |
| a private blob with no disclosure token | `NO_GRANT`, zero bytes |
| a disclosure token naming the transfer's own grant | `SCOPE_MISMATCH`, zero bytes |
| a disclosure the broker itself refuses | that refusal, zero bytes |

`the_disclosure_is_bound_once` counts `bind_disclosure` at two mentions — one
declaration and one call — counts `proxy` at four and `transmit` at two, and
requires the binding to be the first statement. That is the half a behavioural
test cannot make: a second path that skipped the binding would be a private blob
leaving under one grant and no other test would notice.

`private_blob_egress_needs_a_second_grant` is the half a count cannot make. It
runs the public blob under one grant and observes the bytes arrive, runs the
private blob under the same one grant and observes `NO_GRANT` with an empty
transport, runs it with the same grant presented twice and observes
`SCOPE_MISMATCH`, and then runs it with two distinct grants and reads the
disclosure grant identifier **back out of the result** and compares it with the
one the broker minted. `P2-G2`'s `eg04` row records what a discarded identifier
costs: two records agreed only because the fixture put the same value in both.
Both grants are then found in the broker's own consumption rows.

`BlobDenial` has three fields and no payload, for the reason `EgressDenial` has
four and no payload. `a_blob_denial_has_no_payload_field` reads them.

## The IDE adapter writes nothing, and a confirmation is a digest

`IdeWorkspace` has three methods. Each takes `&self` and returns an owned value,
so there is no argument through which a mutation could be handed in and no
`&mut` through which one could be made. `ide_adapter_performs_no_writes`
compares the trait's whole method set and `IdeAdapter`'s whole public method
set, requires no signature in the module to take a mutable reference, and
separately requires the whole set of `std::fs`, `std::net`, `std::process` and
`std::io` reaches in the crate's product source to be **empty** — so a write
made without the seam has no route either. The runtime half drives a full
adapter session against a recording workspace and observes the call count and
the workspace's unchanged state.

File watching is opt-in: `WatchMode::Disabled` is the default and
`IdeAdapter::changed_scope` refuses under it.

A confirmation carries the digest of the scope it was recorded for, not a
boolean. `ScopeConfirmation::record` takes the `ChangedScope` itself, so a
confirmation cannot exist for a scope nobody computed, and
`IdeAdapter::request_snapshot` compares the digest of the scope it is given with
the digest the confirmation carries. `ide_confirms_changed_scope_before_snapshot`
confirms a scope, snapshots it, then changes a file and requires the same
confirmation to be refused with `ScopeChanged` — and then requires it to still
admit the *old* scope, so the refusal is about the change rather than about the
confirmation ageing. That is `P2-R1`'s binding: an admission carries the digest
of the request it was decided for.

## The assistant receives the selection, records the run, and is not evidence

`AssistantContext::minimize` slices nothing itself. It builds `P2-G2`'s
`StagingRequest` with the caller's symbols as the focus and hands it to that
crate's pipeline, so "only the selected ranges" is that crate's structural
minimization observed rather than a second implementation of it. A symbol the
document does not declare is `SCOPE_MISMATCH` there, not a licence to send the
whole file. `assistant_receives_only_selected_ranges` asserts four markers
outside the selection are absent **and** re-runs the same reader over a
selection of all three declarations, which must find them — so the absences are a
property of the selection rather than of a reader that finds nothing.
`AssistantSelection` refuses an empty list and a repeated name rather than
deduplicating: a selection is what the user pointed at.

`GeneratedCode` has six private fields and one producer, which takes `P2-M1`'s
`ModelRun` by reference and stores its `record_digest`. The context digest is
taken from the staged preview rather than from an argument, so what is recorded
is the bytes the assistant actually received.
`generated_code_provenance_is_recorded` reads every field back, compares the
whole field set against a pinned inventory, requires the producer set to be one
entry, and builds a second run differing in one field to show the digest is
load-bearing.

**Assistant use is not competency**, and the type is the smaller half of that.
`AssistantUse::eligibility` is a total `match` returning `EvidenceEligibility`,
whose only variant is `NotEvidence`. The larger half is the graph: this crate has
no edge of any kind to `academic-competency` or `academic-repository-competency`,
and `assistant_use_is_not_competency` reads the transitive product closure out
of the workspace manifests and compares it whole — with a control run over
`crates/role-profile`, which must find its competency edge, because a walker that
returned the empty set would satisfy the comparison. The crate's whole set of
`academic_*` paths and imports is compared beside it, so a reach for a mastery,
a rubric or an evidence strength is an extra key. That is `P2-R5`'s rule —
unmodified generated code makes no `APPLIED` claim — and `P2-Y1`'s — a dependency
being present fills no rubric cell — expressed as a property of the graph.

## The calendar payload has no field a grade could ride out on

Two layers, failing for different reasons.

**The field set.** `CalendarPayload`'s whole `(name, type)` list is pinned, so a
field added is an added key.

**The field types.** Every field's type must be one of four this module admits —
`ExternalId`, `CanonicalRef`, `CalendarEventKind`, `TimestampMillis`. A
`String`, an `f64`, a `u32` or a `MasteryLevel` fails as an inadmissible type
whatever it is named, so a reviewer who widens the pin still has to defend the
type.

There is **no free-text field at all, not even a title**. Section 33 says what
this system keeps is the event identifier; a human-readable label is a decision
about what a label may hold, and inventing one here would be inventing the
position this task exists to close. `CalendarPayload::summary` returns a
`&'static str` chosen by `CalendarEventKind`, so every word a provider displays
was compiled into this crate.

The byte half scans `encode()`'s output for every grade symbol and every
knowledge-state level, read out of `crates/record/src/grade.rs` and
`crates/domain/src/lib.rs` rather than transcribed. It compares **tokens** rather
than substrings, because four grade symbols are one letter long and a substring
scan for `S` finds one in every word. The same scanner is then run over a buffer
that does carry a grade and a mastery and is required to find both.

That the shared `tools/secret-debug-policy.test.mjs` net passes this crate is
not part of this claim. `P2-U8` measured that net passing twelve of twelve over a
`String` field a later injection walked through, and `T197` measured its text
classifier not reaching `crates/review` at all. The guard here is at this
boundary and reads this crate's own declarations.

## `ExternalIdentity` is a mapping and never becomes canonical

`CanonicalRef` has five arms and every one carries an `academic_domain`
identifier by value. Those are UUIDv7 and their own constructors refuse anything
else, so a provider's opaque string cannot become one by parsing. There is no
`From<ExternalId>`, no `TryFrom<&str>`, no `FromStr`, and no arm holding text:
`an_external_id_is_not_a_canonical_reference` in
`crates/integrations/tests/compile_fail/` is those five routes as five compiler
diagnostics.

`external_id_is_never_canonical` is the whole-set half. Every public signature in
the crate is classified: one whose return type names a `CanonicalRef` must have
*received* one — as a parameter, or as `&self` on a type that holds one — and the
holder set is itself read out of the `struct` and `enum` declarations rather than
listed. A conversion added later returns a canonical having taken only text, and
that is a classification over the whole set rather than a name somebody forbade.
The producer set is pinned beside it at two entries.

The runtime half registers a mapping, resolves it, and requires the same
identifier in another system and an identifier nobody registered to resolve to
nothing at all: an external identifier on its own addresses no record.

## A sync conflict keeps both sides

`IdentityMap::register` resolves a disagreement by `SourceAuthority` first and
by valid time second, and stores **both** sides in `SyncConflict` either way.
On a tie — equal authority and equal valid time — `ConflictBasis::Tie` is
recorded, `preferred()` is `None`, and the entry keeps the side that was already
there rather than inventing a winner. That is `P2-N5`'s rule for a tied root and
`P2-Y2`'s for two coexisting bundles. Registering the same mapping twice is not
a conflict.

## What this contract does not claim

- **No migration, and that is deliberate.** Nothing in the ten acceptance rows
  is a durable claim: an identity map, a changed scope, a staged payload and a
  calendar event are all values a caller holds. A migration number would be a
  table nothing in this crate writes. `academic-store` has no edge here in
  either direction.
- **No network was used and none can be.** No transport ships:
  `GitHubRepositoryReader` belongs to `academic-repository` and has no
  implementation, `OutboundTransport` belongs to `academic-egress-boundary` and
  has none, and the three seams this crate declares are traits whose only
  implementations are the synthetic doubles in its own test tree.
- **The webhook is admitted, not authenticated.** What this crate does with a
  delivery is tag it through `P2-G5`. Signature verification of a provider's
  delivery header needs a shared secret and a socket, and belongs to whichever
  task first opens one.
- **`GATE-38-020` stays open.** LMS and course-registration terms are not
  decided here and nothing in this crate reads them.
- **Nothing here is ADR-002 acceptance.** `product_network` remains `NONE` and
  `production_data_allowed` remains `false`.
