//! Expiring permissions and consents, and the dependents they block.
//!
//! Section 25.13's fifth bullet is *`permission/consent expiry`*. Section 32.6
//! fixes what an expiry may not do: *`Provider 정책이 바뀌거나 마지막 확인이
//! 오래되면 permission을 자동 연장하지 않는다`*. Section 34.1's `허가 없는 녹음`
//! row fixes the default and the failure mode: *`default UNKNOWN, Record
//! fail-closed, 학기별 재확인`*.
//!
//! # Blocking is a type, not a check
//!
//! [`LivePermission`] has private fields, no public constructor, no `Default`
//! and no `Clone`. The only thing that produces one is
//! [`PermissionQueue::gate`], and it produces one only when the evaluated
//! instant is strictly before the recorded expiry. A dependent action that
//! needs a permission takes a `LivePermission` by value, so an expired
//! permission does not fail a check — it fails to produce the argument.
//! `tests/compile_fail/a_live_permission_cannot_be_assembled.rs` observes that
//! the struct literal does not compile.
//!
//! Taking it by value is the second half. `LivePermission` is not `Copy` and
//! not `Clone`, so one gate call authorises one dependent action; a caller
//! holding a stale permission cannot spend it twice.
//!
//! # Nothing extends an expiry
//!
//! There is no setter, no `renew`, no `extend` and no `refresh` anywhere in
//! this module: a permission that has lapsed is re-attested, which is a new
//! record with a new expiry, and that is `P2-G6`'s ledger rather than this
//! crate's. `expiring_permission_is_queued_and_blocks_dependents` reads the
//! whole public surface of this module and requires no signature to take a
//! [`PermissionRef`] and return an instant.

use academic_domain::{
    CapturePermissionId, ConsentId, EntityId, PermissionLineageId, TimestampMillis,
};

use crate::CenterError;

/// Which of the two things section 25.13 pairs.
///
/// A capture permission is section 3.7's — permission to record a particular
/// offering. A provider consent is section 32.6's — permission to transmit to
/// a particular provider under a particular policy version. They expire for
/// different reasons and are re-attested by different acts, which is why the
/// reference below is an enum and not a single identifier with a kind beside
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionRef {
    /// Section 3.7's capture permission.
    Capture(CapturePermissionId),
    /// Section 32.6's provider consent.
    Consent(ConsentId),
}

/// What kind of permission a reference names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PermissionKind {
    /// Section 3.7's capture permission.
    Capture,
    /// Section 32.6's provider consent.
    Consent,
}

impl PermissionKind {
    /// Exhaustive listing.
    pub const ALL: [Self; 2] = [Self::Capture, Self::Consent];
}

impl PermissionRef {
    /// Which kind, read off the variant.
    #[must_use]
    pub const fn kind(&self) -> PermissionKind {
        match self {
            Self::Capture(_) => PermissionKind::Capture,
            Self::Consent(_) => PermissionKind::Consent,
        }
    }
}

/// One permission with an expiry.
///
/// The lineage identifier is what connects a re-attestation to the permission
/// it replaces without either overwriting the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpiringPermission {
    reference: PermissionRef,
    lineage: PermissionLineageId,
    granted_at: TimestampMillis,
    expires_at: TimestampMillis,
}

impl ExpiringPermission {
    /// A permission that expires.
    #[must_use]
    pub const fn new(
        reference: PermissionRef,
        lineage: PermissionLineageId,
        granted_at: TimestampMillis,
        expires_at: TimestampMillis,
    ) -> Self {
        Self {
            reference,
            lineage,
            granted_at,
            expires_at,
        }
    }

    /// Which permission.
    #[must_use]
    pub const fn reference(&self) -> PermissionRef {
        self.reference
    }

    /// Which lineage a re-attestation would continue.
    #[must_use]
    pub const fn lineage(&self) -> PermissionLineageId {
        self.lineage
    }

    /// When it was granted.
    #[must_use]
    pub const fn granted_at(&self) -> TimestampMillis {
        self.granted_at
    }

    /// When it lapses. There is no setter.
    #[must_use]
    pub const fn expires_at(&self) -> TimestampMillis {
        self.expires_at
    }

    /// Whether it has lapsed by `at`.
    ///
    /// The comparison is `expires_at <= at`, so an instant exactly on the
    /// expiry is expired. Section 34.1's `Record fail-closed` is what decides
    /// the boundary: the half-open interval that includes the expiry instant
    /// would let one more capture through on a permission that has run out.
    #[must_use]
    pub fn has_lapsed(&self, at: TimestampMillis) -> bool {
        self.expires_at <= at
    }
}

/// A permission proved live at one instant.
///
/// No public constructor, no `Default`, no `Clone`, no `Copy`. The only
/// producer is [`PermissionQueue::gate`].
#[derive(Debug, PartialEq, Eq)]
pub struct LivePermission {
    reference: PermissionRef,
    proved_at: TimestampMillis,
}

