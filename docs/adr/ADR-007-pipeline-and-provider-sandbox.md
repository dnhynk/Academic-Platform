# ADR-007: Pipeline and provider sandbox

- Status: Proposed decision register

## Registered direction

Pipelines are versioned DAG jobs over immutable input descriptors. Workers receive staged minimum inputs and one-time capabilities, write staged outputs, and cannot publish canonical artifacts or claims. Only the core verifies schema, provenance, resource receipts, and policy before appending output acceptance events.

Two worker tiers are anticipated: WASM for portable untrusted transformations and native out-of-process workers where codecs/ML require them. Platform containment is an adapter with measured Windows and Unix backends, not a claim that one sandbox primitive is equivalent everywhere. Network, home directory, raw vault, credentials, child processes, CPU, memory, time, and output size are denied or bounded by policy.

## Acceptance gate

Adversarial tests prove no home/vault read and no network; CPU/memory/time/output limits; malformed and malicious plugin corpus; dependency/advisory update owner and SLA; capability expiry/replay; and core-only staged-output acceptance. Phase 0 runs no worker.
