# ADR-001: Process and surface authority

- Status: Accepted architecture; Phase 0 implementation is partial
- Decision date: 2026-08-27

## Context

Desktop, IDE, CLI, mobile, and optional web surfaces have different lifecycle and trust properties. Allowing each surface to open the canonical store would create multiple writers, distribute keys, and make crash recovery and policy enforcement UI-dependent.

## Decision

`academicd`, running once per signed-in user session, will be the only canonical transaction writer and owner of store, vault, key/policy brokers, projections, and supervised jobs. Desktop, CLI, IDE, and later capture/sync surfaces are clients of a versioned local IPC contract. A UI optimistic update is not canonical until the core returns an immutable object/event ID and local acceptance receipt.

Surface authority is constrained as follows:

| Surface | May do | Must not do |
|---|---|---|
| Desktop | unlock/admin, ingest/correct, evidence review, backup/restore, policy approval | open DB directly, hold provider/root keys, unrestricted filesystem/network |
| CLI | doctor, scripted import/export/replay/rebuild through core | bypass policy or hidden unsafe writes |
| IDE adapter | submit explicit snapshot/symbol/span and show results | automatic untrusted-workspace collection, DB/key/provider access |
| Mobile capture | native permission state and encrypted spool/package | own full knowledge DB or provider credentials |
| Hosted web/PWA | redacted view and draft/outbox | claim canonical save before core ACK, own raw vault |

Initial IPC is length-prefixed Protobuf request/response plus server events: current-user SID protected named pipes on Windows and mode-0600 Unix-domain sockets in the user runtime directory on Unix. Handshake includes protocol version, capabilities, storage/vault versions, and locked state. Incompatible writes fail closed.

The per-profile singleton is a lock the operating system releases when the owning process dies: `flock` on `{runtime}/academic-os/{profile}/academicd.lock` on Unix and an exclusive `LockFileEx` byte range on that same file on Windows. A file object resolves identically from every logon session, so two concurrent same-user sessions aimed at one profile cannot both acquire it and cannot open two writers on one database. A named object in the Windows `Local\` namespace cannot carry that guarantee, because that namespace is per-logon-session. The failure mode is closed: a lock file that cannot be created or locked refuses startup instead of admitting a second writer. The singleton guarantees at most one daemon per profile runtime directory on one machine, across logon sessions. It does not couple distinct profiles, which resolve to distinct runtime directories and stay independent. It also does not make one daemon reachable from another logon session, because the Windows endpoint name still carries the session id, so a second session has no daemon rather than a second writer.

The listener bounds what any one client can consume. Both frames of a connection are read under a deadline and the connection is closed when it expires, and the number of connections served at once is capped below the platform instance ceiling, so a client that connects and sends nothing cannot grow serve tasks, descriptors, or transport instances without limit. A transient accept error — a client that vanished between connect and accept, a momentarily full instance ceiling, descriptor pressure — is retried after a short pause rather than ending the listener, so transport backpressure degrades bounded service instead of becoming a silent, permanent loss of local IPC for every client.

## Phase 0 evidence

`academic-core::Core::accept_signed_batch` checks canonical bytes, expected device/key/user authorization, and signature before append. Contract verification returns an opaque `VerifiedBatch`; the ledger exposes no append accepting caller-provided unsigned bytes or hashes. The CLI exposes only doctor and synthetic fixture workflows. There is no daemon, DB, recorder, or network path yet.

## Acceptance gates

- Windows current-user named-pipe ACL and remote rejection integration test.
- daemon singleton and desktop-crash transaction survival.
- idempotent concurrent commands from two clients.
- incompatible client/daemon version fail-closed behavior.
- Tauri capability and CSP snapshots with no wildcard HTTP/filesystem/shell authority.

## Consequences

The core can outlive any UI and policy has one enforcement point. Packaging and local IPC become first-class native test obligations. Phase 0 CLI success must not be read as evidence that these remaining gates passed.
