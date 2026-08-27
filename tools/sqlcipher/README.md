# SQLCipher Phase 1 evidence collector

`collect-evidence.mjs` is the local-only entry point for E1 evidence. It checks
the exact Rust 1.98.0, Node 24.19.0, and pnpm 11.22.0 toolchains; verifies the
plaintext default posture; runs the seven named SQLCipher tests; runs the
synthetic artifact harness; and writes hashes and source-admission facts to a
new JSON receipt. On the admitted Linux evidence host it also requires the
`sqlcipher_version`, `sqlite3_key`, `sqlite3_key_v2`, and `sqlite3_rekey_v2`
symbols in the compiled binary through the local `nm` tool.

Run it only with the already admitted and cached dependency set:

```sh
node tools/sqlcipher/collect-evidence.mjs \
  --artifact-root /absolute/new/path/sqlcipher-artifacts \
  --receipt /absolute/new/path/sqlcipher-evidence.json
```

Both output paths must be outside the repository and must not exist. Every
Cargo invocation is `--locked --offline`, and `CARGO_NET_OFFLINE=true` is set
for child processes. The collector never fetches, installs, queries an advisory
service, or accepts ADR-002; it fails if the default binary claims encryption
or production readiness, or if the harness finds a plaintext canary.

On a host that cannot compile the admitted native SQLCipher/OpenSSL sources,
the collector exits nonzero and leaves that host as an explicit evidence gap.
It does not substitute the plaintext SQLite build or treat a compile limit as
passing runtime evidence.
