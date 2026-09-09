# macOS device keystore

`MACOS_KEYCHAIN_DATA_PROTECTION_V1` implements the private native leaf used by
`academic-crypto/os-keystore`. It stores a synthetic or admitted 32-byte device
wrapping key as a generic-password item; the VMK never enters this leaf through
the recipient path. The historical generic safe API still accepts 1–4096 bytes
and a 1–128 byte validated ASCII label. No native object, pointer, arbitrary
credential query, keychain selector or access-group selector crosses that API.
This implementation alone does not admit H1 or production data.

## Selection and identity

The minimum API level is macOS 10.15. Every operation uses SecItem with
`kSecUseDataProtectionKeychain = true`, `kSecAttrSynchronizable = false` and
`kSecAttrAccessibleWhenUnlockedThisDeviceOnly`. There is no file-based keychain
fallback, default search-list change, iCloud synchronization, item replacement,
keychain unlock or biometric evaluation. This follows the per-user-session
daemon in ADR-001 and t068 §3.3. It cannot run as a system launchd daemon.

Apple's [TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)
requires a signed-in user context and authorized main-executable entitlements
for the data-protection keychain. Signing the library is insufficient. The
daemon and test executable need an app-like package, a provisioning profile
authorizing their application identifier and keychain access group, and signing
that carries those entitlements. No such provisioned project identity or
disposable signed-in macOS test account has been supplied for this task.
An ordinary Cargo test binary is not evidence of that identity.

Add uses the executable's default authorized access group; copy/delete use its
authorized access groups, always narrowed by the exact service, account and
random generation. Deployment must keep the same default group across daemon
versions, and must not give unrelated software that group. Changes to identity
or access groups can make persisted keys inaccessible; the recovery recipient
remains the recovery route. Packaging and distribution review are separate
prerequisites, not configuration the safe facade accepts from callers.

## Item and blob contracts

The fixed service is `dev.academic-os.device-wrapping-key.data-protection.v1`;
the account is the validated label. `SecItemAdd` is the sole creation operation:
the primary-key collision returns redacted `DuplicateLabel` and never updates
or deletes the original. Each creation has a random 32-byte non-secret generation
stored in `kSecAttrGeneric`, which is not part of the primary key.

Existing AKSB envelope version 1 and Windows/Linux provider tags 1/2 and
payloads remain unchanged. macOS uses provider tag 3 and payload
`01 || u16-le(label length) || label || generation[32]`. The blob contains no
secret bytes. Exact lengths, version, provider and label are checked before any
native call. A changed generation cannot retrieve or delete the original item.

Items persist beyond releasing all local CF/Objective-C objects and beyond
process exit, subject to the user's Keychain and signing identity. `purge`
deletes only the exact generation and reports `Removed`; a missing generation
reports `NothingStored`. The next open fails `NotFound`. Purging an old blob
after a new key is explicitly created at the same label cannot delete that new
generation. Concurrent open may already have obtained a copy when purge occurs;
purge cannot erase previously recovered memory, backups, or ciphertext keys.
This differs from Windows' stateless DPAPI-CNG blob, which purge cannot revoke.

## Recipient publication and restart

An add-only item must never become durable before the caller has its exact
generation identity. `prepare_seal` creates a private, non-Clone `PreparedSeal`
without native mutation. Its blob is available before its consuming `seal`
method; persisted bytes cannot reconstruct that token or reseal an old
generation. The low-level legacy `seal` facade remains available to existing
native callers, but provides no interrupted-publication recovery on its own.
Storage callers that need recovery must use preparation and durable staging.

`academic-crypto::create_recoverable_device_recipient` prepares that identity
and completes device-key generation, AEAD wrapping (including the fallible
nonce), record encoding for the MAC, and the MAC before native add. It passes
the complete encrypted `RecipientRecord` to a required staging callback. That
callback receives no plaintext VMK or device wrapping key and must durably
record an **incomplete** publication before returning success. This crypto
helper writes no files and supplies no automatic fsync or storage durability
guarantee; the storage caller owns those requirements. It must exclusively own
the attempt, refuse an outstanding journal instead of overwriting it, and
serialize publication, cleanup and retry.

If staging returns an error, no native seal is attempted, even if the staging
write partially succeeded. After staging succeeds, the complete record must
remain durable through native errors, KY08, and final publication. A seal error
can be ambiguous: do not infer absence or discard the record from the error
category. After successful creation the returned record is ready for the
caller's final publication; returning it does not establish that publication.
Only confirmed durable final publication or confirmed exact cleanup permits
resolving the incomplete attempt. Cleanup refusal retains its record for retry;
there is no automatic Drop rollback that could lose its only identity.

On restart, first reconcile the incomplete record against final publication.
For a still-incomplete attempt, decode its canonical record and either verify
and open it with `unlock_with_device` before final publication, or call
`purge_incomplete_device_recipient` with the expected profile and provider.
`Removed`/`NothingStored` (true/false in crypto) resolves only that generation,
after which the caller may prepare a fresh attempt. An old or duplicate
attempt's cleanup cannot remove an existing different generation. Never
reconstruct/reseal the old token, discover a generation, delete by label alone,
or treat a cleanup error as absence. A record already published is outside
incomplete cleanup authority; it must follow the separate revocation contract.

