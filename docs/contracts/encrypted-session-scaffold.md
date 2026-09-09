# Optional encrypted core session scaffold (D1)

This scaffold composes existing encrypted physical/key APIs. It does not admit
a synthetic service, personal data, or the X4/H1 end state. The default
plaintext profile, imported details, projection/portability operations and
daemon behavior remain on their existing branch.

## Feature and target matrix

| Selection | Supported targets | Store and consumers |
| --- | --- | --- |
| Default core | Existing library, unit and integration tests | `plaintext-core`, bundled SQLite, existing projections and portability |
| Default daemon | Existing library, `academicd` and all legacy tests | `plaintext-daemon` and the existing plaintext graph |
| Core with defaults off and `encrypted-synthetic-core` | Library, unit tests and doc tests | SQLCipher store, key schedule and AEAD vault; no plaintext projections, portability or imported detail fixture |
| Daemon with defaults off and `encrypted-synthetic-daemon` | Library only | Encrypted core; startup always refuses |
| `academic-encrypted-session-tests` with `encrypted-synthetic-tests` | Required-feature `encrypted_session` integration target | Optional existing normal dependencies on encrypted libraries and test composition APIs |

The new test host has an empty default, no build script or binary, no dependents,
and no external dependency addition. Its recipient and recovery material lives
only in the integration test. The core's encrypted unit-test material is under
`cfg(test)`.

Cargo features are additive. The store still refuses bundled SQLite together
with SQLCipher, and core/daemon also refuse both lanes together. Disabling
top-level defaults alone does not remove dependency defaults or dev edges.
In particular, encrypted daemon `--all-targets` is unsupported: its unchanged
legacy dev dependencies intentionally request plaintext fixtures, projections
and portability. Use the isolated test host. Policy checks inspect actual
package-selected normal/build/dev trees and inventory every existing target.

The host requests its four encrypted/AEAD dependency features only through
`encrypted-synthetic-tests`. Its optional dependency declarations request no
features themselves: a qualified workspace fault-feature selector can activate
an optional dependency independently of the host's test feature. Such activation
must preserve the plaintext fault lane. The policy regression selects the exact
existing CI workspace fault-feature union and rejects encrypted-lane activation.

## Session construction and lifetime

`EncryptedProfileSession::open` consumes a `VaultMasterKey` unlocked by trusted
host composition using the existing recipient APIs, the canonical crypto
`ProfileId`, distinct typed domain IDs, and independently supplied
`DeviceAuthorization` values. Empty or duplicate material refuses. An envelope
cannot supply its own trusted key. The session derives its store key and domain
KEKs internally, drops the VMK, opens the existing encrypted profile with the
key, and opens its concrete encrypted vault at that profile's root.

One non-Clone session owns one acceptance writer and the key material. It offers
no public acceptance/seeding command, raw connection, SQL method, writer handle,
arbitrary verifier or history injection. A reader factory borrows the session;
it cannot outlive it. Debug exposes only the domain count. Writer settings are
read-only diagnostics. Rust ownership excludes aliases to this owned writer;
it does not exclude another independent session open. D3 must acquire the
existing process singleton before any future usable service opens.

Canonical crypto `ProfileId`, typed domain IDs, and local incarnation have
different roles. The session reuses the existing 32-byte `detail-incarnation.v1`
marker through the store's synced create-new/no-follow helper, after a keyed
admitted reader succeeds even when the marker already exists. Same-root reopen
retains that marker. The DTO correlation digest includes the canonical ID,
local marker and root spelling; it is not authorization or synthetic readiness.
Restore/new-root behavior remains a separately checked boundary and is not
implemented by this scaffold.

Per-read incarnation validation uses a separate keyed, existing-marker read.
Missing, corrupt or changed metadata refuses without creating or repairing it.

The private normal-domain projector reuses T252's independently authenticated
history, selector and source checks. Plaintext acquisition keeps its existing
ordering. Its private fixed policy hash matches the existing plaintext registry
and fixed digest. The encrypted adapter verifies real sealed AEAD objects,
retains their verification receipt while reading, and revalidates around each
bounded range. Existing full returned-source digest checks remain in the
projector. This is read provenance, not recognized-corpus authentication.

## Startup refusal and remaining work

`academic_daemon::encrypted::start` always returns
`EncryptedSyntheticStartupUnavailable` before profile or runtime I/O. There is
no encrypted binary, listener, session metadata, capability publication,
handshake, plaintext-posture fallback, or ready-service token in this graph.
No public arbitrary-history marker can authenticate a synthetic service.

The following obligations remain separate:

- D2/T259: six-arm canonical store acceptance and complete normalized-frame closure.
- D3: the [third posture and unavailable RPC scaffold](encrypted-synthetic-domain-v1.md)
  are implemented; operational selected-service composition and host/singleton
  startup remain gated on complete recognized material.
- D4: closed recognized corpus and positive proof through the ordinary registration command.
- D5: encrypted portability and separately checked restore/new-root semantics.

Existing store-schema-2 history can be authenticated by this session, including
an empty concept index from scope-only history. That library result does not
establish six-arm registration acceptance, recognized corpus membership or
service readiness; D2/D3/D4 retain those obligations. No registration-command,
real-data, recovery-choice, signing/provisioning or ordinary-ingest service is
added. T251 provider selection and existing admission gates are unchanged.

## Focused verification

Run the current README required verification unchanged, plus these selected
targets. The existing pinned native SQLCipher/OpenSSL toolchain is required on
Windows; see [the toolchain contract](../../tools/sqlcipher/windows-toolchain.md).

```powershell
cargo clippy -p academic-core --no-default-features --features encrypted-synthetic-core --lib --tests --locked --offline -- -D warnings
cargo test -p academic-core --no-default-features --features encrypted-synthetic-core --lib --locked --offline
cargo test -p academic-core --no-default-features --features encrypted-synthetic-core --doc --locked --offline
cargo clippy -p academic-daemon --no-default-features --features encrypted-synthetic-daemon --lib --locked --offline -- -D warnings
cargo clippy -p academic-encrypted-session-tests --no-default-features --features encrypted-synthetic-tests --test encrypted_session --locked --offline -- -D warnings
cargo test -p academic-encrypted-session-tests --no-default-features --features encrypted-synthetic-tests --test encrypted_session --locked --offline
```

Tests use real profile/vault APIs, independently supplied authorization,
recovery unlock, absent/wrong-material refusal, same-root reopen, exact AEAD
source reads, borrowing/ownership compile failures, and startup refusal even
while a real keyed session is open. No in-memory or direct SQL fixture
substitutes for a session.
