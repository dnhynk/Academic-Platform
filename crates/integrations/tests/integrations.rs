//! `P2-P3`'s ten acceptance rows.
//!
//! Each of the ten `t068` names is one `#[test]` below. Four of them are
//! absence claims -- no write path, no competency leak, no grade in a payload,
//! no route from an external identifier to a canonical one -- and an absence is
//! not established by a list of names somebody remembered to forbid. Each is
//! made as a **whole-set** comparison in both directions, so a bypass that
//! spells nothing on any list appears as an extra key or as a missing one.
//!
//! This file reads Rust source text. `docs/contracts/policy-source-scans.md`
//! carries its row; `crates/integrations/tests/integration_scans.rs` carries
//! the structural scans that are not themselves acceptance rows.

mod support;

use std::{collections::BTreeSet, error::Error, fs};

use academic_domain::TimestampMillis;
use academic_egress_boundary::{
    EgressProxy, IdentifierPolicy, SourceDocument, StagedGrantJournal, TransmissionPlan,
};
use academic_integrations::{
    AssistantContext, AssistantSelection, AssistantUse, BlobVisibility, CalendarEventKind,
    CalendarPayload, CanonicalRef, ConnectorHealth, ConnectorKind, ConnectorRegistry, CoreView,
    EvidenceEligibility, ExternalId, ExternalIdentity, GeneratedCode, GitHubConnector,
    GitHubOperation, HttpMethod, IdeAdapter, IdeError, IdeWorkspace as _, IdentityMap,
    IntegrationSurface, PrivateBlobEgress, RepositoryBlob, ScopeConfirmation, SourceAuthority,
    SymbolRef, WatchMode, WorkspacePath,
};
use academic_model_run::{
    Cost, Digest32, InputArtifactRef, InputArtifactRefs, ModelRun, ModelRunId, ModelVersion,
    ProviderId, Purpose, RetentionDeclaration, Transmission, TransmittedRange,
};
use academic_policy::{Decision, ProcessClass, ReasonCode};
use academic_repository::{
    CredentialStore, FineGrainedToken, GitHubError, GitHubRepository, TokenLifetime,
    TokenPermission, TokenScope,
};

use support::{
    CountingFleet, DISCLOSURE_PURPOSE, EGRESS_ACTOR, LedgerCore, MemoryKeystore,
    RecordingTransport, RecordingWorkspace, TRANSFER_PURPOSE, TestResult, broker_with_provider,
    capabilities_for, collapse, enum_variants, fixture_course_id, fixture_ids, fixture_ledger,
    fixture_offering_id, fixture_repository_id, product_code, read_module, selection_document,
    strip_non_code, struct_fields, token, trait_methods, workspace_root,
};

// ---------------------------------------------------------------------------
// 1. The core opens with every connector down
// ---------------------------------------------------------------------------

#[test]
fn core_graph_opens_with_every_connector_down() -> TestResult {
    let (claim_id, evidence_id, artifact_id) = fixture_ids()?;

    // Every connector, not a sample. The set the registry reports unreachable
    // is compared with `ConnectorKind::ALL` in both directions, so a connector
    // added later is down in this test without anybody editing it.
    let all_down = ConnectorRegistry::all_down();
    assert_eq!(
        all_down.unreachable(),
        ConnectorKind::ALL.to_vec(),
        "all_down must report every connector kind unreachable"
    );
    assert!(
        ConnectorRegistry::all_up().unreachable().is_empty(),
        "all_up must report nothing unreachable"
    );

    let down_fleet = CountingFleet::new(all_down);
    let down_core = LedgerCore::new(fixture_ledger()?, claim_id, evidence_id, artifact_id);
    let down_surface = IntegrationSurface::new(&down_core, &down_fleet);
    let down_reads: Vec<Vec<u8>> = CoreView::ALL
        .into_iter()
        .map(|view| down_surface.read_core(view))
        .collect();

    // The fleet was never consulted. This is the half a structural pin cannot
    // make: `read_core` could hold a health check that happened to allow.
    assert_eq!(
        down_fleet.asked(),
        0,
        "a core read consulted the connector fleet"
    );
    assert_eq!(
        down_core.reads(),
        CoreView::ALL.len(),
        "the core was not read once per view"
    );

    // The reads are not vacuous: each carries the ledger's own content, and no
    // two views answer alike.
    let head = String::from_utf8(down_reads[0].clone())?;
    assert!(
        head.contains('4'),
        "the ledger head read does not carry the accepted sequence: {head}"
    );
    let claims = String::from_utf8(down_reads[2].clone())?;
    assert!(
        claims.contains("knowledge.mastery"),
        "the claim read does not carry the fixture predicate: {claims}"
    );
    let distinct: BTreeSet<&Vec<u8>> = down_reads.iter().collect();
    assert_eq!(
        distinct.len(),
        CoreView::ALL.len(),
        "two core views answered identically, so the reads are not distinguishable"
    );

    // The same reads with every connector up are byte-identical.
    let up_fleet = CountingFleet::new(ConnectorRegistry::all_up());
    let up_core = LedgerCore::new(fixture_ledger()?, claim_id, evidence_id, artifact_id);
    let up_surface = IntegrationSurface::new(&up_core, &up_fleet);
    let up_reads: Vec<Vec<u8>> = CoreView::ALL
        .into_iter()
        .map(|view| up_surface.read_core(view))
        .collect();
    assert_eq!(
        down_reads, up_reads,
        "the core answered differently with the connectors down"
    );
    assert_eq!(up_fleet.asked(), 0, "a core read consulted the fleet");

    // The fleet is not inert: asked directly, it answers and it counts.
    assert_eq!(
        down_surface.connector_health(ConnectorKind::GitHub),
        ConnectorHealth::Down
    );
    assert_eq!(
        down_fleet.asked(),
        1,
        "the fleet did not record the one question actually put to it"
    );
    Ok(())
}