The legacy crypto one-step helper returns `PublicationJournalRequired` before
mutation for macOS. Existing `DeviceKeystore` implementations remain source
compatible through a default false requirement flag, while add-only brokers
must opt into the recoverable extension. Windows stateless sealing and Linux's
historical replacement behavior and envelopes remain unchanged. Their existing
one-step helper does not acquire this new recovery guarantee.

The platform-neutral persistent/add-only fixture stores synthetic broker state
separately from the caller journal and reconnects after discarding creator
state. It checks staging refusal, ambiguous add errors, interruption before and
after add, failed final publication, cleanup refusal/retry, duplicates and stale
identities. These bounded model tests establish the API ordering and recovery
contract, not OS crash/power-loss durability. The historical in-memory KY08
termination fixture establishes no persistent-store cleanup guarantee. The
ignored positive macOS recipient harness now durably stages its record and
retains the journal even on error; provisioned execution is still absent.

## Refusal, threading and memory

SecItem calls block, so this synchronous facade refuses main-thread calls with
`Unavailable`; the caller must use a background worker. Every query has a fresh
`LAContext` with `interactionNotAllowed = true`. Locked or denied access fails
closed without prompting; unavailable service or missing entitlement reports
`Unavailable`, interaction/authentication denial reports `AccessDenied`, and
other failures carry only a category, constant operation and numeric OSStatus.
No fallback key or silently replaced item can make an unlock succeed.

The dictionary owns no value callbacks and is dropped before the retained
values it borrows, including LAContext. Each unsafe call is locally allowed
with its lifetime, type and ownership proof; workspace foreign declarations
and global unsafe allowances are unchanged. Copy results are adopted once,
released by RAII, type-checked as CFData, and bounded before Rust allocation.
The Rust returned buffer and our outgoing mutable CF buffer are zeroized on
drop. Immutable CFData returned by Apple and internal framework/IPC copies
cannot safely be overwritten; this is an explicit memory-review limitation.

Generic-password storage is **not Secure Enclave or hardware-backed evidence**.
The [Secure Enclave key API](https://developer.apple.com/documentation/security/protecting-keys-with-the-secure-enclave)
generates its own supported asymmetric keys; adding SecItem calls for an
existing generic secret does not establish that property.

## Native execution prerequisites

The bounded hosted workflow executes only API/type/format tests and an explicit
unprovisioned-identity refusal test. A refusal, a build, and an ignored positive
test are never a positive broker result. Its unsigned native supplement retains
named executions, raw exits/logs, source and binary identity, and missing
positive, packaging and hardware claims. Historical unsigned encrypted H1 v2
archives and their validation contract remain unchanged.

`h1-native-keystore-evidence` version 1 is a separate unsigned supplement.
It reconciles the full planned compiler executable set, source paths, features,
test profiles, retained Mach-O bytes and each named invocation's before/after
executable hashes. The bundle retains failed command logs and explicit `notRun`
entries; `refusal_and_api_checks_passed` describes only the executed refusal and
API checks. Setup failure before collection can leave no bundle and must be
reported from the hosted job log as missing artifact evidence. Validate with
`node tools/macos-keystore-evidence.mjs validate BUNDLE COMMIT RUN_ID ATTEMPT REPOSITORY`
against the externally verified run/archive identity and exact Git commit.
An unsigned bundle has no authenticity without that external provenance.

Positive acceptance requires a separately reviewed execution environment:

1. A disposable VM and signed-in synthetic local user, no personal keychain
   content or imported secrets, and an existing authorized development identity.
2. A provisioned app-like test harness with the exact main-executable identity,
   entitlement/access-group observation, OS/architecture and binary digest
   recorded. Never attach a signing key, profile, credential or Keychain database
   to evidence. No signing or provisioning action is part of the current lane.
3. Explicit named tests for seal/open, repeated reopen, foreign label/provider,
   corrupt blob, duplicate refusal preserving the original, purge and repeat
   purge, stale-blob isolation after label reuse, and reopen from a new process
   after the writer exited. Cleanup must use only the task's returned blobs and
   exact labels, including on test failure; no enumeration or bulk deletion.
4. A separate controlled locked/unavailable run in that disposable context,
   preserving failure results and proving no prompt or fallback. Do not lock,
   unlock, or modify a personal Keychain for this evidence.
5. Independent native/FFI review before merge. Native positive execution,
   process packaging, distribution signing, hardware protection, memory/crash
   inspection and full H1 admission remain separate unfulfilled claims until
   their actual evidence is retained.

Primary API sources read for this implementation: [SecItemAdd](https://developer.apple.com/documentation/security/secitemadd(_:_:)),
[SecItemCopyMatching](https://developer.apple.com/documentation/security/secitemcopymatching(_:_:)),
[SecItemDelete](https://developer.apple.com/documentation/security/secitemdelete(_:)),
[generic-password class](https://developer.apple.com/documentation/security/ksecclassgenericpassword),
[accessibility](https://developer.apple.com/documentation/security/ksecattraccessiblewhenunlockedthisdeviceonly),
[authentication context](https://developer.apple.com/documentation/security/ksecuseauthenticationcontext),
[noninteractive context](https://developer.apple.com/documentation/localauthentication/lacontext/interactionnotallowed),
and [CFDictionary ownership](https://developer.apple.com/documentation/corefoundation/cfdictionarycreate(_:_:_:_:_:_:)).
