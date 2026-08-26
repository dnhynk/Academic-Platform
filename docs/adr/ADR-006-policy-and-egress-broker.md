# ADR-006: Policy and egress broker

- Status: Proposed decision register

## Registered direction

The core owns a default-deny broker. A decision binds data class and exact byte/range digest, purpose, destination/provider, retention/training terms, policy version, user consent event, expiry, and one-time/replay constraints. UI, plugin, worker, and provider libraries cannot open generic network connections or broaden a grant.

Before external transmission, the broker stages the exact payload, applies allow/deny and secret/DLP rules, presents a byte-accurate preview when user approval is required, issues a narrow capability, and records allow or deny without copying sensitive content into the audit log. Scanner error, unknown binary, expired policy, or scope mismatch denies.

## Implemented now

There is no product network dependency or runtime egress. This is a smaller boundary than the final broker, not proof that egress controls are complete.

## Acceptance gate

Restricted data cannot reach a mock network without the exact grant; token range/provider/purpose/expiry and replay binding; no wildcard desktop HTTP capability; deterministic policy-version replay; complete allowed/denied audit; and prompt-injection content remaining data rather than instructions.
