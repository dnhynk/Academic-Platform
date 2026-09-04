//! Section 19's snapshot comparison: two channels, and what a difference is
//! attributed to.
//!
//! ## Two channels, and they are two types
//!
//! Section 19's sentence is `diff는 단순 dependency diff와 semantic finding
//! diff를 나누고`. So [`SnapshotComparison`] has two accessors returning two
//! different types, and no accessor returns their union — a merged list is how
//! the split stops being a split. [`DependencyChange`] is over what a manifest
//! or lock file declares; [`SemanticChange`] is over what the correlation
//! concluded, which is section 17.5's relations and section 17.5's drift.
//!
//! A dependency added and never used therefore appears in the first channel and
//! not in the second, and a use that adds no dependency appears in the second
//! and not in the first. `dependency_diff_and_semantic_diff_are_separate`
//! observes both directions.
//!
//! ## What a difference is attributed to
//!
//! Section 19's last sentence is `analyzer version 변경으로 생긴 차이는
//! `ANALYSIS_CHANGED`로 표시해 code 변화처럼 보이지 않게 한다`. There are two
//! axes a comparison can move along — the snapshot and the analyzer — and the
//! attribution is decided by which of them moved:
//!
//! | snapshot | analyzer | attribution |
//! |---|---|---|
//! | same | different | `ANALYSIS_CHANGED` |
//! | different | same | `CODE_CHANGED` |
//! | different | different | refused: [`CorrelationError::ConfoundedComparison`] |
//! | same | same | refused: [`CorrelationError::NoComparisonAxis`] |
//!
//! Only `ANALYSIS_CHANGED` is section 19's own word; `CODE_CHANGED` is this
//! contract's spelling for its complement, and `docs/contracts/
//! repository-correlation.md` records that.
//!
//! The two refusals are deliberate and are the point of the table. When both
//! axes moved there is no attribution to make, and reporting the difference
//! anyway would put it in one of the two buckets — which is the display section
//! 19 forbids. The way out is to re-run the older snapshot under the newer
//! analyzer and compare along one axis at a time. When neither moved there is
//! no axis at all: two runs of one snapshot under one analyzer that differ
//! differ because of the arguments beside them, and calling that a code change
//! or an analyzer change would be the same wrong display.
//!
//! The cause is carried on every entry as well as on the comparison, because
//! section 19's requirement is about what a reader sees on the row.

use std::collections::{BTreeMap, BTreeSet};

use crate::{Correlation, CorrelationError, EvidenceRelation, drift::DriftKind};

/// Which axis a difference is attributed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ChangeCause {
    /// The snapshot moved and the analyzer did not.
    CodeChanged,
    /// Section 19's `ANALYSIS_CHANGED`: the analyzer moved and the snapshot did
    /// not, so nothing here is a change in the repository.
    AnalysisChanged,
}

impl ChangeCause {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::CodeChanged, Self::AnalysisChanged];

    /// Stable spelling. `ANALYSIS_CHANGED` is section 19's own.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CodeChanged => "CODE_CHANGED",
            Self::AnalysisChanged => "ANALYSIS_CHANGED",
        }
    }
}

/// Which way a declared dependency moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PresenceChange {
    /// Declared in the later run and not the earlier one.
    Added,
    /// Declared in the earlier run and not the later one.
    Removed,
}

impl PresenceChange {
    /// Exhaustive order.
    pub const ALL: [Self; 2] = [Self::Added, Self::Removed];

    /// Stable spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "ADDED",
            Self::Removed => "REMOVED",
        }
    }
}

/// One entry of section 19's `단순 dependency diff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DependencyChange {
    subject: String,
    direction: PresenceChange,
    cause: ChangeCause,
}

impl DependencyChange {
    /// What the manifest or the lock file calls it.
    ///
    /// The name as declared, not a [`Subject`] identifier: this channel's
    /// population is the manifest, so a dependency no `Subject` names appears
    /// here under its own spelling. The semantic channel is the one keyed on
    /// subjects, and the two disagreeing on a name is the split doing its job.
    ///
    /// [`Subject`]: academic_repository_analysis::Subject
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// Added or removed.
    #[must_use]
    pub const fn direction(&self) -> PresenceChange {
        self.direction
    }