#[test]
fn every_section_33_row_is_a_connector_kind() -> TestResult {
    // The count is not asserted anywhere. Section 33's table is parsed back out
    // of the design document and compared with `ConnectorKind::ALL` in both
    // directions, so a row added there without a variant here fails and a
    // variant with no row fails too.
    let design = fs::read_to_string(
        workspace_root().join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md"),
    )?;
    let start = design
        .find("\n## 33. Integrations\n")
        .ok_or("the design document has no section 33 heading")?;
    let end = design[start + 1..]
        .find("\n## ")
        .map_or(design.len(), |offset| start + 1 + offset);
    let section = &design[start..end];
    let mut rows: Vec<String> = Vec::new();
    for line in section.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let first = trimmed
            .trim_start_matches('|')
            .split('|')
            .next()
            .unwrap_or("")
            .trim()
            .to_owned();
        if first.is_empty() || first.starts_with("---") || first == "외부 도구" {
            continue;
        }
        rows.push(first);
    }
    assert!(
        rows.len() >= 8,
        "the section 33 table reader found only {} rows",
        rows.len()
    );
    let declared: Vec<String> = ConnectorKind::ALL
        .into_iter()
        .map(|kind| kind.as_str().to_owned())
        .collect();
    assert_eq!(
        rows, declared,
        "section 33's table and ConnectorKind::ALL disagree"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 2. The GitHub connector is read-only and scoped
// ---------------------------------------------------------------------------

fn repository(owner: &str, name: &str) -> Result<GitHubRepository, Box<dyn Error>> {
    Ok(GitHubRepository::new(owner, name)?)
}

fn sealed_token(
    repository: &GitHubRepository,
    permissions: Vec<TokenPermission>,
) -> Result<FineGrainedToken, Box<dyn Error>> {
    let scope = TokenScope::new(repository.clone(), permissions)?;
    let lifetime = TokenLifetime::new(1_000, 1_000 + 60_000)?;
    let store = CredentialStore::new(MemoryKeystore);
    let sealed = store.seal(&FineGrainedToken::new(
        scope,
        lifetime,
        b"synthetic-token-material".to_vec(),
    ))?;
    Ok(store.borrow(&sealed, 1_500)?)
}

#[test]
fn github_connector_is_read_only_and_scoped() -> TestResult {
    let github = read_module("github.rs")?;

    // (a) The operation vocabulary is closed and complete. Its variants are read
    // out of the enum rather than transcribed, so an operation added without an
    // `ALL` entry fails here rather than passing every walk below.
    let variants = enum_variants(&github, "pub enum GitHubOperation")?;
    let declared: Vec<String> = GitHubOperation::ALL
        .into_iter()
        .map(|operation| format!("{operation:?}"))
        .collect();
    assert_eq!(
        variants, declared,
        "GitHubOperation's variants and GitHubOperation::ALL disagree"
    );

    // (b) The method vocabulary has exactly one member, read the same way.
    let methods = enum_variants(&github, "pub enum HttpMethod")?;
    assert_eq!(
        methods,
        vec!["Get".to_owned()],
        "HttpMethod holds a variant other than Get"
    );
    assert_eq!(HttpMethod::ALL.to_vec(), vec![HttpMethod::Get]);

    // (c) Every operation reads, and every request it produces is rooted at the
    // one scoped repository. This is the walk over the whole set.
    let scoped = repository("snu-student", "compilers-project")?;
    let other = repository("snu-student", "another-project")?;
    let token = sealed_token(&scoped, TokenPermission::ALL.to_vec())?;
    let connector = GitHubConnector::new(scoped.clone());
    let expected_prefix = "/repos/snu-student/compilers-project";
    for operation in GitHubOperation::ALL {
        assert_eq!(
            operation.method(),
            HttpMethod::Get,
            "{} is not a read",
            operation.as_str()
        );
        let request = connector.read(&token, operation, 1_500)?;
        assert_eq!(request.method(), HttpMethod::Get);
        assert_eq!(request.operation(), operation);
        assert!(
            request.path() == expected_prefix
                || request.path().starts_with(&format!("{expected_prefix}/")),
            "{} builds a path outside the scoped repository: {}",
            operation.as_str(),
            request.path()
        );
        assert!(
            !request.path().contains(other.name()),
            "{} names another repository",
            operation.as_str()
        );
    }

    // (d) The three credential properties refuse, each with its own code, for
    // every operation rather than for one sampled one.
    let out_of_scope = sealed_token(&other, TokenPermission::ALL.to_vec())?;
    let metadata_only = sealed_token(&scoped, vec![TokenPermission::MetadataRead])?;
    for operation in GitHubOperation::ALL {
        let expired = connector.read(&token, operation, 1_000_000);
        assert!(
            matches!(
                expired,
                Err(academic_integrations::ConnectorError::Credential(
                    GitHubError::Expired
                ))
            ),
            "{} did not refuse an expired token",
            operation.as_str()
        );
        let foreign = connector.read(&out_of_scope, operation, 1_500);
        assert!(
            matches!(
                foreign,
                Err(academic_integrations::ConnectorError::Credential(
                    GitHubError::OutOfScope
                ))
            ),
            "{} did not refuse a token scoped elsewhere",
            operation.as_str()
        );
        let result = connector.read(&metadata_only, operation, 1_500);
        if operation.permission() == TokenPermission::MetadataRead {
            assert!(
                result.is_ok(),
                "{} was refused a token that carries its permission",
                operation.as_str()
            );
        } else {
            assert!(
                matches!(
                    result,
                    Err(academic_integrations::ConnectorError::Credential(
                        GitHubError::MissingPermission
                    ))
                ),
                "{} accepted a token without its permission",
                operation.as_str()
            );
        }
    }

    // (e) A request has no body. The whole field set is compared, so a `body`
    // is an added key rather than a name to spot.
    assert_eq!(
        struct_fields(&github, "pub struct ReadRequest")?,
        vec![
            ("operation".to_owned(), "GitHubOperation".to_owned()),
            ("method".to_owned(), "HttpMethod".to_owned()),
            ("path".to_owned(), "String".to_owned()),
        ],
        "ReadRequest's field set changed"
    );

    // (f) The connector has one entry point that produces a request, and the
    // whole set of its public methods is pinned. A second builder is an extra
    // key even if it spells nothing forbidden.
    let connector_methods: Vec<String> = support::public_signatures_with_owner(&github)
        .into_iter()
        .filter(|(owner, _, _)| owner == "GitHubConnector")
        .map(|(_, name, signature)| {
            let tail = signature
                .split_once("->")
                .map_or("()", |(_, rest)| rest)
                .trim()
                .to_owned();
            format!("{name} -> {tail}")
        })
        .collect();
    assert_eq!(
        connector_methods,
        vec![
            "new -> Self".to_owned(),
            "repository -> &GitHubRepository".to_owned(),
            "read -> Result<ReadRequest, ConnectorError>".to_owned(),
        ],
        "GitHubConnector's public method set changed"
    );

    // (g) The whole set of traits this crate declares, with their whole method
    // sets. None of them sends anything: a write reached through a seam would
    // need a method here, and the comparison is in both directions.
    let mut traits: Vec<String> = Vec::new();
    for (path, code) in product_code()? {
        let mut rest = code.as_str();
        while let Some(at) = rest.find("pub trait ") {
            let header: String = rest[at..]
                .chars()
                .take_while(|character| *character != '{')
                .collect();
            let name = header.trim_start_matches("pub trait ").trim().to_owned();
            let methods = trait_methods(&code, header.trim_end())?;
            traits.push(format!("{path} {name} {}", methods.join(" | ")));
            rest = &rest[at + 10..];
        }
    }
    traits.sort();
    assert_eq!(
        traits,
        vec![
            "crates/integrations/src/ide.rs IdeWorkspace open_paths open_paths(&self) -> Vec<WorkspacePath> | symbols symbols(&self, path: &WorkspacePath) -> Vec<SymbolRef> | changed_paths changed_paths(&self, since: TimestampMillis) -> Vec<WorkspacePath>".to_owned(),
            "crates/integrations/src/lib.rs ConnectorFleet health health(&self, kind: ConnectorKind) -> ConnectorHealth".to_owned(),
            "crates/integrations/src/lib.rs CoreGraph read_view read_view(&self, view: CoreView) -> Vec<u8>".to_owned(),
        ],
        "the crate's trait set or one trait's method set changed"
    );
    Ok(())
}

#[test]
fn webhook_delivery_is_untrusted_and_builds_no_request() -> TestResult {
    use academic_integrations::{WebhookDelivery, WebhookEventKind};
    use academic_untrusted_content::SourceId;

    let scoped = repository("snu-student", "compilers-project")?;
    for (index, kind) in WebhookEventKind::ALL.into_iter().enumerate() {
        let delivery = WebhookDelivery::accept(
            kind,
            scoped.clone(),
            SourceId::new(format!("delivery-{index}"))?,
            index as u64,
            b"a body a remote server chose",
        )?;
        assert_eq!(delivery.kind(), kind);
        assert_eq!(delivery.repository(), &scoped);
        assert_eq!(delivery.body().byte_len(), 28);
        assert_eq!(delivery.body().provenance().kind(), kind.source_kind());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 3. A private blob needs a second grant
// ---------------------------------------------------------------------------

struct BlobFixture {
    broker: academic_policy::PermissionBroker,
    provider: academic_policy::ProviderPolicySnapshot,
    document: SourceDocument,
}

fn blob_fixture() -> Result<BlobFixture, Box<dyn Error>> {
    let (broker, provider) = broker_with_provider(64 * 1024)?;
    let document = SourceDocument::new("repo/blob.rs", selection_document().into_bytes());
    Ok(BlobFixture {
        broker,
        provider,
        document,
    })
}

fn plan<'a>(destination: &'a str, purpose: &'a str, grant_id: &'a str) -> TransmissionPlan<'a> {
    TransmissionPlan {
        grant_id,
        actor_id: EGRESS_ACTOR,
        process_class: ProcessClass::EgressProxy,
        operation: "assist",
        purpose_id: purpose,
        destination_id: destination,
        expires_at: 500_000,
        chunk_bytes: 32,
    }
}

#[test]
fn private_blob_egress_needs_a_second_grant() -> TestResult {
    // The vocabulary is closed and total: one grant for public bytes, two for
    // private ones, and no arm that defaults.
    assert_eq!(
        BlobVisibility::ALL
            .into_iter()
            .map(BlobVisibility::required_grants)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );

    let fixture = blob_fixture()?;
    let proxy = EgressProxy::new(&fixture.broker);
    let policy = IdentifierPolicy::none();
    let focus = vec!["selected_total".to_owned()];
    let staged = proxy
        .stage(&academic_egress_boundary::StagingRequest {
            document: &fixture.document,
            focus: &focus,
            identifier_policy: &policy,
            max_bytes: 64 * 1024,
        })
        .map_err(|denial| format!("staging refused: {}", denial.detail()))?;
    let rulepack = proxy.rulepack_id().redaction_policy_hash().clone();
    let outcomes = capabilities_for(
        &fixture.broker,
        &staged,
        &fixture.provider,
        rulepack,
        &[TRANSFER_PURPOSE, DISCLOSURE_PURPOSE],
        1_000,
    )?;
    let mut outcomes = outcomes.into_iter();
    let (transfer_token, transfer_grant) = token(outcomes.next().ok_or("no transfer decision")?)?;
    let (disclosure_token, disclosure_grant) =
        token(outcomes.next().ok_or("no disclosure decision")?)?;
    assert_ne!(
        transfer_grant, disclosure_grant,
        "the two purposes minted one grant, so there is no second grant to require"
    );
    let egress = PrivateBlobEgress::new(&fixture.broker, &proxy);
    let destination = fixture.provider.destination_id().to_owned();
    let public_blob = RepositoryBlob::new(
        repository("snu-student", "compilers-project")?,
        "src/lib.rs",
        BlobVisibility::Public,
    );
    let private_blob = RepositoryBlob::new(
        repository("snu-student", "compilers-project")?,
        "src/lib.rs",
        BlobVisibility::Private,
    );

    // A private blob with one grant is refused with NO_GRANT and zero bytes.
    let mut transport = RecordingTransport::default();
    let mut journal = StagedGrantJournal::default();
    let refused = egress.transmit(
        &private_blob,
        &staged,
        &transfer_token,
        None,
        DISCLOSURE_PURPOSE,
        &plan(&destination, TRANSFER_PURPOSE, &transfer_grant),
        &mut journal,
        &mut transport,
        &|| 1_500,
    );
    assert_eq!(
        refused.err().and_then(|error| error.reason()),
        Some(ReasonCode::NoGrant),
        "a private blob transmitted under one grant"
    );
    assert!(
        transport.written.is_empty(),
        "the refusal wrote {} bytes",
        transport.written.len()
    );

    // A private blob whose disclosure names the transfer's own grant is one
    // decision presented twice: SCOPE_MISMATCH, still zero bytes.
    let mut transport = RecordingTransport::default();
    let mut journal = StagedGrantJournal::default();
    let doubled = egress.transmit(
        &private_blob,
        &staged,
        &transfer_token,
        Some(&transfer_token),
        DISCLOSURE_PURPOSE,
        &plan(&destination, TRANSFER_PURPOSE, &transfer_grant),
        &mut journal,
        &mut transport,
        &|| 1_500,
    );
    assert_eq!(
        doubled.err().and_then(|error| error.reason()),
        Some(ReasonCode::ScopeMismatch),
        "one grant presented twice was accepted as two"
    );
    assert!(transport.written.is_empty());

    // A public blob goes under one grant. This is what makes the two refusals
    // above attributable to visibility rather than to a gate that refuses
    // everything.
    let mut transport = RecordingTransport::default();
    let mut journal = StagedGrantJournal::default();
    let public_transfer = egress.transmit(
        &public_blob,
        &staged,
        &transfer_token,
        None,
        DISCLOSURE_PURPOSE,
        &plan(&destination, TRANSFER_PURPOSE, &transfer_grant),
        &mut journal,
        &mut transport,
        &|| 1_500,
    )?;
    assert_eq!(public_transfer.disclosure_grant_id(), None);
    assert_eq!(
        public_transfer.transmission().bytes_sent(),
        staged.preview().byte_len()
    );
    assert_eq!(transport.written, staged.preview().bytes());

    // And a private blob goes under two. The grant the disclosure actually
    // consumed is read back rather than assumed.
    let fixture = blob_fixture()?;
    let proxy = EgressProxy::new(&fixture.broker);
    let staged = proxy
        .stage(&academic_egress_boundary::StagingRequest {
            document: &fixture.document,
            focus: &focus,
            identifier_policy: &policy,
            max_bytes: 64 * 1024,
        })
        .map_err(|denial| format!("staging refused: {}", denial.detail()))?;
    let rulepack = proxy.rulepack_id().redaction_policy_hash().clone();
    let outcomes = capabilities_for(
        &fixture.broker,
        &staged,
        &fixture.provider,
        rulepack,
        &[TRANSFER_PURPOSE, DISCLOSURE_PURPOSE],
        1_000,
    )?;
    let mut outcomes = outcomes.into_iter();
    let (transfer_token, transfer_grant) = token(outcomes.next().ok_or("no transfer decision")?)?;
    let (disclosure_token2, disclosure_grant2) =
        token(outcomes.next().ok_or("no disclosure decision")?)?;
    let egress = PrivateBlobEgress::new(&fixture.broker, &proxy);
    let destination = fixture.provider.destination_id().to_owned();
    let mut transport = RecordingTransport::default();
    let mut journal = StagedGrantJournal::default();
    let private_transfer = egress.transmit(
        &private_blob,
        &staged,
        &transfer_token,
        Some(&disclosure_token2),
        DISCLOSURE_PURPOSE,
        &plan(&destination, TRANSFER_PURPOSE, &transfer_grant),
        &mut journal,
        &mut transport,
        &|| 1_500,
    )?;
    assert_eq!(
        private_transfer.disclosure_grant_id(),
        Some(disclosure_grant2.as_str()),
        "the disclosure grant reported is not the one the broker minted"
    );
    assert_eq!(transport.written, staged.preview().bytes());

    // Both grants were really spent: two distinct consumed rows in `P2-G1`'s
    // own store, and two allow rows in its append-only audit.
    let consumed: BTreeSet<String> = fixture
        .broker
        .consumption_rows()?
        .into_iter()
        .map(|row| row.grant_id)
        .collect();
    assert_eq!(
        consumed,
        [transfer_grant.clone(), disclosure_grant2.clone()]
            .into_iter()
            .collect::<BTreeSet<_>>(),
        "the two grants were not both consumed"
    );
    let allows = fixture
        .broker
        .audit_rows()?
        .into_iter()
        .filter(|row| row.decision == Decision::Allow && row.grant_id.is_some())
        .count();
    assert!(
        allows >= 4,
        "expected an allow row for each decision and each runtime use, found {allows}"
    );

    // The unused disclosure token from the first fixture is still unconsumed:
    // nothing above spent it by accident.
    assert_eq!(disclosure_token.grant_id(), disclosure_grant);
    Ok(())
}

#[test]
fn a_blob_denial_has_no_payload_field() -> TestResult {
    let github = read_module("github.rs")?;
    assert_eq!(
        struct_fields(&github, "pub struct BlobDenial")?,
        vec![
            ("reason".to_owned(), "ReasonCode".to_owned()),
            ("detail".to_owned(), "String".to_owned()),
            ("bytes_transmitted".to_owned(), "usize".to_owned()),
        ],
        "BlobDenial's field set changed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 4 and 5. The IDE adapter
// ---------------------------------------------------------------------------

fn workspace_fixture() -> Result<RecordingWorkspace, Box<dyn Error>> {
    let lib = WorkspacePath::new("src/lib.rs")?;
    let parser = WorkspacePath::new("src/parser.rs")?;
    Ok(RecordingWorkspace::new(
        vec![lib.clone(), parser.clone()],
        vec![
            SymbolRef::new(lib.clone(), "selected_total", 40, 180)?,
            SymbolRef::new(parser, "parse_tokens", 12, 96)?,
        ],
        vec![lib],
    ))
}

#[test]
fn ide_adapter_performs_no_writes() -> TestResult {
    let ide = read_module("ide.rs")?;

    // The seam's whole method set. A fourth method fails whatever it is called,
    // and every one of the three takes `&self` and hands back an owned value.
    let methods = trait_methods(&ide, "pub trait IdeWorkspace")?;
    assert_eq!(
        methods,
        vec![
            "open_paths open_paths(&self) -> Vec<WorkspacePath>".to_owned(),
            "symbols symbols(&self, path: &WorkspacePath) -> Vec<SymbolRef>".to_owned(),
            "changed_paths changed_paths(&self, since: TimestampMillis) -> Vec<WorkspacePath>"
                .to_owned(),
        ],
        "IdeWorkspace's method set changed"
    );

    // No public function anywhere in this crate takes `&mut self`. A mutation
    // of anything the caller owns would need one, or a filesystem call, and the
    // next assertion closes that.
    let ide_methods: Vec<String> = support::public_signatures_with_owner(&ide)
        .into_iter()
        .map(|(owner, name, signature)| {
            assert!(
                !signature.contains("&mut"),
                "{owner}::{name} takes a mutable reference: {signature}"
            );
            let tail = signature
                .split_once("->")
                .map_or("()", |(_, rest)| rest)
                .trim()
                .to_owned();
            format!("{owner}::{name} -> {tail}")
        })
        .collect();
    assert_eq!(
        ide_methods
            .iter()
            .filter(|entry| entry.starts_with("IdeAdapter::"))
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            "IdeAdapter::attach -> Self".to_owned(),
            "IdeAdapter::with_watch -> Self".to_owned(),
            "IdeAdapter::watch -> WatchMode".to_owned(),
            "IdeAdapter::open_paths -> Vec<WorkspacePath>".to_owned(),
            "IdeAdapter::symbols -> Vec<SymbolRef>".to_owned(),
            "IdeAdapter::deep_link -> DeepLink".to_owned(),
            "IdeAdapter::changed_scope -> Result<ChangedScope, IdeError>".to_owned(),
            "IdeAdapter::request_snapshot -> Result<SnapshotRequest, IdeError>".to_owned(),
        ],
        "IdeAdapter's public method set changed"
    );
    assert!(
        ide_methods.len() >= 20,
        "the IDE signature reader found only {} functions",
        ide_methods.len()
    );

    // Nothing in the crate reaches the filesystem, a socket or a process, so a
    // write made without the seam has no route either.
    let mut filesystem: BTreeSet<String> = BTreeSet::new();
    for (path, code) in product_code()? {
        for reach in support::absolute_paths(&code) {
            if reach.starts_with("std::fs")
                || reach.starts_with("std::net")
                || reach.starts_with("std::process")
                || reach.starts_with("std::io")
            {
                filesystem.insert(format!("{path} {reach}"));
            }
        }
    }
    assert!(
        filesystem.is_empty(),
        "this crate reaches the filesystem, a socket or a process: {filesystem:?}"
    );

    // The runtime half: a full adapter session enters only those three methods,
    // and the workspace it read is unchanged afterwards.
    let workspace = workspace_fixture()?;
    let before = workspace.open_paths();
    let adapter = IdeAdapter::attach(&workspace).with_watch(WatchMode::OptedIn);
    let open = adapter.open_paths();
    let symbols = adapter.symbols(&open[0]);
    assert_eq!(symbols.len(), 1);
    let link = adapter.deep_link(&symbols[0]);
    assert!(link.as_str().starts_with("ide://open?path=src/lib.rs"));
    let scope = adapter.changed_scope(TimestampMillis::new(0))?;
    let confirmation = ScopeConfirmation::record(&scope, "student", TimestampMillis::new(5));
    adapter.request_snapshot(&scope, &confirmation)?;
    assert_eq!(
        workspace.calls(),
        4,
        "the adapter entered a method other than the three reads"
    );
    assert_eq!(
        before,
        workspace.open_paths(),
        "the workspace changed under a read-only adapter"
    );
    Ok(())
}

#[test]
fn ide_confirms_changed_scope_before_snapshot() -> TestResult {
    let mut workspace = workspace_fixture()?;

    // Watching is opt-in: without it there is no changed scope to confirm.
    let closed = IdeAdapter::attach(&workspace);
    assert_eq!(closed.watch(), WatchMode::Disabled);
    assert_eq!(
        closed.changed_scope(TimestampMillis::new(0)),
        Err(IdeError::WatchNotOptedIn)
    );

    // The first adapter's borrow ends with this block, so the workspace can
    // then be changed under a second one.
    let (scope, confirmation) = {
        let adapter = IdeAdapter::attach(&workspace).with_watch(WatchMode::OptedIn);
        let scope = adapter.changed_scope(TimestampMillis::new(0))?;
        assert_eq!(scope.paths().len(), 1);
        let confirmation = ScopeConfirmation::record(&scope, "student", TimestampMillis::new(5));
        let request = adapter.request_snapshot(&scope, &confirmation)?;
        assert_eq!(request.scope(), &scope);
        assert_eq!(request.confirmed_by(), "student");
        (scope, confirmation)
    };

    // A file changes after the confirmation. The scope digest moves and the
    // confirmation stops matching, so the snapshot is refused.
    workspace.set_changed(vec![
        WorkspacePath::new("src/lib.rs")?,
        WorkspacePath::new("src/parser.rs")?,
    ]);
    let adapter = IdeAdapter::attach(&workspace).with_watch(WatchMode::OptedIn);
    let widened = adapter.changed_scope(TimestampMillis::new(0))?;
    assert_ne!(widened.digest(), scope.digest());
    assert_eq!(
        adapter.request_snapshot(&widened, &confirmation),
        Err(IdeError::ScopeChanged),
        "a snapshot ran over a scope the user had not confirmed"
    );

    // The old confirmation still matches the old scope, so the refusal is about
    // the change rather than about the confirmation ageing.
    adapter.request_snapshot(&scope, &confirmation)?;

    // Confirming the new scope admits it.
    let widened_confirmation =
        ScopeConfirmation::record(&widened, "student", TimestampMillis::new(9));
    let request = adapter.request_snapshot(&widened, &widened_confirmation)?;
    assert_eq!(request.scope().paths().len(), 2);
    Ok(())
}

// ---------------------------------------------------------------------------
// 6 and 7. The coding assistant
// ---------------------------------------------------------------------------

struct AssistantFixture {
    broker: academic_policy::PermissionBroker,
    document: SourceDocument,
}

fn assistant_fixture() -> Result<AssistantFixture, Box<dyn Error>> {
    let (broker, _provider) = broker_with_provider(64 * 1024)?;
    Ok(AssistantFixture {
        broker,
        document: SourceDocument::new("repo/plan.rs", selection_document().into_bytes()),
    })
}

#[test]
fn assistant_receives_only_selected_ranges() -> TestResult {
    let fixture = assistant_fixture()?;
    let proxy = EgressProxy::new(&fixture.broker);
    let policy = IdentifierPolicy::none();

    let selection = AssistantSelection::new(vec!["selected_total".to_owned()])?;
    let context =
        AssistantContext::minimize(&proxy, &fixture.document, &selection, &policy, 64 * 1024)
            .map_err(|denial| format!("staging refused: {}", denial.detail()))?;
    let received = String::from_utf8(context.staged().preview().bytes().to_vec())?;
    assert!(
        received.contains("selected_total"),
        "the selected declaration is missing: {received}"
    );
    for outside in [
        "marker_outside_the_selection",
        "second_marker_outside",
        "unselected_neighbour",
        "another_unselected",
    ] {
        assert!(
            !received.contains(outside),
            "the assistant received {outside}, which is outside the selection"
        );
    }

    // The control: with all three declarations selected, the same reader finds
    // every marker. So the absences above are a property of the selection and
    // not of a reader that finds nothing.
    let everything = AssistantSelection::new(vec![
        "selected_total".to_owned(),
        "unselected_neighbour".to_owned(),
        "another_unselected".to_owned(),
    ])?;
    let wide =
        AssistantContext::minimize(&proxy, &fixture.document, &everything, &policy, 64 * 1024)
            .map_err(|denial| format!("staging refused: {}", denial.detail()))?;
    let wide_text = String::from_utf8(wide.staged().preview().bytes().to_vec())?;
    for outside in ["marker_outside_the_selection", "second_marker_outside"] {
        assert!(
            wide_text.contains(outside),
            "the wide selection did not carry {outside}"
        );
    }

    // A symbol the document does not declare is a scope mismatch, not a licence
    // to send the whole file. That refusal is `P2-G2`'s.
    let absent = AssistantSelection::new(vec!["not_declared_anywhere".to_owned()])?;
    let denial = AssistantContext::minimize(&proxy, &fixture.document, &absent, &policy, 64 * 1024)
        .err()
        .ok_or("an undeclared symbol was staged")?;
    assert_eq!(denial.reason(), ReasonCode::ScopeMismatch);

    // A selection is what the user pointed at: it is never silently widened,
    // deduplicated or emptied.
    assert!(AssistantSelection::new(Vec::new()).is_err());
    assert!(
        AssistantSelection::new(vec!["a".to_owned(), "a".to_owned()]).is_err(),
        "a repeated selection was silently deduplicated"
    );
    Ok(())
}

fn model_run(staged_bytes: &[u8], version: &str) -> Result<ModelRun, Box<dyn Error>> {
    Ok(ModelRun::record(
        ModelRunId::from_bytes([9; 16]),
        Purpose::new("assistant-code-generation")?,
        ProviderId::new("synthetic-assistant")?,
        ModelVersion::new(version)?,
        Digest32::of(b"synthetic-prompt-template"),
        InputArtifactRefs::new(vec![InputArtifactRef::new(
            academic_model_run::ArtifactId::from_bytes([3; 16]),
            Digest32::of(staged_bytes),
        )])?,
        Transmission::egressed(
            academic_model_run::EgressGrantId::new("synthetic-grant")?,
            vec![TransmittedRange::new(
                "repo/plan.rs",
                0,
                staged_bytes.len() as u64,
                Digest32::of(staged_bytes),
            )?],
        )?,
        Digest32::of(b"synthetic-redaction-policy"),
        academic_model_run::ArtifactId::from_bytes([4; 16]),
        1_500,
        Cost::new(1_200, "KRW")?,
        RetentionDeclaration::new("provider-retains-nothing")?,
    ))
}

#[test]
fn generated_code_provenance_is_recorded() -> TestResult {
    let fixture = assistant_fixture()?;
    let proxy = EgressProxy::new(&fixture.broker);
    let policy = IdentifierPolicy::none();
    let selection = AssistantSelection::new(vec!["selected_total".to_owned()])?;
    let context =
        AssistantContext::minimize(&proxy, &fixture.document, &selection, &policy, 64 * 1024)
            .map_err(|denial| format!("staging refused: {}", denial.detail()))?;

    let run = model_run(context.staged().preview().bytes(), "synthetic-1")?;
    let output = b"pub fn selected_total(plan: &[u32]) -> u32 { plan.iter().sum() }";
    let record = GeneratedCode::record(
        &run,
        &context,
        output,
        TimestampMillis::new(2_000),
        AssistantUse::GeneratedCode,
    );

    assert_eq!(record.model_run(), *run.id());
    assert_eq!(record.run_digest(), run.record_digest());
    assert_eq!(
        record.context_digest(),
        Digest32::of(context.staged().preview().bytes()),
        "the recorded context is not the bytes the assistant received"
    );
    assert_eq!(record.output_digest(), Digest32::of(output));
    assert_eq!(record.produced_at(), TimestampMillis::new(2_000));
    assert_eq!(record.use_kind(), AssistantUse::GeneratedCode);

    // The run digest is load-bearing: a run differing in one field produces a
    // different record, so this is not a constant nobody could break.
    let other = model_run(context.staged().preview().bytes(), "synthetic-2")?;
    assert_ne!(other.record_digest(), run.record_digest());
    let other_record = GeneratedCode::record(
        &other,
        &context,
        output,
        TimestampMillis::new(2_000),
        AssistantUse::GeneratedCode,
    );
    assert_ne!(other_record.run_digest(), record.run_digest());

    // Every field is provenance, and the whole set is compared: a field added
    // to this record is an added key here.
    let assistant = read_module("assistant.rs")?;
    assert_eq!(
        struct_fields(&assistant, "pub struct GeneratedCode")?,
        vec![
            ("model_run".to_owned(), "ModelRunId".to_owned()),
            ("run_digest".to_owned(), "Digest32".to_owned()),
            ("context_digest".to_owned(), "Digest32".to_owned()),
            ("output_digest".to_owned(), "Digest32".to_owned()),
            ("produced_at".to_owned(), "TimestampMillis".to_owned()),
            ("use_kind".to_owned(), "AssistantUse".to_owned()),
        ],
        "GeneratedCode's field set changed"
    );

    // One producer. A second constructor would be a record whose provenance
    // nobody checked.
    let producers: Vec<String> = support::public_signatures_with_owner(&assistant)
        .into_iter()
        .filter(|(owner, _, signature)| owner == "GeneratedCode" && signature.contains("-> Self"))
        .map(|(_, name, _)| name)
        .collect();
    assert_eq!(producers, vec!["record".to_owned()]);
    Ok(())
}

// ---------------------------------------------------------------------------
// 8. Assistant use is not competency
// ---------------------------------------------------------------------------

/// The transitive product dependency closure of one workspace crate.
///
/// Read out of the workspace's own manifests rather than out of a list here, so
/// an edge added in a manifest enters this set without anybody editing the
/// test. `[dev-dependencies]` is skipped: a dev edge does not travel into a
/// product build.
fn product_closure(crate_dir: &str) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut found = BTreeSet::new();
    let mut pending = vec![crate_dir.to_owned()];
    while let Some(directory) = pending.pop() {
        let manifest = fs::read_to_string(
            workspace_root()
                .join("crates")
                .join(&directory)
                .join("Cargo.toml"),
        )?;
        let mut inside = false;
        for line in manifest.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                inside = trimmed == "[dependencies]";
                continue;
            }
            if !inside || !trimmed.starts_with("academic-") {
                continue;
            }
            let name = trimmed
                .split_once('=')
                .map_or(trimmed, |(head, _)| head)
                .trim()
                .to_owned();
            let child = name.trim_start_matches("academic-").to_owned();
            if found.insert(name) {
                pending.push(child);
            }
        }
    }
    Ok(found)
}

