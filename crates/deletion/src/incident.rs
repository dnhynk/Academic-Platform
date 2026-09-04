//! External leakage, and the lifecycle that is not claim correction.
//!
//! Section 34.6 lists five common recovery principles. The first four are the
//! ordinary correction path — keep the original, supersede or reject the wrong
//! claim, recompute the dependent projections, leave a correction marker on the
//! screens that used it. The fifth is a different sentence about a different
//! kind of event:
//!
//! ```text
//! 5. 외부 유출은 일반 correction이 아니라 security incident lifecycle로 처리한다.
//! ```
//!
//! A leak is not wrong information that a better claim replaces. The bytes left
//! the device. Superseding the claim that described them changes what the graph
//! says and changes nothing about where they are, so a build that let a
//! supersession close a leak would be reporting a containment it never
//! performed.
//!
//! # How the type says it
//!
//! [`ExternalLeakIncident`] advances only by recording [`RecoveryStep`]s, and
//! [`ExternalLeakIncident::close`] returns an [`IncidentClosure`] only when all
//! four are present. `IncidentClosure` has private fields, no `Default`, and one
//! producer, so it cannot be assembled from outside this module.
//!
//! Nothing in this crate converts a correction into a closure or into a state.
//! [`ExternalLeakIncident::record_claim_correction`] exists and deliberately
//! returns nothing that advances the lifecycle: section 34.6's first four
//! principles still apply to the claim, and recording that they happened is how
//! an operator sees that the claim was handled *and* the incident is still open.
//! `leak_incident_cannot_be_closed_by_claim_supersession` drives all three
//! `CorrectionOutcome` arms — including `Modify`, which is supersession — and
//! requires the state to be unchanged and `close` to still refuse; the whole
//! public signature inventory in `deletion_scans.rs` requires no function
//! anywhere in this crate to take a correction type and return an incident
//! state or a closure; and `tests/compile_fail` holds the struct literal.

use academic_domain::{ArtifactId, ContentDigest, TimestampMillis};
use academic_evidence_center::{CorrectionChoice, CorrectionRecord};

/// The four things section 34.4's leak row requires before an incident closes.
///
/// Its recovery column is *`token revoke/rotate, provider deletion request,
/// artifact quarantine, incident log와 범위 조사`*.
/// `the_recovery_steps_are_section_34_4s_own` splits that cell and compares the
/// two sets in both directions, so a step invented here fails and a step the
/// document adds fails too.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecoveryStep {
    /// `token revoke/rotate`
    TokenRevokeOrRotate,
    /// `provider deletion request`
    ProviderDeletionRequest,
    /// `artifact quarantine`
    ArtifactQuarantine,
    /// `incident log와 범위 조사`
    IncidentLogAndScopeInvestigation,
}

impl RecoveryStep {
    /// Exhaustive listing, in the order the cell names them.
    pub const ALL: [Self; 4] = [
        Self::TokenRevokeOrRotate,
        Self::ProviderDeletionRequest,
        Self::ArtifactQuarantine,
        Self::IncidentLogAndScopeInvestigation,
    ];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TokenRevokeOrRotate => "TOKEN_REVOKE_OR_ROTATE",
            Self::ProviderDeletionRequest => "PROVIDER_DELETION_REQUEST",
            Self::ArtifactQuarantine => "ARTIFACT_QUARANTINE",
            Self::IncidentLogAndScopeInvestigation => "INCIDENT_LOG_AND_SCOPE_INVESTIGATION",
        }
    }

    /// The specification's own words for this step.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::TokenRevokeOrRotate => "token revoke/rotate",
            Self::ProviderDeletionRequest => "provider deletion request",
            Self::ArtifactQuarantine => "artifact quarantine",
            Self::IncidentLogAndScopeInvestigation => "incident log와 범위 조사",
        }
    }
}

/// Where a leak incident stands.
///
/// Three states and no fourth. There is no `Superseded` and no `Corrected`,
/// which is section 34.6's fifth principle expressed as an absent arm rather
/// than as a rule a reviewer has to remember.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum LeakIncidentState {
    /// Reported; not every recovery step has happened.
    Open,
    /// Every recovery step has happened; the incident has not been closed yet.
    Contained,
    /// Closed, by a closure that names the four steps and the exposure.
    Closed,
}

impl LeakIncidentState {
    /// Exhaustive listing.
    pub const ALL: [Self; 3] = [Self::Open, Self::Contained, Self::Closed];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Contained => "CONTAINED",
            Self::Closed => "CLOSED",
        }
    }
}

/// What section 34.4's leak row requires an incident to state.
///
/// Its uncertainty column is *`즉시 high-severity incident, 노출
/// byte/source/destination/retention`*, so an exposure that did not name all
/// four of those would be a closure with nothing in it. The byte count is a
/// count: no type in this crate can hold a leaked byte, and the whole field-type
/// inventory in `deletion_scans.rs` is what says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExposureScope {
    exposed_bytes: u64,
    source: ArtifactId,
    destination: ContentDigest,
    provider_retention_days: u32,
}

impl ExposureScope {
    /// Records the measured scope of one exposure.
    #[must_use]
    pub const fn new(
        exposed_bytes: u64,
        source: ArtifactId,
        destination: ContentDigest,
        provider_retention_days: u32,
    ) -> Self {
        Self {
            exposed_bytes,
            source,
            destination,
            provider_retention_days,
        }
    }

    /// How many bytes left.
    #[must_use]
    pub const fn exposed_bytes(&self) -> u64 {
        self.exposed_bytes
    }

