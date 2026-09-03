//! The stages section 12.3 draws, in the order it draws them.
//!
//! A stage that fails ends the run and no later stage runs. The record names
//! the stages that were **reached**, so `lecture_pipeline_dag` can see the
//! prefix rather than infer it -- which is `P2-U6`'s
//! `ingestion_stage_order_is_strict` shape, reused rather than reinvented.
//!
//! The stages are enumerated, not counted. Every rule iterates [`Stage::ALL`]
//! and nothing asserts how long that list is, so adding a stage adds a case.
//!
//! # The fan-out is three siblings, not a fourth stage
//!
//! Section 12.3's diagram ends in three arrows out of one box:
//! `LosslessLectureDocument + PDF`, `CoverageValidator`, and `proposal jobs`.
//! [`DownstreamJob::ALL`] is that list, and `the_downstream_jobs_are_section_12_3s_own`
//! reads it out of the specification's own fenced block rather than
//! transcribing it. Each job gets an identifier of its own and every one cites
//! the **same** input digest, which is what `t001`'s `REQ-12-024` row means by
//! "independent IDs, shared input hash".
//!
//! # What this crate does not run
//!
//! None of the three. `P2-L4` owns the lossless document and the coverage
//! validator; `P2-M2` owns the review queue the proposal jobs feed. What is
//! here is the fan-out: the handles, their identifiers, and the fact that the
//! AI-authored one is marked as producing proposals rather than records.

use academic_domain::ContentDigest;
use academic_model_run::{
    ArtifactId, Cost, Digest32, InputArtifactRef, InputArtifactRefs, ModelRun, ModelRunId,
    ModelVersion, ProviderId, Purpose, RetentionDeclaration, Transmission,
};

use crate::{
    authorize::{InputManifest, be_len},
    fault::PipelineFault,
    provider::{ContractRegistry, FeatureClaim, ProviderContract, ProviderPlacement},
    response::{ProviderResponse, RawResponseArchive, RawResponseId},
    route::{SttPolicy, SttRoute},
    transcript::{RawTranscript, decode},
    version::TranscriptLineage,
};

/// The retention a run that transmitted nothing declares.
///
/// A local run has no provider retention to declare, and leaving the field
/// empty is not available: `academic_model_run::ModelRun` takes all twelve
/// section 27.3 fields by value. So the absence is spelled.
pub const LOCAL_ONLY_RETENTION: &str = "LOCAL_ONLY_NO_EXTERNAL_RETENTION";

/// One stage of section 12.3's pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// Admit the inputs, which are the diagram's first line.
    AdmitAuthorizedInputs,
    /// Read the contract the selected provider declared, and refuse a run that
    /// depends on something the contract declares unsupported.
    ///
    /// Before the route, because the placement a route decides over is a field
    /// of the contract: there is no request that says "run this one remotely".
    ReadProviderContract,
    /// Decide local, scoped remote, or blocked.
    SelectProviderRoute,
    /// Ask the provider.
    Transcribe,
    /// Keep the raw response, immutably, under `P2-G5`'s label.
    RetainRawResponse,
    /// Decode it into segments and open the version lineage.
    NormalizeTranscript,
    /// Hand the normalized transcript to the three downstream jobs.
    FanOutDownstreamJobs,
}

impl Stage {
    /// Exhaustive order: the order section 12.3 draws them in.
    pub const ALL: [Self; 7] = [
        Self::AdmitAuthorizedInputs,
        Self::ReadProviderContract,
        Self::SelectProviderRoute,
        Self::Transcribe,
        Self::RetainRawResponse,
        Self::NormalizeTranscript,
        Self::FanOutDownstreamJobs,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AdmitAuthorizedInputs => "ADMIT_AUTHORIZED_INPUTS",
            Self::SelectProviderRoute => "SELECT_PROVIDER_ROUTE",
            Self::ReadProviderContract => "READ_PROVIDER_CONTRACT",
            Self::Transcribe => "TRANSCRIBE",
            Self::RetainRawResponse => "RETAIN_RAW_RESPONSE",
            Self::NormalizeTranscript => "NORMALIZE_TRANSCRIPT",
            Self::FanOutDownstreamJobs => "FAN_OUT_DOWNSTREAM_JOBS",
        }
    }

    /// The box or line section 12.3's diagram draws for this stage, when it
    /// draws one.
    ///
    /// Not every stage has one: reading a contract and fanning out are steps the
    /// diagram folds into an arrow, and returning `None` for them is more honest
    /// than inventing a phrase and then comparing the invention against the
    /// specification. `the_downstream_jobs_are_section_12_3s_own` checks every
    /// `Some` against the block and asserts nothing about how many there are.
    #[must_use]
    pub const fn spec_anchor(self) -> Option<&'static str> {
        match self {
            Self::AdmitAuthorizedInputs => {
                Some("authorized audio chunks + captures + supplied materials")
            }
            Self::SelectProviderRoute => Some("local or explicitly permitted remote"),
            Self::ReadProviderContract | Self::FanOutDownstreamJobs => None,
            Self::Transcribe => Some("TranscriptionProvider"),
            Self::RetainRawResponse => Some("immutable raw provider output retained"),
            Self::NormalizeTranscript => Some("NormalizedTranscript vN"),
        }
    }
}

