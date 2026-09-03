//! The seven acceptance tests the Phase 2 plan names for `P2-X7`.
//!
//! Each one is written so that deleting the rule it checks fails it. Where a
//! test could pass over an implementation that does nothing — an empty queue
//! partitions correctly, an unconditional marker is always present — it carries
//! a control that must fail, or a negative case that must be refused, beside
//! the positive one.
//!
//! Every value here is synthetic and built in process, as `CONTRIBUTING.md`
//! requires. Nothing opens a profile, a database, a socket or a window.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;

use academic_domain::{
    Actor, AuthorityClass, CapturePermissionId, ClaimId, ConfidencePermille, ConsentId,
    ContentDigest, EgressDecisionId, EntityId, EpistemicStatus, FindingId, LectureDocumentId,
    LectureSessionId, MasteryLevel, ModelRunId, PermissionLineageId, PredicateId, SnapshotId,
    TimestampMillis, TranscriptVersionId, ValidInterval, engines::RuleId, temporal::TimeCoordinates,
};
use academic_evidence_center::{
    CenterError, CenterItem, CenterSection, ConceptMergeProposal, ConflictBoard, ConflictCase,
    ConflictClass, ConflictLane, ConflictSide, CorrectionChoice, CorrectionLedger, CorrectionMarker,
    CorrectionOrigin, CorrectionOutcome, DeletionReceiptRef, DependentAction, DependentActionKind,
    DocumentRegionLocator, EvidenceCenter, ExpiringPermission, FindingClassification, InboxEntry,
    LowConfidenceSpan, ObjectRange, PermissionQueue, PermissionRef, ProjectClassificationProposal,
    ProposalClass, ProposalHeader, ProposalInbox, ProviderRef, ProviderSurface, ReceiptState,
    RelationProposal, Resolution, SourceChangeEntry, SpanKind, StateUpdateProposal,
    TranscriptLocator, TransmissionLog, TransmissionPurpose, TransmissionRecord, user_receipt,
};
use academic_ingestion::{
    Acquisition, AllowedFrequency, AuthenticationMethod, Completeness, ConnectorId, DeclaredTarget,
    Dependency, DependencyGraph, DependentId, DependentKind, DependentNode, DocumentChange,
    FetchOutcome, HeaderValue, HttpMetadata, IngestSeq, LastSuccess, ManifestDraft,
    NextVerification, OfficialDocument, ParserVersion, PersonalDataClass, RetrievalInstant,
    SourceCategory, SourceDiff, SourceOwnership, TermsLedger, TermsStatus, stage,
};
use academic_proposal::{ImpactPermille, ProposalId, RiskTier};

type TestResult = Result<(), Box<dyn Error>>;

// ---------------------------------------------------------------------------
// synthetic identifiers
// ---------------------------------------------------------------------------

/// `academic-domain` re-exports no `Uuid`, so identifiers are parsed from their
/// canonical text. The same helper `academic-requirement`'s suite uses.
mod uuid_bytes {
    /// The minimal surface `parse_id` needs.
    #[derive(Debug, Clone, Copy)]
    pub struct Uuid([u8; 16]);

    impl Uuid {
        #[must_use]
        pub const fn from_bytes(bytes: [u8; 16]) -> Self {
            Self(bytes)
        }

        #[must_use]
        pub fn hyphenated(self) -> String {
            let hex: String = self.0.iter().map(|byte| format!("{byte:02x}")).collect();
            format!(
                "{}-{}-{}-{}-{}",
                &hex[0..8],
                &hex[8..12],
                &hex[12..16],
                &hex[16..20],
                &hex[20..32]
            )
        }
    }
}

macro_rules! parse_id {
    ($kind:ty, $suffix:expr) => {{
        let mut bytes = [0_u8; 16];
        bytes[0..8].copy_from_slice(&[0x01, 0x90, 0x00, 0x00, 0x00, 0x00, 0x70, 0x00]);
        bytes[8] = 0x80;
        bytes[12..16].copy_from_slice(&u32::to_be_bytes($suffix));
        let text = uuid_bytes::Uuid::from_bytes(bytes).hyphenated();
        text.parse::<$kind>()
    }};
}

fn entity(suffix: u32) -> Result<EntityId, Box<dyn Error>> {
    Ok(parse_id!(EntityId, suffix)?)
}

fn claim(suffix: u32) -> Result<ClaimId, Box<dyn Error>> {
    Ok(parse_id!(ClaimId, suffix)?)
}

fn digest(seed: u8) -> ContentDigest {
    ContentDigest::from_sha256_bytes([seed; 32])
}

fn at(millis: i64) -> TimestampMillis {
    TimestampMillis::new(millis)
}

fn header(id: u64, tier: RiskTier) -> Result<ProposalHeader, Box<dyn Error>> {
    Ok(ProposalHeader::new(
        ProposalId::new(id),
        tier,
        ConfidencePermille::new(700)?,
        ImpactPermille::new(400)?,
        parse_id!(ModelRunId, 4_000)?,
        at(1_700_000_000_000),
    ))
}

/// One entry of every class, in section 25.13's order.
fn one_of_each_class() -> Result<Vec<InboxEntry>, Box<dyn Error>> {
    Ok(vec![
        InboxEntry::Relation(RelationProposal::new(
            header(1, RiskTier::MediumReview)?,
            entity(11)?,
            PredicateId::parse("concept.requires")?,
            entity(12)?,
            2,
        )),
        InboxEntry::ConceptMerge(ConceptMergeProposal::new(
            header(2, RiskTier::MediumReview)?,
            entity(13)?,
            entity(14)?,
            9,
        )),
        InboxEntry::ProjectClassification(ProjectClassificationProposal::new(
            header(3, RiskTier::MediumReview)?,
            entity(15)?,
            parse_id!(SnapshotId, 16)?,
            parse_id!(FindingId, 17)?,
            FindingClassification::Observed,
        )),
        InboxEntry::StateUpdate(StateUpdateProposal::new(
            header(4, RiskTier::HighApproval)?,
            entity(18)?,
            MasteryLevel::Practiced,
            MasteryLevel::Applied,
        )),
    ])
}

