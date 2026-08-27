# Doctor contract

`academic doctor --format human|json` (or `pnpm run doctor`) is a local, privacy-safe prerequisite check. It records tool version, resolved executable path when available, whether the pin is supported, and a reproducible remediation command. Use `pnpm run doctor` exactly; bare `pnpm doctor` is pnpm's own network-capable diagnostic command, not this repository script.

The command does not inspect documents, repositories outside the checkout, credentials, audio devices, browser state, cloud accounts, or network endpoints. It performs no network request. A successful result means only that Phase 0 build tools match their pins; it does not mean ADR-002 encrypted storage or later security gates are accepted.

Expected JSON invariants:

- `ready` is true only when every pin matches.
- `data_policy` is `SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED`.
- `network_egress` is `PRODUCT_RUNTIME_NONE`.
- missing or mismatched tools include a remediation string and make the command fail.
- Node and pnpm output equals the exact pin token; rustc and cargo may add only ordinary parenthesized stable commit/date metadata after their exact token. Prerelease, nightly, longer-prefix, and wrapper spellings are mismatches. Bootstrap and doctor execute the same committed conformance corpus for these decisions.