/// One of the three things section 12.3 fans out to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DownstreamJob {
    /// The lossless document and the preservation-rendered PDF. `P2-L4`.
    LosslessLectureDocument,
    /// The deterministic coverage validator. `P2-L4`.
    CoverageValidator,
    /// Concept, relation, question and gap candidates. `P2-M2`'s queue.
    ProposalJobs,
}

impl DownstreamJob {
    /// Exhaustive order: the order section 12.3's three arrows are drawn in.
    pub const ALL: [Self; 3] = [
        Self::LosslessLectureDocument,
        Self::CoverageValidator,
        Self::ProposalJobs,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LosslessLectureDocument => "LOSSLESS_LECTURE_DOCUMENT",
            Self::CoverageValidator => "COVERAGE_VALIDATOR",
            Self::ProposalJobs => "PROPOSAL_JOBS",
        }
    }

    /// The arrow's own text in section 12.3's fenced block.
    #[must_use]
    pub const fn spec_line(self) -> &'static str {
        match self {
            Self::LosslessLectureDocument => "LosslessLectureDocument + PDF",
            Self::CoverageValidator => "CoverageValidator",
            Self::ProposalJobs => "proposal jobs",
        }
    }

    /// Whether this job's outputs are proposals rather than records.
    ///
    /// Section 27.1 lets a model produce a candidate and section 27.2 does not
    /// let it decide, so the one AI-authored job of the three is marked here
    /// and its outputs go to `academic-proposal`'s queue. This crate mints no
    /// tier of its own; what it says is which job needs one.
    #[must_use]
    pub const fn produces_proposals(self) -> bool {
        match self {
            Self::LosslessLectureDocument | Self::CoverageValidator => false,
            Self::ProposalJobs => true,
        }
    }
}

/// One downstream job, with its own identifier and the input every one of them
/// shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobHandle {
    job: DownstreamJob,
    job_id: ContentDigest,
    input_digest: ContentDigest,
    raw_response: RawResponseId,
    produces_proposals: bool,
}

impl JobHandle {
    /// Which job.
    #[must_use]
    pub const fn job(&self) -> DownstreamJob {
        self.job
    }

    /// Its own identifier, distinct from every sibling's.
    #[must_use]
    pub const fn job_id(&self) -> &ContentDigest {
        &self.job_id
    }

    /// The input digest every sibling cites.
    #[must_use]
    pub const fn input_digest(&self) -> &ContentDigest {
        &self.input_digest
    }

    /// The archived raw response behind the transcript it reads.
    #[must_use]
    pub const fn raw_response(&self) -> RawResponseId {
        self.raw_response
    }

    /// Whether its outputs are proposals.
    #[must_use]
    pub const fn produces_proposals(&self) -> bool {
        self.produces_proposals
    }
}

/// Which provider a run asks, and what the caller depends on it for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    provider: ProviderId,
    model_version: ModelVersion,
    required_claims: Vec<FeatureClaim>,
}

impl ProviderSelection {
    /// Selects a provider and declares what the run depends on.
    #[must_use]
    pub fn of(
        provider: ProviderId,
        model_version: ModelVersion,
        required_claims: Vec<FeatureClaim>,
    ) -> Self {
        Self {
            provider,
            model_version,
            required_claims,
        }
    }

    /// Which provider.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which exact model version.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// What the run depends on the provider for.
    #[must_use]
    pub fn required_claims(&self) -> &[FeatureClaim] {
        &self.required_claims
    }
}

/// What one transcription is asked to do.
#[derive(Debug)]
pub struct TranscriptionRequest<'a> {
    /// Every input the job may read.
    pub manifest: &'a InputManifest,
    /// The contract the selected provider declared.
    pub contract: &'a ProviderContract,
    /// Where the request was routed.
    pub route: &'a SttRoute,
    /// What the caller depends on the provider for.
    pub required_claims: &'a [FeatureClaim],
}

