//! `P2-M4`'s named acceptance evidence.
//!
//! The execution plan names seven tests. Six of them are the six actions and
//! the seventh is a different axis, which the module documentation of
//! `academic_non_delegable` states and `graduation_result_cannot_come_from_generation`
//! below measures.
//!
//! Every one of the six drives **two** things: this crate's command layer, and
//! the door that already exists in the crate that owns the action. Driving only
//! the first would prove this crate consistent with itself; driving both is what
//! makes "the compiled constant agrees with the types" a measurement.

mod support;

use std::collections::BTreeSet;

use academic_domain::{
    Actor, ClaimObject, ContentDigest, EntityId, EpistemicStatus, MasteryLevel, TimestampMillis,
    engines::{FrozenInputs, InputKey, InputValue},
    question::{
        ContextLocator, QUESTION_RESOLUTION_OBJECT, QUESTION_RESOLUTION_PREDICATE, Question,
        QuestionImportance, QuestionOrigin, VerifiedQuestionResolution,
    },
};
use academic_non_delegable::{
    Action, ActionCommand, AuthorizedCommand, CandidateGeneration, DecisionEvent, Delegability,
    NonDelegableAction, NonDelegableError, authorise,
};
use academic_proposal::RiskTier;

use support::{
    TestResult, artifact_id, automatic_actors, design_subsection, entity, evidence, evidence_id,
    scope, user_actor, user_confirmed_claim,
};

const AT: TimestampMillis = TimestampMillis::new(1_700_000_000_000);

fn subject(label: &[u8]) -> ContentDigest {
    ContentDigest::sha256(label)
}

fn command(action: Action, actor: &Actor, label: &[u8]) -> ActionCommand {
    ActionCommand::submitted(action, actor.clone(), subject(label), AT)
}

// ---------------------------------------------------------------------------
// the compiled set, read back out of the design document
// ---------------------------------------------------------------------------

