# ADR-010: Sync and device conflict

- Status: Proposed decision register

## Registered direction

Sync transports immutable signed batches and encrypted object chunks. Merge is authenticated set union plus device origin-chain validation; generic LWW and whole-state CRDT overwrite are rejected. Replica-local `accept_seq` is not presented as a universal device clock. Competing claims remain visible and predicate authority/user decisions resolve an active view without deleting conflict history.

Pairing grants a device identity and scoped domain capability through an authenticated user-confirmed exchange. Revocation blocks future grants/keys and triggers the appropriate rotation workflow. A relay, if product need justifies one, sees only opaque envelopes/chunks and the minimum routing metadata declared in its threat model. Filesystem export/import is the first transport conformance surface.

## Acceptance gate

Offline divergence then set-union merge; duplicate/out-of-order/gap/fork corpus; competing user/official/inferred claims; pairing/grant/revoke and lost-device exercise; interrupted transfer; relay metadata/equality leakage analysis; and export-directory round trip. Phase 0 implements only single-device chain validation.
