# SQLCipher Phase 1 canary corpus

These values are deterministic, synthetic, unique plaintext sentinels for E1.
The harness writes every value through the exact S1 schema, exercises memory
temp storage and WAL, creates encrypted online-backup and crash copies, and
then scans every controlled artifact byte-for-byte. The corpus is public test
data and must never be replaced with personal, credential, or production data.