    /// Which artifact they came from.
    #[must_use]
    pub const fn source(&self) -> ArtifactId {
        self.source
    }

    /// The broker's canonical destination digest.
    #[must_use]
    pub const fn destination(&self) -> ContentDigest {
        self.destination
    }

    /// The provider's stated retention, in days.
    #[must_use]
    pub const fn provider_retention_days(&self) -> u32 {
        self.provider_retention_days
    }
}

/// Proof that a leak incident was actually closed.
///
/// Private fields, no `Default`, and one producer — [`ExternalLeakIncident::close`]
/// — which refuses until every [`RecoveryStep`] is recorded. There is no
/// conversion into this type from anything, and in particular none from a
/// correction record: `impl_headers_naming_incident_closure_are_the_two_here`
/// pins the whole set of `impl` blocks that mention it, because `P2-Y3` measured
/// that a `From`/`Into` conversion escapes every public-function sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidentClosure {
    steps: [RecoveryStep; 4],
    scope: ExposureScope,
    closed_at: TimestampMillis,
}

impl IncidentClosure {
    /// The four steps, in specification order.
    #[must_use]
    pub const fn steps(&self) -> &[RecoveryStep; 4] {
        &self.steps
    }

    /// What the exposure was.
    #[must_use]
    pub const fn scope(&self) -> ExposureScope {
        self.scope
    }

    /// When it closed.
    #[must_use]
    pub const fn closed_at(&self) -> TimestampMillis {
        self.closed_at
    }
}

/// Why a leak incident refused to close.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IncidentError {
    /// These recovery steps have not happened.
    #[error("the incident cannot close: {} has not happened", .0.as_str())]
    RecoveryStepMissing(RecoveryStep),
    /// The incident is already closed.
    #[error("the incident is already closed")]
    AlreadyClosed,
}

/// One external leakage of private code or lecture data.
///
/// Section 34.4's row: *private code 또는 lecture data 유출*. It advances by
/// recovery step and by nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLeakIncident {
    scope: ExposureScope,
    opened_at: TimestampMillis,
    steps: Vec<RecoveryStep>,
    corrections: Vec<CorrectionChoice>,
    closure: Option<IncidentClosure>,
}

impl ExternalLeakIncident {
    /// Opens an incident over a measured exposure.
    ///
    /// The scope is required at the point the incident is opened, because
    /// section 34.4 asks for it *immediately* — `즉시 high-severity incident` —
    /// and an incident that could be opened without one would let "we are
    /// looking into it" stand in for "this is what left".
    #[must_use]
    pub const fn reported(scope: ExposureScope, opened_at: TimestampMillis) -> Self {
        Self {
            scope,
            opened_at,
            steps: Vec::new(),
            corrections: Vec::new(),
            closure: None,
        }
    }

    /// Records that one recovery step happened.
    pub fn record_recovery(&mut self, step: RecoveryStep) {
        if !self.steps.contains(&step) {
            self.steps.push(step);
        }
    }

    /// Records that the claim describing the leaked artifact was corrected.
    ///
    /// Section 34.6's first four principles still apply to the claim, and this
    /// is where an operator sees that they were followed. It returns nothing
    /// and advances nothing: the fifth principle is that external leakage is
    /// **not** handled as a correction, so a supersession is a fact recorded
    /// beside the incident rather than a transition inside it.
    pub fn record_claim_correction(&mut self, record: &CorrectionRecord) {
        self.corrections.push(record.choice());
    }

    /// Which corrections were filed against the claim, in recording order.
    #[must_use]
    pub fn claim_corrections(&self) -> &[CorrectionChoice] {
        &self.corrections
    }

    /// The exposure this incident is about.
    #[must_use]
    pub const fn scope(&self) -> ExposureScope {
        self.scope
    }

    /// When it was opened.
    #[must_use]
    pub const fn opened_at(&self) -> TimestampMillis {
        self.opened_at
    }

    /// Which recovery steps have happened, in recording order.
    #[must_use]
    pub fn recorded_steps(&self) -> &[RecoveryStep] {
        &self.steps
    }

    /// The recovery steps that have not happened, in specification order.
    #[must_use]
    pub fn missing_steps(&self) -> Vec<RecoveryStep> {
        RecoveryStep::ALL
            .into_iter()
            .filter(|step| !self.steps.contains(step))
            .collect()
    }

    /// Where the incident stands.
    #[must_use]
    pub fn state(&self) -> LeakIncidentState {
        if self.closure.is_some() {
            LeakIncidentState::Closed
        } else if self.missing_steps().is_empty() {
            LeakIncidentState::Contained
        } else {
            LeakIncidentState::Open
        }
    }

    /// The closure, once there is one.
    #[must_use]
    pub const fn closure(&self) -> Option<&IncidentClosure> {
        self.closure.as_ref()
    }

    /// Closes the incident, if every recovery step has happened.
    ///
    /// # Errors
    ///
    /// [`IncidentError::RecoveryStepMissing`] naming the first step that has
    /// not happened, and [`IncidentError::AlreadyClosed`] for a second close.
    pub fn close(&mut self, closed_at: TimestampMillis) -> Result<&IncidentClosure, IncidentError> {
        if self.closure.is_some() {
            return Err(IncidentError::AlreadyClosed);
        }
        if let Some(step) = self.missing_steps().first() {
            return Err(IncidentError::RecoveryStepMissing(*step));
        }
        self.closure = Some(IncidentClosure {
            steps: RecoveryStep::ALL,
            scope: self.scope,
            closed_at,
        });
        self.closure.as_ref().ok_or(IncidentError::AlreadyClosed)
    }
}