// ---------------------------------------------------------------------------
// proposal_inbox_holds_four_typed_classes
// ---------------------------------------------------------------------------

/// The four classes are four payload types, and the class of an entry is read
/// off the payload rather than carried beside it.
///
/// Four halves, each failing for a different reason:
///
/// 1. every class is produced by an entry of its own payload type, driven from
///    a table, so swapping two arms of `InboxEntry::class` moves two cells;
/// 2. `of_class` partitions the inbox with set equality in both directions and
///    no duplication — not a count, which a partition that dropped one entry
///    and emitted another twice would pass;
/// 3. beside it runs a deliberately lossy partition, and the test requires
///    *that* one to fail the same equality, so the assertion is not vacuous;
/// 4. the four payload types have disjoint field lists, which is what
///    `tests/compile_fail/the_four_proposal_payloads_are_not_interchangeable.rs`
///    observes as a compile error.
#[test]
fn proposal_inbox_holds_four_typed_classes() -> TestResult {
    // ---- Each class comes from its own payload type ----------------------
    let entries = one_of_each_class()?;
    let expected: [ProposalClass; 4] = [
        ProposalClass::Relation,
        ProposalClass::ConceptMerge,
        ProposalClass::ProjectClassification,
        ProposalClass::StateUpdate,
    ];
    assert_eq!(
        entries.len(),
        ProposalClass::ALL.len(),
        "the corpus does not carry one entry per class"
    );
    for (entry, class) in entries.iter().zip(expected) {
        assert_eq!(
            entry.class(),
            class,
            "an entry reports a class its payload type does not name"
        );
    }
    assert_eq!(
        entries
            .iter()
            .map(InboxEntry::class)
            .collect::<BTreeSet<_>>(),
        ProposalClass::ALL.into_iter().collect::<BTreeSet<_>>(),
        "the four entries do not cover the four classes"
    );

    // ---- The partition loses nothing and duplicates nothing ---------------
    //
    // Four hundred proposals, cycling the four classes and both `RiskTier`
    // extremes, so a partition that dropped one is visible.
    let mut inbox = ProposalInbox::new();
    let mut admitted: BTreeMap<u64, ProposalClass> = BTreeMap::new();
    for index in 0_u64..400 {
        let tier = if index % 2 == 0 {
            RiskTier::MediumReview
        } else {
            RiskTier::NonDelegable
        };
        let head = ProposalHeader::new(
            ProposalId::new(index),
            tier,
            ConfidencePermille::new(u16::try_from(index % 1_001)?)?,
            ImpactPermille::new(u16::try_from((index * 2) % 1_001)?)?,
            parse_id!(ModelRunId, 4_000)?,
            at(1_700_000_000_000 + i64::try_from(index)?),
        );
        let entry = match index % 4 {
            0 => InboxEntry::Relation(RelationProposal::new(
                head,
                entity(100)?,
                PredicateId::parse("concept.requires")?,
                entity(101)?,
                1,
            )),
            1 => InboxEntry::ConceptMerge(ConceptMergeProposal::new(
                head,
                entity(102)?,
                entity(103)?,
                3,
            )),
            2 => InboxEntry::ProjectClassification(ProjectClassificationProposal::new(
                head,
                entity(104)?,
                parse_id!(SnapshotId, 105)?,
                parse_id!(FindingId, 106)?,
                FindingClassification::Possible,
            )),
            _ => InboxEntry::StateUpdate(StateUpdateProposal::new(
                head,
                entity(107)?,
                MasteryLevel::Exposed,
                MasteryLevel::Understood,
            )),
        };
        admitted.insert(index, entry.class());
        inbox.admit(entry)?;
    }

    let mut partitioned: Vec<(u64, ProposalClass)> = Vec::new();
    for class in ProposalClass::ALL {
        for entry in inbox.of_class(class) {
            assert_eq!(entry.class(), class, "of_class returned another class");
            partitioned.push((entry.header().id().value(), class));
        }
    }
    let seen: BTreeSet<u64> = partitioned.iter().map(|(id, _)| *id).collect();
    assert_eq!(
        seen.len(),
        partitioned.len(),
        "the partition emitted one proposal twice"
    );
    assert_eq!(
        partitioned.into_iter().collect::<BTreeMap<_, _>>(),
        admitted,
        "the partition is not the admitted set"
    );

    // ---- The control: a lossy partition must fail the same comparison -----
    let lossy: BTreeMap<u64, ProposalClass> = ProposalClass::ALL
        .into_iter()
        .flat_map(|class| {
            inbox
                .of_class(class)
                .into_iter()
                .skip(1)
                .map(move |entry| (entry.header().id().value(), class))
        })
        .collect();
    assert_ne!(
        lossy, admitted,
        "the set comparison passes over a partition that drops entries"
    );

    // ---- A second admission of one identity is refused --------------------
    let duplicate = InboxEntry::StateUpdate(StateUpdateProposal::new(
        header(0, RiskTier::MediumReview)?,
        entity(108)?,
        MasteryLevel::Exposed,
        MasteryLevel::Understood,
    ));
    assert_eq!(
        inbox.admit(duplicate),
        Err(CenterError::ProposalAlreadyAdmitted {
            proposal: ProposalId::new(0)
        }),
        "the inbox admitted one proposal identity twice"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// source_change_links_impacted_rules_and_plans
// ---------------------------------------------------------------------------

/// One official document, driven through `P2-U6`'s own stages one to five.
///
/// The diff and the invalidation this test compares against are that crate's
/// answers, not a local imitation, so the parse, the effective dating and the
/// rule identifiers are the pipeline's. Nothing here reaches a network: the
/// bytes are composed in this function and handed to stage one as an import,
/// which is what `CONTRIBUTING.md`'s synthetic-fixture rule requires and what
/// `GATE-38-020` leaves as the only route in Phase 2.
fn official_document(
    effective: &str,
    rules: &[(&str, &str, &str)],
) -> Result<OfficialDocument, Box<dyn Error>> {
    let mut text = String::from("AUTHORITY: DEPARTMENT_RULE\nISSUED: 2026-01-15\n");
    text.push_str(&format!("EFFECTIVE: {effective}\n"));
    text.push_str("PROGRAM: cse\nCOHORTS: 2023-\nTRANSITION: PRIOR_COHORT_KEEPS_PREVIOUS_RULE\n");
    for (section, rule, body) in rules {
        text.push_str(&format!("SECTION: {section}\n"));
        text.push_str(&format!("RULE: {rule} | {body}\n"));
    }
    let bytes = text.into_bytes();

    let connector = ConnectorId::new("snu-rules")?;
    let target = DeclaredTarget::declared("official/cse/graduation-requirements");
    let retrieved = RetrievalInstant::at(1_772_000_000);
    let manifest = ManifestDraft::for_connector(connector.clone(), SourceCategory::DepartmentPage)
        .declaring(target)
        .source_ownership(SourceOwnership::CollegeOrDepartment)
        .authentication_method(AuthenticationMethod::PublicNoCredential)
        .allowed_frequency(AllowedFrequency::Weekly)
        .terms_status(TermsStatus::PermittedForDeclaredMethod)
        .personal_data_class(PersonalDataClass::Public)
        .completeness(Completeness::Partial)
        .last_success(LastSuccess::Never)
        .next_verification(NextVerification::due_at(RetrievalInstant::at(
            retrieved.seconds() + 86_400,
        )))
        .parser_version(ParserVersion::new(3))
        .build()?;
    let mut ledger = TermsLedger::new();
    ledger.record(connector, TermsStatus::PermittedForDeclaredMethod);

    let observed = ContentDigest::sha256(&bytes);
    let fetched = stage::discover_fetch_import(
        &manifest,
        &ledger,
        retrieved,
        Acquisition::Import {
            target,
            outcome: FetchOutcome::Body {
                at: retrieved,
                http: HttpMetadata::new(
                    Some(200),
                    Some(HeaderValue::new("\"v1\"")?),
                    None,
                    Some(HeaderValue::new("text/plain; charset=utf-8")?),
                ),
                source_bytes: bytes,
                observed,
            },
        },
    )?;
    let cleared = stage::policy_and_terms_check(fetched, &manifest, &ledger)?;
    let snapshotted = stage::immutable_raw_snapshot(cleared, &manifest)?;
    let described =
        stage::source_metadata_and_retrieval_time(snapshotted, &manifest, IngestSeq::at(1))?;
    Ok(academic_ingestion::document::parse(&described.into_snapshot())?)
}

/// An official-source change names exactly the rules `P2-U6`'s diff named and
/// exactly the plans its graph reached, and no others.
///
/// The comparison is whole-set in both directions on both halves, so an
/// over-report fails as an extra entry and an under-report as a missing one.
/// The negative control is a second change that moves a different rule: it must
/// name a different plan set, which an implementation that returned every plan
/// would fail.
#[test]
fn source_change_links_impacted_rules_and_plans() -> TestResult {
    let previous = official_document(
        "2026-03-01",
        &[
            ("art-12", "r-12-1", "major electives require thirty credits"),
            ("art-13", "r-13-1", "a thesis substitutes for the capstone"),
        ],
    )?;
    let current = official_document(
        "2026-03-01",
        &[
            ("art-12", "r-12-1", "major electives require thirty-six credits"),
            ("art-13", "r-13-1", "a thesis substitutes for the capstone"),
        ],
    )?;
    let diff = SourceDiff::between(&previous, &current);

    // The graph: requirement r1 cites rule.alpha; scenario s1 cites r1;
    // course mapping m1 cites rule.beta. Nothing cites m1.
    let requirement = DependentNode::new(DependentKind::Requirement, DependentId::new("r1")?);
    let scenario = DependentNode::new(DependentKind::Scenario, DependentId::new("s1")?);
    let mapping = DependentNode::new(DependentKind::CourseMapping, DependentId::new("m1")?);
    let mut graph = DependencyGraph::new();
    graph.record(requirement.clone(), Dependency::Rule(RuleId::new("r-12-1")?));
    graph.record(scenario.clone(), Dependency::Node(requirement.clone()));
    graph.record(mapping.clone(), Dependency::Rule(RuleId::new("r-13-1")?));

    let entry = SourceChangeEntry::from_diff(
        ConnectorId::new("snu-rules")?,
        digest(1),
        digest(2),
        at(1_700_000_000_000),
        &diff,
        &graph,
    );

    // ---- The rules are the diff's own, whole ------------------------------
    assert_eq!(
        entry
            .impacted_rules()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>(),
        diff.impacted_rules().into_iter().collect::<BTreeSet<_>>(),
        "the entry reports rules the diff did not name"
    );
    assert!(
        !entry.impacted_rules().is_empty(),
        "the fixture produced no impacted rule, so the comparison is empty"
    );
    assert!(
        entry.document_changes().is_empty(),
        "the fixture moved a document header, so every rule would be impacted"
    );

    // ---- The plans are exactly the reachable dependents --------------------
    let plans: BTreeSet<&DependentId> = entry
        .impacted_plans()
        .iter()
        .map(DependentNode::id)
        .collect();
    assert_eq!(
        plans,
        [requirement.id(), scenario.id()].into_iter().collect(),
        "the plan set is not the transitive closure of the impacted rules"
    );
    assert!(
        !plans.contains(mapping.id()),
        "a plan that depends on an unchanged rule was invalidated"
    );
    assert_eq!(
        entry
            .plans_of_kind(DependentKind::Scenario)
            .into_iter()
            .map(DependentNode::id)
            .collect::<Vec<_>>(),
        vec![scenario.id()],
        "the kind filter does not narrow the plan set"
    );

    // ---- The control: a change to the other rule moves the other plan ------
    let other = official_document(
        "2026-03-01",
        &[
            ("art-12", "r-12-1", "major electives require thirty credits"),
            ("art-13", "r-13-1", "a dissertation substitutes for the capstone"),
        ],
    )?;
    let other_entry = SourceChangeEntry::from_diff(
        ConnectorId::new("snu-rules")?,
        digest(1),
        digest(3),
        at(1_700_000_000_001),
        &SourceDiff::between(&previous, &other),
        &graph,
    );
    assert_eq!(
        other_entry
            .impacted_plans()
            .iter()
            .map(DependentNode::id)
            .collect::<BTreeSet<_>>(),
        [mapping.id()].into_iter().collect(),
        "a change to another rule reached the same plans, so the link is unconditional"
    );

    // ---- A document-header change moves every rule in the document ---------
    let redated = official_document(
        "2027-03-01",
        &[
            ("art-12", "r-12-1", "major electives require thirty credits"),
            ("art-13", "r-13-1", "a thesis substitutes for the capstone"),
        ],
    )?;
    let redated_entry = SourceChangeEntry::from_diff(
        ConnectorId::new("snu-rules")?,
        digest(1),
        digest(4),
        at(1_700_000_000_002),
        &SourceDiff::between(&previous, &redated),
        &graph,
    );
    assert_eq!(
        redated_entry.document_changes(),
        [DocumentChange::EffectiveDate],
        "the effective-date move was not reported as a document change"
    );
    assert_eq!(
        redated_entry
            .impacted_plans()
            .iter()
            .map(DependentNode::id)
            .collect::<BTreeSet<_>>(),
        [requirement.id(), scenario.id(), mapping.id()]
            .into_iter()
            .collect(),
        "an effective-date change did not reach every plan in the document"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// both_conflict_classes_are_unresolved_until_user_action
// ---------------------------------------------------------------------------

fn side(
    lane: ConflictLane,
    id: u32,
    status: EpistemicStatus,
    authority: AuthorityClass,
    snapshot: Option<SnapshotId>,
) -> Result<ConflictSide, Box<dyn Error>> {
    Ok(ConflictSide::new(
        lane,
        claim(id)?,
        status,
        authority,
        at(1_700_000_000_000),
        ValidInterval::new(at(1_600_000_000_000), None)?,
        snapshot,
    ))
}

/// One conflict of each class, held side first.
fn one_of_each_conflict() -> Result<Vec<ConflictCase>, Box<dyn Error>> {
    Ok(vec![
        ConflictCase::open(
            ConflictClass::OverrideVersusNewEvidence,
            side(
                ConflictLane::Held,
                201,
                EpistemicStatus::UserConfirmed,
                AuthorityClass::UserExplicit,
                None,
            )?,
            side(
                ConflictLane::Incoming,
                202,
                EpistemicStatus::CodeObserved,
                AuthorityClass::DirectObservation,
                Some(parse_id!(SnapshotId, 210)?),
            )?,
            at(1_700_000_000_000),
        ),
        ConflictCase::open(
            ConflictClass::CodeVersusSpec,
            side(
                ConflictLane::Held,
                203,
                EpistemicStatus::OfficialConfirmed,
                AuthorityClass::Curated,
                None,
            )?,
            side(
                ConflictLane::Incoming,
                204,
                EpistemicStatus::CodeObserved,
                AuthorityClass::DirectObservation,
                Some(parse_id!(SnapshotId, 211)?),
            )?,
            at(1_700_000_000_000),
        ),
    ])
}

/// Neither class of conflict resolves without a user, both show both sides,
/// both offer all three of section 30.4's choices, and settling one rewrites
/// neither side.
///
/// The table drives both classes through the same battery, so a rule that holds
/// for one and not the other fails.
#[test]
fn both_conflict_classes_are_unresolved_until_user_action() -> TestResult {
    let cases = one_of_each_conflict()?;
    assert_eq!(
        cases
            .iter()
            .map(ConflictCase::class)
            .collect::<BTreeSet<_>>(),
        ConflictClass::ALL.into_iter().collect::<BTreeSet<_>>(),
        "the corpus does not carry one conflict per class"
    );

    let automatic: [Actor; 3] = [
        Actor::DeterministicEngine {
            name: "audit".to_owned(),
            version: "1".to_owned(),
        },
        Actor::ModelRun {
            run_id: entity(300)?,
        },
        Actor::Importer {
            name: "import".to_owned(),
            version: "1".to_owned(),
        },
    ];

    for case in &cases {
        let mut board = ConflictBoard::new();
        board.open(case.clone());
        let held_before = *case.both_sides().0;
        let incoming_before = *case.both_sides().1;

        // ---- Both sides are shown ----------------------------------------
        let (held, incoming) = case.both_sides();
        assert_eq!(held.lane(), ConflictLane::Held);
        assert_eq!(incoming.lane(), ConflictLane::Incoming);
        assert_ne!(
            held.claim(),
            incoming.claim(),
            "both sides name the same claim, so only one side is shown"
        );

        // ---- All three choices are offered, for both classes --------------
        assert_eq!(
            case.offered().into_iter().collect::<BTreeSet<_>>(),
            CorrectionChoice::ALL.into_iter().collect::<BTreeSet<_>>(),
            "a class was offered fewer than section 30.4's three choices"
        );

        // ---- It is unresolved, and stays so for every automatic actor -----
        assert_eq!(
            case.resolution(),
            Resolution::Unresolved,
            "a freshly opened conflict is already settled"
        );
        for actor in &automatic {
            let refusal = user_receipt(actor);
            assert!(
                matches!(refusal, Err(CenterError::NotTheUser { .. })),
                "an automatic actor produced a user receipt"
            );
            assert_eq!(
                board.unresolved().len(),
                1,
                "the conflict stopped being unresolved without a user decision"
            );
        }
        assert_eq!(
            board.cases()[0].resolution(),
            Resolution::Unresolved,
            "the board settled a conflict nobody decided"
        );

        // ---- A user settles it, and settling appends ----------------------
        let receipt = user_receipt(&Actor::User {
            user_id: entity(301)?,
        })?;
        board.settle(
            case.class(),
            held_before.claim(),
            CorrectionOutcome::Keep,
            receipt,
            at(1_700_000_100_000),
        )?;
        let settled = &board.cases()[0];
        assert_eq!(
            settled.resolution(),
            Resolution::Settled(CorrectionChoice::Keep)
        );
        assert_eq!(settled.history().len(), 1, "settling did not append");

        // ---- Neither side was rewritten -----------------------------------
        assert_eq!(
            *settled.both_sides().0,
            held_before,
            "settling rewrote the held side"
        );
        assert_eq!(
            *settled.both_sides().1,
            incoming_before,
            "settling rewrote the incoming side"
        );

        // ---- A second decision appends beside the first --------------------
        let second = user_receipt(&Actor::User {
            user_id: entity(301)?,
        })?;
        board.settle(
            case.class(),
            held_before.claim(),
            CorrectionOutcome::EndScope {
                ends_at: at(1_700_000_200_000),
            },
            second,
            at(1_700_000_200_000),
        )?;
        let twice = &board.cases()[0];
        assert_eq!(
            twice.history().len(),
            2,
            "the second decision replaced the first instead of appending"
        );
        assert_eq!(
            twice.history()[0].choice(),
            CorrectionChoice::Keep,
            "the first decision was rewritten"
        );
        assert_eq!(
            twice.resolution(),
            Resolution::Settled(CorrectionChoice::EndScope)
        );
    }

    // ---- The control: a board of two unsettled conflicts reports two -------
    let mut both = ConflictBoard::new();
    for case in one_of_each_conflict()? {
        both.open(case);
    }
    assert_eq!(
        both.unresolved().len(),
        2,
        "an unresolved list that is always empty would pass every check above"
    );
    for class in ConflictClass::ALL {
        assert_eq!(
            both.of_class(class).len(),
            1,
            "the class filter does not narrow the board"
        );
    }

    // ---- Settling a conflict the board does not hold is refused -----------
    let receipt = user_receipt(&Actor::User {
        user_id: entity(302)?,
    })?;
    assert_eq!(
        both.settle(
            ConflictClass::CodeVersusSpec,
            claim(999)?,
            CorrectionOutcome::Keep,
            receipt,
            at(1_700_000_300_000),
        ),
        Err(CenterError::NoSuchConflict {
            class: ConflictClass::CodeVersusSpec,
            claim: claim(999)?,
        })
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// low_confidence_queue_has_three_span_kinds_with_context
// ---------------------------------------------------------------------------

/// One span of each kind, each carrying the context section 34.1 requires.
fn one_span_of_each_kind() -> Result<Vec<LowConfidenceSpan>, Box<dyn Error>> {
    let session = parse_id!(LectureSessionId, 400)?;
    Ok(vec![
        LowConfidenceSpan::Transcript {
            locator: TranscriptLocator::new(
                session,
                parse_id!(TranscriptVersionId, 401)?,
                at(120_000),
                at(134_500),
            ),
            confidence: ConfidencePermille::new(410)?,
        },
        LowConfidenceSpan::Math {
            locator: DocumentRegionLocator::new(
                session,
                parse_id!(LectureDocumentId, 402)?,
                7,
                digest(0x11),
            ),
            confidence: ConfidencePermille::new(320)?,
        },
        LowConfidenceSpan::Code {
            locator: DocumentRegionLocator::new(
                session,
                parse_id!(LectureDocumentId, 403)?,
                9,
                digest(0x22),
            ),
            confidence: ConfidencePermille::new(280)?,
        },
    ])
}

/// The queue holds three kinds, each with a locator that reaches its source.
///
/// The context half is what makes this more than a three-arm enum: a transcript
/// span carries the audio interval section 25.7 requires a reader to be able to
/// return to, and an equation or code span carries the page and the digest of
/// the source image section 34.1 requires beside its marker. The two locators
/// are different types, so neither can stand in for the other.
#[test]
fn low_confidence_queue_has_three_span_kinds_with_context() -> TestResult {
    let spans = one_span_of_each_kind()?;
    assert_eq!(
        spans.iter().map(LowConfidenceSpan::kind).collect::<BTreeSet<_>>(),
        SpanKind::ALL.into_iter().collect::<BTreeSet<_>>(),
        "the corpus does not carry one span per kind"
    );

    let mut queue = academic_evidence_center::LowConfidenceQueue::new();
    for span in &spans {
        queue.queue(*span);
    }

    // ---- Each kind partitions, whole set, both directions ------------------
    let mut partitioned: Vec<LowConfidenceSpan> = Vec::new();
    for kind in SpanKind::ALL {
        let of_kind = queue.of_kind(kind);
        assert_eq!(
            of_kind.len(),
            1,
            "the queue does not hold exactly one span of {kind:?}"
        );
        for span in of_kind {
            assert_eq!(span.kind(), kind, "of_kind returned another kind");
            partitioned.push(*span);
        }
    }
    assert_eq!(
        partitioned.len(),
        queue.spans().len(),
        "the partition lost or duplicated a span"
    );

    // ---- Each kind carries its own context ---------------------------------
    for span in queue.spans() {
        match span {
            LowConfidenceSpan::Transcript {
                locator,
                confidence,
            } => {
                assert!(
                    locator.ends_at().value() > locator.starts_at().value(),
                    "a transcript span carries no audio interval to return to"
                );
                assert_eq!(locator.session(), parse_id!(LectureSessionId, 400)?);
                assert_eq!(
                    span.session(),
                    locator.session(),
                    "the span reaches a different session than its locator"
                );
                assert!(confidence.value() < 500, "the fixture is not low confidence");
                assert_eq!(SpanKind::Transcript.marker_token(), "SEGMENT_CONFIDENCE_LOW");
            }
            LowConfidenceSpan::Math {
                locator,
                confidence,
            }
            | LowConfidenceSpan::Code {
                locator,
                confidence,
            } => {
                assert!(locator.page() > 0, "a document span carries no page");
                assert_eq!(
                    span.session(),
                    locator.session(),
                    "the span reaches a different session than its locator"
                );
                assert!(confidence.value() < 500, "the fixture is not low confidence");
                assert_ne!(
                    locator.source_image(),
                    digest(0),
                    "a document span carries no source image digest"
                );
            }
        }
    }

    // ---- The two document kinds keep their own markers ---------------------
    assert_eq!(SpanKind::Math.marker_token(), "UNVERIFIED_EQUATION");
    assert_eq!(SpanKind::Code.marker_token(), "UNVERIFIED_CODE");
    assert_eq!(
        SpanKind::ALL
            .into_iter()
            .map(SpanKind::marker_token)
            .collect::<BTreeSet<_>>()
            .len(),
        SpanKind::ALL.len(),
        "two kinds share one uncertainty marker"
    );

    // ---- The control: an empty queue partitions into nothing ---------------
    let empty = academic_evidence_center::LowConfidenceQueue::new();
    for kind in SpanKind::ALL {
        assert!(
            empty.of_kind(kind).is_empty(),
            "an empty queue reported a span, so the partition invents entries"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// expiring_permission_is_queued_and_blocks_dependents
// ---------------------------------------------------------------------------

/// A lapsing permission is on the queue and its dependents cannot proceed.
///
/// The blocking half is a type: `PermissionQueue::gate` is the only producer of
/// a `LivePermission`, and it produces one only strictly before the expiry. The
/// test drives the instant across the boundary in both directions, and includes
/// the expiry instant itself, which section 34.1's `Record fail-closed` decides
/// against the caller.
#[test]
fn expiring_permission_is_queued_and_blocks_dependents() -> TestResult {
    let capture = PermissionRef::Capture(parse_id!(CapturePermissionId, 500)?);
    let consent = PermissionRef::Consent(parse_id!(ConsentId, 501)?);
    let lingering = PermissionRef::Consent(parse_id!(ConsentId, 502)?);
    let unrecorded = PermissionRef::Capture(parse_id!(CapturePermissionId, 503)?);

    let expiry = at(1_700_000_500_000);
    let mut queue = PermissionQueue::new();
    queue.record(ExpiringPermission::new(
        capture,
        parse_id!(PermissionLineageId, 510)?,
        at(1_600_000_000_000),
        expiry,
    ));
    queue.record(ExpiringPermission::new(
        consent,
        parse_id!(PermissionLineageId, 511)?,
        at(1_600_000_000_000),
        expiry,
    ));
    queue.record(ExpiringPermission::new(
        lingering,
        parse_id!(PermissionLineageId, 512)?,
        at(1_600_000_000_000),
        at(1_900_000_000_000),
    ));

    let recording = DependentAction::new(entity(520)?, DependentActionKind::Capture, capture);
    let transcribing = DependentAction::new(entity(520)?, DependentActionKind::Transcribe, capture);
    let sending = DependentAction::new(
        entity(521)?,
        DependentActionKind::ProviderTransmission,
        consent,
    );
    let sharing = DependentAction::new(entity(522)?, DependentActionKind::Share, lingering);
    let orphan = DependentAction::new(entity(523)?, DependentActionKind::Capture, unrecorded);
    for action in [recording, transcribing, sending, sharing, orphan] {
        queue.register_dependent(action);
    }

    // ---- It is on the queue ------------------------------------------------
    let queued: BTreeSet<PermissionRef> = queue
        .expiring_by(expiry)
        .into_iter()
        .map(ExpiringPermission::reference)
        .collect();
    assert_eq!(
        queued,
        [capture, consent].into_iter().collect(),
        "the expiry queue is not exactly the permissions that lapse by the horizon"
    );

    // ---- Before the expiry, every recorded permission gates ----------------
    let before = at(expiry.value() - 1);
    for action in [&recording, &transcribing, &sending, &sharing] {
        let live = queue.gate(action, before)?;
        assert_eq!(live.reference(), action.requires());
        assert_eq!(live.proved_at(), before);
    }
    assert_eq!(
        queue
            .blocked_dependents(before)
            .into_iter()
            .map(DependentAction::subject)
            .collect::<Vec<_>>(),
        vec![entity(523)?],
        "before the expiry only the action with no recorded permission is blocked"
    );

    // ---- At the expiry instant, fail-closed --------------------------------
    for action in [&recording, &transcribing, &sending] {
        assert_eq!(
            queue.gate(action, expiry).err(),
            Some(CenterError::PermissionExpired {
                permission: action.requires(),
                expires_at: expiry,
            }),
            "a dependent proceeded at the exact expiry instant"
        );
    }

    // ---- After it, the dependents are exactly the blocked set --------------
    let after = at(expiry.value() + 1);
    let blocked: BTreeSet<(EntityId, DependentActionKind)> = queue
        .blocked_dependents(after)
        .into_iter()
        .map(|action| (action.subject(), action.kind()))
        .collect();
    assert_eq!(
        blocked,
        [
            (entity(520)?, DependentActionKind::Capture),
            (entity(520)?, DependentActionKind::Transcribe),
            (entity(521)?, DependentActionKind::ProviderTransmission),
            (entity(523)?, DependentActionKind::Capture),
        ]
        .into_iter()
        .collect(),
        "the blocked set is not exactly the dependents of the lapsed permissions"
    );
    assert!(
        !blocked.contains(&(entity(522)?, DependentActionKind::Share)),
        "a dependent of a permission that has not lapsed was blocked"
    );
    // The one whose permission is still live still gates after the others lapse.
    let live = queue.gate(&sharing, after)?;
    assert_eq!(live.reference(), lingering);

    // ---- An unrecorded permission is not an unrestricted one ---------------
    assert_eq!(
        queue.gate(&orphan, before).err(),
        Some(CenterError::PermissionAbsent {
            permission: unrecorded
        }),
        "an action whose permission was never recorded proceeded"
    );

    // ---- The control: an empty queue blocks nothing because it holds nothing
    let empty = PermissionQueue::new();
    assert!(
        empty.blocked_dependents(after).is_empty(),
        "a queue with no dependents reported a blocked one"
    );
    assert!(
        empty.expiring_by(expiry).is_empty(),
        "a queue with no permissions reported an expiring one"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// transmission_log_and_deletion_receipts_are_discoverable
// ---------------------------------------------------------------------------

/// A log with one transmission of every receipt state.
fn transmission_corpus() -> Result<TransmissionLog, Box<dyn Error>> {
    let provider = ProviderRef::new(digest(0x31), ProviderSurface::EnterpriseApi);
    let consumer = ProviderRef::new(digest(0x32), ProviderSurface::ConsumerUi);
    let mut log = TransmissionLog::new();
    log.record(TransmissionRecord::new(
        parse_id!(EgressDecisionId, 600)?,
        TransmissionPurpose::RepositoryAnalysis,
        digest(0x41),
        vec![ObjectRange::new(0, 512), ObjectRange::new(2_048, 128)],
        provider,
        at(1_700_000_600_000),
        ReceiptState::Received(DeletionReceiptRef::new(
            digest(0x51),
            digest(0x52),
            at(1_700_000_610_000),
            at(1_700_000_620_000),
        )),
    ));
    log.record(TransmissionRecord::new(
        parse_id!(EgressDecisionId, 601)?,
        TransmissionPurpose::TranscriptComparison,
        digest(0x42),
        vec![ObjectRange::new(0, 4_096)],
        consumer,
        at(1_700_000_700_000),
        ReceiptState::Requested {
            requested_at: at(1_700_000_710_000),
        },
    ));
    log.record(TransmissionRecord::new(
        parse_id!(EgressDecisionId, 602)?,
        TransmissionPurpose::ProposalExtraction,
        digest(0x43),
        vec![ObjectRange::new(64, 256)],
        consumer,
        at(1_700_000_800_000),
        ReceiptState::NotOffered,
    ));
    Ok(log)
}

/// Every transmission and every deletion receipt is reachable from the centre's
/// index, and every row exposes the six things the contract fixes.
///
/// The discoverability half compares the index against the log in both
/// directions, so a record the log holds and the index does not fails as a
/// missing key, and an index entry with no record behind it fails as an extra
/// one. The receipt half does the same for the receipts, and separately
/// requires the `EG07` transmissions — the ones whose provider offers no
/// receipt — to be findable rather than silently absent from both lists.
#[test]
fn transmission_log_and_deletion_receipts_are_discoverable() -> TestResult {
    let log = transmission_corpus()?;
    let mut center = EvidenceCenter::new();
    for record in log.records() {
        center.transmissions_mut().record(record.clone());
    }

    // ---- The index reaches every transmission ------------------------------
    let index = center.index();
    assert_eq!(
        index
            .iter()
            .map(|section| section.section())
            .collect::<Vec<_>>(),
        CenterSection::ALL.to_vec(),
        "the index is not one entry per section, in the specification's order"
    );
    let transmission_section = index
        .iter()
        .find(|section| section.section() == CenterSection::TransmissionLog)
        .ok_or("the index has no transmission section")?;
    let indexed_transmissions: BTreeSet<EgressDecisionId> = transmission_section
        .items()
        .iter()
        .filter_map(|item| match item {
            CenterItem::Transmission(decision) => Some(*decision),
            _ => None,
        })
        .collect();
    assert_eq!(
        indexed_transmissions,
        log.records()
            .iter()
            .map(TransmissionRecord::decision)
            .collect(),
        "the index and the log disagree about which transmissions exist"
    );

    // ---- And every deletion receipt ----------------------------------------
    let indexed_receipts: BTreeSet<EgressDecisionId> = transmission_section
        .items()
        .iter()
        .filter_map(|item| match item {
            CenterItem::DeletionReceipt(decision) => Some(*decision),
            _ => None,
        })
        .collect();
    assert_eq!(
        indexed_receipts,
        log.deletion_receipts()
            .into_iter()
            .map(|(decision, _)| decision)
            .collect(),
        "the index and the log disagree about which deletion receipts exist"
    );
    assert_eq!(
        indexed_receipts,
        [parse_id!(EgressDecisionId, 600)?].into_iter().collect(),
        "the receipt list is not the one transmission that has a receipt"
    );

    // ---- `EG07` is findable, not missing ------------------------------------
    assert_eq!(
        log.without_offered_receipt()
            .into_iter()
            .map(TransmissionRecord::decision)
            .collect::<Vec<_>>(),
        vec![parse_id!(EgressDecisionId, 602)?],
        "a provider that offers no deletion receipt is not findable"
    );

    // ---- Every row exposes the six things, and they are the right values ----
    let first = &log.records()[0];
    assert_eq!(first.purpose(), TransmissionPurpose::RepositoryAnalysis);
    assert_eq!(first.payload_digest(), digest(0x41));
    assert_eq!(
        first.ranges(),
        [ObjectRange::new(0, 512), ObjectRange::new(2_048, 128)]
    );
    assert_eq!(first.provider().surface(), ProviderSurface::EnterpriseApi);
    assert_eq!(first.provider().destination(), digest(0x31));
    assert_eq!(first.transmitted_at(), at(1_700_000_600_000));
    let receipt = first
        .receipt()
        .receipt()
        .copied()
        .ok_or("the first record carries no receipt")?;
    assert_eq!(receipt.receipt_digest(), digest(0x51));
    assert_eq!(receipt.provider_policy_snapshot(), digest(0x52));
    assert_eq!(receipt.requested_at(), at(1_700_000_610_000));
    assert_eq!(receipt.received_at(), at(1_700_000_620_000));

    // ---- A range is two integers and reveals no byte -------------------------
    let total: u64 = first.ranges().iter().map(ObjectRange::length).sum();
    assert_eq!(total, 640, "the recorded ranges do not sum to the sent length");

    // ---- The control: an empty log yields an empty section, not a missing one
    let empty = EvidenceCenter::new();
    let empty_index = empty.index();
    assert_eq!(
        empty_index.len(),
        CenterSection::ALL.len(),
        "an empty centre dropped a section instead of showing it empty"
    );
    for section in &empty_index {
        assert!(
            section.items().is_empty(),
            "an empty centre reported an item"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// correction_marker_appears_in_historical_views
// ---------------------------------------------------------------------------

/// A correction recorded after a reading still marks that reading.
///
/// The shape of the trap: a correction is always recorded *after* the view it
/// corrects, so a marker filtered by the same as-known-at coordinate as the
/// claims would be invisible in exactly the view that needs it. The test reads
/// the past view at a coordinate strictly before the correction's own
/// acceptance sequence and requires the marker to be there anyway — and
/// requires the wrong claim still to be shown, because a view that dropped it
/// could not answer what the decision was made on.
///
/// Two controls stop the assertion being vacuous: a view at a coordinate the
/// wrong claim never reached carries no marker, and an uncorrected claim in the
/// same view carries none either.
#[test]
fn correction_marker_appears_in_historical_views() -> TestResult {
    let wrong = claim(700)?;
    let corrected = claim(701)?;
    let untouched = claim(702)?;
    let later = claim(703)?;

    let mut ledger = CorrectionLedger::new();
    // The wrong claim was accepted at sequence 10 and applied from T1.
    ledger.record_claim(academic_evidence_center::UsedClaim::new(
        wrong,
        10,
        at(1_600_000_000_000),
    ));
    // An unrelated claim in the same reading.
    ledger.record_claim(academic_evidence_center::UsedClaim::new(
        untouched,
        11,
        at(1_600_000_000_000),
    ));
    // A claim that only became known later, and only applies later.
    ledger.record_claim(academic_evidence_center::UsedClaim::new(
        later,
        90,
        at(1_800_000_000_000),
    ));
    // The correction arrives at sequence 50, long after the reading.
    ledger.record_claim(academic_evidence_center::UsedClaim::new(
        corrected,
        50,
        at(1_600_000_000_000),
    ));
    ledger.record_correction(CorrectionMarker::new(
        wrong,
        corrected,
        CorrectionOrigin::EvidenceChange,
        50,
        at(1_700_000_900_000),
    ));

    // ---- The past view: known at 20, valid at T1 ---------------------------
    let past = TimeCoordinates::new(20, at(1_700_000_000_000));
    let view = ledger.view_at(past);
    assert_eq!(view.coordinates(), past);

    let shown: BTreeSet<ClaimId> = view
        .shown()
        .iter()
        .map(academic_evidence_center::UsedClaim::claim)
        .collect();
    assert_eq!(
        shown,
        [wrong, untouched].into_iter().collect(),
        "the past view is not what was known at that coordinate"
    );
    assert!(
        shown.contains(&wrong),
        "the past view dropped the claim the decision was made on"
    );
    assert!(
        !shown.contains(&corrected),
        "the past view shows a claim accepted after its own known-at coordinate"
    );

    // ---- The marker is there, even though it was recorded later -------------
    assert!(
        view.is_marked(wrong),
        "a correction recorded after the reading is hidden from the reading"
    );
    let marker = view
        .markers()
        .iter()
        .find(|marker| marker.corrected() == wrong)
        .ok_or("the marked view carries no marker for the wrong claim")?;
    assert_eq!(marker.superseding(), corrected);
    assert_eq!(marker.origin(), CorrectionOrigin::EvidenceChange);
    assert!(
        marker.recorded_at_seq() > past.known_at_accept_seq,
        "the fixture recorded the correction before the view, so the test proves nothing"
    );

    // ---- Control one: the uncorrected claim in the same view is unmarked ----
    assert!(
        !view.is_marked(untouched),
        "the marker is unconditional: an uncorrected claim is marked too"
    );

    // ---- Control two: a view the wrong claim never reached is unmarked ------
    let earlier = TimeCoordinates::new(5, at(1_700_000_000_000));
    let unreached = ledger.view_at(earlier);
    assert!(
        unreached.shown().is_empty(),
        "the earlier coordinate still shows the claim"
    );
    assert!(
        unreached.markers().is_empty(),
        "a view that never showed the claim carries its correction marker"
    );

    // ---- Control three: valid-at is not defeated by known-at ---------------
    let known_now_valid_early = TimeCoordinates::new(100, at(1_700_000_000_000));
    let reinterpreted = ledger.view_at(known_now_valid_early);
    assert!(
        !reinterpreted
            .shown()
            .iter()
            .any(|claim| claim.claim() == later),
        "a claim that applies from a later instant appeared in an earlier valid-at view"
    );
    assert!(
        reinterpreted.is_marked(wrong),
        "the marker vanished from the fully-known view"
    );

    // ---- The origin vocabulary separates the user from the observer ---------
    assert!(!CorrectionOrigin::EvidenceChange.is_observation_system_change());
    for origin in [
        CorrectionOrigin::OntologyChange,
        CorrectionOrigin::AnalyzerUpgrade,
        CorrectionOrigin::OfficialSourceCorrection,
    ] {
        assert!(
            origin.is_observation_system_change(),
            "{origin:?} was read as a change in the user"
        );
    }
    Ok(())
}
