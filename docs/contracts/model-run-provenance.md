# Model-run provenance and the calibration registry

`academic-model-run` is the `P2-M1` boundary. It holds the twelve fields
section 27.3 of the authoritative spec gives a model execution, the per-model
calibration dataset registry, and the reconciliation of a recorded transmission
against `academic-policy`'s `egress_audit`. It persists nothing: the typed rows
are `academic-store`'s, written by migration `0007` inside the acceptance
transaction that inserts the `MODEL_RUN_RECORDED` event.

Migration `0007` is in the encrypted lane's `STORE_MIGRATION_SQL`, so an
encrypted profile carries these tables from creation and admission fingerprints
them. That set is pinned as a whole -- length and each element -- by
`encrypted_profile_v2_is_created_only_by_cipher_lane`, which compiles only under
`sqlcipher-store`. A change to the migration set therefore has to be run in the
encrypted-store lane, which the README's verification block does not cover: it
is a Linux-only hosted job, for the `openssl-src` toolchain reason t068 section
2.3-17 records.

## The twelve fields, and where each one lives

Section 27.3's YAML block is the authority. `model_run_requires_every_field`
parses that block out of the spec file rather than transcribing it, so the
expected set is whatever the spec says today and a thirteenth key fails the test
instead of passing it.

| Section 27.3 key | `ModelRun` field | Storage |
|---|---|---|
| `id` | `id` | `model_run_provenance.model_run_id` |
| `purpose` | `purpose` | `model_run_provenance.purpose_id` |
| `provider` | `provider` | `model_run_provenance.provider_id` |
| `modelVersion` | `model_version` | `model_run_provenance.model_version` |
| `promptTemplateHash` | `prompt_template_hash` | `model_run_provenance.prompt_template_hash` |
| `inputArtifactRefs` | `input_artifact_refs` | `model_run_input_artifact` |
| `transmittedByteRanges` | `transmitted_byte_ranges` | `model_run_provenance.transmission_kind`, `.transmitted_grant_id`, `model_run_transmitted_range` |
| `redactionPolicyHash` | `redaction_policy_hash` | `model_run_provenance.redaction_policy_hash` |
| `outputArtifact` | `output_artifact` | `model_run_provenance.output_artifact_id` |
| `startedAt` | `started_at` | `model_run_provenance.started_at` |
| `cost` | `cost` | `model_run_provenance.cost_micros`, `.cost_currency` |
| `retentionDeclaration` | `retention_declaration` | `model_run_provenance.retention_declaration_id` |

The Rust field names are the spec's keys in snake case, which is why that half
of the comparison needs no table at all. The storage half does, because two of
the twelve are lists and one is an enumeration over two columns; the map above
is `STORAGE_SITES` in `crates/model-run/tests/model_run.rs`, compared against
the spec's key set in both directions, with each key dropped in turn and each
comparison required to notice.

`ModelRun::record` takes all twelve as distinct types and there is no other
constructor, so a run that omits one does not compile. The database half is
`model_run_row_requires_every_stored_field`, which drops each stored column from
the insert in turn and requires each one to be refused.

## `transmittedByteRanges` is a value, not an absence

A local model transmits nothing, and `Transmission::LocalOnly` says so. That
matters because the reconciliation is total in both directions:

* an `EGRESSED` run must have exactly one consumption record for the grant it
  named, and the allow audit row that record points at must carry exactly the
  recorded ranges and their summed byte count; and
* a `LOCAL_ONLY` run must have no allow row that transmitted the bytes of any
  artifact it read, compared by content digest.

`Transmission::egressed` refuses an empty range list, so an egress that nothing
describes is not representable rather than merely unreconcilable.

## Why the reconciliation keys on `egress_consumption`

`egress_audit.grant_id` carries identifiers from two tables and has no foreign
key: `P2-G7` removed one so process-activity rows could be written at all.
`T146` measured that the typed `(process_class, capability)` pair does not
discriminate them -- `EGRESS_PROXY` x `OPEN_OUTBOUND_SOCKET` is the cell where
the two namespaces overlap exactly, and it is the cell egress auditing cares
most about.

So the reconciliation does not read that column. It reads `egress_consumption`,
whose two foreign keys hold together: `grant_id` references `egress_grant`, and
`(egress_audit_seq, grant_id)` references `egress_audit(audit_seq, grant_id)`.
A consumption row therefore names an audit row whose identifier is a real egress
grant, and `execute` writes it in the same transaction as the allow audit, so
the row it names is the transmission rather than the decision that minted the
grant. `crates/policy/tests/consumption_join.rs` is where both keys are observed
refusing: a process-capability token, a mismatched pair whose grant is real so
the composite key is the only thing left to refuse it, and an unminted grant,
against a control that is accepted.

