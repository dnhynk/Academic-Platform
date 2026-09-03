//! The `P2-L3` acceptance suite.
//!
//! Ten named rows from the execution plan, each with the positive control that
//! makes its refusals mean something, plus the closed-vocabulary and
//! specification-parity rows the contract page cites.

mod common;

use std::error::Error;

use academic_domain::{Actor, ContentDigest, DecisionAction};
use academic_egress_boundary::{CanaryCorpus, EgressProxy};
use academic_model_run::{ProviderId, Transmission};
use academic_policy::PermissionBroker;
use academic_proposal::{
    ImpactPermille, ProposalId, Proposed, ReviewQueue, RiskTier, UserDecision,
};
use academic_transcription::{
    Annotation, AnnotationKind, AnnotationLayer, AuthorizationBinding, CapabilityField,
    CompareFault, ConfidenceSemantics, ContractRegistry, CorrectionAuthor, CorrectionCandidate,
    CorrectionStatus, DecodeFault, Divergence, DownstreamJob, FeatureClaim, InputFault,
    InputManifest, LineageEffect, PipelineFault, ProviderPlacement, ProviderResponse,
    ProviderSelection, RESPONSE_BANNER, RawResponseArchive, RemoteProcessingApproval, RouteDenial,
    SettledCorrection, Speaker, Stage, SttPolicy, SttRoute, Support, TimestampSemantics,
    TokenAddress, TranscriptLineage, VersionFault, compare, decode, run, settles_corrections,
};

use common::{
    DEFAULT_WORDS, FailingProvider, INSIDE, INSIDE_LATER, ImpersonatingProvider, MockLocalProvider,
    RawBytesProvider, SECOND, TestResult, full_manifest, lecture, local_provider, local_selection,
    model_actor, registry_with_local, remote_provider, response_body, run_identity, user,
    version_one, version_two, whole_contract, whole_draft, write_journal,
};

/// A synthetic replacement claim identifier, for a `Replace` disposition.
fn replacement_claim() -> Result<academic_domain::ClaimId, Box<dyn Error>> {
    Ok(academic_domain::ClaimId::try_from_uuid(
        uuid::Uuid::parse_str("01900000-0000-7000-8000-0000000000b7")?,
    )?)
}

/// The specification, which several rows below read their expectations out of.
fn specification() -> Result<String, Box<dyn Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .ok_or("the crate has no workspace root")?
        .to_path_buf();
    Ok(std::fs::read_to_string(root.join(
        "PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md",
    ))?)
}

/// Section 12.3's fenced block, as its own lines.
fn section_12_3_block() -> Result<Vec<String>, Box<dyn Error>> {
    let specification = specification()?;
    let start = specification
        .find("### 12.3")
        .ok_or("the specification has no section 12.3")?;
    let open = specification[start..]
        .find("```text")
        .ok_or("section 12.3 has no fenced block")?
        + start
        + "```text".len();
    let close = specification[open..]
        .find("```")
        .ok_or("section 12.3's fenced block does not close")?
        + open;
    Ok(specification[open..close]
        .lines()
        .map(|line| line.trim().to_owned())
        .filter(|line| !line.is_empty())
        .collect())
}

/// One completed local run over the full manifest.
fn complete_run() -> Result<
    (
        academic_transcription::RunRecord,
        RawResponseArchive,
        InputManifest,
    ),
    Box<dyn Error>,
> {
    let directory = tempfile::tempdir()?;
    let recovery = write_journal(&directory, "lecture", INSIDE)?;
    let manifest = full_manifest(&recovery)?;
    let registry = registry_with_local()?;
    let policy = SttPolicy::new();
    let provider = MockLocalProvider::answering(
        local_provider()?,
        version_one()?,
        response_body(&DEFAULT_WORDS),
    );
    let mut archive = RawResponseArchive::new();
    let record = run(
        &manifest,
        &registry,
        &policy,
        &local_selection()?,
        &provider,
        &mut archive,
        &run_identity()?,
    );
    Ok((record, archive, manifest))
}

// ---------------------------------------------------------------------------
// pipeline_input_authorization
// ---------------------------------------------------------------------------

