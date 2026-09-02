# ADR-006: Policy and egress broker

- Status: Proposed decision register

## Registered direction

The core owns a default-deny broker. A decision binds data class and exact byte/range digest, purpose, destination/provider, retention/training terms, policy version, user consent event, expiry, and one-time/replay constraints. `academic-policy` implements the socket-free decision half and the closed P2-G7 process/capability matrix. Its runtime entrypoints compare actor, typed process class, capability, actual payload digest, and exact ranges; atomically change `consumed_at` from null once; write the runtime audit; and only then release authority. Six minimal executable packages establish the capture-client, indexer, repository-analyzer, connector, egress-proxy, and export-job process identities. UI, plugin, worker, and provider libraries are expected to use these scoped entrypoints; P2-G2 still owns the actual egress transport and P2-G4 owns platform sandbox enforcement.

Before external transmission, the broker stages the exact payload, applies allow/deny and secret/DLP rules, presents a byte-accurate preview when user approval is required, issues a narrow capability, and records allow or deny without copying sensitive content into the audit log. Scanner error, unknown binary, expired policy, or scope mismatch denies.

## Implemented now

There is no product network dependency or runtime egress. Only the egress process class may receive `OPEN_OUTBOUND_SOCKET`; the egress executable does not yet open one. Whole-closure and whole-source checks hold the indexer's and export-job's dependency closures to a reviewed set that contains no network or key-material crate, and hold each entrypoint to one fixed process-class binding that reaches for neither. The standard library still puts a raw socket and a file read within reach of any process, so those checks bound what a package depends on and what its source uses, not what the operating system permits it. This is still smaller than the final broker and is not proof that egress or platform sandbox controls are complete; `P2-G4` owns the sandbox half.

## Acceptance gate

Restricted data cannot reach a mock network without the exact grant; token range/provider/purpose/expiry and replay binding; no wildcard desktop HTTP capability; deterministic policy-version replay; complete allowed/denied audit; and prompt-injection content remaining data rather than instructions.
