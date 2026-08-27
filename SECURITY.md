# Security policy

This is a Phase 0 semantic foundation plus a Phase 1 F0 contract scaffold, not a production vault. F0 admits a bundled plaintext SQLite build dependency but creates no profile, schema, table, or persistence path. It must not process real personal data until ADR-002 is accepted.

Report suspected signing, canonicalization, authority-resolution, fixture-integrity, or dependency issues privately to the repository owner. Do not attach real academic records, audio, private source code, access tokens, or secrets to a report; use a minimal synthetic reproduction.

The implemented boundary verifies deterministic CBOR, an independently expected Ed25519 device key, and the signature before append-only acceptance. The Phase 1 daemon is only a banner-printing compile scaffold and opens no profile or transport. The repository does not yet provide functional or encrypted storage, OS key management, process sandboxing, local IPC authentication, backup/restore, secure deletion, or egress brokering. Those remain explicit ADR acceptance gates.
