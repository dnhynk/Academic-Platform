# ADR-004: Artifact vault format

- Status: Proposed decision register

## Registered direction

Artifact identity is SHA-256 over exact plaintext bytes plus descriptor metadata (`media_type`, length, encryption domain, retention class, permission lineage, format version). The logical digest is stored only inside encrypted metadata. A physical locator is `HMAC-SHA-256(domain_locator_key, format_version || media_type || 0x00 || plaintext_digest)` in a domain namespace; a locked directory listing must not expose global plaintext equality.

Deduplication is permitted only when encryption domain, retention class, and permission lineage all match. Global/convergent deduplication is rejected. Evidence locators include the source digest and exact byte/time/page/repository coordinate so a changed source cannot silently satisfy an old claim.

The eventual object format will use random per-artifact DEKs, domain KEK wrapping, versioned headers, independently authenticated chunks, and AAD binding object identity and chunk position. AEAD algorithm and chunk size remain acceptance-gated.

## Implemented now

Algorithm-prefixed digest and keyed locator newtypes, domain-key separation tests, byte-length/media metadata, and exact evidence locator validation are implemented. Each usable locator is bound to immutable `ArtifactRepresentation` metadata containing the exact locator, representation digest, and byte length. Text spans must remain within the registered source bytes; page, time, and repository locators fail closed unless an exact representation is registered. Evidence acceptance also enforces artifact/event domain closure. No encrypted object writer exists.

## Acceptance gate

Zero-byte/small/multi-GB/seekable-audio vectors; wrong key, truncation, reorder, splice, and wrong-domain detection; every crash-point closure outcome; cross-policy dedupe rejection; quarantine/GC dry run; and format N/N-1 read/migration.
