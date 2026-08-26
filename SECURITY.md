# Security policy

This is a Phase 0 scaffold, not a production vault. It must not process real personal data until ADR-002 is accepted.

Report suspected signing, canonicalization, authority-resolution, fixture-integrity, or dependency issues privately to the repository owner. Do not attach real academic records, audio, private source code, access tokens, or secrets to a report; use a minimal synthetic reproduction.

The implemented boundary verifies deterministic CBOR, an independently expected Ed25519 device key, and the signature before append-only acceptance. It does not yet provide encrypted storage, OS key management, process sandboxing, local IPC authentication, backup/restore, secure deletion, or egress brokering. Those remain explicit ADR acceptance gates.
