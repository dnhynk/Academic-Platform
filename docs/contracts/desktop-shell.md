# Desktop shell and runtime contract

P2-X1b binds the existing route, palette, backlink, evidence drawer and sealed receipt contracts to a real Tauri window under approved `gate_59451294e004`. Only the non-default `desktop-runtime` feature selects Tauri and its build dependency. Default builds retain the contract library and the original network prohibition.

## Build and launch

Run `node tools/source-preflight.mjs` before dependency retrieval and use the repository's pinned bootstrap. Linux needs `libwebkit2gtk-4.1-dev` and its system dependencies; Windows uses installed WebView2. The explicit hosted desktop lane installs only that named Linux package through `tools/desktop-prerequisites.mjs`.

```powershell
pnpm --filter @academic-os/ui build
cargo build -p academic-desktop --features desktop-runtime --locked --offline
cargo run -p academic-desktop --features desktop-runtime --locked --offline
cargo run -p academic-desktop --features desktop-runtime --locked --offline -- --smoke
```

The ordinary window opens without a daemon and reports unavailable. To connect to an existing synthetic daemon started through the README workflow, replace `--smoke` with `--session <runtime>/academic-os/<profile-key>/session.meta`. The host selects one session file; the frontend cannot supply a path, endpoint, fixture, URL, process or key.

`--smoke` runs a fixed bundled module in the real webview. Without a session it checks shell navigation and unavailable native operations. With `--session <session.meta> --smoke`, its imported detail checks require a disposable seeded synthetic profile and append an actual relation rejection followed by an exact undo through that session. The report distinguishes the shell, profile navigation and imported detail checks. Do not run the session smoke against a profile whose history must remain unchanged. Compilation, programmatic IPC tests and a separately staged browser DOM harness do not establish native visual or playback QA.

## Frame and accessibility

`runtime-entry.ts` renders the existing `shell.ts`/`views.ts` state into native HTML. The canonical route manifest, independent view registry, entity corpus and relation table are unchanged. One specification tree line remains one route; detail parameters remain destinations of that route. Course, Concept, Project and Question are reachable through the global palette, their detail views expose backlinks and Pin evidence, and `navigate` carries drawer state across views. Sections explicitly have no live records; the shell does not invent academic results or claim a finished interactive atlas.

The user-supplied baseline-ui and fixing-accessibility references apply within the existing stack. Controls have native semantics and visible focus; the named native dialog has a labeled search input and browser-owned modal trapping/restoration. Ctrl/Cmd K opens the palette and Escape closes it. State feedback uses status regions, mutation errors appear beside the action, and empty states offer an explicit next action. No animation or framework migration is added. Further atlas/content/performance and accessibility review belong to the later surface tasks.

The original named tests remain: `route_manifest_matches_ia_exactly`, `every_destination_opens`, `palette_reaches_four_entity_types_from_every_route`, `backlinks_resolve_for_four_entity_types`, `evidence_drawer_persists_across_views`, `capability_snapshot_has_no_wildcard`, `desktop_cannot_open_the_database_or_read_keys`, and `optimistic_update_is_not_canonical_before_receipt`.

The imported lecture, concept, question and project detail surfaces use only the host-selected profile's `details_read` result. The explicit corpus builder and 12-second WAV are seed inputs, excluded from bundled runtime imports. Missing, failed and loading detail reads cannot pin legacy example evidence. The palette, titles and backlinks use accepted profile IDs; a pinned evidence snapshot retains its profile, projector, watermark and digest while navigating. Current and historical selectors are available under the evidence time control; historical views disable relation writes.

The lecture starts with the preserved document and exposes raw/corrected transcript, segment/capture/timecode round trips, coverage dispositions and review queues. Relations expose original source text and reported status/confidence before optional AI explanation. Projects retain the stale snapshot banner and distinguish frozen read-only findings and local UTF-8 byte inspection from unavailable analysis dispatch and policy-staged egress preview. Imported labels remain importer metadata, including labels that say confirmed; they do not establish normal-domain mastery, personal authorship or resolution authority.

Imported rejection/undo requires the matched immutable original receipt. Before sending, the UI stores the exact request identity and undo target in its local retry journal; this is unconfirmed request metadata, never acceptance evidence. An ambiguous outcome blocks new decisions and exposes an explicit original-request retry, including after reload/restart, source expiry or alias replacement. Distinct requests for the same profile/relation/action/revision remain separate v1 journal entries; retry controls identify the full original request ID. A validated receipt or definite refusal releases only that entry, and a retry cannot recreate an entry already removed by another context. Profile mismatches and ambiguous replies retain it. Unavailable retry storage prevents a new write. Reordered reads cannot replace a newer selected snapshot. A late confirmation after navigation reconciles the request while retaining the destination's snapshot, actual controls, drafts, focus, disclosures, scroll and live audio; a visible refresh notice requires an explicit validated read before further decisions. Historical selection remains read-only. Bounded WAV playback cancels pending reads/decode/resume and releases its audio nodes on navigation.