    /// What the difference is attributed to.
    #[must_use]
    pub const fn cause(&self) -> ChangeCause {
        self.cause
    }
}

/// What changed about one subject's correlation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SemanticTransition {
    /// The later run holds relations for a subject the earlier one held none
    /// for.
    Appeared,
    /// Section 18.1's `NO_LONGER_OBSERVED`: the earlier run held relations and
    /// the later one holds none. It is not `never used`.
    NoLongerObserved,
    /// Both runs hold relations and the sets differ.
    RelationsChanged,
    /// A drift the earlier run did not have.
    DriftAppeared,
    /// A drift the earlier run had and the later one does not.
    DriftResolved,
}

impl SemanticTransition {
    /// Exhaustive order.
    pub const ALL: [Self; 5] = [
        Self::Appeared,
        Self::NoLongerObserved,
        Self::RelationsChanged,
        Self::DriftAppeared,
        Self::DriftResolved,
    ];

    /// Stable spelling. `NO_LONGER_OBSERVED` is section 18.1's own.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Appeared => "APPEARED",
            Self::NoLongerObserved => "NO_LONGER_OBSERVED",
            Self::RelationsChanged => "RELATIONS_CHANGED",
            Self::DriftAppeared => "DRIFT_APPEARED",
            Self::DriftResolved => "DRIFT_RESOLVED",
        }
    }
}

/// One entry of section 19's `semantic finding diff`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticChange {
    subject: String,
    transition: SemanticTransition,
    cause: ChangeCause,
    before: Vec<EvidenceRelation>,
    after: Vec<EvidenceRelation>,
    drift: Option<DriftKind>,
}

impl SemanticChange {
    /// Which subject.
    #[must_use]
    pub fn subject(&self) -> &str {
        &self.subject
    }

    /// What changed.
    #[must_use]
    pub const fn transition(&self) -> SemanticTransition {
        self.transition
    }

    /// What the difference is attributed to.
    #[must_use]
    pub const fn cause(&self) -> ChangeCause {
        self.cause
    }

    /// The earlier run's relations for the subject.
    #[must_use]
    pub fn before(&self) -> &[EvidenceRelation] {
        &self.before
    }

    /// The later run's relations for the subject.
    #[must_use]
    pub fn after(&self) -> &[EvidenceRelation] {
        &self.after
    }

    /// Which drift, for a drift transition.
    #[must_use]
    pub const fn drift(&self) -> Option<DriftKind> {
        self.drift
    }
}

/// Section 19's comparison of two correlation runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotComparison {
    cause: ChangeCause,
    dependency: Vec<DependencyChange>,
    semantic: Vec<SemanticChange>,
}

impl SnapshotComparison {
    /// What every difference in this comparison is attributed to.
    #[must_use]
    pub const fn cause(&self) -> ChangeCause {
        self.cause
    }

    /// Section 19's `단순 dependency diff` channel.
    #[must_use]
    pub fn dependency_diff(&self) -> &[DependencyChange] {
        &self.dependency
    }

    /// Section 19's `semantic finding diff` channel.
    #[must_use]
    pub fn semantic_diff(&self) -> &[SemanticChange] {
        &self.semantic
    }
}

/// Compares two correlation runs along the one axis that moved.
///
/// # Errors
///
/// [`CorrelationError::ConfoundedComparison`] when both the snapshot and the
/// analyzer differ, and [`CorrelationError::NoComparisonAxis`] when neither
/// does. See the module documentation for why each is a refusal.
pub fn compare(
    before: &Correlation,
    after: &Correlation,
) -> Result<SnapshotComparison, CorrelationError> {
    let snapshot_moved = before.snapshot_id() != after.snapshot_id();
    let analyzer_moved = before.analyzer_version() != after.analyzer_version()
        || before.analyzer_tool() != after.analyzer_tool();
    let cause = match (snapshot_moved, analyzer_moved) {
        (true, false) => ChangeCause::CodeChanged,
        (false, true) => ChangeCause::AnalysisChanged,
        (true, true) => {
            return Err(CorrelationError::ConfoundedComparison(
                before.snapshot_id().to_owned(),
                after.snapshot_id().to_owned(),
            ));
        }
        (false, false) => {
            return Err(CorrelationError::NoComparisonAxis(
                before.snapshot_id().to_owned(),
            ));
        }
    };

    Ok(SnapshotComparison {
        cause,
        dependency: dependency_diff(before, after, cause),
        semantic: semantic_diff(before, after, cause),
    })
}