/// Something that turns authorized audio into a provider response.
///
/// **No implementation of this trait ships in this repository.** The acceptance
/// suite supplies mock providers built from committed literals; nothing here
/// records audio, links a speech engine, or opens a socket.
///
/// A remote implementation cannot build its answer without an
/// `academic_egress_boundary::AcceptedResponse`, because
/// [`ProviderResponse::from_remote`] is the only producer of a response whose
/// placement is `Remote` and that is what it takes.
pub trait SttProvider {
    /// Transcribes, or fails.
    ///
    /// `None` is a provider failure and halts the run at
    /// [`Stage::Transcribe`].
    fn transcribe(&self, request: &TranscriptionRequest<'_>) -> Option<ProviderResponse>;
}

/// The section 27.3 fields a caller supplies, because the pipeline cannot
/// derive them.
///
/// The other five -- provider, model version, input artifacts, transmitted
/// ranges and retention declaration -- are derived from the manifest and the
/// route, so a run cannot record a provider it did not ask or a transmission
/// its route did not permit.
#[derive(Debug, Clone)]
pub struct RunIdentity {
    /// The run identifier.
    pub id: ModelRunId,
    /// What the run was for.
    pub purpose: Purpose,
    /// Digest of the prompt template.
    pub prompt_template_hash: Digest32,
    /// Digest of the redaction policy applied to the input.
    pub redaction_policy_hash: Digest32,
    /// The artifact the run produced.
    pub output_artifact: ArtifactId,
    /// When it started, on the caller's own axis.
    pub started_at: u64,
    /// What it cost.
    pub cost: Cost,
    /// What the egress recorded, for a scoped-remote run and for no other.
    pub transmission: Option<Transmission>,
}

/// What one run produced.
#[derive(Debug)]
pub struct RunRecord {
    reached: Vec<Stage>,
    outcome: RunOutcome,
}

impl RunRecord {
    /// The stages this run reached, in order.
    #[must_use]
    pub fn reached(&self) -> &[Stage] {
        &self.reached
    }

    /// What it produced.
    #[must_use]
    pub const fn outcome(&self) -> &RunOutcome {
        &self.outcome
    }

    /// The completed run, if the run completed.
    #[must_use]
    pub const fn completed(&self) -> Option<&CompletedRun> {
        match &self.outcome {
            RunOutcome::Completed(run) => Some(run),
            RunOutcome::Halted { .. } => None,
        }
    }

    /// The stage that failed, and why, when the run halted.
    #[must_use]
    pub const fn failure(&self) -> Option<(Stage, &PipelineFault)> {
        match &self.outcome {
            RunOutcome::Halted { stage, fault } => Some((*stage, fault)),
            RunOutcome::Completed(_) => None,
        }
    }
}

/// How a run ended.
///
/// The completed arm is boxed. A `CompletedRun` carries the whole lineage and
/// the twelve section 27.3 fields, and a halted run carries a stage and a
/// fault; without the box every `RunRecord` in every caller would be as large
/// as the completed one, which `clippy::large_enum_variant` refuses.
#[derive(Debug)]
pub enum RunOutcome {
    /// It reached the last stage.
    Completed(Box<CompletedRun>),
    /// A stage failed and no later stage ran.
    Halted {
        /// Which stage.
        stage: Stage,
        /// Why.
        fault: PipelineFault,
    },
}

/// Everything a completed run produced.
#[derive(Debug)]
pub struct CompletedRun {
    model_run: ModelRun,
    route: SttRoute,
    raw_response: RawResponseId,
    lineage: TranscriptLineage,
    jobs: Vec<JobHandle>,
}

impl CompletedRun {
    /// The section 27.3 record of the model execution. `P2-M1`'s twelve
    /// fields; this crate creates no provenance of its own.
    #[must_use]
    pub const fn model_run(&self) -> &ModelRun {
        &self.model_run
    }

    /// Where the request was routed.
    #[must_use]
    pub const fn route(&self) -> &SttRoute {
        &self.route
    }

    /// The archived raw response.
    #[must_use]
    pub const fn raw_response(&self) -> RawResponseId {
        self.raw_response
    }

    /// The version lineage, opened at version one.
    #[must_use]
    pub const fn lineage(&self) -> &TranscriptLineage {
        &self.lineage
    }

    /// The lineage, so a caller can append a correction to it.
    pub const fn lineage_mut(&mut self) -> &mut TranscriptLineage {
        &mut self.lineage
    }

    /// The normalized transcript, which is the same value at every version.
    #[must_use]
    pub const fn transcript(&self) -> &RawTranscript {
        self.lineage.raw()
    }