## Local IPC and receipts

Tauri registers only `desktop_request_v1`. Its build-time `AppManifest::commands` names the same command and the local main-window capability grants only `allow-desktop-request-v1`. No core default permission bundle or remote capability is granted. Version 1 requests contain a closed operation enum mapping to `DesktopCommand`; unknown versions, fields and operations fail closed. Empty struct variants make Serde reject surplus fields even for argument-free operations.

The client reads at most 4097 bytes from host-selected `session.meta`, requires the exact three-line v1 format and 64 lowercase hexadecimal nonce, and refuses non-local endpoints. Windows admits only `\\.\pipe\academic-os\<session>\<profile-key>`; Unix requires an absolute socket path ending in `d.sock`. Session nonce and path stay in Rust and are never returned or logged. The desktop opens no profile/database and acquires no root/provider key.

The existing bounded, semantically validated RPC framing is used. One five-second deadline covers connection, handshake, request and acknowledgement. Windows retries only pre-send errors 2 and 231, at 20 ms intervals within 25 attempts and a 500 ms connection deadline; other errors return promptly. No mutation is resent after delivery can be ambiguous. Missing acknowledgement reports no canonical save is **confirmed**, not that the daemon necessarily rolled back. The monotonic clock is checked before each desktop open, including after a delayed executor poll. The CLI now has its separately tested pre-send retry after PR #113; it preserves the last native error at expiry, while desktop reports a timeout, so this is not a claim of identical client behavior.

Diagnostics reports validated handshake availability/lock state. Ingest names only `SyntheticFixtureId::Phase1BitemporalLedgerV2`. Backup and restore use existing mutable commands and can be refused by the core. Export reports unsupported because the frozen local protocol has no export response; it never opens the store to imitate the CLI export path.

The original pure `mutable_request_digest` moved from core into `academic_rpc::digest` and is re-exported at its former core path. Digest domain, length framing and bytes are unchanged. Six independently assembled fixed SHA-256 vectors cover all command arms and absent/present expected revision; no new hash dependency was needed.

A mutable response must have accepted/duplicate status and the submitted request ID. `Optimistic::confirm` then checks request ID, client instance, idempotency key and request digest against the core's immutable receipt. Only that promotion can produce accepted state and the UI's receipt ID. A shared request controller serializes diagnostics and saves, disabling both actions until the owner finishes; deferred-reply tests ensure diagnostics cannot release a pending save or leave a previous receipt beside an unconfirmed new save. Diagnostics preserves the last completed save, while starting a new save clears its receipt. The UI shows saving until this response, then saved; an initially collapsed Save details control exposes the receipt without putting protocol terminology in the ordinary status. Locked/incompatible/rejected/unavailable replies have no canonical receipt. Runtime stream tests exercise actual framing, locked state, dropped acknowledgements and mismatched receipts. Existing compile-fail tests still prevent reading, serializing or converting an unaccepted optimistic value; the TypeScript WeakMap seal is unchanged.

## Runtime dependency and capability policy

Tauri 2.11.5 disables defaults and selects only `wry`, `custom-protocol`, `x11`; tauri-build 2.6.3 selects only `config-json`. Dynamic ACL, TLS, filesystem asset protocol, updater and plugins remain disabled. No fs/http/shell Tauri plugin is installed. Workspace `unsafe_code = "forbid"` remains intact, including the context macro boundary.

`dependency-admission-phase2-x1b.json` enumerates 327 new exact lock tuples with owner, source/checksum, SPDX license, resolved features, advisory path and trust-boundary admission. The full optional closure is pinned and checked against Cargo metadata without glob exemptions. It contains HTTP implementation dependencies required by Tauri and is not described as network-free; there is no product HTTP transport. Both default and optional desktop closures are checked for database/key owners and drivers. Public signature verification dependencies are not key custody. Platform link/build measurements and CI times belong to the task report and CI budget record.

`tauri.conf.json` is compiled into the runtime. The checked snapshot now uses `desktop-dist`, enables the official global invoke API, disables drag/drop and replaces `core:default` with the one application permission. CSP permits bundled assets and the exact Windows IPC host `http://ipc.localhost` alongside `ipc:`; no wildcard host is allowed. Native smoke records IPC resource timing names where the WebView exposes them. Tauri's CSP nonce/hash modification stays enabled. Both changed snapshot files and the vendored schemas retain explicit SHA-256 pins and closed-value/authority checks; the snapshot is not claimed unchanged.

The whole-package source scan still checks exact path roots and all module/include targets. The new local IPC file and optional build script are admitted individually, with no broad source exclusion. `tools/desktop-runtime-policy.test.mjs` checks the optional closure, runtime command manifest, bounded session read and feature isolation in the UI verification lane.

## Limits

Production data remains forbidden and ADR-002 remains unaccepted. This gate creates no recorder, product HTTP transport, installer release, signing, updater or publication. Native platform observations, exact CI timings and any missing UI/platform evidence are recorded honestly in the external task report; the later X4/X6 reviews inherit the user's UI reference.
