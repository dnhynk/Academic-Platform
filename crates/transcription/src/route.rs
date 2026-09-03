//! Which provider raw audio is allowed to reach.
//!
//! Three outcomes and no fourth: **local**, **scoped remote**, and **blocked**.
//! [`SttPolicy::route_for`] is a total function over
//! [`ProviderPlacement::ALL`], so every request lands in exactly one of them,
//! and `stt_provider_policy` drives all three.
//!
//! # Absence never falls through to remote
//!
//! [`SttPolicy::new`] holds no approval, and the remote arm reads an approval
//! by exact `(provider, model version)`. So an unconfigured profile routes
//! every remote request to [`SttRoute::Blocked`] with
//! [`RouteDenial::ProviderNotApproved`], and the only way to leave that state
//! is [`SttPolicy::approve_remote`], which takes all three of section
//! `REQ-32-040`'s facets by value. There is no `Option::unwrap_or` and no
//! `Default` on this path: `no_default_reaches_the_remote_arm` in
//! `tests/transcription_scans.rs` pins `route_for` as whole text and holds its
//! call sites at one.
//!
//! # What the remote arm is not
//!
//! It is not permission to transmit. What [`SttRoute::ScopedRemote`] carries is
//! a [`RemoteAdmission`], and the only function that consumes one also takes an
//! `academic_egress_boundary::AcceptedResponse`, whose one producer is
//! `EgressProxy::accept_response` behind `PermissionBroker::execute` and
//! `bind_grant`. This crate holds no socket, implements no
//! `OutboundTransport`, and stages nothing: the decision here is a *route*, and
//! `P2-G1`, `P2-G2` and `P2-G3` are what actually let a byte leave.

use academic_model_run::{ModelVersion, ProviderId, RetentionDeclaration};

use crate::provider::{ProviderContract, ProviderPlacement};

/// Why a route was blocked.
///
/// The three variants are `REQ-32-040`'s three facets: the permission has to
/// cover external processing, the user has to approve the exact provider, and a
/// retention declaration has to exist. A fourth reason would mean a fourth
/// facet, which the requirement does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RouteDenial {
    /// No approval names this provider and model version.
    #[error("no approval names that provider and model version")]
    ProviderNotApproved,
    /// The approval exists and its capture permission does not cover external
    /// processing.
    #[error("the capture permission does not cover external processing")]
    NoExternalProcessingPermission,
    /// The approval exists and declares no retention.
    #[error("the approval declares no provider retention")]
    NoRetentionDeclaration,
}

impl RouteDenial {
    /// Exhaustive order.
    pub const ALL: [Self; 3] = [
        Self::ProviderNotApproved,
        Self::NoExternalProcessingPermission,
        Self::NoRetentionDeclaration,
    ];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProviderNotApproved => "PROVIDER_NOT_APPROVED",
            Self::NoExternalProcessingPermission => "NO_EXTERNAL_PROCESSING_PERMISSION",
            Self::NoRetentionDeclaration => "NO_RETENTION_DECLARATION",
        }
    }
}

/// One user approval of one remote provider.
///
/// All three of `REQ-32-040`'s facets are fields, and
/// [`SttPolicy::approve_remote`] takes the whole value, so an approval that
/// covers two of the three is a value that exists and is refused by name rather
/// than a value that cannot be built. That is deliberate: the point of the
/// three named denials is that a reader can tell which facet was missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteProcessingApproval {
    provider: ProviderId,
    model_version: ModelVersion,
    external_processing_permitted: bool,
    retention: Option<RetentionDeclaration>,
}

impl RemoteProcessingApproval {
    /// Records an approval.
    ///
    /// `external_processing_permitted` is the section 3.7 capture permission's
    /// own answer -- `academic_consent`'s `PermittedUse` carries it -- and is
    /// passed in rather than re-derived, because this crate adds no second
    /// section 3.7 comparison beside `mint_capture_capability`.
    #[must_use]
    pub const fn record(
        provider: ProviderId,
        model_version: ModelVersion,
        external_processing_permitted: bool,
        retention: Option<RetentionDeclaration>,
    ) -> Self {
        Self {
            provider,
            model_version,
            external_processing_permitted,
            retention,
        }
    }

    /// Which provider is approved.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which exact model version is approved.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// Whether the capture permission covers external processing.
    #[must_use]
    pub const fn external_processing_permitted(&self) -> bool {
        self.external_processing_permitted
    }

    /// The retention the user approved, if any was declared.
    #[must_use]
    pub const fn retention(&self) -> Option<&RetentionDeclaration> {
        self.retention.as_ref()
    }
}