/// Sections 27.1 and 27.4 are the action set, in both directions.
///
/// This is the guard that stops the set being a list this crate invented. It
/// reads section 27.1's table and section 27.4's four rows out of the design
/// document and compares them against the two halves of [`Action`]; a row the
/// document adds, renames or drops fails here, and so does an action this crate
/// adds that the document does not name.
///
/// It also records the three readings that do not agree, rather than choosing
/// one and hiding the others:
///
/// * section 27.4's `non-delegable` row names **three** things;
/// * its `high risk` row names **three** more, and two of those are in the
///   execution plan's non-delegable set;
/// * deletion confirmation is in **neither**, and in no other part of section
///   27 either.
#[test]
fn the_spec_tables_are_this_action_set() -> TestResult {
    // Section 27.1: the first cell of every table row, in the table's order.
    let generation = design_subsection("### 27.1 AI가 담당하는 후보 생성")?;
    let rows: Vec<String> = generation
        .lines()
        .filter(|line| line.starts_with('|'))
        .skip(2) // the header row and the `|---|` separator
        .filter_map(|line| line.split('|').nth(1))
        .map(|cell| cell.trim().to_owned())
        .collect();
    let declared: Vec<String> = CandidateGeneration::ALL
        .iter()
        .map(|row| row.spec_row().to_owned())
        .collect();
    assert_eq!(
        rows, declared,
        "section 27.1's rows and CandidateGeneration no longer agree"
    );
    // The extractor is not vacuous: it read something, and it stopped at the
    // subsection boundary rather than absorbing 27.2's bullets.
    assert!(
        rows.len() >= 2 && !generation.contains("### 27.2"),
        "the 27.1 extractor read nothing or read past its subsection"
    );

    // Section 27.4: four bullets, `label: body`.
    let intensity = design_subsection("### 27.4 Human-in-the-loop 강도")?;
    let bullets: Vec<(String, String)> = intensity
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .filter_map(|line| line.split_once(": "))
        .map(|(label, body)| (label.trim().to_owned(), body.trim().to_owned()))
        .collect();
    let labels: Vec<&str> = bullets.iter().map(|(label, _)| label.as_str()).collect();
    assert_eq!(
        labels,
        ["low risk", "medium risk", "high risk", "non-delegable"],
        "section 27.4's four rows changed"
    );
    // `P2-M2`'s four tiers are these four rows, in this order. If that stops
    // being true the mapping below is reading the wrong body.
    assert_eq!(
        RiskTier::ALL.len(),
        bullets.len(),
        "section 27.4's row count and RiskTier::ALL no longer agree"
    );
    let row_for = |tier: RiskTier| -> &str {
        match tier {
            RiskTier::LowAutosave => "low risk",
            RiskTier::MediumReview => "medium risk",
            RiskTier::HighApproval => "high risk",
            RiskTier::NonDelegable => "non-delegable",
        }
    };

    // Every action that claims a row is named in that row and in no other.
    let mut placed: Vec<NonDelegableAction> = Vec::new();
    let mut unplaced: Vec<NonDelegableAction> = Vec::new();
    for action in NonDelegableAction::ALL {
        match action.declared_tier() {
            Some(tier) => {
                let phrase = action.declared_phrase();
                assert!(
                    !phrase.is_empty(),
                    "{action} claims section 27.4's {} row with no phrase",
                    row_for(tier)
                );
                for (label, body) in &bullets {
                    let names_it = body.contains(phrase);
                    let expected = label == row_for(tier);
                    assert_eq!(
                        names_it, expected,
                        "section 27.4's {label} row and {action} disagree about `{phrase}`"
                    );
                }
                placed.push(action);
            }
            None => {
                assert!(
                    action.declared_phrase().is_empty(),
                    "{action} names a section 27.4 phrase but claims no row"
                );
                unplaced.push(action);
            }
        }
    }

    // The enumeration, not a count. Three actions are section 27.4's own
    // non-delegable row, two are its high-risk row, and one is in neither.
    let of_tier = |tier: RiskTier| -> Vec<NonDelegableAction> {
        placed
            .iter()
            .copied()
            .filter(|action| action.declared_tier() == Some(tier))
            .collect()
    };
    assert_eq!(
        of_tier(RiskTier::NonDelegable),
        vec![
            NonDelegableAction::ResolveQuestion,
            NonDelegableAction::DecideEnrollmentOrCareer,
            NonDelegableAction::AttestPermission,
        ]
    );
    assert_eq!(
        of_tier(RiskTier::HighApproval),
        vec![
            NonDelegableAction::ConfirmMastery,
            NonDelegableAction::ApproveEgress,
        ]
    );
    assert_eq!(unplaced, vec![NonDelegableAction::ConfirmDeletion]);

    // And the reason that last one is unplaced: section 27 does not discuss
    // deletion at all. Measured over the whole of section 27, not just 27.4.
    let whole_section_27: String = std::fs::read_to_string(support::DESIGN_DOCUMENT)?
        .split("## 27. AI Responsibilities")
        .nth(1)
        .and_then(|rest| rest.split("\n## ").next())
        .ok_or("section 27 is no longer in the design document")?
        .to_owned();
    for word in ["삭제", "deletion", "delete"] {
        assert!(
            !whole_section_27.contains(word),
            "section 27 now says `{word}`; ConfirmDeletion's basis has to be restated"
        );
    }
    Ok(())
}

