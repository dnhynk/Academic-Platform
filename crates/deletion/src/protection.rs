//! Protected artifacts, and the policy reason a refusal has to carry.
//!
//! t068 section 5 fixes it in one clause: *protected artifacts return a policy
//! reason instead of silent refusal*. The shape that satisfies the words and
//! not the contract is a boolean — a predicate that answers "no" and leaves the
//! caller to invent a sentence, or worse, to drop the artifact out of the plan
//! the way a class with nothing in it must not drop out of a `P2-K5` report.
//!
//! So there is no boolean here. [`ProtectionDecision::Protected`] carries a
//! [`ProtectionReason`], the reason names one arm of a closed
//! [`ProtectionPolicyKind`] and the exact clause of the specification that arm
//! comes from, and the type has no `Default`: a decision exists because a
//! registry produced one.

use academic_domain::TimestampMillis;

use crate::target::DeletionTarget;

/// Why an artifact may not be deleted.
///
/// Closed. Each arm is a rule the specification already states, and the
/// citation is checked against the design document by
/// `every_protection_policy_cites_a_clause_that_exists`, so an arm invented
/// here fails against the document rather than against a second list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ProtectionPolicyKind {
    /// Section 34.6's first recovery principle: the original artifact and the
    /// claims over it are preserved, so a correction never deletes an original.
    OriginalIsPreserved,
    /// Section 32.5: a capture's permission conditions are inherited by every
    /// derivative as an artifact policy, and a condition that forbids deletion
    /// of the instructor's own recording binds this build too.
    InstructorConditionForbidsIt,
    /// Section 32.5 again: audio and transcript retention are two bounds and a
    /// derivative inherits the stricter. An artifact still inside a retention
    /// floor is not deletable yet.
    RetentionFloorNotReached,
    /// Section 34.4's leak row: a quarantined artifact is evidence in an open
    /// security incident, and section 34.6's fifth principle keeps that
    /// lifecycle separate from ordinary correction.
    QuarantinedByOpenIncident,
}

impl ProtectionPolicyKind {
    /// Exhaustive listing, in the order the specification reaches them.
    pub const ALL: [Self; 4] = [
        Self::OriginalIsPreserved,
        Self::InstructorConditionForbidsIt,
        Self::RetentionFloorNotReached,
        Self::QuarantinedByOpenIncident,
    ];

    /// The stable external spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OriginalIsPreserved => "ORIGINAL_IS_PRESERVED",
            Self::InstructorConditionForbidsIt => "INSTRUCTOR_CONDITION_FORBIDS_IT",
            Self::RetentionFloorNotReached => "RETENTION_FLOOR_NOT_REACHED",
            Self::QuarantinedByOpenIncident => "QUARANTINED_BY_OPEN_INCIDENT",
        }
    }

    /// The specification section this arm is read out of.
    #[must_use]
    pub const fn spec_section(self) -> &'static str {
        match self {
            Self::OriginalIsPreserved => "34.6",
            Self::InstructorConditionForbidsIt | Self::RetentionFloorNotReached => "32.5",
            Self::QuarantinedByOpenIncident => "34.4",
        }
    }

    /// The specification's own words this arm rests on.
    ///
    /// Checked against the design document rather than restated for a reader's
    /// benefit: an arm whose sentence is no longer in section 32.5, 34.4 or
    /// 34.6 fails.
    #[must_use]
    pub const fn spec_words(self) -> &'static str {
        match self {
            Self::OriginalIsPreserved => "원본 artifact와 기존 Claim은 보존한다",
            Self::InstructorConditionForbidsIt => "허가 조건을 artifact policy로 상속",
            Self::RetentionFloorNotReached => {
                "각 derivative는 부모의 허가 조건과 더 엄격한 만료일을 상속하고"
            }
            Self::QuarantinedByOpenIncident => "artifact quarantine",
        }
    }
}

/// A refusal that says which policy refused, and for how long.
///
/// Private fields and one producer. The `until` is the honest half: a retention
/// floor lapses and an incident closes, so a refusal that could never be
/// revisited would be a different claim from the one the specification makes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtectionReason {
    kind: ProtectionPolicyKind,
    detail: String,
    revisit_at: Option<TimestampMillis>,
}

impl ProtectionReason {
    /// Records a refusal under one policy.
    #[must_use]
    pub fn under(
        kind: ProtectionPolicyKind,
        detail: String,
        revisit_at: Option<TimestampMillis>,
    ) -> Self {
        Self {
            kind,
            detail,
            revisit_at,
        }
    }

    /// Which policy refused.
    #[must_use]
    pub const fn kind(&self) -> ProtectionPolicyKind {
        self.kind
    }

    /// The registry's own words about this artifact.
    #[must_use]
    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// When the refusal could be revisited, when it can be at all.
    #[must_use]
    pub const fn revisit_at(&self) -> Option<TimestampMillis> {
        self.revisit_at
    }

    /// The sentence a surface shows.
    ///
    /// It always names a policy and a section. There is no rendering of a
    /// refusal that omits either, which is what "not a silent refusal" means
    /// once it has to survive a caller that only prints one string.
    #[must_use]
    pub fn to_row(&self) -> String {
        format!(
            "{} (section {}): {}",
            self.kind.as_str(),
            self.kind.spec_section(),
            self.detail
        )
    }
}

/// What a protection registry says about one target.
///
/// Two arms, and the refusing one carries its reason in the type. There is no
/// `Default`, no `bool` conversion, and no arm meaning "refused, reason
/// unavailable".
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProtectionDecision {
    /// Nothing protects this artifact.
    NotProtected,
    /// This policy does, in these words.
    Protected(ProtectionReason),
}

impl ProtectionDecision {
    /// The reason, when there is one.
    #[must_use]
    pub const fn reason(&self) -> Option<&ProtectionReason> {
        match self {
            Self::NotProtected => None,
            Self::Protected(reason) => Some(reason),
        }
    }
}

/// Answers whether one artifact is protected, and by what.
///
/// The trait returns a [`ProtectionDecision`] and not a `bool`, so an
/// implementation cannot refuse without saying why.
pub trait ProtectionRegistry {
    /// Decides one target.
    fn decide(&self, target: &DeletionTarget) -> ProtectionDecision;
}
