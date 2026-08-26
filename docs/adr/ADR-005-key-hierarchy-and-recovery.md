# ADR-005: Key hierarchy and recovery

- Status: Proposed decision register

## Registered direction

Use a Vault Master Key wrapped by one or more recovery/device recipients, domain KEKs below it, and random artifact DEKs wrapped by the appropriate domain KEK. OS adapters use DPAPI/Keychain/Secret Service or an approved hardware-backed mechanism; general logs, models, UI code, plugins, and provider SDKs never receive root or provider credentials.

Password recovery, device recovery, and account recovery are distinct. Password-based recovery, if offered, uses a reviewed Argon2id profile and an explicit offline recovery recipient. Rotation normally rewraps keys; full data re-encryption is a versioned migration, not a silent overwrite. Revoking a device prevents future keys and objects but cannot pretend to erase plaintext already obtained by that device.

## Acceptance gate

OS reimage/lost device/lost password decision table; fresh-machine recovery; interrupted KEK rewrap; revoked-device future-access test; key-memory/lifecycle review; backup key independence; and UX that accurately states irrecoverability. No production key material exists in Phase 0.
