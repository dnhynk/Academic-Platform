# Contributing

Phase 0 changes must preserve the following review rules:

1. Use only synthetic fixtures. Never add personal records, lecture media, private repository content, secrets, tokens, or externally fetched payloads.
2. Canonical events, claims, evidence links, and decisions are append-only. A correction is a new event plus an explicit relation or decision.
3. Keep origin order, local acceptance order, and valid time separate in types, fixtures, and tests.
4. Do not add runtime network dependencies or a cloud-required test path.
5. Update a golden fixture only through the deterministic builder and explain the semantic change in the relevant ADR.
6. Run every command in the README verification block before commit.

Dependency additions require an owner, license review, feature review, advisory path, and an explanation of why the dependency belongs inside its trust boundary. Git dependencies and insecure package tarballs are rejected by the structural `pnpm security` lock parser; update its source-encoding fixtures with any pnpm lock-format change.
