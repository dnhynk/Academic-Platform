//! The egress boundary: scan, minimize, preview, and the one outbound seam.
//!
//! `academic-egress-boundary` stages for the egress-proxy process at the edge of
//! the section 3.6 topology. It is a separate package from `P2-G7`'s
//! `academic-egress`, whose whole manifest and whole product source that task
//! pins as one fixed process-class binding; the socket, when one exists, belongs
//! to the process, and what this crate owns is everything that must be right
//! before the process may open one.
//!
//! Everything above it — the core, the broker, the provider registry — holds no
//! outbound socket, and `only_egress_crate_has_a_socket` in
//! `tools/phase1-scaffold-policy.test.mjs` is what keeps that true by crate
//! rather than by convention. No socket implementation ships today: ADR-002 is
//! unaccepted, the admission receipt is incomplete, and `product_network` is
//! still `NONE`, so the transport is a trait the caller supplies and the guard
//! is what confines a future implementation to the two egress crates.
//!
//! What does ship is the part that has to be right before a socket can exist:
//!
//! - a versioned DLP rulepack whose identity is recorded in every grant;
//! - structural minimization, so a whole-file request becomes the declarations
//!   it actually named;
//! - a byte-accurate preview that is the same buffer the transport writes; and
//! - a refusal path with no byte on it.
//!
//! ## What "zero bytes" means here
//!
//! [`EgressProxy::stage`] returns either a [`StagedPayload`] or an
//! [`EgressDenial`]. The denial holds reason codes, ranges, and rule names and
//! no payload. A caller holding one has nothing it could transmit, because
//! [`OutboundTransport`] is reached only through [`EgressProxy::transmit`],
//! which takes a `&StagedPayload`. The staged bytes are therefore not withheld
//! on a refusal; they are never built.
//!
//! A transfer already past the capability boundary is the one case with a
//! non-zero count: a grant that expires mid-transfer aborts and audits the
//! bytes already handed to the transport, which is fault `EG04`'s stated
//! outcome and is reported by [`EgressDenial::bytes_transmitted`].

mod minimize;
mod response;
mod rulepack;
mod stage;
mod transport;

pub use minimize::{ClassificationError, Item, SourceRange, classify, items, minimal_ranges};
pub use response::{
    AcceptedResponse, CanaryCorpus, CanaryHit, HitSource, Incident, IncidentSeverity,
};
pub use rulepack::{Finding, Rule, Rulepack, RulepackId, ScanError, SpanKind};
pub use stage::{
    EgressDenial, IdentifierPolicy, Preview, SourceDocument, StagedPayload, StagingRequest,
    Substitution,
};
pub use transport::{
    JournalEntry, OutboundTransport, ReconstructedAudit, StagedGrantJournal, Transmission,
    TransmissionPlan, TransportError,
};

use academic_policy::{BrokerError, CapabilityToken, GrantRow, PermissionBroker, ReasonCode};

/// Where a decision sends the work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// The payload is staged and may be transmitted under a live grant.
    StagedForEgress,
    /// The work stays on this machine, or it does not happen.
    LocalOnlyOrStop,
}

impl Route {
    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StagedForEgress => "STAGED_FOR_EGRESS",
            Self::LocalOnlyOrStop => "LOCAL_ONLY_OR_STOP",
        }
    }
}

/// The cloud-egress default when local model quality is insufficient.
///
/// `GATE-38-028` is open. It takes no argument, so no quality score, benchmark,
/// or confidence estimate can be passed to it, and none can change it. What
/// closes the gate is the user configuring a per-tuple egress rule in the
/// broker; until a rule exists the broker denies with `NO_GRANT` and this is
/// the route.
#[must_use]
pub const fn cloud_egress_default() -> Route {
    Route::LocalOnlyOrStop
}

/// A refusal that is not a policy decision.
#[derive(Debug, thiserror::Error)]
pub enum EgressError {
    /// Content or policy refused the transfer. No byte left, except the count
    /// [`EgressDenial::bytes_transmitted`] reports for an aborted transfer.
    #[error(transparent)]
    Denied(EgressDenial),
    /// The broker's own store failed. No capability was consumed.
    #[error(transparent)]
    Broker(BrokerError),
}

impl EgressError {
    /// The denial, when the refusal was a decision rather than a store failure.
    #[must_use]
    pub const fn denial(&self) -> Option<&EgressDenial> {
        match self {
            Self::Denied(denial) => Some(denial),
            Self::Broker(_) => None,
        }
    }

    /// The closed reason code, when the refusal was a decision.
    #[must_use]
    pub const fn reason(&self) -> Option<ReasonCode> {
        match self {
            Self::Denied(denial) => Some(denial.reason()),
            Self::Broker(_) => None,
        }
    }
}