    /// The three downstream job handles, in section 12.3's order.
    #[must_use]
    pub fn jobs(&self) -> &[JobHandle] {
        &self.jobs
    }
}

/// Runs the stages in section 12.3's order.
///
/// The first failure ends the run. Each argument is one boundary this crate
/// refuses to own: the admitted inputs, the declared contracts, the user's
/// policy, the selection, the provider, the archive, and the section 27.3
/// fields a caller supplies.
pub fn run(
    manifest: &InputManifest,
    registry: &ContractRegistry,
    policy: &SttPolicy,
    selection: &ProviderSelection,
    provider: &dyn SttProvider,
    archive: &mut RawResponseArchive,
    identity: &RunIdentity,
) -> RunRecord {
    let mut reached = Vec::new();

    macro_rules! step {
        ($stage:expr, $call:expr) => {{
            reached.push($stage);
            match $call {
                Ok(value) => value,
                Err(fault) => {
                    return RunRecord {
                        reached,
                        outcome: RunOutcome::Halted {
                            stage: $stage,
                            fault,
                        },
                    };
                }
            }
        }};
    }

    let input_refs = step!(
        Stage::AdmitAuthorizedInputs,
        admit_authorized_inputs(manifest)
    );
    let contract = step!(
        Stage::ReadProviderContract,
        read_provider_contract(registry, selection)
    );
    // The route is decided from the contract, so a caller cannot ask for a
    // placement the provider did not declare.
    let route = step!(
        Stage::SelectProviderRoute,
        admitted_route(policy.route_for(contract))
    );
    let response = step!(
        Stage::Transcribe,
        transcribe(provider, manifest, contract, &route, selection)
    );
    let raw_response = step!(Stage::RetainRawResponse, retain(archive, &response));
    let (transcript, model_run) = step!(
        Stage::NormalizeTranscript,
        normalize(
            &response,
            contract,
            manifest,
            raw_response,
            identity,
            &route,
            selection,
            input_refs,
        )
    );
    let jobs = step!(Stage::FanOutDownstreamJobs, fan_out(manifest, raw_response));

    RunRecord {
        reached,
        outcome: RunOutcome::Completed(Box::new(CompletedRun {
            model_run,
            route,
            raw_response,
            lineage: TranscriptLineage::open(transcript),
            jobs,
        })),
    }
}

fn admit_authorized_inputs(manifest: &InputManifest) -> Result<InputArtifactRefs, PipelineFault> {
    if manifest.is_empty() {
        return Err(PipelineFault::Input(
            crate::fault::InputFault::EmptyManifest,
        ));
    }
    let mut refs = Vec::new();
    for chunk in manifest.chunks() {
        refs.push(artifact_ref(chunk.digest()));
    }
    for capture in manifest.captures() {
        refs.push(artifact_ref(capture.digest()));
    }
    for supplied in manifest.materials() {
        refs.push(artifact_ref(supplied.digest()));
    }
    InputArtifactRefs::new(refs)
        .map_err(|_| PipelineFault::Input(crate::fault::InputFault::EmptyManifest))
}

fn artifact_ref(digest: &ContentDigest) -> InputArtifactRef {
    let bytes = digest.as_bytes();
    let mut identifier = [0_u8; 16];
    identifier.copy_from_slice(&bytes[..16]);
    InputArtifactRef::new(
        ArtifactId::from_bytes(identifier),
        Digest32::from_bytes(*bytes),
    )
}

fn read_provider_contract<'a>(
    registry: &'a ContractRegistry,
    selection: &ProviderSelection,
) -> Result<&'a ProviderContract, PipelineFault> {
    let contract = registry
        .get(selection.provider(), selection.model_version())
        .ok_or(PipelineFault::NoCapabilityContract)?;
    for claim in selection.required_claims() {
        if !contract.supports(*claim) {
            return Err(PipelineFault::CapabilityUnsupported(claim.decided_by()));
        }
    }
    Ok(contract)
}

/// Decodes the response and records the section 27.3 run, in one stage.
///
/// Both belong to `NormalizeTranscript`: a transcript nothing recorded a run
/// for has no provenance, and a run recorded for a response that did not decode
/// names an output that does not exist. Doing them in one step is what keeps
/// `RunRecord::reached` a prefix of [`Stage::ALL`].
#[expect(
    clippy::too_many_arguments,
    reason = "the two halves of this stage need the response, the contract, the manifest, the archive position, the caller's section 27.3 fields, the route, the selection and the input artifacts"
)]
fn normalize(
    response: &ProviderResponse,
    contract: &ProviderContract,
    manifest: &InputManifest,
    raw_response: RawResponseId,
    identity: &RunIdentity,
    route: &SttRoute,
    selection: &ProviderSelection,
    input_artifact_refs: InputArtifactRefs,
) -> Result<(RawTranscript, ModelRun), PipelineFault> {
    let transcript = decode(
        response,
        contract,
        manifest.binding().lecture(),
        raw_response,
        manifest.input_digest(),
    )?;
    let model_run = record_model_run(identity, route, selection, input_artifact_refs)?;
    Ok((transcript, model_run))
}

