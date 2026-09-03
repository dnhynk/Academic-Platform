//! What a classification is bound to: one snapshot, one goal version, one
//! concept.
//!
//! Section 18.4's third bullet is `classification은 snapshot과 project goal
//! 버전에 종속된다`. Two things follow, and both are shape rather than check:
//!
//! * a classification's identity is [`ClassificationKey`], which holds all
//!   three parts and has no constructor that omits one, so there is no value
//!   naming a concept without saying which snapshot and which goal version it
//!   was decided under; and
//! * [`GoalScope`] carries a version beside the goal identifier, so the same
//!   goal at two versions is two scopes. `classification_is_snapshot_and_goal_
//!   scoped` compares the keys a corpus produces across both axes.
//!
//! `P2-R6` owns the `ProjectGoal` schema itself — text, criteria, constraints
//! and unresolved decisions. This crate needs none of that: what a
//! classification depends on is *which version was in force*, so what is here
//! is the identity and the version and nothing else.

use crate::ClassificationError;

/// Whether `value` is an identifier this system may hold and hand back.
///
/// `[A-Za-z0-9._-]` within 64 bytes: the shape
/// [`academic_repository_analysis::SubjectId`] accepts, so a goal identifier
/// and a subject identifier cannot differ in what they admit.
pub(crate) fn validated(value: String, field: &'static str) -> Result<String, ClassificationError> {
    let valid = !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if valid {
        Ok(value)
    } else {
        Err(ClassificationError::InvalidIdentifier(field, value))
    }
}

/// Names one project goal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoalId {
    identifier: String,
}

impl GoalId {
    /// Validates and takes a goal identifier.
    ///
    /// # Errors
    ///
    /// [`ClassificationError::InvalidIdentifier`] when it is empty, over 64
    /// bytes, or holds a byte outside `[A-Za-z0-9._-]`.
    pub fn new(value: impl Into<String>) -> Result<Self, ClassificationError> {
        Ok(Self {
            identifier: validated(value.into(), "goal")?,
        })
    }

    /// The identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.identifier
    }
}

/// One goal at one version.
///
/// The version is not optional and has no default. A goal identifier alone
/// would let a classification decided under an earlier statement of the goal be
/// read as an answer about the current one, which is what section 18.4's third
/// bullet refuses.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GoalScope {
    goal: GoalId,
    version: u64,
}

impl GoalScope {
    /// Names a goal at a version.
    #[must_use]
    pub const fn at(goal: GoalId, version: u64) -> Self {
        Self { goal, version }
    }

    /// Which goal.
    #[must_use]
    pub const fn goal(&self) -> &GoalId {
        &self.goal
    }

    /// Which version of it.
    #[must_use]
    pub const fn version(&self) -> u64 {
        self.version
    }
}

/// The identity of one classification: snapshot, goal version, concept.
///
/// Section 18.4's third bullet as a value. Two classifications of one concept
/// under two snapshots are two keys, and so are two classifications under two
/// versions of one goal.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClassificationKey {
    snapshot_id: String,
    goal: GoalScope,
    concept: String,
}

impl ClassificationKey {
    /// Binds a concept to a snapshot and a goal version.
    pub(crate) fn seal(snapshot_id: String, goal: GoalScope, concept: String) -> Self {
        Self {
            snapshot_id,
            goal,
            concept,
        }
    }

    /// Which snapshot the classification was decided over.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.snapshot_id
    }

    /// Which goal version it was decided under.
    #[must_use]
    pub const fn goal(&self) -> &GoalScope {
        &self.goal
    }

    /// Which concept it is about.
    #[must_use]
    pub fn concept(&self) -> &str {
        &self.concept
    }
}