/// A runtime call that could not be built is a scope refusal, not a store failure.
///
/// `P2-G7` binds the process class into `RuntimeToolCall::new`, which refuses a
/// class that does not hold `OpenOutboundSocket` -- only `EgressProxy` does. A
/// call from any other process is therefore rejected before the capability
/// boundary is reached, and reporting it as a scope mismatch says which side was
/// out of scope rather than which type failed to construct.
fn unbuildable_call(error: BrokerError) -> EgressError {
    match error {
        BrokerError::InvalidRuntimeCall => EgressError::Denied(EgressDenial::new(
            ReasonCode::ScopeMismatch,
            "the runtime call is outside the process class or range the capability allows",
        )),
        other => from_broker(other, "the staged payload has no valid grant range"),
    }
}

fn from_broker(error: BrokerError, context: &str) -> EgressError {
    match error {
        BrokerError::Denied(reason) => {
            EgressError::Denied(EgressDenial::new(reason, context.to_owned()))
        }
        other => EgressError::Broker(other),
    }
}

/// The staging and transmission boundary.
#[derive(Debug)]
pub struct EgressProxy<'broker> {
    broker: &'broker PermissionBroker,
    rulepack: Rulepack,
}

impl<'broker> EgressProxy<'broker> {
    /// A proxy over `broker` using the shipped rulepack.
    #[must_use]
    pub const fn new(broker: &'broker PermissionBroker) -> Self {
        Self {
            broker,
            rulepack: Rulepack::builtin(),
        }
    }

    /// A proxy over `broker` using an explicit rulepack.
    #[must_use]
    pub const fn with_rulepack(broker: &'broker PermissionBroker, rulepack: Rulepack) -> Self {
        Self { broker, rulepack }
    }

    /// The versioned rulepack identity every grant must record.
    #[must_use]
    pub fn rulepack_id(&self) -> RulepackId {
        self.rulepack.id()
    }

    /// Classifies, minimizes, scans, redacts, rescans, and stages, or refuses.
    ///
    /// # Errors
    ///
    /// Returns the closed reason code for the first step that refused. Every
    /// arm is a refusal; there is no arm that returns a partial payload.
    pub fn stage(&self, request: &StagingRequest<'_>) -> Result<StagedPayload, EgressDenial> {
        stage::stage(&self.rulepack, request)
    }

    /// Reads the grant this transmission will actually spend and binds it.
    ///
    /// Two bindings, and neither one is a property of the caller's arguments
    /// alone, which is why both are read from the store before any byte is
    /// built.
    ///
    /// The first is that the plan and the token name the same grant. They are
    /// separate inputs: `execute` consumes the row the *token* names, while the
    /// journal and the audit reconciliation record the row the *plan* names. A
    /// plan naming another grant leaves the journal pointing at a row that was
    /// never spent, and makes the rulepack comparison below read a row that is
    /// not the one being consumed.
    ///
    /// The second is that the row's recorded rulepack digest is the digest of
    /// the pack that produced these staged bytes: a grant reviewed under one
    /// redaction policy may not carry a payload produced by another.
    ///
    /// Every path to a transport calls this first. `the_byte_path_has_one_derivation`
    /// in `byte_path_pin.rs` pins this function as whole text and counts its
    /// call sites, because a second path that skipped it would be a send with
    /// an unchecked grant and no test would otherwise notice.
    ///
    /// # Errors
    ///
    /// [`ReasonCode::ScopeMismatch`] when the plan names a grant other than the
    /// token's or the rulepack digests differ, [`ReasonCode::NoGrant`] when the
    /// row is absent, and [`EgressError::Broker`] when the store itself fails.
    fn bind_grant(
        &self,
        plan: &TransmissionPlan<'_>,
        capability: &CapabilityToken,
        staged: &StagedPayload,
    ) -> Result<GrantRow, EgressError> {
        if plan.grant_id != capability.grant_id() {
            return Err(EgressError::Denied(EgressDenial::new(
                ReasonCode::ScopeMismatch,
                format!(
                    "the plan names grant {} but the capability consumes {}",
                    plan.grant_id,
                    capability.grant_id()
                ),
            )));
        }
        let grant = self
            .broker
            .grant_row(plan.grant_id)
            .map_err(EgressError::Broker)?
            .ok_or_else(|| {
                EgressError::Denied(EgressDenial::new(
                    ReasonCode::NoGrant,
                    format!("no grant row for {}", plan.grant_id),
                ))
            })?;
        let recorded = staged.preview().rulepack().redaction_policy_hash().as_str();
        if grant.redaction_policy_hash != recorded {
            return Err(EgressError::Denied(EgressDenial::new(
                ReasonCode::ScopeMismatch,
                format!(
                    "grant records redaction policy {} but the payload was produced by {recorded}",
                    grant.redaction_policy_hash
                ),
            )));
        }
        Ok(grant)
    }