fn admitted_route(route: SttRoute) -> Result<SttRoute, PipelineFault> {
    match route.denial() {
        Some(denial) => Err(PipelineFault::Route(denial)),
        None => Ok(route),
    }
}

fn transcribe(
    provider: &dyn SttProvider,
    manifest: &InputManifest,
    contract: &ProviderContract,
    route: &SttRoute,
    selection: &ProviderSelection,
) -> Result<ProviderResponse, PipelineFault> {
    let request = TranscriptionRequest {
        manifest,
        contract,
        route,
        required_claims: selection.required_claims(),
    };
    let response = provider
        .transcribe(&request)
        .ok_or(PipelineFault::ProviderFailed)?;
    // The response's placement is decided by which constructor built it, and
    // the remote one takes an `AcceptedResponse`. So this comparison is what
    // ties the route decision to the egress boundary: a `Local` route that came
    // back with a remote-built response, or the other way round, is refused.
    let expected = match route {
        SttRoute::Local { .. } => ProviderPlacement::Local,
        SttRoute::ScopedRemote { .. } => ProviderPlacement::Remote,
        SttRoute::Blocked { .. } => return Err(PipelineFault::RouteMismatch),
    };
    if response.placement() != expected
        || response.provider() != selection.provider()
        || response.model_version() != selection.model_version()
    {
        return Err(PipelineFault::RouteMismatch);
    }
    Ok(response)
}

fn retain(
    archive: &mut RawResponseArchive,
    response: &ProviderResponse,
) -> Result<RawResponseId, PipelineFault> {
    archive
        .retain(response)
        .map_err(|_| PipelineFault::NotSealable)
}

fn record_model_run(
    identity: &RunIdentity,
    route: &SttRoute,
    selection: &ProviderSelection,
    input_artifact_refs: InputArtifactRefs,
) -> Result<ModelRun, PipelineFault> {
    // A local run transmits nothing and a scoped-remote run transmits exactly
    // what the egress recorded. Neither is a caller's choice: the arm decides.
    let (transmission, retention) = match route {
        SttRoute::Local { .. } => {
            if identity.transmission.is_some() {
                return Err(PipelineFault::LocalRunTransmitted);
            }
            let retention = RetentionDeclaration::new(LOCAL_ONLY_RETENTION)
                .map_err(|_| PipelineFault::NoTransmissionRecord)?;
            (Transmission::LocalOnly, retention)
        }
        SttRoute::ScopedRemote { admission } => {
            let transmission = identity
                .transmission
                .clone()
                .ok_or(PipelineFault::NoTransmissionRecord)?;
            if matches!(transmission, Transmission::LocalOnly) {
                return Err(PipelineFault::NoTransmissionRecord);
            }
            (transmission, admission.retention().clone())
        }
        SttRoute::Blocked { .. } => return Err(PipelineFault::RouteMismatch),
    };
    Ok(ModelRun::record(
        identity.id,
        identity.purpose.clone(),
        selection.provider().clone(),
        selection.model_version().clone(),
        identity.prompt_template_hash,
        input_artifact_refs,
        transmission,
        identity.redaction_policy_hash,
        identity.output_artifact,
        identity.started_at,
        identity.cost.clone(),
        retention,
    ))
}

fn fan_out(
    manifest: &InputManifest,
    raw_response: RawResponseId,
) -> Result<Vec<JobHandle>, PipelineFault> {
    let input_digest = manifest.input_digest();
    let mut jobs = Vec::with_capacity(DownstreamJob::ALL.len());
    for job in DownstreamJob::ALL {
        let mut material = Vec::new();
        material.extend_from_slice(b"academic-transcription-downstream-job-v1\0");
        material.extend_from_slice(&be_len(job.as_str().len()));
        material.extend_from_slice(job.as_str().as_bytes());
        material.extend_from_slice(input_digest.as_bytes());
        material.extend_from_slice(&raw_response.value().to_be_bytes());
        jobs.push(JobHandle {
            job,
            job_id: ContentDigest::sha256(&material),
            input_digest,
            raw_response,
            produces_proposals: job.produces_proposals(),
        });
    }
    Ok(jobs)
}
