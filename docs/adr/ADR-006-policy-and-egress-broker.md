# ADR-006: Policy and egress broker

- Status: Proposed decision register

## Registered direction

The core owns a default-deny broker. A decision binds data class and exact byte/range digest, purpose, destination/provider, retention/training terms, policy version, user consent event, expiry, and one-time/replay constraints. `academic-policy` implements the socket-free decision half; the product graph still contains no egress crate. Its runtime entrypoint compares the actual payload digest and exact ranges, atomically changes `consumed_at` from null once, writes the runtime audit, and only then invokes its supplied tool closure. UI, plugin, worker, and provider libraries are expected to use this scoped entrypoint; the later `academic-egress` integration task owns the process/topology enforcement outside this crate.

Before external transmission, the broker stages the exact payload, applies allow/deny and secret/DLP rules, presents a byte-accurate preview when user approval is required, issues a narrow capability, and records allow or deny without copying sensitive content into the audit log. Scanner error, unknown binary, expired policy, or scope mismatch denies.

## Implemented now

There is no product network dependency or runtime egress. This is a smaller boundary than the final broker, not proof that egress controls are complete.

## Acceptance gate

Restricted data cannot reach a mock network without the exact grant; token range/provider/purpose/expiry and replay binding; no wildcard desktop HTTP capability; deterministic policy-version replay; complete allowed/denied audit; and prompt-injection content remaining data rather than instructions.