/// Every automatic actor is refused for every non-delegable action, and the
/// refusals are the whole cross product.
///
/// The pairs are collected and compared against the product of
/// [`NonDelegableAction::ALL`] and the three non-user variants of
/// `academic_domain::Actor`, so a refusal that stopped happening for one pair
/// fails here rather than being invisible behind seventeen that still do.
#[test]
fn the_non_delegable_set_refuses_every_automatic_actor() -> TestResult {
    let automatic = automatic_actors()?;
    let mut refused: BTreeSet<(&'static str, &'static str)> = BTreeSet::new();
    let mut expected: BTreeSet<(&'static str, &'static str)> = BTreeSet::new();
    for action in NonDelegableAction::ALL {
        for actor in &automatic {
            expected.insert((action.as_str(), actor.kind_name()));
            let error = authorise(command(Action::Decide(action), actor, b"subject"))
                .err()
                .ok_or_else(|| format!("{action} was authorised for {}", actor.kind_name()))?;
            let NonDelegableError::AutomaticActor {
                action: refused_action,
                actor: refused_actor,
            } = error
            else {
                return Err(format!("{action} was refused for the wrong reason").into());
            };
            assert_eq!(refused_action, action);
            // And the same refusal one layer down, from the event itself, so
            // the door does not depend on the dispatch above it.
            assert!(
                DecisionEvent::recorded(action, actor, subject(b"subject"), AT).is_err(),
                "{action} produced a decision event for {}",
                actor.kind_name()
            );
            refused.insert((refused_action.as_str(), refused_actor));
        }
    }
    assert_eq!(refused, expected);
    assert_eq!(
        refused.len(),
        NonDelegableAction::ALL.len() * automatic.len()
    );
    Ok(())
}

/// A user's non-delegable command comes back as a decision, not a proposal.
///
/// This is the other half of the dispatch, and it is what stops the two guards
/// masking each other. Deleting the actor check leaves
/// `the_non_delegable_set_refuses_every_automatic_actor` failing; deleting the
/// delegability dispatch leaves this one failing, because every command would
/// come back as an AI candidate.
#[test]
fn a_user_command_is_a_decision_not_a_proposal() -> TestResult {
    let user = user_actor()?;
    for action in NonDelegableAction::ALL {
        let authorized = authorise(command(Action::Decide(action), &user, b"subject"))?;
        let AuthorizedCommand::Decision(event) = authorized else {
            return Err(format!("{action} came back as a proposal for a user").into());
        };
        assert_eq!(event.action(), action);
        assert_eq!(event.subject(), subject(b"subject"));
    }
    // And the mirror: every section 27.1 row is a proposal for every actor,
    // including the user, so the dispatch is not "refuse everything".
    for generation in CandidateGeneration::ALL {
        for actor in automatic_actors()?.iter().chain([user.clone()].iter()) {
            let authorized = authorise(command(Action::Generate(generation), actor, b"candidate"))?;
            let AuthorizedCommand::Proposal(proposal) = authorized else {
                return Err(format!("{} came back as a decision", generation.spec_row()).into());
            };
            assert_eq!(proposal.generation(), generation);
        }
    }
    // The classification carries the action, so no caller re-derives it.
    for action in Action::all() {
        match action.delegability() {
            Delegability::AutomaticActorMayPropose(generation) => {
                assert_eq!(action, Action::Generate(generation));
            }
            Delegability::AuthenticatedUserOnly(decide) => {
                assert_eq!(action, Action::Decide(decide));
            }
        }
    }
    Ok(())
}

