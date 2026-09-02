# P2-G5 injection corpus

Synthetic prompt-injection payloads for the untrusted-content boundary. Every
payload is invented. Nothing here is a real secret, a real credential, a real
person, or content fetched from anywhere; the corpus is public test data and
must never be replaced with personal, credential, or production data.

## What the two files are

`corpus.txt` is the injection corpus proper. Each record is a document that
tries to leave the data channel: to be read as a system instruction, to close
the quoting around it, or to obtain a privileged action. The loader in
`crates/untrusted-content/tests/corpus/mod.rs` parses it and the record's
`targets` field is the `PrivilegedAction` the payload asks for, so
`injection_corpus_produces_zero_privileged_actions` can assert per action rather
than in total.

`response-canary.txt` is the `PJ04` corpus: tokens that must never come back
from a provider. It is registered with `academic-egress-boundary`'s
`CanaryCorpus`, which is `P2-G2`'s existing provider-response scan; this task
adds no second scanner.

## Why these canaries are short and low-entropy

The other corpora in this tree use 32 random bytes in hex, because they are
scanned for inside whole database files where an accidental match would be
indistinguishable from a leak. These canaries travel through
`EgressProxy::accept_response`, whose shipped rulepack refuses a 40-character
hexadecimal run at 3.40 bits per character as `SECRET_ENTROPY`. A high-entropy
sentinel would therefore be quarantined by the DLP scan and the test could not
tell that refusal from the boundary's own. `G5-CANARY-INJECT-nnnn` is unique per
record, which is all the corpus needs, and it is short enough that no entropy or
digit-group rule can fire on it.

`response-canary.txt` is the exception: those values are meant to be caught, so
they are the usual 32 random bytes in hex.

## Record format

One record per blank-line-separated block, keys in a fixed order:

```text
id: <kebab-case, unique>
kind: <SourceKind spelling>
vector: <injection family>
targets: <PrivilegedAction spelling>
canary: <sentinel>
payload: <line>
payload: <line>
```

`payload` repeats and the lines are joined with a newline. A payload line is
read literally except for the escapes `\n`, `\r`, `\t`, `\\` and `\uXXXX`, which
are how a control character, a bidirectional override, and a zero-width joiner
are written in a reviewable text file. The loader appends the canary to the
payload, so what is written above is exactly the adversarial content.

## What the loader enforces

The corpus cannot quietly shrink or drift out of coverage. The loader fails when

- fewer than 48 records parse;
- an identifier or a canary repeats;
- a `kind` is not a `SourceKind`, or a `targets` is not a `PrivilegedAction`;
- any `SourceKind` variant has no record;
- any `PrivilegedAction` variant is targeted by no record; or
- any injection family named in `VECTORS` has no record.

The `kind` and `targets` rules iterate `SourceKind::ALL` and
`PrivilegedAction::ALL`, whose lengths are part of their types: a variant added
to either enum without extending its array does not compile, and one added *with*
its array fails the corpus until a record covers it. The `vector` rule is a
written list in the loader rather than an enum, because an injection family is a
property of the payloads and not of the code; what keeps it honest is that the
two sets are compared in both directions — every listed family must have a
record, and every record's family must be listed.