`an_audit_row_from_the_other_namespace_is_not_the_grant` is the observation that
this matters here. It mints a real egress grant, spends it, and then mints a
process-capability token for the overlapping cell and spends that too through
`ProcessActivity::external_transmission`. The two allow rows agree on decision,
process class, capability, byte count, destination, artifact ranges and
external-transmission digest; only the identifier differs, and only the join
says which namespace it came from. A model run naming the token as the grant it
spent is refused `GrantNotConsumed`, and the same test runs a
`grant_id`-only reconciliation beside the product one and requires it to
*accept* the forged grant -- so what the join buys is executed on every run
rather than described.

A discriminator column on `egress_audit` would answer the same question and is
not here: `T149` measured that the two foreign keys above already resolve it for
a consumed grant. What a column would additionally have covered -- deny rows and
process-capability activity rows, which no join resolves -- is `S-16` in
[policy source scans](policy-source-scans.md), and it stays open.

## Calibration

A provider's raw number and another provider's raw number mean different things,
so the type offers no way to rank them and no way to read the number back out
and rank it by hand. `RawScore` implements neither `PartialOrd` nor `Ord`, has
no accessor returning its units, and hand-writes `Debug` so no formatting trait
prints one. `<`, `>=`, `cmp`, `partial_cmp`, `max`, `sort`, `iter().max()` and
`BTreeSet<RawScore>` are eight separate cases in
`crates/model-run/tests/compile_fail/raw_scores_are_not_ordered.rs`, and the
suite passes only when each fails to compile with the committed diagnostic.

`CalibrationRegistry::interpret` is the only producer of a
`CalibratedConfidence`, and `DisplayedConfidence::of` takes one, so an
uninterpreted score reaching a reader is a type error rather than a run-time
check one layer out has already skipped. Interpretation needs a dataset
registered for the exact provider, model version and purpose, and needs it to be
fresh: a dataset is stale once `refreshed_at + refresh_interval_millis` has
passed, and also when the clock is before its refresh.

A calibration dataset carries `sample_count`, `refreshed_at`,
`refresh_interval_millis`, a content digest, and a bin curve that must increase
in raw units and not decrease in permille. One dataset per
`(provider, model_version, purpose)`; a second is refused.

`CalibratedConfidence` *is* ordered, by the permille and by nothing else. That
is what calibrating buys, and the ordering is hand-written rather than derived
so it cannot fall back to comparing dataset identifiers or provider names.
`cross_provider_raw_scores_are_not_ordered` shows the inversion that makes the
prohibition matter: a generous provider's raw 900 and a strict provider's raw
300 read as 200 and 800 permille, so the raw ranking is the reverse of the
calibrated one.

## Reanalysis appends

ADR-003's rule is the one that applies -- a correction appends a new assertion
and never edits a row -- so this adds no second mechanism. A later run's
candidate names the earlier candidate it supersedes; both rows stay, and the
diff is read from the pair.

`model_run_candidate` is INSERT-only under a trigger pair and the SQLite
authorizer. `UNIQUE (model_run_id, subject_digest)` stops one run recording two
candidates about one source, `UNIQUE (supersedes_candidate_id)` refuses a fork,
and `guard_model_run_candidate_supersession` refuses a supersession that
addresses another subject or comes from the same run.

Those three overlap, and only the trigger refuses one shape: a revision about a
*different* subject, from a third run, of a candidate nothing has superseded
yet. `a_reanalysis_addresses_the_subject_it_supersedes` is that case, and
`M-I14` is the observation that deleting the trigger makes it pass. The
same-run clause of the trigger has no case of its own -- `UNIQUE
(model_run_id, subject_digest)` already refuses every insert that would reach it
-- and it is kept as the second layer rather than removed.

## What binds a typed row to a signed event

Two things:

* `model_run_provenance.model_run_id` is a foreign key to `model_run`, so the
  row cannot exist without the accepted `MODEL_RUN_RECORDED` event; and
* `guard_model_run_provenance_authorized` refuses an insert whose
  `record_digest` is not that event's `source_digest`.

The digest covers the child rows, and a `BEFORE INSERT` trigger on the parent
cannot see rows that do not exist yet. So the trigger is the write half and
recomputation on read is the other: `ModelRun::record_digest` rebuilds it from
the whole record, and `the_record_constructor_takes_every_field` requires every
struct field to appear in that computation, so a field added without being
hashed fails rather than producing a digest that does not describe the record.

## What this is not

It is not an inference pipeline, a proposal boundary, or a review queue: `P2-M2`
owns `Proposed<T>`, the risk tiers, and dispositions. It records nothing to the
ledger itself and mints no grant. And it opens and closes no section 38 gate.

Nor does it own every number a reader might see. `ConfidencePermille` is
`academic-domain`'s and any crate can construct one, so what is refused here is
narrower and exact: there is no way to get the number out of a `RawScore` at
all, so a provider's uninterpreted score cannot become a displayed confidence.
Fabricating a permille from a literal is a different thing, and this crate does
not claim to stop it.

## Posture

Nothing here is ADR-002 acceptance. The default lane remains
`storage_encryption=NONE`, `production_data_allowed=false`,
`adr_002_accepted=false`. Every score, dataset, audit row and candidate in this
crate's tests is synthetic and built in process; the crate calls no provider and
no model, and its link closure holds nothing that can open a socket.