/// The right to hand this crate a remote provider's response.
///
/// Private fields and one producer, [`SttPolicy::route_for`]'s remote arm. It
/// carries the retention declaration the approval named, because
/// `academic_model_run::ModelRun` records that field for every run and a run
/// that egressed has to name the retention it was approved under rather than
/// one the caller chose afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteAdmission {
    provider: ProviderId,
    model_version: ModelVersion,
    retention: RetentionDeclaration,
}

impl RemoteAdmission {
    /// Which provider the response may come from.
    #[must_use]
    pub const fn provider(&self) -> &ProviderId {
        &self.provider
    }

    /// Which exact model version.
    #[must_use]
    pub const fn model_version(&self) -> &ModelVersion {
        &self.model_version
    }

    /// The retention the approval declared.
    #[must_use]
    pub const fn retention(&self) -> &RetentionDeclaration {
        &self.retention
    }
}

/// Where one transcription request is routed.
///
/// Three arms. There is no `Default`, no `From`, and no constructor: the one
/// producer is [`SttPolicy::route_for`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SttRoute {
    /// The default route for raw audio: a provider on this machine, which
    /// transmits nothing and needs no approval.
    Local {
        /// Which local provider.
        provider: ProviderId,
        /// Which exact model version.
        model_version: ModelVersion,
    },
    /// A remote provider the user approved for this exact model version, under
    /// a permission that covers external processing and a declared retention.
    ScopedRemote {
        /// What a remote response has to be accompanied by.
        admission: RemoteAdmission,
    },
    /// Everything else.
    Blocked {
        /// Which of the three facets was missing.
        denial: RouteDenial,
    },
}

impl SttRoute {
    /// Stable spelling of the arm, for an audit row that records the decision.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Local { .. } => "LOCAL",
            Self::ScopedRemote { .. } => "SCOPED_REMOTE",
            Self::Blocked { .. } => "BLOCKED",
        }
    }

    /// The admission, when the route is scoped remote.
    #[must_use]
    pub const fn admission(&self) -> Option<&RemoteAdmission> {
        match self {
            Self::ScopedRemote { admission } => Some(admission),
            Self::Local { .. } | Self::Blocked { .. } => None,
        }
    }

    /// The denial, when the route is blocked.
    #[must_use]
    pub const fn denial(&self) -> Option<RouteDenial> {
        match self {
            Self::Blocked { denial } => Some(*denial),
            Self::Local { .. } | Self::ScopedRemote { .. } => None,
        }
    }
}

/// The user's standing decisions about remote transcription.
///
/// A new profile has none. Nothing here reads a file, an environment variable
/// or a default table.
#[derive(Debug, Clone, Default)]
pub struct SttPolicy {
    approvals: Vec<RemoteProcessingApproval>,
}

impl SttPolicy {
    /// A policy holding no remote approval, which is what a new profile has.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            approvals: Vec::new(),
        }
    }

    /// Records one user approval.
    #[must_use]
    pub fn approve_remote(mut self, approval: RemoteProcessingApproval) -> Self {
        self.approvals.push(approval);
        self
    }

    /// Every recorded approval, in the order they were made.
    #[must_use]
    pub fn approvals(&self) -> &[RemoteProcessingApproval] {
        &self.approvals
    }

    /// Routes one request.
    ///
    /// The whole route decision, in one place, matching over
    /// [`ProviderPlacement`]'s whole set. `stt_provider_policy` drives all
    /// three arms and `WHOLE_ROUTE_FOR` pins this body, because a comparison
    /// that is deleted rather than edited is what `P2-RF10` measured passing
    /// every other check.
    #[must_use]
    pub fn route_for(&self, contract: &ProviderContract) -> SttRoute {
        match contract.placement() {
            ProviderPlacement::Local => SttRoute::Local {
                provider: contract.provider().clone(),
                model_version: contract.model_version().clone(),
            },
            ProviderPlacement::Remote => {
                let Some(approval) = self.approvals.iter().find(|approval| {
                    approval.provider() == contract.provider()
                        && approval.model_version() == contract.model_version()
                }) else {
                    return SttRoute::Blocked {
                        denial: RouteDenial::ProviderNotApproved,
                    };
                };
                if !approval.external_processing_permitted() {
                    return SttRoute::Blocked {
                        denial: RouteDenial::NoExternalProcessingPermission,
                    };
                }
                let Some(retention) = approval.retention() else {
                    return SttRoute::Blocked {
                        denial: RouteDenial::NoRetentionDeclaration,
                    };
                };
                SttRoute::ScopedRemote {
                    admission: RemoteAdmission {
                        provider: contract.provider().clone(),
                        model_version: contract.model_version().clone(),
                        retention: retention.clone(),
                    },
                }
            }
        }
    }
}
