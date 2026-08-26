# ADR-004: Artifact vault format

- Status: Proposed decision register

## Registered direction

Artifact identity is SHA-256 over exact plaintext bytes plus descriptor metadata (`media_type`, length, encryption domain, retention class, permission lineage, format version). The logical digest is stored only inside encrypted metadata. A physical locator is `HMAC-SHA-256(domain_locator_key, format_version || media_type || 0x00 || plaintext_digest)` in a domain namespace; a locked directory listing must not expose global plaintext equality.

Deduplication is permitted only when encryption domain, retention class, and permission lineage all match. Global/convergent deduplication is rejected. Evidence locators include the source digest and exact byte/time/page/repository coordinate so a changed source cannot silently satisfy an old claim.

The eventual object format will use random per-artifact DEKs, domain KEK wrapping, versioned headers, independently authenticated chunks, and AAD binding object identity and chunk position. AEAD algorithm and chunk size remain acceptance-gated.

## Implemented now

Algorithm-prefixed digest and keyed locator newtypes, domain-key separation tests, byte-length/media metadata, and exact evidence locator validation are implemented. A complete `TEXT_BYTES 0..artifact.byte_length` representation must use the artifact content digest for both source and representation, cryptographically closing the excerpt to the registered bytes. Partial text, page, transcript-time, and repository representations remain valid descriptor vocabulary, but Phase 0 event/evidence acceptance fails closed because no byte-resolving verifier capability exists; an actor label alone is not trusted proof of resolved bytes. Evidence acceptance also enforces artifact/event domain closure and never compares two caller-controlled digests as proof. No encrypted object writer exists.

The artifact JSON boundary first parses the raw text with unique decoded property names, Unicode-scalar-only strings, and canonical unsigned integer lexemes. It then rejects unsafe integers and nonportable paths at schema level and executes a semantic post-validator for ranges, artifact bounds, span lengths, source/full-range digest binding, and locator-identity uniqueness. Rust and Ajv/TypeScript run the same committed structured and exact-raw mutation corpus, including duplicate names, lone surrogates, and positive text/page/time/repository Unicode descriptors.

## Acceptance gate

Zero-byte/small/multi-GB/seekable-audio vectors; a trusted byte-resolving verifier capability for partial/page/time/repository evidence; wrong key, truncation, reorder, splice, and wrong-domain detection; every crash-point closure outcome; cross-policy dedupe rejection; quarantine/GC dry run; and format N/N-1 read/migration.
