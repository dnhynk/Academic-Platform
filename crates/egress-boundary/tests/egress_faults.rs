//! Faults `EG01`-`EG08` from the execution plan's fault matrix.
//!
//! `EG07` — a provider that offers no deletion receipt — is `P2-G3`'s row and
//! is exercised by `provider_without_deletion_api_requires_explicit_user_policy`
//! in `crates/policy/tests/provider_registry.rs`. It appears here only as the
//! reason code this crate must be able to report, not as a second
//! implementation of the registry's decision.

mod common;

use std::cell::Cell;
use std::error::Error;

use academic_egress_boundary::{
    CanaryCorpus, EgressProxy, IdentifierPolicy, IncidentSeverity, JournalEntry, OutboundTransport,
    Route, Rulepack, SourceDocument, StagedGrantJournal, TransmissionPlan, TransportError,
};
use academic_policy::{Decision, ReasonCode};

use common::TestResult;

const MAX_BYTES: u64 = 4_096;

/// Records what it was handed so a partial transfer can be counted.
#[derive(Debug, Default)]
struct RecordingSink {
    written: Vec<u8>,
}

impl OutboundTransport for RecordingSink {
    fn send_chunk(&mut self, chunk: &[u8]) -> Result<(), TransportError> {
        self.written.extend_from_slice(chunk);
        Ok(())
    }
}

fn refused<T: std::fmt::Debug, E>(result: Result<T, E>, what: &str) -> Result<E, Box<dyn Error>> {
    match result {
        Ok(value) => Err(format!("{what} was not refused: {value:?}").into()),
        Err(error) => Ok(error),
    }
}

/// `EG01`: the DLP scanner fails. The payload is denied, not sent unscanned.
#[test]
fn eg01_scanner_error_denies() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::with_rulepack(&broker, Rulepack::builtin().with_token_budget(2));
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();
    let denial = refused(
        proxy.stage(&common::staging_request(
            &document, &focus, &policy, MAX_BYTES,
        )),
        "a payload whose scan could not finish",
    )?;
    assert_eq!(denial.reason(), ReasonCode::ScannerError);
    assert_eq!(denial.bytes_transmitted(), 0);
    assert_eq!(denial.route(), Route::LocalOnlyOrStop);
    Ok(())
}

/// `EG02`: the payload is over the destination's size threshold.
#[test]
fn eg02_oversize_denies() -> TestResult {
    let (broker, provider) = common::broker_with_provider(32)?;
    let proxy = EgressProxy::new(&broker);
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();
    let denial = refused(
        proxy.stage(&common::staging_request(
            &document,
            &focus,
            &policy,
            provider.maximum_input_bytes(),
        )),
        "a payload over the provider's declared maximum input",
    )?;
    assert_eq!(denial.reason(), ReasonCode::Oversize);
    assert_eq!(denial.bytes_transmitted(), 0);
    assert_eq!(provider.maximum_input_bytes(), 32);
    Ok(())
}

/// `EG03`: an unknown binary sits inside the slice the request selected.
///
/// Classification runs over the whole document rather than over the slice, so
/// the refusal is at least as strict as the fault requires. The binary is
/// placed inside `total_weight`'s body so the case the matrix names — inside
/// the slice — is the one being observed.
#[test]
fn eg03_unknown_binary_inside_the_slice_denies() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::none();

    let mut bytes = common::document_with("let blob = [0x01, 0x02];").into_bytes();
    let anchor = b"let blob = [0x01, 0x02];";
    let at = bytes
        .windows(anchor.len())
        .position(|window| window == anchor)
        .ok_or("the fixture anchor is missing")?;
    bytes.splice(at..at, [0x00_u8, 0x1b, 0xff]);
    let document = SourceDocument::new("synthetic-module", bytes);
    let denial = refused(
        proxy.stage(&common::staging_request(
            &document, &focus, &policy, MAX_BYTES,
        )),
        "a slice holding unclassifiable bytes",
    )?;
    assert_eq!(denial.reason(), ReasonCode::UnknownBinary);
    assert_eq!(denial.bytes_transmitted(), 0);
    Ok(())
}

