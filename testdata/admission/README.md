# Incomplete admission receipt fixture

`incomplete-receipt.cbor.hex` is the lowercase hexadecimal encoding of one
canonical deterministic-CBOR Ed25519 envelope. Its signed payload contains the
first Windows row (`windows-x86_64`) and the second Linux row
(`linux-x86_64`), in that order. Each row carries its platform triple, build
digest, SQLCipher/SQLite/crypto-provider versions, zero-hit canary file and
byte counts, fault-matrix digest, and independent-restore digest.

The envelope is signed by the `cfg(test)` fixture key in
`academic-admission`; it is not the user's offline admission key and is not an
admission receipt that a product build accepts. Three required rows are absent,
and the compiled acceptance public key is unprovisioned, so the fixture proves
the intended denied state rather than five-platform admission. The repository
contains no user acceptance private key.
