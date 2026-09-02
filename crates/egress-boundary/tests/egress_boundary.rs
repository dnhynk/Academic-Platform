//! The named `P2-G2` acceptance evidence, minus the socket scan.
//!
//! `only_egress_crate_has_a_socket` is a workspace-wide source and link scan and
//! lives in `tools/phase1-scaffold-policy.test.mjs`, beside the network gate it
//! narrows and beside the `os-keystore` by-crate precedent that `t068` section
//! 2.3-14 names. Every other named row is here, and the byte-path pins that
//! keep `preview_bytes_equal_transmitted_bytes` from becoming a coincidence are
//! in `byte_path_pin.rs`.

mod common;

use std::error::Error;

use academic_egress_boundary::{
    EgressError, EgressProxy, IdentifierPolicy, OutboundTransport, Route, Rulepack, SourceDocument,
    StagedGrantJournal, StagedPayload, Transmission, TransmissionPlan, TransportError,
    cloud_egress_default,
};
use academic_policy::{CapabilityToken, ContentDigest, Decision, ProcessClass, ReasonCode};

use common::TestResult;

/// A transport that keeps every byte it was handed, so the test can compare.
#[derive(Debug, Default)]
struct RecordingSink {
    written: Vec<u8>,
    chunks: usize,
}

impl OutboundTransport for RecordingSink {
    fn send_chunk(&mut self, chunk: &[u8]) -> Result<(), TransportError> {
        self.written.extend_from_slice(chunk);
        self.chunks = self.chunks.saturating_add(1);
        Ok(())
    }
}

/// A transport that refuses the first write. Nothing may reach it afterwards.
#[derive(Debug, Default)]
struct RefusingSink {
    calls: usize,
}

impl OutboundTransport for RefusingSink {
    fn send_chunk(&mut self, _chunk: &[u8]) -> Result<(), TransportError> {
        self.calls = self.calls.saturating_add(1);
        Err(TransportError::WriteFailed {
            sent: 0,
            detail: "synthetic destination refused".to_owned(),
        })
    }
}

/// Takes the refusal out of a result, failing the test when there was none.
///
/// `expect_err` would do it, but `clippy::expect_used` is denied workspace-wide
/// and a test is not the place to make an exception to a lint that exists so a
/// deterministic engine never panics.
fn refused<T: std::fmt::Debug, E>(result: Result<T, E>, what: &str) -> Result<E, Box<dyn Error>> {
    match result {
        Ok(value) => Err(format!("{what} was not refused: {value:?}").into()),
        Err(error) => Ok(error),
    }
}

const MAX_BYTES: u64 = 4_096;

/// The two public functions that reach a transport, so a refusal can be
/// asserted over both rather than over whichever one a test happened to pick.
///
/// `T146` found the second one skipping the grant read and the rulepack
/// comparison the first one makes. A test written against `transmit` alone
/// observes nothing about it, which is how that gap survived; the tests that
/// say "every transmit path" iterate this table instead.
type TransmitFn = fn(
    &EgressProxy<'_>,
    &CapabilityToken,
    &StagedPayload,
    &TransmissionPlan<'_>,
    &mut StagedGrantJournal,
    &mut RecordingSink,
) -> Result<Transmission, EgressError>;

struct TransmitPath {
    name: &'static str,
    send: TransmitFn,
}

const TRANSMIT_PATHS: [TransmitPath; 2] = [
    TransmitPath {
        name: "transmit",
        send: |proxy, capability, staged, plan, journal, sink| {
            proxy.transmit(capability, staged, plan, journal, sink, &|| 1_000)
        },
    },
    TransmitPath {
        name: "transmit_without_completion",
        send: |proxy, capability, staged, plan, journal, sink| {
            proxy.transmit_without_completion(capability, staged, plan, journal, sink, &|| 1_000)
        },
    },
];

// ---------------------------------------------------------------------------
// The byte path
// ---------------------------------------------------------------------------

