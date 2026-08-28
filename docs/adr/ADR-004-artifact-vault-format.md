# ADR-004: Artifact vault format

- Status: Proposed decision register

## Registered direction

Artifact identity is SHA-256 over exact plaintext bytes plus descriptor metadata (`media_type`, length, encryption domain, retention class, permission lineage, format version). The logical digest is stored only inside encrypted metadata. A physical locator is `HMAC-SHA-256(domain_locator_key, format_version || media_type || 0x00 || plaintext_digest)` in a domain namespace; a locked directory listing must not expose global plaintext equality.

Deduplication is permitted only when encryption domain, retention class, and permission lineage all match. Global/convergent deduplication is rejected. Evidence locators include the source digest and exact byte/time/page/repository coordinate so a changed source cannot silently satisfy an old claim.

The eventual object format will use random per-artifact DEKs, domain KEK wrapping, versioned headers, independently authenticated chunks, and AAD binding object identity and chunk position. AEAD algorithm and chunk size remain acceptance-gated.

## Implemented now

Algorithm-prefixed digest and keyed locator newtypes, domain-key separation tests, byte-length/media metadata, and exact evidence locator validation are implemented. A complete `TEXT_BYTES 0..artifact.byte_length` representation must use the artifact content digest for both source and representation, cryptographically closing the excerpt to the registered bytes. Partial text, page, transcript-time, and repository representations remain valid descriptor vocabulary, but Phase 0 event/evidence acceptance fails closed because no byte-resolving verifier capability exists; an actor label alone is not trusted proof of resolved bytes. Evidence acceptance also enforces artifact/event domain closure and never compares two caller-controlled digests as proof. No encrypted object writer exists.

The artifact JSON boundary first parses the raw text with unique decoded property names, Unicode-scalar-only strings, and canonical unsigned integer lexemes. It then rejects unsafe integers and nonportable paths at schema level and executes a semantic post-validator for ranges, artifact bounds, span lengths, source/full-range digest binding, and locator-identity uniqueness. Rust and Ajv/TypeScript run the same committed structured and exact-raw mutation corpus, including duplicate names, lone surrogates, positive text/page/time/repository Unicode descriptors, and unknown properties at the descriptor, representation, and locator levels. Mutation checks independently require Rust's descriptor and representation structs to retain closed-object deserialization.

The synthetic Phase 1 vault also binds object liveness to Store transaction lifetime. A sealed
capability owns a live no-follow object handle, its exact host file identity, and a shared lease in
the policy-namespaced `vault/leases/v1` tree. Ingest and verification acquire that shared lease;
product-controlled quarantine, removal, and replacement must acquire the corresponding exclusive
lease. Store re-hashes the retained handle and reopens/re-hashes the canonical path under that same
lease immediately before every successful new, duplicate, or idempotent commit, rejecting missing,
truncated, replaced, or identity-changed objects without a new durable receipt.

One `Vault` value is safe to share by reference across threads. Ingest, sealed-object
verification, and pre-commit revalidation may run concurrently on `&Vault`, for the same artifact
or for different artifacts, without a caller-supplied lock; the daemon must not serialize ingest to
work around them. Concurrent ingest of identical bytes into one policy namespace publishes exactly
one object and adopts it for every other caller, so no caller receives a spurious failure, no
object is published without a receipt, and no partial is leaked into `vault/tmp`. Reconciliation is
excluded from that contract: it is a startup pass that removes expired partials and quarantines
orphans, and it must complete before the daemon accepts clients.

Windows makes that contract concrete. The explicit durable directory handle is opened with
`GENERIC_WRITE` because `FlushFileBuffers` refuses a read-only directory handle, so the flush the
design names actually executes instead of being swallowed as unsupported. Every synchronization
still writes the write-through directory barrier, which is the ordering mechanism only on a host
that rejects the flush; the barrier permits read and write sharing and its write is covered by the
same bounded sharing-violation retry as publication, because concurrent ingests synchronize the
same `vault/tmp`, object fan-out, and lease directories. Lease-file creation requests read access
only, so it cannot collide with a shared lease another thread took inside the existence-check
window.

This lease is a product coordination boundary, not an OS sandbox claim. Windows file sharing and
Unix advisory `flock` provide a portable cross-process protocol for every Academic Platform owner,
but an unrelated same-user process, malware, administrator, or storage failure can ignore or bypass
the Unix advisory lease. Immediate pre-commit revalidation detects mutations visible at that gate;
SQLite and a separate filesystem cannot be made atomic against a hostile mutation in the final
instruction window. The single-owner daemon, protected local profile, and out-of-process trust
boundary therefore remain required, and this Phase 1 mechanism does not accept or close ADR-004's
encrypted production format gate.

## Acceptance gate

Zero-byte/small/multi-GB/seekable-audio vectors; a trusted byte-resolving verifier capability for partial/page/time/repository evidence; wrong key, truncation, reorder, splice, and wrong-domain detection; every crash-point closure outcome; cross-policy dedupe rejection; quarantine/GC dry run; and format N/N-1 read/migration.