/// `EG04`: the grant expires mid-transfer. The transfer aborts and the partial
/// count is what the journal and the denial both report.
///
/// Where that count lives is worth being exact about. `PermissionBroker::execute`
/// commits its allow row before it calls the tool, so `egress_audit` carries the
/// count the grant *authorized*, which is the whole staged payload. The count
/// actually written to the transport is in the staged grant journal's
/// `SendOutcome`. Both are asserted here and both name the same `grant_id`, so
/// the pair reconciles; neither one alone is the audited partial count, and this
/// crate does not reach into `P2-G1`'s append-only table to rewrite the other.
#[test]
fn eg04_grant_expiring_mid_transfer_aborts_and_audits_the_partial_count() -> TestResult {
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

    // The clock advances one millisecond per read, so the transfer runs out of
    // time after exactly three chunks.
    let clock = Cell::new(1_000_u64);
    let tick = || {
        let now = clock.get();
        clock.set(now.saturating_add(1));
        now
    };
    let plan = TransmissionPlan {
        grant_id: &grant_id,
        actor_id: common::EGRESS_ACTOR,
        process_class: common::EGRESS_CLASS,
        operation: "classify",
        purpose_id: "architecture-classification",
        destination_id: provider.destination_id(),
        expires_at: 1_004,
        chunk_bytes: 8,
    };
    let mut journal = StagedGrantJournal::new();
    let mut sink = RecordingSink::default();
    let error = refused(
        proxy.transmit(&capability, &staged, &plan, &mut journal, &mut sink, &tick),
        "a transfer that outlived its grant",
    )?;
    let denial = error
        .denial()
        .ok_or("a mid-transfer expiry was reported as a store failure")?;
    assert_eq!(denial.reason(), ReasonCode::GrantExpired);

    let sent = denial.bytes_transmitted();
    assert!(sent > 0, "the abort happened before any chunk");
    assert!(
        sent < staged.preview().byte_len(),
        "the transfer was not aborted"
    );
    assert_eq!(sink.written.len(), sent, "the count and the sink disagree");
    assert_eq!(
        sink.written,
        staged
            .preview()
            .bytes()
            .get(..sent)
            .ok_or("the partial count exceeds the staged payload")?,
        "the partial transfer was not a prefix of the previewed bytes"
    );

    let outcomes: Vec<&JournalEntry> = journal
        .entries()
        .iter()
        .filter(|entry| matches!(entry, JournalEntry::SendOutcome { .. }))
        .collect();
    assert_eq!(outcomes.len(), 1);
    match outcomes[0] {
        JournalEntry::SendOutcome {
            bytes_sent,
            complete,
            ..
        } => {
            assert_eq!(*bytes_sent, sent);
            assert!(!complete, "an aborted transfer was journalled as complete");
        }
        JournalEntry::SendIntent { .. } => return Err("the filter kept the wrong arm".into()),
    }

    let audits = broker.audit_rows()?;
    let allow = audits
        .iter()
        .find(|row| row.decision == Decision::Allow && row.grant_id.as_deref() == Some(&grant_id))
        .ok_or("the aborted transfer has no allow audit row")?;
    assert_eq!(
        usize::try_from(allow.byte_count)?,
        staged.preview().byte_len(),
        "the audit row carries something other than the authorized byte count"
    );
    assert_eq!(
        allow.payload_digest.as_deref(),
        Some(staged.preview().digest().as_str()),
        "the audit row and the journal do not name the same payload"
    );
    Ok(())
}