/// A job reads authorized chunks, authorized captures, and explicitly supplied
/// materials, and nothing else.
///
/// The refusal is made before anything reads a byte, and it is made by
/// comparing the journal's own header against the manifest's binding rather
/// than by trusting what the caller said the buffer was.
#[test]
fn pipeline_input_authorization() -> TestResult {
    let directory = tempfile::tempdir()?;
    let mine = write_journal(&directory, "mine", INSIDE)?;
    // A second capture at a second instant. `token_id` hashes the instant, so
    // this journal carries a different capability token for the same lecture
    // under the same permission -- which is the case a manifest that compared
    // only the lecture would admit.
    let foreign = write_journal(&directory, "foreign", INSIDE_LATER)?;
    assert_ne!(
        mine.header().token_id(),
        foreign.header().token_id(),
        "the two fixture journals carry the same token, so the row below proves nothing"
    );

    // The positive control.
    let manifest = full_manifest(&mine)?;
    assert_eq!(manifest.chunks().len(), 2, "the fixture admitted no audio");
    assert_eq!(
        manifest.captures().len(),
        1,
        "the fixture admitted no capture"
    );
    assert_eq!(
        manifest.materials().len(),
        1,
        "the fixture admitted no supplied material"
    );

    // The exact input digests are recorded, so a later run cannot claim to
    // have read something else.
    let expected: Vec<ContentDigest> = mine
        .records()
        .iter()
        .filter_map(|record| match record.body() {
            academic_capture::RecordBody::AudioChunk { bytes } => Some(bytes.digest()),
            _ => None,
        })
        .collect();
    let recorded: Vec<ContentDigest> = manifest
        .chunks()
        .iter()
        .map(|chunk| *chunk.digest())
        .collect();
    assert_eq!(recorded, expected, "the manifest recorded other digests");

    // A frame from another authorization is refused, whichever kind it is.
    let mut mixed = InputManifest::for_binding(AuthorizationBinding::of(lecture()?, &mine));
    assert_eq!(
        mixed.admit_audio_chunk(&foreign, 0),
        Err(InputFault::ForeignJournal),
        "a chunk from another authorization was admitted"
    );
    assert_eq!(
        mixed.admit_capture(&foreign, 2),
        Err(InputFault::ForeignJournal),
        "a capture from another authorization was admitted"
    );
    assert!(
        mixed.is_empty(),
        "a refused admission left something in the manifest"
    );

    // A frame that exists and is the wrong kind, and one that does not exist.
    assert_eq!(
        mixed.admit_audio_chunk(&mine, 2),
        Err(InputFault::WrongFrameKind { frame_seq: 2 }),
        "an image frame was admitted as audio"
    );
    assert_eq!(
        mixed.admit_capture(&mine, 0),
        Err(InputFault::WrongFrameKind { frame_seq: 0 }),
        "an audio frame was admitted as a capture"
    );
    assert_eq!(
        mixed.admit_audio_chunk(&mine, 99),
        Err(InputFault::NoSuchFrame { frame_seq: 99 }),
        "a frame nothing recorded was admitted"
    );

    // A mark frame is in the journal and is not an input either.
    assert_eq!(
        mixed.admit_audio_chunk(&mine, 3),
        Err(InputFault::WrongFrameKind { frame_seq: 3 }),
        "a mark frame was admitted as audio"
    );

    // Supplied material is the user's own act. Every other actor class is
    // refused, and the four are `academic-domain`'s whole closed set.
    for actor in [
        model_actor()?,
        Actor::DeterministicEngine {
            name: "normalizer".to_owned(),
            version: "1".to_owned(),
        },
        Actor::Importer {
            name: "csv".to_owned(),
            version: "1".to_owned(),
        },
    ] {
        assert_eq!(
            mixed.admit_supplied_material(&actor, "slides", b"bytes"),
            Err(InputFault::MaterialNotUserSupplied),
            "an automatic actor supplied material"
        );
    }
    mixed.admit_supplied_material(&user()?, "slides", b"bytes")?;
    assert_eq!(
        mixed.admit_supplied_material(&user()?, "slides", b"other"),
        Err(InputFault::DuplicateInput),
        "one identifier was admitted twice"
    );
    for identifier in ["", "has space", "../escape"] {
        assert_eq!(
            mixed.admit_supplied_material(&user()?, identifier, b"bytes"),
            Err(InputFault::MaterialIdentifier),
            "`{identifier}` was admitted as an identifier"
        );
    }

    // An empty manifest halts the run at its first stage, before a provider is
    // asked anything.
    let empty = InputManifest::for_binding(AuthorizationBinding::of(lecture()?, &mine));
    let mut archive = RawResponseArchive::new();
    let record = run(
        &empty,
        &registry_with_local()?,
        &SttPolicy::new(),
        &local_selection()?,
        &FailingProvider,
        &mut archive,
        &run_identity()?,
    );
    assert_eq!(
        record.reached(),
        [Stage::AdmitAuthorizedInputs],
        "an empty manifest reached a later stage"
    );
    assert!(
        archive.is_empty(),
        "an unauthorized run retained a provider response"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// stt_provider_policy
// ---------------------------------------------------------------------------

/// Default local, scoped remote, everything else blocked.
///
/// The three arms are driven, and the absence of configuration is driven twice:
/// once as the whole policy and once as an approval that covers two of the
/// three facets `REQ-32-040` names.
#[test]
fn stt_provider_policy() -> TestResult {
    let local = whole_contract(local_provider()?, version_one()?, ProviderPlacement::Local)?;
    let remote = whole_contract(
        remote_provider()?,
        version_one()?,
        ProviderPlacement::Remote,
    )?;

    // Arm one: the default route for raw audio, on a policy holding nothing.
    let empty = SttPolicy::new();
    assert!(empty.approvals().is_empty(), "a new policy holds approvals");
    match empty.route_for(&local) {
        SttRoute::Local {
            ref provider,
            ref model_version,
        } => {
            assert_eq!(provider, &local_provider()?);
            assert_eq!(model_version, &version_one()?);
        }
        other => return Err(format!("a local provider routed {}", other.as_str()).into()),
    }

    // Arm three, from the same empty policy: the absence of configuration is a
    // block and never a remote route.
    assert_eq!(
        empty.route_for(&remote).denial(),
        Some(RouteDenial::ProviderNotApproved),
        "an unconfigured profile reached a remote provider"
    );

    // Arm three again, one facet at a time. Each approval is complete except
    // for the facet under test, so the refusal is about that facet.
    let missing_processing = SttPolicy::new().approve_remote(RemoteProcessingApproval::record(
        remote_provider()?,
        version_one()?,
        false,
        Some(academic_model_run::RetentionDeclaration::new("30_DAYS")?),
    ));
    assert_eq!(
        missing_processing.route_for(&remote).denial(),
        Some(RouteDenial::NoExternalProcessingPermission),
        "a permission that does not cover external processing reached a provider"
    );
    let missing_retention = SttPolicy::new().approve_remote(RemoteProcessingApproval::record(
        remote_provider()?,
        version_one()?,
        true,
        None,
    ));
    assert_eq!(
        missing_retention.route_for(&remote).denial(),
        Some(RouteDenial::NoRetentionDeclaration),
        "an approval with no retention reached a provider"
    );
    // An approval for the same vendor's other model version is not an approval
    // for this one.
    let other_version = SttPolicy::new().approve_remote(RemoteProcessingApproval::record(
        remote_provider()?,
        version_two()?,
        true,
        Some(academic_model_run::RetentionDeclaration::new("30_DAYS")?),
    ));
    assert_eq!(
        other_version.route_for(&remote).denial(),
        Some(RouteDenial::ProviderNotApproved),
        "an approval for another model version reached this one"
    );

    // Arm two: all three facets, and the exact provider and version.
    let scoped = SttPolicy::new().approve_remote(RemoteProcessingApproval::record(
        remote_provider()?,
        version_one()?,
        true,
        Some(academic_model_run::RetentionDeclaration::new("30_DAYS")?),
    ));
    let route = scoped.route_for(&remote);
    let admission = route
        .admission()
        .ok_or("a complete approval did not route to the scoped remote arm")?;
    assert_eq!(admission.provider(), &remote_provider()?);
    assert_eq!(admission.retention().as_str(), "30_DAYS");

    // The scoped approval does not change what a local provider does, and it
    // does not reach a different remote provider either.
    assert_eq!(scoped.route_for(&local).as_str(), "LOCAL");
    let unapproved = whole_contract(
        ProviderId::new("mock-other-remote")?,
        version_one()?,
        ProviderPlacement::Remote,
    )?;
    assert_eq!(
        scoped.route_for(&unapproved).denial(),
        Some(RouteDenial::ProviderNotApproved),
        "an approval for one provider reached another"
    );

    // A blocked route halts the run at the routing stage: nothing is asked and
    // nothing is retained.
    let directory = tempfile::tempdir()?;
    let recovery = write_journal(&directory, "lecture", INSIDE)?;
    let manifest = full_manifest(&recovery)?;
    let mut registry = ContractRegistry::new();
    registry.declare(whole_contract(
        remote_provider()?,
        version_one()?,
        ProviderPlacement::Remote,
    )?)?;
    let mut archive = RawResponseArchive::new();
    let record = run(
        &manifest,
        &registry,
        &SttPolicy::new(),
        &ProviderSelection::of(remote_provider()?, version_one()?, vec![]),
        &FailingProvider,
        &mut archive,
        &run_identity()?,
    );
    assert_eq!(
        record.reached(),
        [
            Stage::AdmitAuthorizedInputs,
            Stage::ReadProviderContract,
            Stage::SelectProviderRoute
        ],
        "a blocked run asked a provider anyway"
    );
    assert!(
        matches!(
            record.failure(),
            Some((
                Stage::SelectProviderRoute,
                PipelineFault::Route(RouteDenial::ProviderNotApproved)
            ))
        ),
        "the block was reported as something else"
    );
    assert!(archive.is_empty(), "a blocked run retained a response");
    Ok(())
}

/// A remote provider's response can only be built from what the egress
/// boundary accepted.
///
/// `ProviderResponse::from_remote` takes both the admission the scoped-remote
/// arm produced and an `academic_egress_boundary::AcceptedResponse`, whose one
/// producer is `EgressProxy::accept_response`. This row builds a real proxy
/// over a real broker and drives the whole scoped-remote run through it.
#[test]
fn a_remote_response_comes_through_the_egress_boundary() -> TestResult {
    let broker = PermissionBroker::new_profile()?;
    let proxy = EgressProxy::new(&broker);
    let body = response_body(&DEFAULT_WORDS);
    let accepted = proxy
        .accept_response(&CanaryCorpus::default(), body.as_bytes())
        .map_err(|incident| format!("the DLP scan refused a synthetic response: {incident}"))?;

    let remote = whole_contract(
        remote_provider()?,
        version_one()?,
        ProviderPlacement::Remote,
    )?;
    let policy = SttPolicy::new().approve_remote(RemoteProcessingApproval::record(
        remote_provider()?,
        version_one()?,
        true,
        Some(academic_model_run::RetentionDeclaration::new("30_DAYS")?),
    ));
    let route = policy.route_for(&remote);
    let admission = route
        .admission()
        .ok_or("the fixture did not route remote")?;
    let response = ProviderResponse::from_remote(admission, &accepted);
    assert_eq!(
        response.placement(),
        ProviderPlacement::Remote,
        "a response built from an accepted egress response is not remote"
    );
    assert_eq!(
        response.digest(),
        &ContentDigest::sha256(body.as_bytes()),
        "the accepted bytes were not the ones retained"
    );

    // The run records `EGRESSED` with the ranges the egress reported, and the
    // retention the approval declared -- not one the caller chose afterwards.
    struct RemoteProvider(ProviderResponse);
    impl academic_transcription::SttProvider for RemoteProvider {
        fn transcribe(
            &self,
            _request: &academic_transcription::TranscriptionRequest<'_>,
        ) -> Option<ProviderResponse> {
            Some(self.0.clone())
        }
    }
    let directory = tempfile::tempdir()?;
    let recovery = write_journal(&directory, "lecture", INSIDE)?;
    let manifest = full_manifest(&recovery)?;
    let mut registry = ContractRegistry::new();
    registry.declare(remote)?;
    let mut identity = run_identity()?;
    identity.transmission = Some(Transmission::egressed(
        academic_model_run::EgressGrantId::new("grant-0001")?,
        vec![academic_model_run::TransmittedRange::new(
            "staged-audio",
            0,
            64,
            academic_model_run::Digest32::of(b"staged"),
        )?],
    )?);
    let mut archive = RawResponseArchive::new();
    let record = run(
        &manifest,
        &registry,
        &policy,
        &ProviderSelection::of(remote_provider()?, version_one()?, vec![]),
        &RemoteProvider(response),
        &mut archive,
        &identity,
    );
    let completed = record
        .completed()
        .ok_or_else(|| format!("the scoped-remote run halted: {:?}", record.failure()))?;
    assert_eq!(
        completed.model_run().transmitted_byte_ranges().kind(),
        "EGRESSED",
        "a scoped-remote run recorded no transmission"
    );
    assert_eq!(
        completed.model_run().retention_declaration().as_str(),
        "30_DAYS",
        "the run recorded a retention the approval did not declare"
    );

    // A local run that claims a transmission, and a remote run that claims
    // none, are both refused.
    let mut local_registry = ContractRegistry::new();
    local_registry.declare(whole_contract(
        local_provider()?,
        version_one()?,
        ProviderPlacement::Local,
    )?)?;
    let provider = MockLocalProvider::answering(
        local_provider()?,
        version_one()?,
        response_body(&DEFAULT_WORDS),
    );
    let mut archive = RawResponseArchive::new();
    let record = run(
        &manifest,
        &local_registry,
        &SttPolicy::new(),
        &local_selection()?,
        &provider,
        &mut archive,
        &identity,
    );
    assert!(
        matches!(
            record.failure(),
            Some((
                Stage::NormalizeTranscript,
                PipelineFault::LocalRunTransmitted
            ))
        ),
        "a local run recorded a transmission"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// stt_capability_contract
// ---------------------------------------------------------------------------

/// Every one of the eight technical declarations is required, and a declared
/// absence blocks the feature that depends on it.
#[test]
fn stt_capability_contract() -> TestResult {
    // The positive control: the whole draft declares.
    let whole = whole_draft(local_provider()?, version_one()?, ProviderPlacement::Local)?;
    let contract = whole.declare()?;
    for claim in FeatureClaim::ALL {
        assert!(
            contract.supports(claim),
            "the whole contract does not support {}",
            claim.as_str()
        );
    }

    // Each declaration dropped in turn. Every other setter is called, so the
    // refusal is about the field and not the order the setters ran in.
    for field in CapabilityField::ALL {
        let mut draft = academic_transcription::ContractDraft::for_provider(
            local_provider()?,
            version_one()?,
            ProviderPlacement::Local,
        );
        if field != CapabilityField::AudioFormat {
            draft = draft.audio_format(academic_transcription::AudioFormat::new(
                "wav/pcm_s16le",
                16_000,
                1,
            ));
        }
        if field != CapabilityField::ChunkBoundary {
            draft = draft.chunk_boundary(academic_transcription::ChunkBoundary::new(
                30 * SECOND,
                2 * SECOND,
            )?);
        }
        if field != CapabilityField::LanguageHints {
            draft = draft.language_hints(Support::Offered);
        }
        if field != CapabilityField::VocabularyHints {
            draft = draft.vocabulary_hints(Support::Offered);
        }
        if field != CapabilityField::TimestampSemantics {
            draft = draft.timestamp_semantics(TimestampSemantics::WordAndSegment);
        }
        if field != CapabilityField::ConfidenceSemantics {
            draft = draft.confidence_semantics(ConfidenceSemantics::PerToken);
        }
        if field != CapabilityField::Diarization {
            draft = draft.diarization(Support::Offered);
        }
        if field != CapabilityField::MathAndCode {
            draft = draft.math_and_code(Support::Offered);
        }
        assert_eq!(
            draft.declare().err(),
            Some(academic_transcription::CapabilityFault::Undeclared(field)),
            "{} was not required",
            field.as_str()
        );
    }

    // A declared absence is a value and not an omission: the contract declares,
    // and the claim that depends on it is refused at the stage that reads it.
    let unsupported = whole_draft(local_provider()?, version_one()?, ProviderPlacement::Local)?
        .diarization(Support::Unsupported)
        .timestamp_semantics(TimestampSemantics::SegmentOnly)
        .confidence_semantics(ConfidenceSemantics::None)
        .vocabulary_hints(Support::Unsupported)
        .math_and_code(Support::Unsupported)
        .language_hints(Support::Unsupported)
        .declare()?;
    for claim in FeatureClaim::ALL {
        assert!(
            !unsupported.supports(claim),
            "{} survived a contract that declares it unsupported",
            claim.as_str()
        );
    }

    let directory = tempfile::tempdir()?;
    let recovery = write_journal(&directory, "lecture", INSIDE)?;
    let manifest = full_manifest(&recovery)?;
    let mut registry = ContractRegistry::new();
    registry.declare(unsupported)?;
    for claim in FeatureClaim::ALL {
        let mut archive = RawResponseArchive::new();
        let record = run(
            &manifest,
            &registry,
            &SttPolicy::new(),
            &ProviderSelection::of(local_provider()?, version_one()?, vec![claim]),
            &FailingProvider,
            &mut archive,
            &run_identity()?,
        );
        assert!(
            matches!(
                record.failure(),
                Some((
                    Stage::ReadProviderContract,
                    PipelineFault::CapabilityUnsupported(field)
                )) if *field == claim.decided_by()
            ),
            "a run depending on {} was not refused",
            claim.as_str()
        );
        assert!(archive.is_empty(), "the refused run retained a response");
    }

    // An unregistered provider is refused before a route is decided.
    let mut archive = RawResponseArchive::new();
    let record = run(
        &manifest,
        &ContractRegistry::new(),
        &SttPolicy::new(),
        &local_selection()?,
        &FailingProvider,
        &mut archive,
        &run_identity()?,
    );
    assert!(
        matches!(
            record.failure(),
            Some((
                Stage::ReadProviderContract,
                PipelineFault::NoCapabilityContract
            ))
        ),
        "a provider with no declared contract was used"
    );

    // One contract per provider and model version.
    let mut registry = registry_with_local()?;
    assert_eq!(
        registry
            .declare(whole_contract(
                local_provider()?,
                version_one()?,
                ProviderPlacement::Local,
            )?)
            .err(),
        Some(academic_transcription::CapabilityFault::AlreadyDeclared),
        "a second contract was declared for one provider and version"
    );
    registry.declare(whole_contract(
        local_provider()?,
        version_two()?,
        ProviderPlacement::Local,
    )?)?;

    // A chunk boundary with no window, or an overlap that is not below it.
    for (window, overlap) in [(0_u64, 0_u64), (SECOND, SECOND), (SECOND, 2 * SECOND)] {
        assert!(
            academic_transcription::ChunkBoundary::new(window, overlap).is_err(),
            "a boundary of {window}/{overlap} was declared"
        );
    }
    Ok(())
}

/// Section 12.3's eight technical declarations are the specification's own
/// words, and the four privacy ones are deliberately not here.
#[test]
fn the_capability_fields_are_section_12_3s_own() -> TestResult {
    let specification = specification()?;
    let sentence = specification
        .lines()
        .find(|line| line.starts_with("Provider contract"))
        .ok_or("section 12.3 has no provider-contract sentence")?;
    for field in CapabilityField::ALL {
        assert!(
            sentence.contains(field.spec_phrase()),
            "the specification does not name `{}`",
            field.spec_phrase()
        );
    }
    // The four this crate deliberately does not restate. `P2-G3`'s
    // `provider_policy_snapshot` owns them; a second copy here would be a
    // second thing to keep true.
    for elsewhere in [
        "data retention",
        "training use",
        "region",
        "deletion receipt",
    ] {
        assert!(
            sentence.contains(elsewhere),
            "the specification does not name `{elsewhere}`"
        );
        assert!(
            !CapabilityField::ALL
                .iter()
                .any(|field| field.spec_phrase() == elsewhere),
            "`{elsewhere}` is declared here and in academic-policy"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// raw_stt_response_immutable
// ---------------------------------------------------------------------------

/// Every raw provider response is retained, and a re-transcription adds one
/// beside the first rather than replacing it.
#[test]
fn raw_stt_response_immutable() -> TestResult {
    let directory = tempfile::tempdir()?;
    let recovery = write_journal(&directory, "lecture", INSIDE)?;
    let manifest = full_manifest(&recovery)?;
    let mut registry = registry_with_local()?;
    registry.declare(whole_contract(
        local_provider()?,
        version_two()?,
        ProviderPlacement::Local,
    )?)?;
    let mut archive = RawResponseArchive::new();

    let first_body = response_body(&DEFAULT_WORDS);
    let record = run(
        &manifest,
        &registry,
        &SttPolicy::new(),
        &local_selection()?,
        &MockLocalProvider::answering(local_provider()?, version_one()?, first_body.clone()),
        &mut archive,
        &run_identity()?,
    );
    let first = record
        .completed()
        .ok_or("the first run halted")?
        .raw_response();
    let first_digest = *archive
        .get(first)
        .ok_or("the first response is gone")?
        .digest();

    let second_body = response_body(&["serialisability", "is", "the", "goal"]);
    let record = run(
        &manifest,
        &registry,
        &SttPolicy::new(),
        &ProviderSelection::of(local_provider()?, version_two()?, vec![]),
        &MockLocalProvider::answering(local_provider()?, version_two()?, second_body.clone()),
        &mut archive,
        &run_identity()?,
    );
    let second = record
        .completed()
        .ok_or("the second run halted")?
        .raw_response();

    assert_ne!(first, second, "the second run overwrote the first");
    assert_eq!(archive.len(), 2, "the archive did not keep both");
    assert_eq!(
        archive
            .get(first)
            .ok_or("the first entry is gone")?
            .digest(),
        &first_digest,
        "the first response changed when the second arrived"
    );
    assert_eq!(
        archive
            .get(first)
            .ok_or("the first entry is gone")?
            .digest(),
        &ContentDigest::sha256(first_body.as_bytes()),
        "the retained bytes are not what the provider returned"
    );
    assert_eq!(
        archive
            .get(second)
            .ok_or("the second entry is gone")?
            .digest(),
        &ContentDigest::sha256(second_body.as_bytes()),
        "the retained bytes are not what the second provider returned"
    );
    assert_eq!(
        archive
            .get(first)
            .ok_or("the first entry is gone")?
            .model_version(),
        &version_one()?,
        "the exact model version was not kept beside the response"
    );

    // Both entries leave the archive under `P2-G5`'s label and in no other
    // form. `Untrusted<T>` implements no `Deref`, no `Display` and no `Into`,
    // and prints no payload.
    for entry in archive.entries() {
        let printed = format!("{:?}", entry.labelled());
        assert!(
            !printed.contains("serializability") && !printed.contains("serialisability"),
            "the label printed the payload: {printed}"
        );
        assert_eq!(
            entry.labelled().provenance().kind(),
            academic_untrusted_content::SourceKind::ProviderResponse,
            "a provider response was sealed as another kind of source"
        );
        assert_eq!(
            format!("sha256:{}", entry.labelled().digest()),
            format!("{}", entry.digest()),
            "the seal and the archive disagree about the digest"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// token_correction_new_version and raw_token_write_protection
// ---------------------------------------------------------------------------

/// Correcting a token appends a version; the raw token is the same value at
/// every version and its digest does not move.
#[test]
fn token_correction_new_version() -> TestResult {
    let (record, _archive, _manifest) = complete_run()?;
    let mut completed = match record.outcome() {
        academic_transcription::RunOutcome::Completed(_) => {
            let academic_transcription::RunOutcome::Completed(run) = record.outcome() else {
                return Err("unreachable".into());
            };
            run
        }
        academic_transcription::RunOutcome::Halted { stage, fault } => {
            return Err(format!("the run halted at {} : {fault}", stage.as_str()).into());
        }
    };
    let before = completed.transcript().token_sequence_digest();
    let raw_text = completed
        .transcript()
        .segments()
        .first()
        .and_then(|segment| segment.tokens().first())
        .ok_or("the fixture transcript has no token")?
        .text()
        .to_owned();
    assert_eq!(raw_text, "serializability");

    // A lineage of its own, so the run's own value stays borrowed.
    let mut lineage = TranscriptLineage::open(completed.transcript().clone());
    let version_one_digest = lineage.current().digest();
    let address = TokenAddress::new(0, 0);
    let settled = SettledCorrection::confirmed(confirm_correction(
        address,
        "serialisability",
        ProposalId::new(1),
    )?);
    let number = lineage.append_correction(settled)?;
    assert_eq!(number, 2, "the correction did not append version two");

    // Version one is unchanged, and it is still readable.
    let one = lineage
        .versions()
        .first()
        .ok_or("version one is not in the lineage")?;
    assert_eq!(one.number(), 1);
    assert_eq!(one.supersedes(), None);
    assert_eq!(
        one.digest(),
        version_one_digest,
        "version one's digest moved when version two was appended"
    );
    assert_eq!(
        lineage.current().supersedes(),
        Some(1),
        "version two does not name the version it supersedes"
    );

    // The raw layer did not move.
    assert_eq!(
        lineage.raw().token_sequence_digest(),
        before,
        "a correction changed the raw token digest"
    );
    assert_eq!(
        lineage
            .raw()
            .segments()
            .first()
            .and_then(|segment| segment.tokens().first())
            .ok_or("the raw token is gone")?
            .text(),
        raw_text,
        "a correction rewrote a raw token"
    );

    // The current projection selects the new version, and both halves are
    // readable from it.
    let at_one = lineage
        .segment_at(1, 0)
        .ok_or("segment zero is not readable at version one")?;
    let at_two = lineage
        .segment_at(2, 0)
        .ok_or("segment zero is not readable at version two")?;
    assert_eq!(
        at_one.tokens().first().map(|token| token.text()),
        Some("serializability"),
        "version one does not read what the provider said"
    );
    assert_eq!(
        at_two.tokens().first().map(|token| token.text()),
        Some("serialisability"),
        "version two does not read the correction"
    );
    assert_eq!(
        at_two.tokens().first().map(|token| token.raw().text()),
        Some("serializability"),
        "the corrected token lost the raw token it came from"
    );
    assert_eq!(at_two.correction_status(), CorrectionStatus::Corrected);
    assert_eq!(at_one.correction_status(), CorrectionStatus::Uncorrected);
    assert_eq!(
        at_two.versions(),
        [2],
        "the segment does not name its versions"
    );

    // The verbatim text and the source chunk mapping are the raw ones at every
    // version.
    assert_eq!(at_one.verbatim_text(), at_two.verbatim_text());
    assert_eq!(
        at_one.source_audio_chunks(),
        at_two.source_audio_chunks(),
        "a correction moved a segment's source chunks"
    );

    // A correction that changes nothing, and one that addresses no token.
    assert_eq!(
        lineage
            .append_correction(SettledCorrection::confirmed(confirm_correction(
                address,
                "serialisability",
                ProposalId::new(2),
            )?))
            .err(),
        Some(VersionFault::NoChange),
        "a correction that changed nothing appended a version"
    );
    assert_eq!(
        lineage
            .append_correction(SettledCorrection::confirmed(confirm_correction(
                TokenAddress::new(9, 9),
                "elsewhere",
                ProposalId::new(3),
            )?))
            .err(),
        Some(VersionFault::NoSuchToken {
            segment: 9,
            position: 9
        }),
        "a correction addressing nothing appended a version"
    );
    for text in ["", "with\nnewline"] {
        assert_eq!(
            CorrectionCandidate::proposing(address, text).err(),
            Some(VersionFault::ReplacementText),
            "`{text}` was accepted as replacement text"
        );
    }
    let _ = &mut completed;
    Ok(())
}

/// Runs one correction candidate through `academic-proposal`'s medium-risk
/// door and returns what `commit` released.
fn confirm_correction(
    address: TokenAddress,
    text: &str,
    id: ProposalId,
) -> Result<academic_proposal::Approved<CorrectionCandidate>, Box<dyn Error>> {
    let mut queue: ReviewQueue<CorrectionCandidate> = ReviewQueue::new();
    queue.admit(Proposed::new(
        id,
        RiskTier::MediumReview,
        academic_domain::ConfidencePermille::new(630)?,
        ImpactPermille::new(400)?,
        CorrectionCandidate::proposing(address, text)?,
    ))?;
    let decision = UserDecision::by(&user()?)?;
    queue.review(id, DecisionAction::Confirm, &decision, INSIDE)?;
    Ok(queue.commit(id)?)
}

// ---------------------------------------------------------------------------
// annotation_layer_separation
// ---------------------------------------------------------------------------

/// Applying, removing and rebuilding every kind of annotation leaves the raw
/// token sequence byte-identical.
#[test]
fn annotation_layer_separation() -> TestResult {
    let (record, _archive, _manifest) = complete_run()?;
    let completed = record
        .completed()
        .ok_or_else(|| format!("the run halted: {:?}", record.failure()))?;
    let raw = completed.transcript();
    let before = raw.token_sequence_digest();

    let mut layer = AnnotationLayer::new();
    // Every kind, applied in turn, with the raw digest checked after each.
    for (index, kind) in AnnotationKind::ALL.into_iter().enumerate() {
        layer.apply(raw, Annotation::over(kind, 0, index, 1, kind.as_str()))?;
        assert_eq!(
            raw.token_sequence_digest(),
            before,
            "applying {} moved the raw token digest",
            kind.as_str()
        );
    }
    let full = layer.digest();
    assert_eq!(
        layer.annotations().len(),
        AnnotationKind::ALL.len(),
        "an annotation did not reach the layer"
    );

    // Each kind is independently removable, and removing one leaves the others.
    for kind in AnnotationKind::ALL {
        let mut reduced = layer.clone();
        assert_eq!(
            reduced.remove_kind(kind),
            1,
            "{} did not come off",
            kind.as_str()
        );
        assert!(
            reduced.of_kind(kind).is_empty(),
            "{} survived its own removal",
            kind.as_str()
        );
        for other in AnnotationKind::ALL {
            if other != kind {
                assert_eq!(
                    reduced.of_kind(other).len(),
                    1,
                    "removing {} took {} with it",
                    kind.as_str(),
                    other.as_str()
                );
            }
        }
        assert_eq!(
            raw.token_sequence_digest(),
            before,
            "removing {} moved the raw token digest",
            kind.as_str()
        );
    }

    // The whole layer is removable and rebuildable, and the rebuild is equal.
    let mut emptied = layer.clone();
    for kind in AnnotationKind::ALL {
        let removed = emptied.remove_kind(kind);
        assert_eq!(
            removed,
            1,
            "{} did not come off the whole layer",
            kind.as_str()
        );
    }
    assert!(emptied.is_empty(), "the layer did not empty");
    let mut rebuilt = AnnotationLayer::new();
    for (index, kind) in AnnotationKind::ALL.into_iter().enumerate() {
        rebuilt.apply(raw, Annotation::over(kind, 0, index, 1, kind.as_str()))?;
    }
    assert_eq!(
        rebuilt.digest(),
        full,
        "the rebuilt layer is not the same layer"
    );
    assert_eq!(
        raw.token_sequence_digest(),
        before,
        "a rebuild moved the raw token digest"
    );

    // An annotation over a range no segment covers is refused.
    for (segment, first, count) in [(0_usize, 0_usize, 0_usize), (0, 0, 99), (9, 0, 1)] {
        assert_eq!(
            AnnotationLayer::new()
                .apply(
                    raw,
                    Annotation::over(AnnotationKind::Punctuation, segment, first, count, "."),
                )
                .err(),
            Some(VersionFault::AnnotationRange),
            "an annotation over {segment}/{first}+{count} was applied"
        );
    }

    // A formatting change is a version too, and it does not touch the raw
    // layer either.
    let mut lineage = TranscriptLineage::open(raw.clone());
    let number = lineage.append_annotations(layer);
    assert_eq!(number, 2, "an annotation layer did not append a version");
    assert_eq!(
        lineage.raw().token_sequence_digest(),
        before,
        "appending an annotation version moved the raw token digest"
    );
    assert_eq!(
        lineage
            .versions()
            .first()
            .ok_or("version one is gone")?
            .annotations()
            .annotations()
            .len(),
        0,
        "version one gained annotations"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// raw_token_write_protection
// ---------------------------------------------------------------------------

/// Nothing this crate offers changes a raw token, and every version reads the
/// same raw layer.
///
/// The type half is `tests/compile_fail` and the workspace-wide signature
/// sweep is `raw_token_write_protection` in `tests/transcription_scans.rs`.
/// This row is the behavioural half: many corrections, many versions, one raw
/// layer.
#[test]
fn raw_token_write_protection() -> TestResult {
    let (record, _archive, _manifest) = complete_run()?;
    let completed = record
        .completed()
        .ok_or_else(|| format!("the run halted: {:?}", record.failure()))?;
    let raw = completed.transcript();
    let before = raw.token_sequence_digest();
    let raw_texts: Vec<String> = raw
        .segments()
        .iter()
        .flat_map(|segment| segment.tokens().iter().map(|token| token.text().to_owned()))
        .collect();

    let mut lineage = TranscriptLineage::open(raw.clone());
    for (index, word) in ["alpha", "beta", "gamma"].into_iter().enumerate() {
        let settled = SettledCorrection::confirmed(confirm_correction(
            TokenAddress::new(0, index),
            word,
            ProposalId::new(u64::try_from(index)? + 10),
        )?);
        lineage.append_correction(settled)?;
    }
    assert_eq!(
        lineage.versions().len(),
        4,
        "three corrections, four versions"
    );
    assert_eq!(
        lineage.raw().token_sequence_digest(),
        before,
        "three corrections moved the raw token digest"
    );
    let after: Vec<String> = lineage
        .raw()
        .segments()
        .iter()
        .flat_map(|segment| segment.tokens().iter().map(|token| token.text().to_owned()))
        .collect();
    assert_eq!(after, raw_texts, "a raw token's text changed");

    // Every version still reads the raw token beside the effective one.
    for version in lineage.versions() {
        let segment = lineage
            .segment_at(version.number(), 0)
            .ok_or("a version cannot read segment zero")?;
        let raw_at_version: Vec<&str> = segment
            .tokens()
            .iter()
            .map(|token| token.raw().text())
            .collect();
        assert_eq!(
            raw_at_version,
            raw_texts
                .iter()
                .take(raw_at_version.len())
                .map(String::as_str)
                .collect::<Vec<_>>(),
            "version {} reads a different raw token",
            version.number()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// provider_retranscription_compare
// ---------------------------------------------------------------------------

/// Two providers' results are compared and never ranked.
#[test]
fn provider_retranscription_compare() -> TestResult {
    let directory = tempfile::tempdir()?;
    let recovery = write_journal(&directory, "lecture", INSIDE)?;
    let manifest = full_manifest(&recovery)?;
    let mut registry = registry_with_local()?;
    registry.declare(whole_contract(
        ProviderId::new("mock-other-local")?,
        version_two()?,
        ProviderPlacement::Local,
    )?)?;
    let mut archive = RawResponseArchive::new();

    let one = run(
        &manifest,
        &registry,
        &SttPolicy::new(),
        &local_selection()?,
        &MockLocalProvider::answering(
            local_provider()?,
            version_one()?,
            response_body(&DEFAULT_WORDS),
        ),
        &mut archive,
        &run_identity()?,
    );
    let two = run(
        &manifest,
        &registry,
        &SttPolicy::new(),
        &ProviderSelection::of(ProviderId::new("mock-other-local")?, version_two()?, vec![]),
        &MockLocalProvider::answering(
            ProviderId::new("mock-other-local")?,
            version_two()?,
            response_body(&["serialisability", "is", "the", "girl"]),
        ),
        &mut archive,
        &run_identity()?,
    );
    let left = one.completed().ok_or("the first run halted")?.transcript();
    let right = two.completed().ok_or("the second run halted")?.transcript();

    // Both raw responses are still there, and each names its own model version.
    assert_eq!(
        archive.len(),
        2,
        "a re-transcription discarded a raw response"
    );
    assert_ne!(
        archive
            .get(left.raw_response())
            .ok_or("the first response is gone")?
            .digest(),
        archive
            .get(right.raw_response())
            .ok_or("the second response is gone")?
            .digest(),
        "the two runs retained one response"
    );

    let comparison = compare(left, right)?;
    assert_eq!(
        comparison.left().model_version(),
        &version_one()?,
        "the comparison lost a model version"
    );
    assert_eq!(comparison.right().model_version(), &version_two()?);
    assert_eq!(
        comparison.left().raw_response(),
        left.raw_response(),
        "the comparison does not name the raw response behind it"
    );

    // The diff is where they disagree: token zero and token three of segment
    // zero, and nothing else.
    let divergences: Vec<&Divergence> = comparison.divergences().iter().collect();
    assert_eq!(
        divergences,
        vec![
            &Divergence::TokenText {
                segment: 0,
                position: 0
            },
            &Divergence::TokenText {
                segment: 0,
                position: 3
            },
        ],
        "the diff is not where the two fixtures differ"
    );
    assert_eq!(comparison.compared_tokens(), 5);
    assert_eq!(comparison.agreeing_tokens(), 3);

    // The comparison carries no order. Swapping the arguments gives the same
    // digest and the mirror-image divergence set.
    let mirrored = compare(right, left)?;
    assert_eq!(
        mirrored.divergence_digest(),
        comparison.divergence_digest(),
        "the comparison depends on which run was passed first, which is a ranking"
    );
    assert_eq!(mirrored.divergences(), comparison.divergences());
    assert_eq!(mirrored.agreeing_tokens(), comparison.agreeing_tokens());

    // Structural divergences, so the report is not only about text.
    let three = run(
        &manifest,
        &registry,
        &SttPolicy::new(),
        &ProviderSelection::of(ProviderId::new("mock-other-local")?, version_two()?, vec![]),
        &MockLocalProvider::answering(
            ProviderId::new("mock-other-local")?,
            version_two()?,
            format!(
                "{RESPONSE_BANNER}\nsegment: raw_segment_0001 0 5000000000 unresolved 0\nverbatim: one\nword: 0 900 serializability\n"
            ),
        ),
        &mut archive,
        &run_identity()?,
    );
    let structural = compare(
        left,
        three
            .completed()
            .ok_or("the third run halted")?
            .transcript(),
    )?;
    let kinds: Vec<&str> = structural
        .divergences()
        .iter()
        .map(Divergence::as_str)
        .collect();
    assert_eq!(
        kinds,
        ["SEGMENT_COUNT", "SPEAKER", "TOKEN_COUNT"],
        "a structural difference was not reported"
    );

    // Two runs that read different inputs, and a run against itself.
    assert_eq!(compare(left, left).err(), Some(CompareFault::SameRun));
    let other_directory = tempfile::tempdir()?;
    let other_recovery = write_journal(&other_directory, "other", INSIDE_LATER)?;
    let other_manifest = full_manifest(&other_recovery)?;
    let mut other_archive = RawResponseArchive::new();
    let elsewhere = run(
        &other_manifest,
        &registry,
        &SttPolicy::new(),
        &ProviderSelection::of(ProviderId::new("mock-other-local")?, version_two()?, vec![]),
        &MockLocalProvider::answering(
            ProviderId::new("mock-other-local")?,
            version_two()?,
            response_body(&DEFAULT_WORDS),
        ),
        &mut other_archive,
        &run_identity()?,
    );
    assert_eq!(
        compare(
            left,
            elsewhere
                .completed()
                .ok_or("the fourth run halted")?
                .transcript()
        )
        .err(),
        Some(CompareFault::DifferentInputs),
        "two runs over different audio were compared"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// user_correction_lineage
// ---------------------------------------------------------------------------

/// A user correction is one of `P2-M2`'s three dispositions, and only two of
/// them append a version.
#[test]
fn user_correction_lineage() -> TestResult {
    // The mapping is total over `academic-domain`'s closed enum.
    assert_eq!(
        LineageEffect::of(&DecisionAction::Confirm),
        LineageEffect::AppendsVersion
    );
    assert_eq!(
        LineageEffect::of(&DecisionAction::Replace {
            replacement_claim_id: replacement_claim()?,
        }),
        LineageEffect::AppendsVersion
    );
    assert_eq!(
        LineageEffect::of(&DecisionAction::Reject),
        LineageEffect::AppendsNothing
    );

    let (record, mut archive, _manifest) = complete_run()?;
    let completed = record
        .completed()
        .ok_or_else(|| format!("the run halted: {:?}", record.failure()))?;
    let raw = completed.transcript();
    let raw_digest = raw.token_sequence_digest();
    let response_digest = *archive
        .get(raw.raw_response())
        .ok_or("the raw response is gone")?
        .digest();
    let mut lineage = TranscriptLineage::open(raw.clone());
    let address = TokenAddress::new(0, 0);

    // A model proposes; the token is under review until somebody settles it.
    lineage.open_review(address)?;
    assert_eq!(
        lineage
            .segment_at(1, 0)
            .ok_or("segment zero is unreadable")?
            .correction_status(),
        CorrectionStatus::NeedsReview,
        "an open correction is not reported as needing review"
    );

    let mut queue: ReviewQueue<CorrectionCandidate> = ReviewQueue::new();
    let rejected = ProposalId::new(1);
    let confirmed = ProposalId::new(2);
    let replaced = ProposalId::new(3);
    for (id, text) in [
        (rejected, "serialisabilty"),
        (confirmed, "serialisability"),
        (replaced, "serializability!"),
    ] {
        queue.admit(Proposed::new(
            id,
            RiskTier::MediumReview,
            academic_domain::ConfidencePermille::new(630)?,
            ImpactPermille::new(400)?,
            CorrectionCandidate::proposing(address, text)?,
        ))?;
    }
    let decision = UserDecision::by(&user()?)?;

    // 거절: nothing is appended and the proposal is retained.
    queue.review(rejected, DecisionAction::Reject, &decision, INSIDE)?;
    assert!(
        queue.commit(rejected).is_err(),
        "a rejected proposal released its payload"
    );
    assert_eq!(
        lineage.versions().len(),
        1,
        "a rejection appended a version"
    );
    assert!(
        queue.history_of(rejected).len() == 1,
        "the rejection was not recorded"
    );

    // 승인: the model's own candidate becomes the next version.
    queue.review(confirmed, DecisionAction::Confirm, &decision, INSIDE)?;
    let approved = queue.commit(confirmed)?;
    let settled = SettledCorrection::confirmed(approved);
    assert_eq!(settled.author(), CorrectionAuthor::ConfirmedModelCandidate);
    assert_eq!(settled.author().disposition_token(), "CONFIRM");
    let number = lineage.append_correction(settled)?;
    assert_eq!(number, 2);
    assert_eq!(
        lineage.open_reviews(),
        [],
        "settling a correction left it open"
    );

    // 수정: `commit` refuses to release the model's payload, and the user's own
    // text is what becomes the version.
    let replacement_record = queue
        .review(
            replaced,
            DecisionAction::Replace {
                replacement_claim_id: replacement_claim()?,
            },
            &decision,
            INSIDE,
        )?
        .clone();
    assert!(
        queue.commit(replaced).is_err(),
        "a REPLACE released the model's candidate"
    );
    let own = CorrectionCandidate::proposing(address, "serializability")?;
    let settled = SettledCorrection::replaced(replaced, &replacement_record, own)?;
    assert_eq!(settled.author(), CorrectionAuthor::UserReplacement);
    assert_eq!(settled.author().disposition_token(), "REPLACE");
    let number = lineage.append_correction(settled)?;
    assert_eq!(number, 3);

    // A record that is not a replacement cannot mint one.
    let confirm_record = queue
        .history_of(confirmed)
        .first()
        .copied()
        .ok_or("the confirmation is not in the history")?
        .clone();
    assert_eq!(
        SettledCorrection::replaced(
            confirmed,
            &confirm_record,
            CorrectionCandidate::proposing(address, "elsewhere")?
        )
        .err(),
        Some(VersionFault::NotSettled),
        "a CONFIRM record minted a replacement"
    );

    // The raw layer and the raw response are untouched by all three.
    assert_eq!(
        lineage.raw().token_sequence_digest(),
        raw_digest,
        "settling three dispositions moved the raw token digest"
    );
    assert_eq!(
        archive
            .get(raw.raw_response())
            .ok_or("the raw response is gone")?
            .digest(),
        &response_digest,
        "settling three dispositions changed the retained provider response"
    );
    assert_eq!(
        archive.len(),
        1,
        "settling a correction retained a response"
    );

    // Only a user settles. `UserDecision::by` is the enforcement; this is the
    // same rule read from the reporting side.
    assert!(settles_corrections(&user()?));
    for actor in [
        model_actor()?,
        Actor::DeterministicEngine {
            name: "normalizer".to_owned(),
            version: "1".to_owned(),
        },
        Actor::Importer {
            name: "csv".to_owned(),
            version: "1".to_owned(),
        },
    ] {
        assert!(!settles_corrections(&actor));
        assert!(
            UserDecision::by(&actor).is_err(),
            "an automatic actor issued a decision"
        );
    }
    let _ = &mut archive;
    Ok(())
}

// ---------------------------------------------------------------------------
// lecture_pipeline_dag
// ---------------------------------------------------------------------------

/// A failed stage means no publication, and no stage after it runs.
///
/// The stages are enumerated, not counted: the loop is over [`Stage::ALL`] and
/// nothing here asserts how long that list is.
#[test]
fn lecture_pipeline_dag() -> TestResult {
    // The positive control. Without it every assertion below would also hold
    // for a pipeline that completed nothing ever.
    let (record, _archive, manifest) = complete_run()?;
    assert_eq!(
        record.reached(),
        Stage::ALL.as_slice(),
        "the unpoisoned run did not reach every stage"
    );
    let completed = record
        .completed()
        .ok_or_else(|| format!("the unpoisoned run halted: {:?}", record.failure()))?;

    // Every downstream job has an identifier of its own and every one cites
    // the same input digest.
    let jobs = completed.jobs();
    assert_eq!(
        jobs.iter().map(|job| job.job()).collect::<Vec<_>>(),
        DownstreamJob::ALL.to_vec(),
        "the fan-out is not section 12.3's three arrows"
    );
    let identifiers: std::collections::BTreeSet<String> =
        jobs.iter().map(|job| job.job_id().to_string()).collect();
    assert_eq!(
        identifiers.len(),
        jobs.len(),
        "two downstream jobs share an identifier"
    );
    for job in jobs {
        assert_eq!(
            job.input_digest(),
            &manifest.input_digest(),
            "{} does not cite the shared input digest",
            job.job().as_str()
        );
        assert_eq!(
            job.raw_response(),
            completed.raw_response(),
            "{} does not name the raw response behind it",
            job.job().as_str()
        );
        assert_eq!(
            job.produces_proposals(),
            job.job() == DownstreamJob::ProposalJobs,
            "{} disagrees with its own job about proposals",
            job.job().as_str()
        );
    }

    // One stage has no failure of its own, and saying so is more honest than
    // inventing one. The list is compared against `Stage::ALL`, so a stage
    // added without an arranged failure has to be classified here.
    for (stage, reason) in INFALLIBLE_STAGES {
        assert!(
            Stage::ALL.contains(&stage),
            "{} is not a stage",
            stage.as_str()
        );
        assert!(
            reason.len() >= 80,
            "{} has no written reason",
            stage.as_str()
        );
        assert!(
            record.reached().contains(&stage),
            "{} is classified infallible and the completing run never ran it",
            stage.as_str()
        );
    }

    // Each remaining stage arranged to fail in turn. The run reports the
    // failure against that stage, produces nothing, and runs no stage after it.
    for stage in Stage::ALL
        .into_iter()
        .filter(|stage| !INFALLIBLE_STAGES.iter().any(|(named, _)| named == stage))
    {
        let driven = drive_failing(stage)?;
        let (failed, _fault) = driven
            .failure()
            .ok_or_else(|| format!("{} was arranged to fail and did not", stage.as_str()))?;
        assert_eq!(
            failed, stage,
            "the failure was reported against another stage"
        );
        assert!(
            driven.completed().is_none(),
            "{} failed and the run completed anyway",
            stage.as_str()
        );
        let expected: Vec<Stage> = Stage::ALL
            .into_iter()
            .take_while(|candidate| *candidate != stage)
            .chain(core::iter::once(stage))
            .collect();
        assert_eq!(
            driven.reached(),
            expected,
            "{} failed and a later stage ran anyway",
            stage.as_str()
        );
    }
    Ok(())
}

/// The stages that have no failure of their own, each with the reason.
const INFALLIBLE_STAGES: [(Stage, &str); 1] = [(
    Stage::FanOutDownstreamJobs,
    "the fan-out derives three handles from values every earlier stage has      already validated: the job list is `DownstreamJob::ALL`, each identifier is      a digest over that constant and the input digest, and the input digest is      the manifest's own. There is no input that makes it fail, and an invented      failure would be a case that tests the invention. What covers it instead is      the positive control above, which asserts the run reached it and then      asserts every property of what it produced.",
)];

/// Arranges exactly one stage to fail and runs the pipeline.
fn drive_failing(stage: Stage) -> Result<academic_transcription::RunRecord, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let recovery = write_journal(&directory, "lecture", INSIDE)?;
    let good = full_manifest(&recovery)?;
    let empty = InputManifest::for_binding(AuthorizationBinding::of(lecture()?, &recovery));
    let mut archive = RawResponseArchive::new();
    let identity = run_identity()?;
    let mut transmitting = run_identity()?;
    transmitting.transmission = Some(Transmission::egressed(
        academic_model_run::EgressGrantId::new("grant-0001")?,
        vec![academic_model_run::TransmittedRange::new(
            "staged-audio",
            0,
            64,
            academic_model_run::Digest32::of(b"staged"),
        )?],
    )?);
    let body = response_body(&DEFAULT_WORDS);
    let good_provider =
        MockLocalProvider::answering(local_provider()?, version_one()?, body.clone());
    let impersonating = ImpersonatingProvider {
        provider: ProviderId::new("mock-other-local")?,
        model_version: version_one()?,
        body: body.clone(),
    };
    let undecodable =
        MockLocalProvider::answering(local_provider()?, version_one()?, "not a response\n");
    // `academic-untrusted-content` refuses bytes that are not UTF-8, so the
    // seal fails and the response is not retained. A `String` cannot carry
    // this, which is why the fixture is a byte provider.
    let unsealable = RawBytesProvider {
        provider: local_provider()?,
        model_version: version_one()?,
        bytes: vec![0xFF_u8, 0xFE, 0xFD],
    };

    // Each arrangement changes exactly one thing from the completing run.
    let (manifest, registry, policy, selection, provider, identity): (
        &InputManifest,
        ContractRegistry,
        SttPolicy,
        ProviderSelection,
        &dyn academic_transcription::SttProvider,
        &academic_transcription::RunIdentity,
    ) = match stage {
        Stage::AdmitAuthorizedInputs => (
            &empty,
            registry_with_local()?,
            SttPolicy::new(),
            local_selection()?,
            &good_provider,
            &identity,
        ),
        Stage::ReadProviderContract => (
            &good,
            ContractRegistry::new(),
            SttPolicy::new(),
            local_selection()?,
            &good_provider,
            &identity,
        ),
        Stage::SelectProviderRoute => {
            let mut registry = ContractRegistry::new();
            registry.declare(whole_contract(
                remote_provider()?,
                version_one()?,
                ProviderPlacement::Remote,
            )?)?;
            (
                &good,
                registry,
                SttPolicy::new(),
                ProviderSelection::of(remote_provider()?, version_one()?, vec![]),
                &good_provider,
                &identity,
            )
        }
        Stage::Transcribe => (
            &good,
            registry_with_local()?,
            SttPolicy::new(),
            local_selection()?,
            &impersonating,
            &identity,
        ),
        Stage::RetainRawResponse => (
            &good,
            registry_with_local()?,
            SttPolicy::new(),
            local_selection()?,
            &unsealable,
            &identity,
        ),
        // Two arrangements reach the same stage; the decode one is used here
        // and the transmission one is driven by
        // `a_remote_response_comes_through_the_egress_boundary`.
        Stage::NormalizeTranscript => (
            &good,
            registry_with_local()?,
            SttPolicy::new(),
            local_selection()?,
            &undecodable,
            &identity,
        ),
        // Not driven: it is in `INFALLIBLE_STAGES`. The arrangement below is
        // the transmission mismatch, which the run reports against
        // `NormalizeTranscript` because that is where the record is built.
        Stage::FanOutDownstreamJobs => (
            &good,
            registry_with_local()?,
            SttPolicy::new(),
            local_selection()?,
            &good_provider,
            &transmitting,
        ),
    };
    let record = run(
        manifest,
        &registry,
        &policy,
        &selection,
        provider,
        &mut archive,
        identity,
    );
    Ok(record)
}

/// The three downstream jobs are section 12.3's own three arrows, in its order.
#[test]
fn the_downstream_jobs_are_section_12_3s_own() -> TestResult {
    let block = section_12_3_block()?;
    let arrows: Vec<String> = block
        .iter()
        .filter_map(|line| {
            line.strip_prefix("├──>")
                .or_else(|| line.strip_prefix("└──>"))
        })
        .map(|rest| rest.trim().to_owned())
        .collect();
    let declared: Vec<String> = DownstreamJob::ALL
        .into_iter()
        .map(|job| job.spec_line().to_owned())
        .collect();
    assert_eq!(
        arrows, declared,
        "the fan-out is not section 12.3's block; the specification is authoritative"
    );

    // Every stage that claims a box in the diagram has one.
    for stage in Stage::ALL {
        if let Some(anchor) = stage.spec_anchor() {
            assert!(
                block.iter().any(|line| line.contains(anchor)),
                "{} claims the diagram draws `{anchor}` and it does not",
                stage.as_str()
            );
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// The wire grammar
// ---------------------------------------------------------------------------

/// Every way a response can be malformed is refused, and each `DecodeFault`
/// variant is produced by one of the cases.
#[test]
fn a_malformed_provider_response_is_refused() -> TestResult {
    let contract = whole_contract(local_provider()?, version_one()?, ProviderPlacement::Local)?;
    let segment_only = whole_draft(local_provider()?, version_one()?, ProviderPlacement::Local)?
        .timestamp_semantics(TimestampSemantics::SegmentOnly)
        .declare()?;
    let no_diarization = whole_draft(local_provider()?, version_one()?, ProviderPlacement::Local)?
        .diarization(Support::Unsupported)
        .declare()?;

    let decode_with = |contract: &academic_transcription::ProviderContract, body: &str| {
        let response =
            ProviderResponse::from_local(local_provider()?, version_one()?, body.as_bytes());
        Ok::<_, Box<dyn Error>>(decode(
            &response,
            contract,
            lecture()?,
            academic_transcription::RawResponseArchive::new()
                .retain(&response)
                .map_err(|_| "the fixture response could not be sealed")?,
            ContentDigest::sha256(b"input"),
        ))
    };

    // The positive control.
    decode_with(&contract, &response_body(&DEFAULT_WORDS))??;

    let cases: Vec<(&str, String, DecodeFault)> = vec![
        (
            "no banner",
            "segment: a 0 1 unresolved 0\nverbatim: x\nword: 0 900 x\n".to_owned(),
            DecodeFault::Banner,
        ),
        (
            "no trailing newline",
            format!("{RESPONSE_BANNER}\nsegment: a 0 1 unresolved 0\nverbatim: x\nword: 0 900 x"),
            DecodeFault::Banner,
        ),
        (
            "unknown key",
            format!("{RESPONSE_BANNER}\nspeaker: instructor\n"),
            DecodeFault::UnknownKey("speaker".to_owned()),
        ),
        (
            "verbatim before segment",
            format!("{RESPONSE_BANNER}\nverbatim: x\n"),
            DecodeFault::MissingKey("segment"),
        ),
        (
            "segment with no verbatim",
            format!("{RESPONSE_BANNER}\nsegment: a 0 1000 unresolved 0\nword: 0 900 x\n"),
            DecodeFault::MissingKey("verbatim"),
        ),
        (
            "segment with no word",
            format!("{RESPONSE_BANNER}\nsegment: a 0 1000 unresolved 0\nverbatim: x\n"),
            DecodeFault::MissingKey("word"),
        ),
        (
            "two verbatim lines",
            format!(
                "{RESPONSE_BANNER}\nsegment: a 0 1000 unresolved 0\nverbatim: x\nverbatim: y\nword: 0 900 x\n"
            ),
            DecodeFault::DuplicateKey("verbatim"),
        ),
        (
            "segment field count",
            format!("{RESPONSE_BANNER}\nsegment: a 0 1000 unresolved\n"),
            DecodeFault::FieldCount("segment"),
        ),
        (
            "word field count",
            format!(
                "{RESPONSE_BANNER}\nsegment: a 0 1000 unresolved 0\nverbatim: x\nword: 0 900\n"
            ),
            DecodeFault::FieldCount("word"),
        ),
        (
            "not a number",
            format!("{RESPONSE_BANNER}\nsegment: a zero 1000 unresolved 0\n"),
            DecodeFault::NotANumber("zero".to_owned()),
        ),
        (
            "empty segment interval",
            format!("{RESPONSE_BANNER}\nsegment: a 1000 1000 unresolved 0\n"),
            DecodeFault::SegmentInterval,
        ),
        (
            "token outside its segment",
            format!(
                "{RESPONSE_BANNER}\nsegment: a 0 1000 unresolved 0\nverbatim: x\nword: 2000 900 x\n"
            ),
            DecodeFault::TokenOutsideSegment,
        ),
        (
            "segments out of order",
            format!(
                "{RESPONSE_BANNER}\nsegment: a 2000 3000 unresolved 0\nverbatim: x\nword: 2000 900 x\nsegment: b 0 1000 unresolved 0\nverbatim: y\nword: 0 900 y\n"
            ),
            DecodeFault::SegmentOrder,
        ),
        (
            "no segment at all",
            format!("{RESPONSE_BANNER}\n"),
            DecodeFault::NoSegments,
        ),
        (
            "unknown speaker",
            format!("{RESPONSE_BANNER}\nsegment: a 0 1000 lecturer 0\n"),
            DecodeFault::UnknownSpeaker("lecturer".to_owned()),
        ),
    ];
    let mut produced: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for (name, body, expected) in cases {
        let outcome = decode_with(&contract, &body)?;
        assert_eq!(
            outcome.err(),
            Some(expected.clone()),
            "`{name}` was not refused as expected"
        );
        produced.insert(
            format!("{expected:?}")
                .split('(')
                .next()
                .unwrap_or("")
                .to_owned(),
        );
    }

    // The three contradictions, each against a contract that declares the
    // opposite of what the body carries.
    for (name, contract, body, field) in [
        (
            "word time against a segment-only declaration",
            &segment_only,
            format!(
                "{RESPONSE_BANNER}\nsegment: a 0 1000 instructor 0\nverbatim: x\nword: 0 900 x\n"
            ),
            CapabilityField::TimestampSemantics,
        ),
        (
            "no confidence against a per-token declaration",
            &contract,
            format!(
                "{RESPONSE_BANNER}\nsegment: a 0 1000 instructor 0\nverbatim: x\nword: 0 - x\n"
            ),
            CapabilityField::ConfidenceSemantics,
        ),
        (
            "an attributed speaker against no diarization",
            &no_diarization,
            format!(
                "{RESPONSE_BANNER}\nsegment: a 0 1000 instructor 0\nverbatim: x\nword: 0 900 x\n"
            ),
            CapabilityField::Diarization,
        ),
    ] {
        assert_eq!(
            decode_with(contract, &body)?.err(),
            Some(DecodeFault::ContradictsDeclaration(field)),
            "`{name}` was not refused"
        );
    }

    // Every variant of the vocabulary is produced by one of the cases above.
    for variant in [
        "Banner",
        "UnknownKey",
        "MissingKey",
        "DuplicateKey",
        "FieldCount",
        "NotANumber",
        "SegmentInterval",
        "TokenOutsideSegment",
        "SegmentOrder",
        "NoSegments",
        "UnknownSpeaker",
    ] {
        assert!(
            produced.contains(variant),
            "no case produces DecodeFault::{variant}"
        );
    }
    Ok(())
}

/// Section 12.4's three speaker spellings round-trip, and nothing else parses.
#[test]
fn every_closed_vocabulary_is_the_list_its_enum_declares() -> TestResult {
    for speaker in [
        Speaker::Instructor,
        Speaker::StudentUnknown(2),
        Speaker::Unresolved,
    ] {
        assert_eq!(
            Speaker::parse(&speaker.spelling()),
            Some(speaker),
            "a speaker spelling does not round-trip"
        );
    }
    assert_eq!(Speaker::parse("lecturer"), None);
    assert_eq!(Speaker::parse("student_unknown_"), None);
    assert!(Speaker::Instructor.is_attributed());
    assert!(Speaker::StudentUnknown(2).is_attributed());
    assert!(!Speaker::Unresolved.is_attributed());

    // Section 12.4's own example spelling.
    let specification = specification()?;
    assert!(
        specification.contains("speaker: instructor | student_unknown_2 | unresolved"),
        "section 12.4's speaker line changed"
    );

    // Every stable spelling is distinct inside its own vocabulary.
    fn distinct(name: &str, spellings: &[&str]) {
        let unique: std::collections::BTreeSet<&&str> = spellings.iter().collect();
        assert_eq!(unique.len(), spellings.len(), "{name} repeats a spelling");
    }
    distinct("Stage", &Stage::ALL.map(Stage::as_str));
    distinct(
        "DownstreamJob",
        &DownstreamJob::ALL.map(DownstreamJob::as_str),
    );
    distinct(
        "CapabilityField",
        &CapabilityField::ALL.map(CapabilityField::as_str),
    );
    distinct(
        "AnnotationKind",
        &AnnotationKind::ALL.map(AnnotationKind::as_str),
    );
    distinct("RouteDenial", &RouteDenial::ALL.map(RouteDenial::as_str));
    distinct(
        "CorrectionStatus",
        &CorrectionStatus::ALL.map(CorrectionStatus::as_str),
    );
    distinct("FeatureClaim", &FeatureClaim::ALL.map(FeatureClaim::as_str));
    Ok(())
}

/// Every model execution records `P2-M1`'s twelve fields, and this crate
/// creates no provenance of its own.
#[test]
fn every_run_records_the_twelve_model_run_fields() -> TestResult {
    let (record, _archive, manifest) = complete_run()?;
    let completed = record
        .completed()
        .ok_or_else(|| format!("the run halted: {:?}", record.failure()))?;
    let model_run = completed.model_run();
    assert_eq!(model_run.provider(), &local_provider()?);
    assert_eq!(model_run.model_version(), &version_one()?);
    assert_eq!(model_run.purpose().as_str(), "LECTURE_TRANSCRIPTION");
    assert_eq!(
        model_run.transmitted_byte_ranges().kind(),
        "LOCAL_ONLY",
        "a local run recorded a transmission"
    );
    assert_eq!(
        model_run.retention_declaration().as_str(),
        academic_transcription::LOCAL_ONLY_RETENTION
    );
    assert_eq!(
        model_run.input_artifact_refs().as_slice().len(),
        manifest.chunks().len() + manifest.captures().len() + manifest.materials().len(),
        "the run did not record one artifact per admitted input"
    );
    // The record digest covers every field, which is `P2-M1`'s own rule.
    assert_ne!(
        model_run.record_digest(),
        {
            let mut other = run_identity()?;
            other.started_at = INSIDE + 1;
            let elsewhere = run(
                &manifest,
                &registry_with_local()?,
                &SttPolicy::new(),
                &local_selection()?,
                &MockLocalProvider::answering(
                    local_provider()?,
                    version_one()?,
                    response_body(&DEFAULT_WORDS),
                ),
                &mut RawResponseArchive::new(),
                &other,
            );
            elsewhere
                .completed()
                .ok_or("the second run halted")?
                .model_run()
                .record_digest()
        },
        "two runs at different instants have the same record digest"
    );
    Ok(())
}