impl LivePermission {
    /// Which permission was proved live.
    #[must_use]
    pub const fn reference(&self) -> PermissionRef {
        self.reference
    }

    /// The instant it was proved live at.
    #[must_use]
    pub const fn proved_at(&self) -> TimestampMillis {
        self.proved_at
    }
}

/// What a dependent action is.
///
/// Section 34.1's `허가 없는 녹음` row names what a lapsed capture permission
/// stops — capture, sharing and AI processing — and section 32.6 names what a
/// lapsed provider consent stops. Both are the same shape: something that may
/// not proceed while the permission it rests on is not live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependentActionKind {
    /// Recording a lecture.
    Capture,
    /// Transcribing captured audio.
    Transcribe,
    /// Transmitting to an external provider.
    ProviderTransmission,
    /// Sharing an artefact outside the profile.
    Share,
}

impl DependentActionKind {
    /// Exhaustive listing.
    pub const ALL: [Self; 4] = [
        Self::Capture,
        Self::Transcribe,
        Self::ProviderTransmission,
        Self::Share,
    ];
}

/// One action that cannot proceed without a live permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DependentAction {
    subject: EntityId,
    kind: DependentActionKind,
    requires: PermissionRef,
}

impl DependentAction {
    /// An action that depends on a permission.
    #[must_use]
    pub const fn new(subject: EntityId, kind: DependentActionKind, requires: PermissionRef) -> Self {
        Self {
            subject,
            kind,
            requires,
        }
    }

    /// What the action is about.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// What kind of action.
    #[must_use]
    pub const fn kind(&self) -> DependentActionKind {
        self.kind
    }

    /// Which permission it rests on.
    #[must_use]
    pub const fn requires(&self) -> PermissionRef {
        self.requires
    }
}

/// The permission and consent expiry queue, and the dependents it gates.
#[derive(Debug, Clone, Default)]
pub struct PermissionQueue {
    permissions: Vec<ExpiringPermission>,
    dependents: Vec<DependentAction>,
}

impl PermissionQueue {
    /// An empty queue.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            permissions: Vec::new(),
            dependents: Vec::new(),
        }
    }

    /// Records a permission with an expiry.
    pub fn record(&mut self, permission: ExpiringPermission) {
        self.permissions.push(permission);
    }

    /// Registers an action that depends on a permission.
    pub fn register_dependent(&mut self, action: DependentAction) {
        self.dependents.push(action);
    }

    /// Every recorded permission.
    #[must_use]
    pub fn permissions(&self) -> &[ExpiringPermission] {
        &self.permissions
    }

    /// Every registered dependent action.
    #[must_use]
    pub fn dependents(&self) -> &[DependentAction] {
        &self.dependents
    }

    /// Exactly the permissions that lapse at or before `horizon`.
    ///
    /// A permission that has already lapsed is in this list too: section 25.13
    /// calls the section `expiry`, and a queue that dropped an expired
    /// permission would hide the thing the user most needs to act on.
    #[must_use]
    pub fn expiring_by(&self, horizon: TimestampMillis) -> Vec<&ExpiringPermission> {
        self.permissions
            .iter()
            .filter(|permission| permission.expires_at() <= horizon)
            .collect()
    }

    /// Exactly the dependent actions blocked at `at`.
    ///
    /// An action is blocked when the permission it names has lapsed, and also
    /// when the queue holds no such permission at all. The second half is
    /// section 34.1's `default UNKNOWN`: an unrecorded permission is not an
    /// unrestricted one.
    #[must_use]
    pub fn blocked_dependents(&self, at: TimestampMillis) -> Vec<&DependentAction> {
        self.dependents
            .iter()
            .filter(|action| self.gate(action, at).is_err())
            .collect()
    }

    /// Proves the permission an action rests on is live at `at`.
    ///
    /// # Errors
    ///
    /// [`CenterError::PermissionAbsent`] when the queue holds no permission of
    /// that identity, and [`CenterError::PermissionExpired`] when it holds one
    /// that has lapsed. Both refuse; neither extends anything.
    pub fn gate(
        &self,
        action: &DependentAction,
        at: TimestampMillis,
    ) -> Result<LivePermission, CenterError> {
        let permission = self
            .permissions
            .iter()
            .find(|permission| permission.reference() == action.requires())
            .ok_or(CenterError::PermissionAbsent {
                permission: action.requires(),
            })?;
        if permission.has_lapsed(at) {
            return Err(CenterError::PermissionExpired {
                permission: permission.reference(),
                expires_at: permission.expires_at(),
            });
        }
        Ok(LivePermission {
            reference: permission.reference(),
            proved_at: at,
        })
    }
}