/// `EG05`: a kill after the provider send and before the audit write.
///
/// Three things must hold and all three are read from the shipped API rather
/// than asserted about it: the journal reconstructs the interrupted transfer,
/// the grant is already consumed, and a second send is refused.
#[test]
fn eg05_kill_after_send_reconstructs_the_audit_and_refuses_a_second_send() -> TestResult {
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
    let plan = TransmissionPlan {
        grant_id: &grant_id,
        actor_id: common::EGRESS_ACTOR,
        process_class: common::EGRESS_CLASS,
        operation: "classify",
        purpose_id: "architecture-classification",
        destination_id: provider.destination_id(),
        expires_at: u64::MAX,
        chunk_bytes: 16,
    };

    let mut journal = StagedGrantJournal::new();
    let mut sink = RecordingSink::default();
    let transmission = proxy.transmit_without_completion(
        &capability,
        &staged,
        &plan,
        &mut journal,
        &mut sink,
        &|| 1_000,
    )?;
    assert_eq!(transmission.bytes_sent(), staged.preview().byte_len());

    let reconstructed = journal.reconstruct();
    assert_eq!(
        reconstructed.len(),
        1,
        "the interrupted send was not recovered"
    );
    let recovered = &reconstructed[0];
    assert_eq!(recovered.grant_id, grant_id);
    assert_eq!(recovered.payload_digest, staged.preview().digest().as_str());
    assert_eq!(recovered.byte_count, staged.preview().byte_len());
    assert_eq!(recovered.destination_id, provider.destination_id());

    let grant = broker
        .grant_row(&grant_id)?
        .ok_or("the grant row vanished after the send")?;
    assert!(
        grant.consumed_at.is_some(),
        "the grant was not marked consumed before the send"
    );

    let mut second = RecordingSink::default();
    let error = refused(
        proxy.transmit(
            &capability,
            &staged,
            &plan,
            &mut journal,
            &mut second,
            &|| 1_001,
        ),
        "a second send of a consumed grant",
    )?;
    assert_eq!(error.reason(), Some(ReasonCode::GrantConsumed));
    assert!(second.written.is_empty(), "the second send wrote bytes");

    // The completion record the second attempt appended resolves the intent, so
    // a recovery run after it no longer reports the transfer as unresolved.
    assert!(
        journal.reconstruct().is_empty(),
        "the journal still reports an unresolved transfer"
    );

    let audits = broker.audit_rows()?;
    assert!(
        audits.iter().any(
            |row| row.decision == Decision::Allow && row.grant_id.as_deref() == Some(&grant_id)
        ),
        "the allow audit for the interrupted send is missing"
    );
    assert!(
        audits
            .iter()
            .any(|row| row.reason_code == Some(ReasonCode::GrantConsumed)),
        "the refused second send was not audited"
    );
    Ok(())
}

/// `EG06`: a provider response carries a canary. It is quarantined, not stored.
#[test]
fn eg06_canary_in_a_provider_response_quarantines_it() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    // Deliberately not secret-shaped. A high-entropy canary would be caught by
    // the entropy rule whether the corpus held it or not, and this row is
    // evidence about the corpus. The assertion below checks every hit came from
    // the corpus, so the rulepack cannot be what refused the response.
    let canary = "canary-lecture-marker-zeta".to_owned();
    let corpus = CanaryCorpus::new(vec![canary.clone()]);
    let response = format!("Summary of the slice. Stored value: {canary}.");

    let incident = refused(
        proxy.accept_response(&corpus, response.as_bytes()),
        "a response carrying a canary",
    )?;
    assert_eq!(incident.reason(), ReasonCode::CanaryInResponse);
    assert_eq!(incident.severity(), IncidentSeverity::High);
    assert_eq!(incident.response_byte_count(), response.len());
    assert!(!incident.response_digest().is_empty());
    assert!(!incident.hits().is_empty());
    for hit in incident.hits() {
        assert!(hit.start < hit.end);
        assert!(hit.end <= response.len());
        assert!(
            matches!(
                hit.source,
                academic_egress_boundary::HitSource::Canary { .. }
            ),
            "a rulepack rule matched, so the corpus is not what refused this response"
        );
    }
    assert!(
        !format!("{incident:?}{incident}").contains(&canary),
        "the incident record repeats the canary"
    );
    Ok(())
}

/// `EG07`: the provider offers no deletion receipt.
///
/// The decision belongs to `P2-G3`. What this row asserts is only that its
/// reason code is reachable from this crate's vocabulary, so an integration
/// that surfaces a registry denial through the egress boundary has a code to
/// use rather than inventing one.
#[test]
fn eg07_no_deletion_receipt_is_a_registry_decision() {
    assert_eq!(
        ReasonCode::NoDeletionReceipt.as_str(),
        "NO_DELETION_RECEIPT"
    );
}

/// `EG08`: the redaction destroys meaning, so the work stays local or stops.
#[test]
fn eg08_redaction_destroying_meaning_routes_local_or_stops() -> TestResult {
    let (broker, _provider) = common::broker_with_provider(MAX_BYTES)?;
    let proxy = EgressProxy::new(&broker);
    let document = SourceDocument::new("synthetic-module", common::clean_document());
    let focus = common::focus_total_weight();
    let policy = IdentifierPolicy::new(vec!["total_weight".to_owned()], 100);
    let denial = refused(
        proxy.stage(&common::staging_request(
            &document, &focus, &policy, MAX_BYTES,
        )),
        "a redaction that renames the requested symbol",
    )?;
    assert_eq!(denial.reason(), ReasonCode::RedactionDestroysMeaning);
    assert_eq!(denial.route(), Route::LocalOnlyOrStop);
    assert_eq!(denial.bytes_transmitted(), 0);
    Ok(())
}