/// A decision event authorises one action over one subject and nothing else.
#[test]
fn a_decision_event_does_not_authorise_another_action_or_subject() -> TestResult {
    let user = user_actor()?;
    let event = DecisionEvent::recorded(
        NonDelegableAction::ConfirmDeletion,
        &user,
        subject(b"one artifact"),
        AT,
    )?;
    event.authorises(
        NonDelegableAction::ConfirmDeletion,
        subject(b"one artifact"),
    )?;
    for other in NonDelegableAction::ALL {
        if other == NonDelegableAction::ConfirmDeletion {
            continue;
        }
        assert!(matches!(
            event.authorises(other, subject(b"one artifact")),
            Err(NonDelegableError::DecisionNamesAnotherAction { .. })
        ));
    }
    assert!(matches!(
        event.authorises(
            NonDelegableAction::ConfirmDeletion,
            subject(b"another artifact")
        ),
        Err(NonDelegableError::DecisionNamesAnotherSubject { .. })
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// ai_cannot_resolve_a_question
// ---------------------------------------------------------------------------

/// `academic-domain`'s resolution door and this crate's constant agree.
///
/// `INV-C-010`. The claim offered is the user-explicit, user-confirmed one a
/// forgery would have to carry, so the refusal comes from ADR-003's matrix and
/// not from a malformed fixture.
#[test]
fn ai_cannot_resolve_a_question() -> TestResult {
    let question = Question::new(
        entity(4)?,
        scope(1)?,
        "Why does a B+ Tree improve fan-out?",
        TimestampMillis::new(10),
        QuestionOrigin::Lecture {
            entity: entity(500)?,
            locator: ContextLocator::parse("audio@42:18")?,
        },
        [entity(600)?],
        QuestionImportance::UserSet,
    )?;
    let item = evidence(711, b"synthetic answer")?;
    let claim = user_confirmed_claim(
        711,
        question.id(),
        question.scope_id(),
        QUESTION_RESOLUTION_PREDICATE,
        ClaimObject::Text(QUESTION_RESOLUTION_OBJECT.to_owned()),
        vec![item.id],
    )?;

    for actor in &automatic_actors()? {
        assert!(
            VerifiedQuestionResolution::user_decision(&question, actor, &claim, &item).is_err(),
            "academic-domain resolved a question for {}",
            actor.kind_name()
        );
        assert!(
            authorise(command(
                Action::Decide(NonDelegableAction::ResolveQuestion),
                actor,
                b"question"
            ))
            .is_err(),
            "the command layer resolved a question for {}",
            actor.kind_name()
        );
    }

    // Both doors open for the user, so both refusals above are attributable to
    // the actor and not to the fixture.
    let user = user_actor()?;
    VerifiedQuestionResolution::user_decision(&question, &user, &claim, &item)?;
    assert!(matches!(
        authorise(command(
            Action::Decide(NonDelegableAction::ResolveQuestion),
            &user,
            b"question"
        ))?,
        AuthorizedCommand::Decision(_)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// ai_cannot_confirm_mastery
// ---------------------------------------------------------------------------

/// `P2-N2`'s confirmation door and this crate's constant agree, and `FLUENT` is
/// still a value the automatic path cannot name.
#[test]
fn ai_cannot_confirm_mastery() -> TestResult {
    use academic_knowledge_state::{
        STATE_CONFIRMATION_PREDICATE, confirmation::UserConfirmation, ladder::AutomaticLevel,
    };

    let concept: EntityId = entity(77)?;
    let item = evidence(780, b"synthetic mastery evidence")?;
    let claim = user_confirmed_claim(
        781,
        concept,
        scope(1)?,
        STATE_CONFIRMATION_PREDICATE,
        ClaimObject::Mastery(MasteryLevel::Fluent),
        vec![item.id],
    )?;
    for actor in &automatic_actors()? {
        assert!(
            UserConfirmation::verify(
                actor,
                &claim,
                &item,
                concept,
                MasteryLevel::Fluent,
                TimestampMillis::new(20)
            )
            .is_err(),
            "academic-knowledge-state confirmed mastery for {}",
            actor.kind_name()
        );
        assert!(
            authorise(command(
                Action::Decide(NonDelegableAction::ConfirmMastery),
                actor,
                b"concept"
            ))
            .is_err(),
            "the command layer confirmed mastery for {}",
            actor.kind_name()
        );
    }
    let user = user_actor()?;
    UserConfirmation::verify(
        &user,
        &claim,
        &item,
        concept,
        MasteryLevel::Fluent,
        TimestampMillis::new(20),
    )?;

    // And the level itself: no automatic projection can name `FLUENT`.
    assert!(
        AutomaticLevel::ALL
            .iter()
            .all(|level| level.level() != MasteryLevel::Fluent),
        "an automatic level now maps to FLUENT"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// ai_cannot_decide_enrollment_or_career
// ---------------------------------------------------------------------------

/// The command layer is the only refusal for this action, and that is measured
/// rather than assumed.
///
/// `academic_record::RegistrationConfirmation::new` takes a course code, a
/// term, a credit count and evidence identifiers — and **no actor**. This test
/// calls it with values a model run could supply and observes it succeed, which
/// is what makes "the record layer cannot refuse this" an observation. If that
/// crate ever grows an actor parameter this test stops compiling, which is the
/// signal that the two layers changed and have to be reconciled.
#[test]
fn ai_cannot_decide_enrollment_or_career() -> TestResult {
    use academic_record::{
        attempt::RegistrationConfirmation,
        decimal,
        term::{Semester, TermKey},
    };

    let confirmation = RegistrationConfirmation::new(
        "M1522.000100",
        TermKey::new(2026, Semester::Spring)?,
        decimal::parse("3")?,
        vec![evidence_id(900)?],
    )?;
    assert_eq!(confirmation.course_code(), "M1522.000100");

    for actor in &automatic_actors()? {
        assert!(
            authorise(command(
                Action::Decide(NonDelegableAction::DecideEnrollmentOrCareer),
                actor,
                b"course"
            ))
            .is_err(),
            "the command layer decided enrollment for {}",
            actor.kind_name()
        );
    }
    let user = user_actor()?;
    assert!(matches!(
        authorise(command(
            Action::Decide(NonDelegableAction::DecideEnrollmentOrCareer),
            &user,
            b"course"
        ))?,
        AuthorizedCommand::Decision(_)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// ai_cannot_attest_permission
// ---------------------------------------------------------------------------

/// Same shape, same measurement, for `P2-G6`'s grant.
///
/// `academic_consent::AuthorityGrant::record` takes the written authority, the
/// permitted use, the retention terms, the conditions and an expiry — and **no
/// actor**. The grant below is built with no user identity anywhere in it.
#[test]
fn ai_cannot_attest_permission() -> TestResult {
    use academic_consent::{
        AuthorityGrant, CaptureMedium, CaptureProcessing, GrantAuthority, PermittedUse,
        RetentionBound, RetentionTerms, WrittenAuthority, WrittenEvidenceKind,
        evidence::EvidenceArtifact,
    };

    let grant = AuthorityGrant::record(
        WrittenAuthority::new(
            GrantAuthority::Instructor,
            WrittenEvidenceKind::Syllabus,
            EvidenceArtifact::new(artifact_id(910)?, ContentDigest::sha256(b"syllabus"), 64),
        ),
        PermittedUse::new(
            vec![CaptureMedium::Audio],
            vec![CaptureProcessing::LocalStt],
            false,
            false,
        ),
        RetentionTerms::new(
            RetentionBound::Until(1_600_000),
            RetentionBound::Until(1_900_000),
        ),
        Vec::new(),
        1_900_000,
    );
    assert_eq!(grant.authority().authority(), GrantAuthority::Instructor);

    for actor in &automatic_actors()? {
        assert!(
            authorise(command(
                Action::Decide(NonDelegableAction::AttestPermission),
                actor,
                b"offering"
            ))
            .is_err(),
            "the command layer attested a permission for {}",
            actor.kind_name()
        );
    }
    let user = user_actor()?;
    assert!(matches!(
        authorise(command(
            Action::Decide(NonDelegableAction::AttestPermission),
            &user,
            b"offering"
        ))?,
        AuthorizedCommand::Decision(_)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// ai_cannot_approve_egress
// ---------------------------------------------------------------------------

/// The broker has no notion of actor *kind*, so an egress approval cannot be
/// refused underneath it — and this is where that is measured.
///
/// `academic_policy::PermissionRequest::actor_id` is an `Option<String>` and
/// `EgressRule::actor_id` is a `String` compared for equality. So the test
/// installs **two** rules identical in all fifteen other fields and different
/// only in that one, one naming a user and one naming a model run, and observes
/// the real broker **allow both**. `P2-G1` is not wrong to work this way — an
/// opaque actor identifier is what its ten-field tuple specifies — but it does
/// mean nothing at or under the broker can refuse an egress approval on the
/// grounds that a model asked for it.
///
/// The allow is the load-bearing half. A pair of denials would have agreed for
/// any reason at all, including a malformed fixture, and measured nothing.
#[test]
fn ai_cannot_approve_egress() -> TestResult {
    use academic_policy::{
        ContentDigest as PolicyDigest, Decision, EgressRule, ObjectRange, PermissionBroker,
        PermissionRequest, PolicySnapshot, ProcessClass, ProviderIdentity, ProviderPolicyDraft,
        ProviderSurface,
    };

    const USER_ACTOR_ID: &str = "user:01920000-0000-7000-8000-00000000000d";
    const MODEL_ACTOR_ID: &str = "model-run:01920000-0000-7000-8000-000000000385";
    const PAYLOAD: &[u8] = b"eight by!";

    let broker = PermissionBroker::new_profile()?;
    let provider = broker.register_provider_policy(
        ProviderPolicyDraft {
            identity: Some(ProviderIdentity::new(
                "provider-y",
                ProviderSurface::EnterpriseApi,
            )?),
            training_use_enabled: Some(false),
            training_opt_out_applied: Some(false),
            server_retention_millis: Some(0),
            abuse_logging_enabled: Some(false),
            residency_regions: Some(vec!["kr".to_owned()]),
            subprocessors: Some(Vec::new()),
            transit_encryption_declared: Some(true),
            at_rest_encryption_declared: Some(true),
            deletion_api_available: Some(true),
            deletion_receipt_capable: Some(true),
            maximum_input_bytes: Some(1_024),
            logging_configuration: Some("content-logging-disabled".to_owned()),
            policy_source_digest: Some(PolicyDigest::of(b"provider-y-policy-source")),
            last_verified_at: Some(0),
            ttl_millis: Some(10_000),
        },
        0,
    )?;
    let slice = ObjectRange::new("synthetic-object", 10, 18, PolicyDigest::of(b"slice"))?;
    let rule_for = |actor_id: &str| EgressRule {
        actor_id: actor_id.to_owned(),
        process_class: ProcessClass::EgressProxy,
        data_class: "synthetic-private-code".to_owned(),
        operation: "classify".to_owned(),
        purpose_id: "architecture-classification".to_owned(),
        destination_id: provider.destination_id().to_owned(),
        retention_terms_hash: provider.retention_terms_hash(),
        consent_evidence_id: "synthetic-consent-event".to_owned(),
        valid_from: 100,
        valid_until: 1_000,
        minimal_ranges: vec![slice.clone()],
        payload_digest: PolicyDigest::of(PAYLOAD),
        provider_policy_snapshot_digest: provider.snapshot_digest().clone(),
        training_use_allowed: false,
        redaction_policy_hash: PolicyDigest::of(b"redaction-policy-v1"),
    };
    let version = broker.install_policy(PolicySnapshot::from_rules(vec![
        rule_for(USER_ACTOR_ID),
        rule_for(MODEL_ACTOR_ID),
    ])?)?;
    let request_by = |actor_id: &str| PermissionRequest {
        actor_id: Some(actor_id.to_owned()),
        process_class: ProcessClass::EgressProxy,
        data_class: "synthetic-private-code".to_owned().into(),
        object_range_digest_set: Some(vec![slice.clone()]),
        operation: Some("classify".to_owned()),
        purpose_id: Some("architecture-classification".to_owned()),
        destination_id: Some(provider.destination_id().to_owned()),
        retention_terms_hash: Some(provider.retention_terms_hash()),
        requested_at: Some(120),
        consent_evidence_id: Some("synthetic-consent-event".to_owned()),
        policy_version: Some(version.clone()),
    };
    let as_user = broker.evaluate(request_by(USER_ACTOR_ID), 120)?;
    let as_model = broker.evaluate(request_by(MODEL_ACTOR_ID), 120)?;
    assert_eq!(
        as_user.receipt.fingerprint().decision,
        Decision::Allow,
        "the fixture did not reach the allow path, so the comparison below would measure nothing"
    );
    assert_eq!(
        as_model.receipt.fingerprint().decision,
        Decision::Allow,
        "the broker refused a model-run actor identifier, which would make this task's refusal redundant"
    );
    assert!(as_model.capability.is_some());

    // So the refusal is here, and it is not an identifier comparison: it is the
    // closed `Actor` enum.
    for actor in &automatic_actors()? {
        assert!(
            authorise(command(
                Action::Decide(NonDelegableAction::ApproveEgress),
                actor,
                b"payload"
            ))
            .is_err(),
            "the command layer approved egress for {}",
            actor.kind_name()
        );
    }
    let user = user_actor()?;
    assert!(matches!(
        authorise(command(
            Action::Decide(NonDelegableAction::ApproveEgress),
            &user,
            b"payload"
        ))?,
        AuthorizedCommand::Decision(_)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// ai_cannot_confirm_deletion
// ---------------------------------------------------------------------------

/// `P2-P2`'s confirmation door and this crate's constant agree.
///
/// This is the one the execution plan warns about implementing twice. It is not
/// implemented twice: `academic_deletion::DeletionConfirmation` goes through
/// `academic_proposal::UserDecision` and so does
/// `academic_non_delegable::DecisionEvent`, so the test drives the real preview
/// and the real confirmation rather than a description of them.
#[test]
fn ai_cannot_confirm_deletion() -> TestResult {
    use academic_deletion::{
        ClassTargets, DeletionConfirmation, DeletionDryRun, DeletionImpactPreview, DerivativeIndex,
        EvidenceCitations, ProtectionDecision, ProtectionRegistry,
    };
    use academic_retention::DerivativeClass;
    use academic_student_voice::EvidenceIndex;

    struct EmptyIndex;
    impl DerivativeIndex for EmptyIndex {
        fn resolve(
            &self,
            _class: DerivativeClass,
            _subject: &academic_deletion::DeletionTarget,
        ) -> ClassTargets {
            ClassTargets::NothingToDelete {
                reason: "synthetic fixture holds no derivative".to_owned(),
            }
        }
    }
    struct NothingProtects;
    impl ProtectionRegistry for NothingProtects {
        fn decide(&self, _target: &academic_deletion::DeletionTarget) -> ProtectionDecision {
            ProtectionDecision::NotProtected
        }
    }

    let target = academic_deletion::DeletionTarget::new(artifact_id(920)?, [7_u8; 32]);
    let dry_run = DeletionDryRun::of(target, &EmptyIndex, &NothingProtects);
    let mut citations = EvidenceCitations::default();
    citations.cite(target, ContentDigest::sha256(b"the artifact's evidence"));
    let index = EvidenceIndex::of(Vec::new())?;
    let preview = DeletionImpactPreview::of(dry_run, &index, &citations, 500)?;
    let shown = preview.digest();

    for actor in &automatic_actors()? {
        assert!(
            DeletionConfirmation::given(preview.clone(), actor, shown, TimestampMillis::new(501))
                .is_err(),
            "academic-deletion confirmed a deletion for {}",
            actor.kind_name()
        );
        assert!(
            authorise(command(
                Action::Decide(NonDelegableAction::ConfirmDeletion),
                actor,
                b"artifact"
            ))
            .is_err(),
            "the command layer confirmed a deletion for {}",
            actor.kind_name()
        );
    }
    let user = user_actor()?;
    DeletionConfirmation::given(preview, &user, shown, TimestampMillis::new(501))?;
    assert!(matches!(
        authorise(command(
            Action::Decide(NonDelegableAction::ConfirmDeletion),
            &user,
            b"artifact"
        ))?,
        AuthorizedCommand::Decision(_)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// graduation_result_cannot_come_from_generation
// ---------------------------------------------------------------------------

/// Section 27.2's ninth bullet, on the axis it is actually on.
///
/// A graduation result is **not** a member of the non-delegable set, and this
/// test is where that is argued rather than assumed. The six actions refuse
/// `DETERMINISTIC_ENGINE`, and a deterministic engine is section 28's own author
/// of a graduation audit; adding graduation to the set would have refused the
/// correct author. What section 27.2 forbids is a *generation* deciding it,
/// which is a statement about the input, so three separate facts hold it:
///
/// 1. no row of section 27.1 is a graduation row, so a model may not even
///    produce a candidate for one, and no [`Action`] in this layer yields one;
/// 2. `academic_domain::InputValue::Reference` is identifier-shaped, so free
///    text cannot enter the frozen inputs the engine is a function of; and
/// 3. `academic_audit::DeterminateVerdict::new` is `pub(crate)`, so no crate
///    outside `academic-audit` can assemble a verdict —
///    `tests/compile_fail/a_verdict_cannot_be_assembled_outside_its_crate.rs`
///    names it from here.
#[test]
fn graduation_result_cannot_come_from_generation() -> TestResult {
    // The bullet is still there, in the document's own words.
    let refusals = design_subsection("### 27.2 AI가 하지 않는 일")?;
    let bullets: Vec<&str> = refusals
        .lines()
        .filter_map(|line| line.strip_prefix("- "))
        .map(str::trim)
        .collect();
    assert!(
        bullets.contains(&"graduation pass/fail을 자유 텍스트 generation으로 결정"),
        "section 27.2 no longer forbids deciding graduation by generation"
    );

    // 1. No action in this layer produces a graduation result.
    for action in Action::all() {
        let text = match action {
            Action::Generate(generation) => generation.spec_row().to_owned(),
            Action::Decide(decide) => decide.as_str().to_owned(),
        };
        for word in ["graduation", "GRADUATION", "졸업", "degree", "DEGREE"] {
            assert!(
                !text.contains(word),
                "an action in this layer names `{word}`: {text}"
            );
        }
    }

    // 2. Free text cannot become a frozen input. The engine is a function of
    //    these, so a sentence has no way in.
    let sentence = "The student appears to have satisfied the requirements.";
    assert!(
        FrozenInputs::new([(
            InputKey::new("graduation.verdict")?,
            InputValue::Reference(sentence.to_owned()),
        )])
        .is_err(),
        "a sentence became a frozen engine input"
    );
    // The control: an identifier-shaped reference does get in, so the refusal
    // above is about the sentence and not about the call.
    FrozenInputs::new([(
        InputKey::new("graduation.verdict")?,
        InputValue::Reference("rule-set.2026".to_owned()),
    )])?;

    // 3. And the axis itself: a deterministic engine is refused for all six
    //    actions, which is exactly why graduation is not one of them.
    let engine = Actor::DeterministicEngine {
        name: "graduation-audit".to_owned(),
        version: "1".to_owned(),
    };
    for action in NonDelegableAction::ALL {
        assert!(
            authorise(command(Action::Decide(action), &engine, b"subject")).is_err(),
            "{action} admitted a deterministic engine"
        );
    }
    Ok(())
}

/// The epistemic status a non-delegable decision carries is the user's own.
///
/// A small cross-check that the vocabulary this crate refuses in is the same one
/// ADR-003 uses, so a reader does not have to trust that two enums line up.
#[test]
fn a_decision_is_user_confirmed_and_nothing_else() -> TestResult {
    let user = user_actor()?;
    let event = DecisionEvent::recorded(
        NonDelegableAction::ResolveQuestion,
        &user,
        subject(b"question"),
        AT,
    )?;
    let Actor::User { user_id } = &user else {
        return Err("the fixture user actor is not a user".into());
    };
    assert_eq!(
        event.decision().user_id(),
        u128::from_be_bytes(*user_id.as_bytes())
    );
    // And the claim vocabulary the door verifies against is ADR-003's own.
    assert_eq!(
        format!("{:?}", EpistemicStatus::UserConfirmed),
        "UserConfirmed",
        "ADR-003's user-confirmed status changed name"
    );
    Ok(())
}