    /// Transmits the staged bytes under a live capability.
    ///
    /// [`EgressProxy::bind_grant`] runs first, so the grant row exists, is the
    /// row the token will consume, and records the rulepack that produced these
    /// bytes. The broker then re-derives the payload digest at the capability
    /// boundary, so the bytes written are the previewed bytes on two
    /// independent grounds.
    ///
    /// # Errors
    ///
    /// Returns [`EgressError::Denied`] when the grant, its rulepack binding, the
    /// capability scope, or the transport refuses, and [`EgressError::Broker`]
    /// when the broker's own store fails.
    pub fn transmit<T: OutboundTransport>(
        &self,
        capability: &CapabilityToken,
        staged: &StagedPayload,
        plan: &TransmissionPlan<'_>,
        journal: &mut StagedGrantJournal,
        transport: &mut T,
        now: &dyn Fn() -> u64,
    ) -> Result<Transmission, EgressError> {
        let grant = self.bind_grant(plan, capability, staged)?;

        let digest = staged.preview().digest().as_str().to_owned();
        let started_at = now();
        journal.append(JournalEntry::SendIntent {
            grant_id: grant.grant_id.clone(),
            staged_object_id: staged.staged_object_id().to_owned(),
            payload_digest: digest.clone(),
            byte_count: staged.preview().byte_len(),
            destination_id: plan.destination_id.to_owned(),
            at: started_at,
        });

        let call = transport::staged_runtime_call(staged, plan).map_err(unbuildable_call)?;
        let written = self
            .broker
            .execute(capability, call, started_at, |authorized| {
                transport::write_authorized_bytes(
                    &authorized,
                    transport,
                    plan.chunk_bytes,
                    now,
                    plan.expires_at,
                )
            });

        match written {
            Err(error) => {
                journal.append(JournalEntry::SendOutcome {
                    grant_id: grant.grant_id.clone(),
                    bytes_sent: 0,
                    complete: false,
                    at: now(),
                });
                Err(from_broker(
                    error,
                    "the capability boundary refused the staged payload",
                ))
            }
            Ok(Err(transport_error)) => {
                journal.append(JournalEntry::SendOutcome {
                    grant_id: grant.grant_id.clone(),
                    bytes_sent: transport_error.sent(),
                    complete: false,
                    at: now(),
                });
                Err(EgressError::Denied(transport::transport_denial(
                    &transport_error,
                )))
            }
            Ok(Ok(sent)) => {
                journal.append(JournalEntry::SendOutcome {
                    grant_id: grant.grant_id.clone(),
                    bytes_sent: sent,
                    complete: sent == staged.preview().byte_len(),
                    at: now(),
                });
                Ok(transport::transmission(sent, digest))
            }
        }
    }

    /// Transmits without recording the completion half of the journal.
    ///
    /// This is fault `EG05`: the process is killed after the provider send and
    /// before the audit write. It exists so the recovery path can be tested
    /// without a real kill. Every refusal [`EgressProxy::transmit`] makes before
    /// the transport is reached, this one makes too --
    /// [`EgressProxy::bind_grant`] is the first statement of both, and
    /// `the_byte_path_has_one_derivation` counts its call sites so that stays
    /// true. What is missing here is the completion half of the journal, and
    /// only that.
    ///
    /// # Errors
    ///
    /// As [`EgressProxy::transmit`].
    pub fn transmit_without_completion<T: OutboundTransport>(
        &self,
        capability: &CapabilityToken,
        staged: &StagedPayload,
        plan: &TransmissionPlan<'_>,
        journal: &mut StagedGrantJournal,
        transport: &mut T,
        now: &dyn Fn() -> u64,
    ) -> Result<Transmission, EgressError> {
        let grant = self.bind_grant(plan, capability, staged)?;
        let started_at = now();
        journal.append(JournalEntry::SendIntent {
            grant_id: grant.grant_id.clone(),
            staged_object_id: staged.staged_object_id().to_owned(),
            payload_digest: staged.preview().digest().as_str().to_owned(),
            byte_count: staged.preview().byte_len(),
            destination_id: plan.destination_id.to_owned(),
            at: started_at,
        });
        let call = transport::staged_runtime_call(staged, plan).map_err(unbuildable_call)?;
        let sent = self
            .broker
            .execute(capability, call, started_at, |authorized| {
                transport::write_authorized_bytes(
                    &authorized,
                    transport,
                    plan.chunk_bytes,
                    now,
                    plan.expires_at,
                )
            })
            .map_err(|error| {
                from_broker(error, "the capability boundary refused the staged payload")
            })?
            .map_err(|error| EgressError::Denied(transport::transport_denial(&error)))?;
        Ok(transport::transmission(
            sent,
            staged.preview().digest().as_str().to_owned(),
        ))
    }

    /// Scans a provider response and quarantines it on any hit.
    ///
    /// # Errors
    ///
    /// Returns the [`Incident`] for a quarantined response. The response bytes
    /// are dropped inside this call and are not part of the incident.
    pub fn accept_response(
        &self,
        corpus: &CanaryCorpus,
        response: &[u8],
    ) -> Result<AcceptedResponse, Incident> {
        response::accept_response(&self.rulepack, corpus, response)
    }
}