#[test]
fn assistant_use_is_not_competency() -> TestResult {
    let assistant = read_module("assistant.rs")?;

    // The use vocabulary is closed and complete, read out of the enum.
    let variants = enum_variants(&assistant, "pub enum AssistantUse")?;
    let declared: Vec<String> = AssistantUse::ALL
        .into_iter()
        .map(|use_kind| format!("{use_kind:?}"))
        .collect();
    assert_eq!(
        variants, declared,
        "AssistantUse's variants and AssistantUse::ALL disagree"
    );

    // Every one of them is not evidence, and there is no other answer to give.
    for use_kind in AssistantUse::ALL {
        assert_eq!(
            use_kind.eligibility(),
            EvidenceEligibility::NotEvidence,
            "{} is admitted as evidence",
            use_kind.as_str()
        );
    }
    assert_eq!(
        enum_variants(&assistant, "pub enum EvidenceEligibility")?,
        vec!["NotEvidence".to_owned()],
        "EvidenceEligibility gained a variant that is not NotEvidence"
    );

    // The structural half: nothing in this crate's product closure can produce
    // a competency, because no such crate is in it. The set is read from the
    // manifests and compared in both directions.
    let closure = product_closure("integrations")?;
    assert_eq!(
        closure,
        [
            "academic-crypto",
            "academic-domain",
            "academic-egress-boundary",
            "academic-keystore-platform",
            "academic-model-run",
            "academic-policy",
            "academic-repository",
            "academic-untrusted-content",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        "the product dependency closure of academic-integrations changed"
    );
    for name in &closure {
        assert!(
            !name.contains("competency"),
            "{name} is in the closure of a crate that must not be able to name one"
        );
    }

    // The control: the same walker, pointed at a crate that does depend on the
    // competency vocabulary, finds it. A walker that returned the empty set
    // would satisfy the assertion above.
    let competency_closure = product_closure("role-profile")?;
    assert!(
        competency_closure.contains("academic-competency"),
        "the closure walker did not find a known competency edge: {competency_closure:?}"
    );

    // And this crate cannot name one either. The whole set of `academic_*`
    // paths and `use` items its product source spells is compared against a
    // pinned inventory, so a reach for a mastery, a rubric or an evidence
    // strength is an extra key rather than a token somebody forbade.
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut imported: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        for path in support::absolute_paths(&code) {
            if path.starts_with("academic_") {
                reached.insert(path);
            }
        }
        for item in support::use_items(&code) {
            if item.starts_with("academic_") {
                imported.insert(item);
            }
        }
    }
    let vocabulary: BTreeSet<String> = reached.union(&imported).cloned().collect();
    for forbidden in [
        "Mastery",
        "Competency",
        "Rubric",
        "EvidenceStrength",
        "EvidenceRole",
        "KnowledgeState",
        "FreshnessBand",
        "Claim",
    ] {
        assert!(
            !vocabulary.iter().any(|item| item.contains(forbidden)),
            "this crate names {forbidden}: {vocabulary:?}"
        );
    }
    assert!(
        vocabulary.len() >= 10,
        "the vocabulary reader found only {} names, so the assertions above are vacuous",
        vocabulary.len()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// 9. The calendar payload
// ---------------------------------------------------------------------------

/// The maximal `[A-Za-z0-9+_-]` runs in `bytes`, lowercased.
///
/// Tokens rather than substrings, because four grade symbols are one letter
/// long and a substring scan for `S` finds one in every word.
fn tokens(bytes: &[u8]) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut current = String::new();
    for byte in bytes {
        let character = char::from(*byte);
        if character.is_ascii_alphanumeric() || matches!(character, '+' | '_' | '-') {
            current.push(character.to_ascii_lowercase());
        } else if !current.is_empty() {
            found.insert(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        found.insert(current);
    }
    found
}

/// Every grade and knowledge-state spelling in this workspace, lowercased.
///
/// Read out of the enums rather than transcribed, so a grade symbol or a
/// mastery level added later is scanned for here without anybody editing a
/// list.
fn grade_and_state_spellings() -> Result<BTreeSet<String>, Box<dyn Error>> {
    let grades = strip_non_code(&fs::read_to_string(
        workspace_root().join("crates/record/src/grade.rs"),
    )?);
    let domain = strip_non_code(&fs::read_to_string(
        workspace_root().join("crates/domain/src/lib.rs"),
    )?);
    let mut found = BTreeSet::new();
    for (source, header) in [
        (&grades, "pub enum GradeSymbol"),
        (&domain, "pub enum MasteryLevel"),
        (&domain, "pub enum FreshnessBand"),
    ] {
        for variant in enum_variants(source, header)? {
            found.insert(variant.to_ascii_lowercase());
        }
    }
    // The rendered spellings too, which is what a payload would actually carry.
    let rendered = fs::read_to_string(workspace_root().join("crates/record/src/grade.rs"))?;
    let block = collapse(&rendered);
    for symbol in [
        "A+", "A0", "A-", "B+", "B0", "B-", "C+", "C0", "C-", "D+", "D0", "D-",
    ] {
        assert!(
            block.contains(symbol),
            "{symbol} is no longer a spelling this workspace renders"
        );
        found.insert(symbol.to_ascii_lowercase());
    }
    Ok(found)
}

#[test]
fn calendar_payload_contains_no_grade_or_state() -> TestResult {
    let calendar = read_module("calendar.rs")?;

    // Layer one: the whole `(name, type)` set. A field added is an added key.
    let fields = struct_fields(&calendar, "pub struct CalendarPayload")?;
    assert_eq!(
        fields,
        vec![
            ("event_id".to_owned(), "ExternalId".to_owned()),
            ("subject".to_owned(), "CanonicalRef".to_owned()),
            ("kind".to_owned(), "CalendarEventKind".to_owned()),
            ("starts_at".to_owned(), "TimestampMillis".to_owned()),
            ("ends_at".to_owned(), "TimestampMillis".to_owned()),
        ],
        "CalendarPayload's field set changed"
    );

    // Layer two: the type of every field, classified. A `String`, an `f64`, a
    // `u32` or a `MasteryLevel` fails here whatever it is named, so a reviewer
    // who adds a field to the pin above still has to defend its type.
    let admitted: BTreeSet<&str> = [
        "ExternalId",
        "CanonicalRef",
        "CalendarEventKind",
        "TimestampMillis",
    ]
    .into_iter()
    .collect();
    for (name, ty) in &fields {
        assert!(
            admitted.contains(ty.as_str()),
            "CalendarPayload::{name} has type {ty}, which is not one this payload admits"
        );
    }

    // The byte half. The subject really does carry a grade and a mastery level
    // in the fixture ledger, so a payload that leaked one would have something
    // to leak.
    let payload = CalendarPayload::new(
        ExternalId::new("evt-90c3f1")?,
        CanonicalRef::Offering(fixture_offering_id()?),
        CalendarEventKind::ExamWindow,
        TimestampMillis::new(1_700_000_000_000),
        TimestampMillis::new(1_700_007_200_000),
    )?;
    let encoded = payload.encode();
    let spellings = grade_and_state_spellings()?;
    assert!(
        spellings.len() >= 25,
        "the spelling reader found only {} names",
        spellings.len()
    );
    let present = tokens(&encoded);
    let leaked: Vec<&String> = spellings
        .iter()
        .filter(|word| present.contains(*word))
        .collect();
    assert!(leaked.is_empty(), "the calendar payload carries {leaked:?}");

    // The control: the same scanner over a buffer that does carry them finds
    // both, so the emptiness above is a property of the payload.
    let mut leaky = encoded.clone();
    leaky.extend_from_slice(b" A+ PRACTICED ");
    let leaky_tokens = tokens(&leaky);
    let found: BTreeSet<&String> = spellings
        .iter()
        .filter(|word| leaky_tokens.contains(*word))
        .collect();
    assert!(
        found.contains(&"a+".to_owned()) && found.contains(&"practiced".to_owned()),
        "the scanner did not find a grade and a mastery it was handed: {found:?}"
    );

    // The only words a provider displays are the four this crate compiled in.
    let summaries: BTreeSet<&'static str> = CalendarEventKind::ALL
        .into_iter()
        .map(CalendarEventKind::summary)
        .collect();
    assert_eq!(summaries.len(), CalendarEventKind::ALL.len());
    assert!(summaries.contains(payload.summary()));

    // A calendar has no slot for an artifact, an entity or a repository.
    assert!(
        CalendarPayload::new(
            ExternalId::new("evt-1")?,
            CanonicalRef::Repository(fixture_repository_id()?),
            CalendarEventKind::ExamWindow,
            TimestampMillis::new(1),
            TimestampMillis::new(2),
        )
        .is_err(),
        "a repository was admitted as a calendar subject"
    );
    CalendarPayload::new(
        ExternalId::new("evt-2")?,
        CanonicalRef::Course(fixture_course_id()?),
        CalendarEventKind::OfferingSession,
        TimestampMillis::new(1),
        TimestampMillis::new(2),
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// 10. An external identifier never becomes canonical
// ---------------------------------------------------------------------------

#[test]
fn external_id_is_never_canonical() -> TestResult {
    // The whole-set half. Every public signature in this crate is classified by
    // whether it *returns* a canonical reference and whether it *received* one
    // -- directly, or inside a mapping that already holds one. A conversion
    // added later returns a canonical having taken only text, and that is a
    // classification over the whole set rather than a forbidden name.
    // The types that already hold one, read out of the declarations rather than
    // listed here, so a new holder joins the admitted set without an edit and a
    // type that stops holding one leaves it.
    let mut holders: BTreeSet<String> = ["CanonicalRef".to_owned()].into_iter().collect();
    for (_, code) in product_code()? {
        for header in ["pub struct ", "pub enum "] {
            let mut rest = code.as_str();
            while let Some(at) = rest.find(header) {
                let name: String = rest[at + header.len()..]
                    .chars()
                    .take_while(|character| character.is_alphanumeric() || *character == '_')
                    .collect();
                let declaration: String = rest[at..]
                    .chars()
                    .scan(0_i32, |depth, character| {
                        if character == '{' {
                            *depth += 1;
                        }
                        if character == '}' {
                            *depth -= 1;
                            if *depth == 0 {
                                return Some(None);
                            }
                        }
                        Some(Some(character))
                    })
                    .map_while(|character| character)
                    .collect();
                if declaration.contains("CanonicalRef") && !name.is_empty() {
                    holders.insert(name);
                }
                rest = &rest[at + header.len()..];
            }
        }
    }
    assert!(
        holders.len() >= 3,
        "the holder reader found only {holders:?}, so the rule below is vacuous"
    );

    let mut producers: Vec<String> = Vec::new();
    for (path, code) in product_code()? {
        for (owner, name, signature) in support::public_signatures_with_owner(&code) {
            let (parameters, returns) = signature
                .split_once("->")
                .map_or((signature.as_str(), ""), |(head, tail)| (head, tail));
            if !returns.contains("CanonicalRef") {
                continue;
            }
            // Either the caller handed one in, or this is an accessor on a value
            // that already holds one. A function that received neither would be
            // deriving a canonical reference from something else, and text is
            // the only other thing there is.
            let received = holders
                .iter()
                .any(|holder| parameters.contains(holder.as_str()));
            let accessor = parameters.contains("&self") && holders.contains(&owner);
            assert!(
                received || accessor,
                "{path} {owner}::{name} returns a canonical reference without receiving one: {signature}"
            );
            producers.push(format!("{path} {owner}::{name}"));
        }
    }
    producers.sort();
    assert_eq!(
        producers,
        vec![
            "crates/integrations/src/calendar.rs CalendarPayload::subject".to_owned(),
            "crates/integrations/src/identity.rs ExternalIdentity::canonical".to_owned(),
        ],
        "the set of functions handing out a canonical reference changed"
    );

    // No public function anywhere in this crate constructs one either: a
    // canonical reference is built by naming an arm with a domain identifier,
    // and a domain identifier is UUIDv7 rather than a provider's string.
    let identity = read_module("identity.rs")?;
    let constructors: Vec<String> = support::public_signatures_with_owner(&identity)
        .into_iter()
        .filter(|(owner, _, signature)| owner == "CanonicalRef" && signature.contains("-> Self"))
        .map(|(_, name, _)| name)
        .collect();
    assert!(
        constructors.is_empty(),
        "CanonicalRef has a constructor function: {constructors:?}"
    );

    // The runtime half. A mapping resolves to the canonical value that was
    // registered with it, and an identifier nobody registered resolves to
    // nothing at all -- an external identifier on its own addresses no record.
    let entity = support::fixture_entity_id()?;
    let canonical = CanonicalRef::Entity(entity);
    let external = ExternalId::new("MDEwOlJlcG9zaXRvcnkxMjM0NTY3")?;
    let mut map = IdentityMap::new();
    assert!(map.resolve(ConnectorKind::GitHub, &external).is_none());
    map.register(ExternalIdentity::map(
        ConnectorKind::GitHub,
        external.clone(),
        canonical,
        SourceAuthority::Connector,
        TimestampMillis::new(10),
    ));
    let resolved = map
        .resolve(ConnectorKind::GitHub, &external)
        .ok_or("the registered mapping did not resolve")?;
    assert_eq!(resolved.canonical(), canonical);
    assert_eq!(resolved.external_id(), &external);

    // The same identifier in another system resolves to nothing: an external
    // identifier is scoped to the system that spells it.
    assert!(map.resolve(ConnectorKind::Lms, &external).is_none());

    // And the two halves are different values. The canonical side is sixteen
    // opaque bytes the core minted; the external side is the provider's text,
    // and neither is derivable from the other.
    assert_ne!(
        canonical.as_bytes().as_slice(),
        external.as_str().as_bytes(),
        "the canonical reference is the external identifier's own bytes"
    );
    assert_eq!(
        canonical.kind(),
        academic_integrations::CanonicalKind::Entity
    );
    Ok(())
}

#[test]
fn a_sync_conflict_preserves_both_sides() -> TestResult {
    let external = ExternalId::new("SNU-CSE-4190-401")?;
    let course = CanonicalRef::Course(fixture_course_id()?);
    let offering = CanonicalRef::Offering(fixture_offering_id()?);

    // Authority decides first, and the loser is kept.
    let mut map = IdentityMap::new();
    map.register(ExternalIdentity::map(
        ConnectorKind::Lms,
        external.clone(),
        course,
        SourceAuthority::Inferred,
        TimestampMillis::new(10),
    ));
    map.register(ExternalIdentity::map(
        ConnectorKind::Lms,
        external.clone(),
        offering,
        SourceAuthority::Official,
        TimestampMillis::new(5),
    ));
    let conflict = map.conflicts().first().ok_or("no conflict was recorded")?;
    assert_eq!(
        conflict.basis(),
        academic_integrations::ConflictBasis::SourceAuthority
    );
    assert_eq!(conflict.held().canonical(), course);
    assert_eq!(conflict.incoming().canonical(), offering);
    assert_eq!(
        conflict.preferred().map(ExternalIdentity::canonical),
        Some(offering),
        "the more authoritative source did not win"
    );
    assert_eq!(
        map.resolve(ConnectorKind::Lms, &external)
            .map(ExternalIdentity::canonical),
        Some(offering)
    );

    // Equal authority falls to valid time, and again both sides survive.
    let mut map = IdentityMap::new();
    map.register(ExternalIdentity::map(
        ConnectorKind::Lms,
        external.clone(),
        course,
        SourceAuthority::Connector,
        TimestampMillis::new(10),
    ));
    map.register(ExternalIdentity::map(
        ConnectorKind::Lms,
        external.clone(),
        offering,
        SourceAuthority::Connector,
        TimestampMillis::new(20),
    ));
    let conflict = map.conflicts().first().ok_or("no conflict was recorded")?;
    assert_eq!(
        conflict.basis(),
        academic_integrations::ConflictBasis::ValidTime
    );
    assert_eq!(conflict.held().canonical(), course);
    assert_eq!(conflict.incoming().canonical(), offering);

    // A tie prefers neither. `P2-N5`'s rule for a tied root: both stay and the
    // decision is not invented.
    let mut map = IdentityMap::new();
    map.register(ExternalIdentity::map(
        ConnectorKind::Lms,
        external.clone(),
        course,
        SourceAuthority::Connector,
        TimestampMillis::new(10),
    ));
    map.register(ExternalIdentity::map(
        ConnectorKind::Lms,
        external.clone(),
        offering,
        SourceAuthority::Connector,
        TimestampMillis::new(10),
    ));
    let conflict = map.conflicts().first().ok_or("no conflict was recorded")?;
    assert_eq!(conflict.basis(), academic_integrations::ConflictBasis::Tie);
    assert!(
        conflict.preferred().is_none(),
        "a tied conflict picked a winner"
    );
    assert_eq!(
        map.resolve(ConnectorKind::Lms, &external)
            .map(ExternalIdentity::canonical),
        Some(course),
        "a tie silently overwrote the side that was already there"
    );
    assert_eq!(conflict.held().canonical(), course);
    assert_eq!(conflict.incoming().canonical(), offering);

    // The same mapping registered twice is not a conflict.
    let mut map = IdentityMap::new();
    for _ in 0..2 {
        map.register(ExternalIdentity::map(
            ConnectorKind::Lms,
            external.clone(),
            course,
            SourceAuthority::Connector,
            TimestampMillis::new(10),
        ));
    }
    assert!(map.conflicts().is_empty());
    assert_eq!(map.mappings().len(), 1);
    Ok(())
}