/// Section 19's first channel, over declared dependencies alone.
fn dependency_diff(
    before: &Correlation,
    after: &Correlation,
    cause: ChangeCause,
) -> Vec<DependencyChange> {
    let earlier = before.declared_dependencies();
    let later = after.declared_dependencies();
    let mut changes = Vec::new();
    for subject in later.difference(earlier) {
        changes.push(DependencyChange {
            subject: subject.clone(),
            direction: PresenceChange::Added,
            cause,
        });
    }
    for subject in earlier.difference(later) {
        changes.push(DependencyChange {
            subject: subject.clone(),
            direction: PresenceChange::Removed,
            cause,
        });
    }
    changes.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then_with(|| left.direction.cmp(&right.direction))
    });
    changes
}

/// Section 19's second channel, over relations and drift alone.
fn semantic_diff(
    before: &Correlation,
    after: &Correlation,
    cause: ChangeCause,
) -> Vec<SemanticChange> {
    let earlier = relations_by_subject(before);
    let later = relations_by_subject(after);
    let subjects: BTreeSet<&String> = earlier.keys().chain(later.keys()).collect();

    let mut changes = Vec::new();
    for subject in subjects {
        let empty = BTreeSet::new();
        let was = earlier.get(subject).unwrap_or(&empty);
        let now = later.get(subject).unwrap_or(&empty);
        if was == now {
            continue;
        }
        let transition = if was.is_empty() {
            SemanticTransition::Appeared
        } else if now.is_empty() {
            SemanticTransition::NoLongerObserved
        } else {
            SemanticTransition::RelationsChanged
        };
        changes.push(SemanticChange {
            subject: subject.clone(),
            transition,
            cause,
            before: was.iter().copied().collect(),
            after: now.iter().copied().collect(),
            drift: None,
        });
    }

    let earlier_drift = drifts_by_subject(before);
    let later_drift = drifts_by_subject(after);
    for (key, kind) in &later_drift {
        if !earlier_drift.contains_key(key) {
            changes.push(drift_change(
                key.0.clone(),
                SemanticTransition::DriftAppeared,
                cause,
                *kind,
            ));
        }
    }
    for (key, kind) in &earlier_drift {
        if !later_drift.contains_key(key) {
            changes.push(drift_change(
                key.0.clone(),
                SemanticTransition::DriftResolved,
                cause,
                *kind,
            ));
        }
    }

    changes.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then_with(|| left.transition.cmp(&right.transition))
    });
    changes
}

fn drift_change(
    subject: String,
    transition: SemanticTransition,
    cause: ChangeCause,
    kind: DriftKind,
) -> SemanticChange {
    SemanticChange {
        subject,
        transition,
        cause,
        before: Vec::new(),
        after: Vec::new(),
        drift: Some(kind),
    }
}

fn relations_by_subject(correlation: &Correlation) -> BTreeMap<String, BTreeSet<EvidenceRelation>> {
    let mut found: BTreeMap<String, BTreeSet<EvidenceRelation>> = BTreeMap::new();
    for edge in correlation.relations() {
        found
            .entry(edge.subject().to_owned())
            .or_default()
            .insert(edge.relation());
    }
    found
}

fn drifts_by_subject(correlation: &Correlation) -> BTreeMap<(String, DriftKind), DriftKind> {
    correlation
        .drifts()
        .iter()
        .map(|drift| ((drift.subject().to_owned(), drift.kind()), drift.kind()))
        .collect()
}