/// The preview the user reads is the buffer the transport writes.
///
/// Three independent statements, because one of them alone would be a
/// coincidence rather than an invariant. The recorded bytes equal
/// `preview().bytes()`. The grant the broker minted carries the preview's
/// digest, so a payload that is not the preview is refused at the capability
/// boundary by code this crate does not own. And the two functions that carry
/// the payload are pinned as whole text in `byte_path_pin.rs`, so a second
/// derivation cannot be introduced without changing that pin.
#[test]
fn preview_bytes_equal_transmitted_bytes() -> TestResult {
    let (broker, provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::new(vec!["weights".to_owned(), "weight".to_owned()], 60);
    let staged = proxy.stage(&common::staging_request(
        &document, &focus, &policy, MAX_BYTES,
    ))?;

    let expected = staged.preview().bytes().to_vec();
    assert!(!expected.is_empty(), "the staged payload has no bytes");

    let hash = proxy.rulepack_id().redaction_policy_hash().clone();
    let outcome = common::capability_for(&broker, &staged, &provider, hash, 1_000)?;
    let (capability, grant_id) = common::token(outcome)?;

    let grant = broker
        .grant_row(&grant_id)?
        .ok_or("the broker allowed without writing a grant row")?;
    assert_eq!(
        grant.payload_digest,
        staged.preview().digest().as_str(),
        "the grant was minted over bytes other than the previewed ones"
    );

    let mut sink = RecordingSink::default();
    let mut journal = StagedGrantJournal::new();
    let plan = TransmissionPlan {
        grant_id: &grant_id,
        actor_id: common::EGRESS_ACTOR,
        process_class: common::EGRESS_CLASS,
        operation: "classify",
        purpose_id: "architecture-classification",
        destination_id: provider.destination_id(),
        expires_at: u64::MAX,
        chunk_bytes: 7,
    };
    let transmission = proxy.transmit(
        &capability,
        &staged,
        &plan,
        &mut journal,
        &mut sink,
        &|| 1_000,
    )?;

    assert_eq!(
        sink.written, expected,
        "the transport wrote bytes the preview does not hold"
    );
    assert_eq!(transmission.bytes_sent(), expected.len());
    assert_eq!(
        transmission.payload_digest(),
        staged.preview().digest().as_str()
    );
    assert!(sink.chunks > 1, "the chunked path was not exercised");

    // The preview names exactly what a reader needs to check the bytes: the
    // source ranges they came from and every identifier that was replaced.
    let rendered = staged.preview().render();
    assert!(rendered.contains(staged.preview().digest().as_str()));
    for substitution in staged.preview().substitutions() {
        assert!(rendered.contains(substitution.original()));
        assert!(rendered.contains(substitution.placeholder()));
        let placeholder = staged
            .preview()
            .bytes()
            .get(substitution.staged_start()..substitution.staged_end())
            .ok_or("a substitution range falls outside the staged bytes")?;
        assert_eq!(
            placeholder,
            substitution.placeholder().as_bytes(),
            "the preview's staged range does not hold the placeholder it names"
        );
    }
    assert!(
        !staged.preview().substitutions().is_empty(),
        "the fixture substituted nothing, so the range check proved nothing"
    );
    Ok(())
}

/// A transmission from any process class but the egress proxy is refused.
///
/// `P2-G7` gives `ProcessCapability::OpenOutboundSocket` to `EgressProxy` and to
/// no other class, and `RuntimeToolCall::new` refuses a call whose class does
/// not hold it. This crate carries the class through `TransmissionPlan`, so the
/// refusal happens before the capability boundary and before any byte is
/// offered to a transport.
#[test]
fn a_transmission_from_another_process_class_is_refused() -> TestResult {
    let (broker, provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();
    let staged = proxy.stage(&common::staging_request(
        &document, &focus, &policy, MAX_BYTES,
    ))?;
    let hash = proxy.rulepack_id().redaction_policy_hash().clone();
    let outcome = common::capability_for(&broker, &staged, &provider, hash, 1_000)?;
    let (capability, grant_id) = common::token(outcome)?;

    let mut journal = StagedGrantJournal::new();
    for class in ProcessClass::ALL {
        if class == common::EGRESS_CLASS {
            continue;
        }
        let plan = TransmissionPlan {
            grant_id: &grant_id,
            actor_id: common::EGRESS_ACTOR,
            process_class: class,
            operation: "classify",
            purpose_id: "architecture-classification",
            destination_id: provider.destination_id(),
            expires_at: u64::MAX,
            chunk_bytes: 8,
        };
        let mut sink = RecordingSink::default();
        let error = refused(
            proxy.transmit(
                &capability,
                &staged,
                &plan,
                &mut journal,
                &mut sink,
                &|| 1_000,
            ),
            &format!("a transmission from {class:?}"),
        )?;
        assert_eq!(
            error.reason(),
            Some(ReasonCode::ScopeMismatch),
            "{class:?} was refused as {:?}",
            error.reason()
        );
        assert!(sink.written.is_empty(), "{class:?} wrote bytes");
    }

    // The egress proxy class, everything else equal, still transmits, so the
    // refusals above came from the class and not from the rest of the plan.
    let plan = TransmissionPlan {
        grant_id: &grant_id,
        actor_id: common::EGRESS_ACTOR,
        process_class: common::EGRESS_CLASS,
        operation: "classify",
        purpose_id: "architecture-classification",
        destination_id: provider.destination_id(),
        expires_at: u64::MAX,
        chunk_bytes: 8,
    };
    let mut sink = RecordingSink::default();
    proxy.transmit(
        &capability,
        &staged,
        &plan,
        &mut journal,
        &mut sink,
        &|| 1_000,
    )?;
    assert_eq!(sink.written, staged.preview().bytes());
    Ok(())
}

/// A grant reviewed under another rulepack carries nothing, on either path.
///
/// `EgressProxy` has two public functions that reach a transport, and `T146`
/// measured the difference between them: with a grant whose recorded
/// `redaction_policy_hash` was another pack's, `transmit` refused with zero
/// bytes and `transmit_without_completion` wrote a hundred and eighty. The
/// second one read no grant row at all.
///
/// So the assertion is written over both, from one table, and each path is
/// named in the failure message. `bind_grant` is what makes them agree and
/// `the_byte_path_has_one_derivation` counts its two call sites; this is the
/// observation that the binding refuses rather than merely existing.
#[test]
fn a_grant_reviewed_under_another_rulepack_is_refused_on_every_transmit_path() -> TestResult {
    let (broker, provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();
    let staged = proxy.stage(&common::staging_request(
        &document, &focus, &policy, MAX_BYTES,
    ))?;

    // The grant records another pack's digest. Everything else -- the ranges,
    // the payload digest, the destination, the purpose -- matches, so the only
    // thing left to refuse on is the rulepack binding.
    let other_pack = ContentDigest::of(b"synthetic-rulepack-that-did-not-produce-these-bytes");
    assert_ne!(
        other_pack.as_str(),
        proxy.rulepack_id().redaction_policy_hash().as_str(),
        "the substitute digest is the shipped pack's own"
    );
    let outcome = common::capability_for(&broker, &staged, &provider, other_pack, 1_000)?;
    let (capability, grant_id) = common::token(outcome)?;

    let plan = TransmissionPlan {
        grant_id: &grant_id,
        actor_id: common::EGRESS_ACTOR,
        process_class: common::EGRESS_CLASS,
        operation: "classify",
        purpose_id: "architecture-classification",
        destination_id: provider.destination_id(),
        expires_at: u64::MAX,
        chunk_bytes: 8,
    };

    for path in TRANSMIT_PATHS {
        let mut journal = StagedGrantJournal::new();
        let mut sink = RecordingSink::default();
        let error = refused(
            (path.send)(&proxy, &capability, &staged, &plan, &mut journal, &mut sink),
            &format!("{} under another pack's grant", path.name),
        )?;
        assert_eq!(
            error.reason(),
            Some(ReasonCode::ScopeMismatch),
            "{} refused as {:?}",
            path.name,
            error.reason()
        );
        assert_eq!(
            sink.written.len(),
            0,
            "{} wrote {} bytes under a grant reviewed by another pack",
            path.name,
            sink.written.len()
        );
    }

    // The grant was never spent, on either path.
    let grant = broker
        .grant_row(&grant_id)?
        .ok_or("the broker allowed without writing a grant row")?;
    assert_eq!(
        grant.consumed_at, None,
        "a refused transfer consumed the grant"
    );
    Ok(())
}

/// A plan naming a grant other than the token's is refused, on either path.
///
/// The plan and the token are separate inputs. `execute` consumes the row the
/// token names; the journal records the row the plan names. `T146` observed
/// what that costs: token A with `plan.grant_id = B` transmitted a hundred and
/// eighty bytes, the journal named B twice, and the row actually consumed was
/// A -- so the record pointed at a grant nobody spent, and the rulepack
/// comparison had read B rather than the row being consumed.
#[test]
fn a_plan_naming_another_grant_is_refused() -> TestResult {
    let (broker, provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();
    let staged = proxy.stage(&common::staging_request(
        &document, &focus, &policy, MAX_BYTES,
    ))?;
    let hash = proxy.rulepack_id().redaction_policy_hash().clone();

    // Two grants over the same staged payload, both live, both recording the
    // shipped pack. Nothing but the identifier separates them, so the refusal
    // below cannot come from any other field.
    let (capability_a, grant_a) = common::token(common::capability_for(
        &broker,
        &staged,
        &provider,
        hash.clone(),
        1_000,
    )?)?;
    let (_capability_b, grant_b) = common::token(common::capability_for(
        &broker, &staged, &provider, hash, 1_001,
    )?)?;
    assert_ne!(grant_a, grant_b, "the broker minted one grant twice");

    let plan = TransmissionPlan {
        grant_id: &grant_b,
        actor_id: common::EGRESS_ACTOR,
        process_class: common::EGRESS_CLASS,
        operation: "classify",
        purpose_id: "architecture-classification",
        destination_id: provider.destination_id(),
        expires_at: u64::MAX,
        chunk_bytes: 8,
    };

    for path in TRANSMIT_PATHS {
        let mut journal = StagedGrantJournal::new();
        let mut sink = RecordingSink::default();
        let error = refused(
            (path.send)(
                &proxy,
                &capability_a,
                &staged,
                &plan,
                &mut journal,
                &mut sink,
            ),
            &format!("{} with a plan naming another grant", path.name),
        )?;
        assert_eq!(
            error.reason(),
            Some(ReasonCode::ScopeMismatch),
            "{} refused as {:?}",
            path.name,
            error.reason()
        );
        assert_eq!(sink.written.len(), 0, "{} wrote bytes", path.name);
        assert_eq!(
            journal.entries().len(),
            0,
            "{} journalled an intent for a grant it never bound",
            path.name
        );
    }

    for (label, id) in [("A", &grant_a), ("B", &grant_b)] {
        let grant = broker
            .grant_row(id)?
            .ok_or("the broker allowed without writing a grant row")?;
        assert_eq!(
            grant.consumed_at, None,
            "grant {label} was consumed by a refused transfer"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The refusal paths
// ---------------------------------------------------------------------------

/// A scanner that cannot finish denies. It does not return a clean result.
///
/// The budget is a real property of the shipped scanner rather than an injected
/// fault: a scan that would examine more tokens than its budget allows cannot
/// answer, and an unanswered scan is `SCANNER_ERROR`. Lowering the budget in a
/// test drives that path with an ordinary payload.
#[test]
fn scanner_error_emits_zero_bytes() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::with_rulepack(&broker, Rulepack::builtin().with_token_budget(3));
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();

    let denial = refused(
        proxy.stage(&common::staging_request(
            &document, &focus, &policy, MAX_BYTES,
        )),
        "a scan that could not finish was treated as clean",
    )?;
    assert_eq!(denial.reason(), ReasonCode::ScannerError);
    assert_eq!(denial.bytes_transmitted(), 0);
    assert_eq!(denial.route(), Route::LocalOnlyOrStop);

    // The same document under the shipped budget stages, so the refusal came
    // from the scan failing and not from the document.
    let full = EgressProxy::new(&broker);
    assert!(
        full.stage(&common::staging_request(
            &document, &focus, &policy, MAX_BYTES
        ))
        .is_ok(),
        "the fixture document does not stage even with a whole scan"
    );
    Ok(())
}

/// A payload that cannot be read as text is refused, not guessed at.
#[test]
fn unknown_binary_emits_zero_bytes() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();

    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("zip archive", b"PK\x03\x04rest of a small archive".to_vec()),
        ("elf executable", b"\x7fELF\x02\x01\x01\x00padding".to_vec()),
        (
            "portable executable",
            b"MZ\x90\x00\x03\x00\x00\x00".to_vec(),
        ),
        ("png image", b"\x89PNG\r\n\x1a\n".to_vec()),
        ("invalid utf-8", vec![0x66, 0x6e, 0xff, 0xfe, 0x20]),
        ("embedded nul", b"pub fn f() {\x00}".to_vec()),
    ];
    for (label, bytes) in cases {
        let document = SourceDocument::new("synthetic-binary", bytes);
        let denial = refused(
            proxy.stage(&common::staging_request(
                &document, &focus, &policy, MAX_BYTES,
            )),
            label,
        )?;
        assert_eq!(
            denial.reason(),
            ReasonCode::UnknownBinary,
            "{label} was not refused as unknown binary"
        );
        assert_eq!(denial.bytes_transmitted(), 0, "{label} emitted bytes");
        assert_eq!(denial.route(), Route::LocalOnlyOrStop);
    }
    Ok(())
}

/// Size is checked before readability, so an oversize archive says so.
///
/// The order matters and is fixed: a caller told `UNKNOWN_BINARY` would go
/// looking for a decoder, and a caller told `OVERSIZE` would send less. Both
/// halves are asserted here so neither ordering can drift silently.
#[test]
fn oversize_archive_emits_zero_bytes() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(64)?;
    let proxy = EgressProxy::new(&broker);
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();

    let mut archive = b"PK\x03\x04".to_vec();
    archive.extend(std::iter::repeat_n(b'A', 512));
    let document = SourceDocument::new("synthetic-archive", archive);
    let denial = refused(
        proxy.stage(&common::staging_request(&document, &focus, &policy, 64)),
        "an oversize archive",
    )?;
    assert_eq!(denial.reason(), ReasonCode::Oversize);
    assert_eq!(denial.bytes_transmitted(), 0);
    assert_eq!(denial.route(), Route::LocalOnlyOrStop);

    // Under the bound the same archive is refused for what it is.
    let small = SourceDocument::new("synthetic-archive", b"PK\x03\x04small".to_vec());
    let denial = refused(
        proxy.stage(&common::staging_request(&small, &focus, &policy, 64)),
        "an in-bounds archive",
    )?;
    assert_eq!(denial.reason(), ReasonCode::UnknownBinary);

    // A redaction that grows the payload past the bound is refused too.
    let text = common::clean_document();
    let bound = u64::try_from(text.len()).unwrap_or(u64::MAX);
    let document = SourceDocument::new("synthetic-module", text);
    let growing = IdentifierPolicy::new(
        vec![
            "total".to_owned(),
            "weights".to_owned(),
            "weight".to_owned(),
        ],
        100,
    );
    let denial = refused(
        proxy.stage(&common::staging_request(&document, &focus, &growing, 24)),
        "a redacted payload over the bound",
    )?;
    assert_eq!(denial.reason(), ReasonCode::Oversize);
    assert!(bound > 24, "the fixture bound is not below the document");
    Ok(())
}

/// Every secret shape in the corpus blocks, and each blocks by its own rule.
#[test]
fn entropy_and_pattern_secret_corpus_blocks() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();

    let mut entries = common::secret_corpus();
    entries.extend(common::entropy_corpus());
    assert!(
        entries.len() >= 15,
        "the corpus is too small to be evidence"
    );

    for entry in entries {
        let document = SourceDocument::new("synthetic-module", common::document_with(&entry.text));
        let denial = refused(
            proxy.stage(&common::staging_request(
                &document, &focus, &policy, MAX_BYTES,
            )),
            entry.label,
        )?;
        assert!(
            matches!(
                denial.reason(),
                ReasonCode::SecretPattern | ReasonCode::SecretEntropy
            ),
            "{} was refused as {:?}, not as a secret",
            entry.label,
            denial.reason()
        );
        assert!(
            denial
                .findings()
                .iter()
                .any(|finding| finding.rule_id() == entry.rule_id),
            "{} did not trip {}",
            entry.label,
            entry.rule_id
        );
        assert_eq!(
            denial.bytes_transmitted(),
            0,
            "{} emitted bytes",
            entry.label
        );
        assert_eq!(denial.route(), Route::LocalOnlyOrStop);
    }
    Ok(())
}

/// Personal data blocks wherever it sits, comments and fixtures included.
///
/// This is the case a code-only scanner misses. Each entry is placed three
/// ways — in a line comment, in a block comment, and inside a string literal
/// standing for a test fixture — and the span kind the scanner reports is
/// checked, so a scanner that found the code copy and skipped the other two
/// would fail here rather than pass on one third of the evidence.
#[test]
fn comment_and_test_fixture_pii_blocks() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();

    /// One placement: a label, the wrapper that puts a value there, and the
    /// span kind the scanner must report for a hit inside it.
    type Placement = (
        &'static str,
        fn(&str) -> String,
        academic_egress_boundary::SpanKind,
    );

    let placements: [Placement; 3] = [
        (
            "line comment",
            |value| format!("// contact {value}"),
            academic_egress_boundary::SpanKind::LineComment,
        ),
        (
            "block comment",
            |value| format!("/* contact {value} */"),
            academic_egress_boundary::SpanKind::BlockComment,
        ),
        (
            "test fixture literal",
            |value| format!("let fixture = \"{value}\";"),
            academic_egress_boundary::SpanKind::StringLiteral,
        ),
    ];

    for entry in common::pii_corpus() {
        for (label, wrap, expected_span) in placements {
            let document = SourceDocument::new(
                "synthetic-module",
                common::document_with(&wrap(&entry.text)),
            );
            let denial = refused(
                proxy.stage(&common::staging_request(
                    &document, &focus, &policy, MAX_BYTES,
                )),
                entry.label,
            )?;
            assert_eq!(
                denial.reason(),
                ReasonCode::PiiDetected,
                "{} in a {label} was refused as {:?}",
                entry.label,
                denial.reason()
            );
            let matched = denial
                .findings()
                .iter()
                .find(|finding| finding.rule_id() == entry.rule_id)
                .ok_or_else(|| {
                    format!(
                        "{} in a {label} did not trip {}",
                        entry.label, entry.rule_id
                    )
                })?;
            assert_eq!(
                matched.span_kind(),
                expected_span,
                "{} in a {label} was reported in the wrong span",
                entry.label
            );
            assert_eq!(denial.bytes_transmitted(), 0);
            assert_eq!(denial.route(), Route::LocalOnlyOrStop);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Minimization and redaction
// ---------------------------------------------------------------------------

/// A whole-file request becomes the declaration it named, and nothing else.
#[test]
fn whole_file_request_is_reduced_to_minimal_slice() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let text = common::clean_document();
    let document = SourceDocument::new("synthetic-module", text.clone());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();

    let staged = proxy.stage(&common::staging_request(
        &document, &focus, &policy, MAX_BYTES,
    ))?;
    let ranges = staged.preview().source_ranges();
    assert_eq!(ranges.len(), 1, "one symbol produced more than one range");
    let range = ranges[0];
    let slice = text
        .get(range.start()..range.end())
        .ok_or("the reported range falls outside the document")?;
    assert_eq!(
        staged.preview().bytes(),
        slice.as_bytes(),
        "the staged bytes are not the reported range"
    );
    assert!(
        slice.starts_with("/// Adds up the weights"),
        "the slice does not begin at the declaration's doc comment"
    );
    assert!(slice.ends_with('}'), "the slice does not end at the brace");
    assert!(
        staged.preview().byte_len() < text.len(),
        "minimization did not reduce the payload"
    );
    for absent in ["term_label", "round_credit", "synthetic module"] {
        assert!(
            !slice.contains(absent),
            "the minimal slice still carries {absent}"
        );
    }

    // Two symbols produce two ranges, and a symbol the document does not
    // declare is out of scope rather than a licence to send everything.
    let both = vec!["total_weight".to_owned(), "round_credit".to_owned()];
    let staged_both = proxy.stage(&common::staging_request(
        &document, &both, &policy, MAX_BYTES,
    ))?;
    assert_eq!(staged_both.preview().source_ranges().len(), 2);
    assert!(staged_both.preview().byte_len() < text.len());

    let missing = vec!["no_such_symbol".to_owned()];
    let denial = refused(
        proxy.stage(&common::staging_request(
            &document, &missing, &policy, MAX_BYTES,
        )),
        "an undeclared symbol",
    )?;
    assert_eq!(denial.reason(), ReasonCode::ScopeMismatch);
    assert_eq!(denial.bytes_transmitted(), 0);
    Ok(())
}

/// A redaction that removes the meaning routes local-only-or-stop.
///
/// Two exact conditions, and both are checked. Substituting the symbol the
/// request is about renames the question. Passing the policy's own share bound
/// leaves a slice nobody reviewed. Neither is a heuristic and neither can be
/// argued down: `cloud_egress_default` takes no argument, so no quality signal
/// can be handed to it.
#[test]
fn redaction_meaning_loss_routes_local_or_stops() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();

    let renames_focus = IdentifierPolicy::new(vec!["total_weight".to_owned()], 100);
    let denial = refused(
        proxy.stage(&common::staging_request(
            &document,
            &focus,
            &renames_focus,
            MAX_BYTES,
        )),
        "a policy that renames the requested symbol staged",
    )?;
    assert_eq!(denial.reason(), ReasonCode::RedactionDestroysMeaning);
    assert_eq!(denial.route(), Route::LocalOnlyOrStop);
    assert_eq!(denial.bytes_transmitted(), 0);

    let over_share = IdentifierPolicy::new(
        vec![
            "weights".to_owned(),
            "weight".to_owned(),
            "total".to_owned(),
            "pub".to_owned(),
            "for".to_owned(),
            "in".to_owned(),
            "let".to_owned(),
            "mut".to_owned(),
            "u32".to_owned(),
        ],
        5,
    );
    let denial = refused(
        proxy.stage(&common::staging_request(
            &document,
            &focus,
            &over_share,
            MAX_BYTES,
        )),
        "a redaction over the share bound staged",
    )?;
    assert_eq!(denial.reason(), ReasonCode::RedactionDestroysMeaning);
    assert_eq!(denial.route(), Route::LocalOnlyOrStop);

    // The same identifiers under a policy that tolerates the loss stage, so the
    // refusal came from the bound and not from the identifiers.
    let tolerant = IdentifierPolicy::new(over_share.substituted().to_vec(), 95);
    assert!(
        proxy
            .stage(&common::staging_request(
                &document, &focus, &tolerant, MAX_BYTES
            ))
            .is_ok(),
        "the identifiers alone refuse, so the bound proved nothing"
    );

    assert_eq!(cloud_egress_default(), Route::LocalOnlyOrStop);
    assert_eq!(
        cloud_egress_default().as_str(),
        "LOCAL_ONLY_OR_STOP",
        "GATE-38-028's default changed spelling"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The response side
// ---------------------------------------------------------------------------

/// A canary or a secret in a provider response quarantines it.
#[test]
fn canary_in_provider_response_raises_incident() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let mut rng = common::Lcg::new(0x5EC0_0D15_5EED_0003);
    let canary = rng.token(24, common::ALNUM);
    let corpus = academic_egress_boundary::CanaryCorpus::new(vec![canary.clone()]);
    assert_eq!(corpus.len(), 1);

    let clean = b"The function totals a slice of unsigned weights.";
    let accepted = proxy
        .accept_response(&corpus, clean)
        .map_err(|incident| format!("a clean response was quarantined: {incident}"))?;
    assert_eq!(accepted.bytes(), clean);

    let with_canary = format!("Here is the value you stored: {canary}");
    let incident = refused(
        proxy.accept_response(&corpus, with_canary.as_bytes()),
        "a response carrying a canary",
    )?;
    assert_eq!(incident.reason(), ReasonCode::CanaryInResponse);
    assert_eq!(
        incident.severity(),
        academic_egress_boundary::IncidentSeverity::High
    );
    assert_eq!(incident.response_byte_count(), with_canary.len());
    assert!(!incident.hits().is_empty());
    let hit = incident
        .hits()
        .iter()
        .find(|hit| {
            matches!(
                hit.source,
                academic_egress_boundary::HitSource::Canary { .. }
            )
        })
        .ok_or("the canary hit was not reported as a canary")?;
    let quoted = with_canary
        .get(hit.start..hit.end)
        .ok_or("the hit range falls outside the response")?;
    assert_eq!(quoted, canary);

    // The incident carries digests, ranges, and rule names. It does not carry
    // the canary, which is the thing it exists to keep out of a record.
    let rendered = format!("{incident:?}");
    assert!(
        !rendered.contains(&canary),
        "the incident record repeats the canary it quarantined"
    );
    assert!(!incident.response_digest().is_empty());

    // A secret shape the corpus never registered is caught by the rulepack.
    let entry = common::secret_corpus()
        .into_iter()
        .next()
        .ok_or("the secret corpus is empty")?;
    let unregistered = format!("Try this credential: {}", entry.text);
    let incident = refused(
        proxy.accept_response(&corpus, unregistered.as_bytes()),
        "a response carrying a secret pattern",
    )?;
    assert_eq!(incident.reason(), ReasonCode::CanaryInResponse);
    assert!(incident.hits().iter().any(|hit| matches!(
        hit.source,
        academic_egress_boundary::HitSource::Rule { rule_id } if rule_id == entry.rule_id
    )));

    // A response that could not be scanned is quarantined, not accepted.
    let starved = EgressProxy::with_rulepack(&broker, Rulepack::builtin().with_token_budget(1));
    let incident = refused(
        starved.accept_response(&corpus, clean),
        "an unscanned response",
    )?;
    assert_eq!(incident.reason(), ReasonCode::ScannerError);
    Ok(())
}

// ---------------------------------------------------------------------------
// The closed reason enum
// ---------------------------------------------------------------------------

/// Compiler-checked witness that `every_reason_code` names every variant.
///
/// A new variant makes this match non-exhaustive and the suite stops compiling,
/// which is the only check that cannot be forgotten. The index it returns is
/// then used to prove the list below omits none.
const fn witness(code: ReasonCode) -> usize {
    match code {
        ReasonCode::NoGrant => 0,
        ReasonCode::GrantExpired => 1,
        ReasonCode::GrantConsumed => 2,
        ReasonCode::ScopeMismatch => 3,
        ReasonCode::PolicyStale => 4,
        ReasonCode::ProviderPolicyIncompatible => 5,
        ReasonCode::ScannerError => 6,
        ReasonCode::SecretPattern => 7,
        ReasonCode::SecretEntropy => 8,
        ReasonCode::PiiDetected => 9,
        ReasonCode::UnknownBinary => 10,
        ReasonCode::Oversize => 11,
        ReasonCode::RedactionDestroysMeaning => 12,
        ReasonCode::CanaryInResponse => 13,
        ReasonCode::NoDeletionReceipt => 14,
    }
}

/// Every variant, in the order execution-plan section 3.5 writes them.
const EVERY_REASON_CODE: [ReasonCode; 15] = [
    ReasonCode::NoGrant,
    ReasonCode::GrantExpired,
    ReasonCode::GrantConsumed,
    ReasonCode::ScopeMismatch,
    ReasonCode::PolicyStale,
    ReasonCode::ProviderPolicyIncompatible,
    ReasonCode::ScannerError,
    ReasonCode::SecretPattern,
    ReasonCode::SecretEntropy,
    ReasonCode::PiiDetected,
    ReasonCode::UnknownBinary,
    ReasonCode::Oversize,
    ReasonCode::RedactionDestroysMeaning,
    ReasonCode::CanaryInResponse,
    ReasonCode::NoDeletionReceipt,
];

/// The section 3.5 sentence, transcribed. This is the specification side.
const SPEC_REASON_CODES: [&str; 15] = [
    "NO_GRANT",
    "GRANT_EXPIRED",
    "GRANT_CONSUMED",
    "SCOPE_MISMATCH",
    "POLICY_STALE",
    "PROVIDER_POLICY_INCOMPATIBLE",
    "SCANNER_ERROR",
    "SECRET_PATTERN",
    "SECRET_ENTROPY",
    "PII_DETECTED",
    "UNKNOWN_BINARY",
    "OVERSIZE",
    "REDACTION_DESTROYS_MEANING",
    "CANARY_IN_RESPONSE",
    "NO_DELETION_RECEIPT",
];

/// Who denies with each code. Every code has exactly one entry.
///
/// `Egress` means this crate produces it and the suite below observes it.
/// `Broker` and `Registry` mean `P2-G1` or `P2-G3` owns the decision and its
/// own acceptance suite observes it; naming the owner is what keeps a code from
/// sitting in the enum with nothing behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Owner {
    Egress,
    Broker,
    Registry,
}

const REASON_OWNERS: [(ReasonCode, Owner); 15] = [
    (ReasonCode::NoGrant, Owner::Broker),
    (ReasonCode::GrantExpired, Owner::Egress),
    (ReasonCode::GrantConsumed, Owner::Egress),
    (ReasonCode::ScopeMismatch, Owner::Egress),
    (ReasonCode::PolicyStale, Owner::Broker),
    (ReasonCode::ProviderPolicyIncompatible, Owner::Registry),
    (ReasonCode::ScannerError, Owner::Egress),
    (ReasonCode::SecretPattern, Owner::Egress),
    (ReasonCode::SecretEntropy, Owner::Egress),
    (ReasonCode::PiiDetected, Owner::Egress),
    (ReasonCode::UnknownBinary, Owner::Egress),
    (ReasonCode::Oversize, Owner::Egress),
    (ReasonCode::RedactionDestroysMeaning, Owner::Egress),
    (ReasonCode::CanaryInResponse, Owner::Egress),
    (ReasonCode::NoDeletionReceipt, Owner::Registry),
];

/// The deny reason codes are enumerated, not counted, and each has a producer.
///
/// Four halves, each failing for a different edit. The witness match stops the
/// suite compiling when a variant is added. The index set fails when the list
/// omits one. The specification transcription fails when a spelling drifts from
/// section 3.5, and the operational store's `CHECK` is read from
/// `crates/policy/src/schema.sql` so the database and the enum cannot disagree.
/// Last, every code this crate owns is actually produced by running the
/// pipeline, so a code with no path behind it fails here rather than passing as
/// an unused variant.
#[test]
fn deny_reason_codes_are_exhaustive() -> TestResult {
    let mut seen = [false; 15];
    for code in EVERY_REASON_CODE {
        let index = witness(code);
        assert!(!seen[index], "{} appears twice", code.as_str());
        seen[index] = true;
    }
    assert!(
        seen.iter().all(|present| *present),
        "a reason code the witness knows is missing from EVERY_REASON_CODE"
    );

    let spelled: Vec<&str> = EVERY_REASON_CODE.iter().map(|code| code.as_str()).collect();
    assert_eq!(
        spelled,
        SPEC_REASON_CODES.to_vec(),
        "the enum and the section 3.5 transcription disagree"
    );

    let schema = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("policy")
            .join("src")
            .join("schema.sql"),
    )?;
    let check = schema
        .split_once("AND reason_code IN (")
        .and_then(|(_, rest)| rest.split_once("))"))
        .map(|(body, _)| body.to_owned())
        .ok_or("the operational schema has no reason-code CHECK")?;
    let quoted: Vec<&str> = check.split('\'').skip(1).step_by(2).collect();
    assert_eq!(
        quoted,
        SPEC_REASON_CODES.to_vec(),
        "the egress_audit CHECK and the closed enum disagree"
    );

    let owners: Vec<ReasonCode> = REASON_OWNERS.iter().map(|(code, _)| *code).collect();
    assert_eq!(
        owners,
        EVERY_REASON_CODE.to_vec(),
        "a reason code has no named producer"
    );

    let produced = produce_every_egress_reason()?;
    let declared: Vec<ReasonCode> = REASON_OWNERS
        .iter()
        .filter(|(_, owner)| *owner == Owner::Egress)
        .map(|(code, _)| *code)
        .collect();
    let mut produced_sorted: Vec<&str> = produced.iter().map(|code| code.as_str()).collect();
    let mut declared_sorted: Vec<&str> = declared.iter().map(|code| code.as_str()).collect();
    produced_sorted.sort_unstable();
    declared_sorted.sort_unstable();
    assert_eq!(
        produced_sorted, declared_sorted,
        "the codes this crate claims to own and the codes it produced differ"
    );
    Ok(())
}

/// Runs every path this crate owns and collects the code each produced.
fn produce_every_egress_reason() -> Result<Vec<ReasonCode>, Box<dyn Error>> {
    let (broker, provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let focus = common::focus_total_weight();
    let none = IdentifierPolicy::none();
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let mut produced = Vec::new();

    let deny = |result: Result<_, academic_egress_boundary::EgressDenial>| match result {
        Ok(_) => None,
        Err(denial) => Some(denial.reason()),
    };

    let oversize = SourceDocument::new("synthetic-module", common::clean_document());
    produced.extend(deny(
        proxy.stage(&common::staging_request(&oversize, &focus, &none, 8)),
    ));
    let binary = SourceDocument::new("synthetic-binary", b"PK\x03\x04tiny".to_vec());
    produced.extend(deny(
        proxy.stage(&common::staging_request(&binary, &focus, &none, MAX_BYTES)),
    ));
    let starved = EgressProxy::with_rulepack(&broker, Rulepack::builtin().with_token_budget(2));
    produced.extend(deny(starved.stage(&common::staging_request(
        &document, &focus, &none, MAX_BYTES,
    ))));
    let secret = SourceDocument::new(
        "synthetic-module",
        common::document_with(
            &common::secret_corpus()
                .first()
                .ok_or("the secret corpus is empty")?
                .text,
        ),
    );
    produced.extend(deny(
        proxy.stage(&common::staging_request(&secret, &focus, &none, MAX_BYTES)),
    ));
    let entropy = SourceDocument::new(
        "synthetic-module",
        common::document_with(
            &common::entropy_corpus()
                .first()
                .ok_or("the entropy corpus is empty")?
                .text,
        ),
    );
    produced.extend(deny(
        proxy.stage(&common::staging_request(&entropy, &focus, &none, MAX_BYTES)),
    ));
    let personal = SourceDocument::new(
        "synthetic-module",
        common::document_with("// contact j.doe@students.invalid"),
    );
    produced.extend(deny(proxy.stage(&common::staging_request(
        &personal, &focus, &none, MAX_BYTES,
    ))));
    let renames = IdentifierPolicy::new(vec!["total_weight".to_owned()], 100);
    produced.extend(deny(proxy.stage(&common::staging_request(
        &document, &focus, &renames, MAX_BYTES,
    ))));
    let missing = vec!["no_such_symbol".to_owned()];
    produced.extend(deny(proxy.stage(&common::staging_request(
        &document, &missing, &none, MAX_BYTES,
    ))));

    // The response half.
    let corpus =
        academic_egress_boundary::CanaryCorpus::new(vec!["canary-token-synthetic".to_owned()]);
    if let Err(incident) = proxy.accept_response(&corpus, b"contains canary-token-synthetic here") {
        produced.push(incident.reason());
    }

    // The transmission half: a live grant, then an expiry mid-transfer and a
    // second use of the same token.
    let staged = proxy.stage(&common::staging_request(
        &document, &focus, &none, MAX_BYTES,
    ))?;
    let hash = proxy.rulepack_id().redaction_policy_hash().clone();
    let outcome = common::capability_for(&broker, &staged, &provider, hash, 1_000)?;
    let (capability, grant_id) = common::token(outcome)?;
    let plan = TransmissionPlan {
        grant_id: &grant_id,
        actor_id: common::EGRESS_ACTOR,
        process_class: common::EGRESS_CLASS,
        operation: "classify",
        purpose_id: "architecture-classification",
        destination_id: provider.destination_id(),
        expires_at: 1_001,
        chunk_bytes: 4,
    };
    let mut journal = StagedGrantJournal::new();
    let mut sink = RecordingSink::default();
    let clock = std::cell::Cell::new(1_000_u64);
    let expiring = proxy.transmit(
        &capability,
        &staged,
        &plan,
        &mut journal,
        &mut sink,
        &|| {
            let now = clock.get();
            clock.set(now.saturating_add(1));
            now
        },
    );
    if let Err(error) = expiring
        && let Some(reason) = error.reason()
    {
        produced.push(reason);
    }
    let mut second = RecordingSink::default();
    let replay = proxy.transmit(
        &capability,
        &staged,
        &plan,
        &mut journal,
        &mut second,
        &|| 1_000,
    );
    if let Err(error) = replay
        && let Some(reason) = error.reason()
    {
        produced.push(reason);
    }
    assert_eq!(second.written.len(), 0, "a consumed grant wrote bytes");

    // A refusing transport is reported as a scope mismatch of the destination.
    let mut refusing = RefusingSink::default();
    let fresh = proxy.stage(&common::staging_request(
        &document,
        &["round_credit".to_owned()],
        &none,
        MAX_BYTES,
    ))?;
    let hash = proxy.rulepack_id().redaction_policy_hash().clone();
    let outcome = common::capability_for(&broker, &fresh, &provider, hash, 2_000)?;
    let (capability, grant_id) = common::token(outcome)?;
    let plan = TransmissionPlan {
        grant_id: &grant_id,
        actor_id: common::EGRESS_ACTOR,
        process_class: common::EGRESS_CLASS,
        operation: "classify",
        purpose_id: "architecture-classification",
        destination_id: provider.destination_id(),
        expires_at: u64::MAX,
        chunk_bytes: 4,
    };
    let refused = proxy.transmit(
        &capability,
        &fresh,
        &plan,
        &mut journal,
        &mut refusing,
        &|| 2_000,
    );
    if let Err(error) = refused
        && let Some(reason) = error.reason()
    {
        produced.push(reason);
    }
    assert!(
        refusing.calls > 0,
        "the refusing transport was never called"
    );

    let audits = broker.audit_rows()?;
    assert!(
        audits.iter().any(|row| row.decision == Decision::Deny),
        "no deny audit row was written"
    );
    produced.sort_unstable_by_key(|code| witness(*code));
    produced.dedup();
    Ok(produced)
}
