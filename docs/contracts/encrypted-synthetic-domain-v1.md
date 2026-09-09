# Encrypted synthetic posture and unavailable service scaffold (D3)

This is the bounded D3 amendment to T068 section 2.3-13 and its old two-posture
representation. It adds a truthful encrypted, non-admitted description without
making a service available. D4 recognized material is absent. Opening a keyed
D1 session or authenticating a scope-only empty index cannot satisfy that gate.

## Exact identity and compatibility matrix

| Identity | Data policy / production permission / network | Storage identity | Local IPC vault and availability |
| --- | --- | --- | --- |
| Legacy synthetic | `SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED` / false / `NONE` | `PLAINTEXT_TEMPORARY_SQLITE`, encryption `NONE`, schema 1 / `1.0.0`, absent object format | Exact legacy `PLAINTEXT_SYNTHETIC_V1` read/write fields and existing negotiation |
| Legacy admitted | `REAL_PERSONAL_DATA_PERMITTED` / true / `BROKERED_EGRESS_ONLY` | `SQLCIPHER_ENCRYPTED_PROFILE_V2`, schema 2 / `2.0.0`, object `AEAD_CHUNKED_V2` | Exact legacy `PLAINTEXT_SYNTHETIC_V1` read/write fields remain; no encrypted service identity claim |
| Encrypted synthetic | `SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED` / false / `NONE` | `SQLCIPHER_ENCRYPTED_PROFILE_V2`, schema 2 / `2.0.0`, object `AEAD_CHUNKED_V2` | Only `AEAD_CHUNKED_V2` read/write identity, locked, no projections, unavailable |

Both encrypted descriptions use the existing exact encryption spelling
`SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000`. Only admitted carries a receipt
digest and required platforms. Its authority still comes exclusively through
`Posture::from_verified(&VerifiedAdmission)` and the unchanged compiled-key
verifier. No marker, flag, feature, wire value or synthetic description grants
real-data permission. The legacy admitted vault discrepancy is preserved
compatibility behavior, not evidence of encryption in the selected default service.

The new canonical JSON has exactly these fields, in this order:

```json
{"data_policy":"SYNTHETIC_FIXTURES_ONLY_UNTIL_ADR_002_ACCEPTED","storage_mode":"SQLCIPHER_ENCRYPTED_PROFILE_V2","storage_encryption":"SQLCIPHER_4_AES_256_CBC_HMAC_SHA512_PBKDF2_256000","object_format":"AEAD_CHUNKED_V2","production_data_allowed":false,"product_network":"NONE"}
```

Existing synthetic and admitted canonical JSON and legacy Proto goldens remain
byte-exact. Schema 3 is not introduced. `storage_schema_for(bool)` retains its old
public behavior; the current emitter and validator use three explicit identities.
Validation classifies the complete wire posture, including canonical bytes,
before selecting schema/vault expectations and never constructs admission authority.

## Explicit negotiation, no command permission

`ServerHandshakeConfig::encrypted_synthetic_scaffold()` selects only a pure RPC
description. Same-major compatible clients must explicitly request
`academic.encrypted-synthetic-posture.v1`. This is understanding of this identity
contract, not the future operational `academic.encrypted-synthetic-domain.v1`
capability. Protocol `learning-platform.local-core` stays at 1.0, minimum 1.0.

The reply has exactly the identity-support capability, `Locked`, no projections,
`WRITE_DISPOSITION_DENIED_SERVICE_UNAVAILABLE = 16`, and exact reason
`ENCRYPTED_SYNTHETIC_SERVICE_UNAVAILABLE`. Enum values 0 through 4, reserved
values 5 through 15, and every existing field tag remain unchanged. No domain
read, diagnostics, export, import, backup, restore or write capability is granted.
Configuration with an unlocked/repair-required state or any projection refuses.
Missing opt-in, incompatible major or any unknown requested capability refuses
the whole scaffold handshake. Mixed posture fields, authority fields, schema,
vault, capabilities, disposition and reason refuse in both encoder and decoder.
Legacy replies may carry neither the new support capability nor disposition 16.

Current legacy RPC recognizes the new support token without returning it, so
explicit opt-in does not change its existing response to otherwise identical
inputs. Unknown unrelated tokens retain the existing write-denial behavior.
Older RPC implementations may refuse an unknown opt-in token. Ordinary desktop
hellos do not add it: existing v3, imported v1/v2 and receipt bytes remain exact,
preserving new-client/old-plaintext-server behavior with no replay, downgrade or
retry. Tests exercise actual daemon nonce removal and its missing/stale/duplicate
refusals before negotiation. A bounded native duplex test sends an unsolicited
exact scaffold through the production decoder and client guard and observes EOF
before command bytes. That test is refusal evidence, not a successful encrypted
service negotiation or a production listener.

## Startup and remaining gates

`academic_daemon::encrypted::start` still returns
`Result<Infallible, EncryptedSyntheticStartupUnavailable>` unconditionally before
profile/runtime I/O, singleton acquisition, listener creation or metadata/nonce
publication. Its tests refuse absent paths, invalid paths, an empty path and a
real open keyed D1 session. There is no ready token, alternate writer, seeder,
arbitrary-history verifier, corpus label or environment override.

D2 six-arm registration acceptance and complete normalized-frame closure, D4
recognized source material and positive ordinary registration-command proof,
and D5 encrypted portability/restore remain distinct obligations. A future usable
service must reconcile actual keyed storage/session identity, complete source
authorization, existing path rules and process singleton before publication.
This scaffold claims none of those positive results and leaves X4/H1 incomplete.
Imported sources, provenance/actor versions, signed historical bytes, store SQL,
acceptance, provider choices, crypto and retention semantics remain unchanged.

Verification includes the unchanged README command block, legacy and encrypted
posture/codec tests, actual nonce and native refusal tests, and the selected D1
encrypted target/graph matrix in the [session contract](encrypted-session-scaffold.md).
No dependency, target identity or feature-host edge is added by D3.
