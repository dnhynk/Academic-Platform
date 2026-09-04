//! The dry run: what a deletion would reach, one node per derivative class,
//! always.
//!
//! # The enumeration is the specification's, read out of it
//!
//! Section 32.10's first bullet is the whole list:
//!
//! ```text
//! artifact 삭제 요청은 파생 transcript, embedding, graph claim, PDF, cache,
//! sync replica, backup expiry까지 dependency plan을 보여준다.
//! ```
//!
//! Seven items. [`SPEC_DERIVATIVE_WORDS`] pairs each of `P2-K5`'s
//! `DerivativeClass` variants with the phrase that bullet uses for it, and
//! `the_derivative_classes_are_section_32_10s_own` parses the bullet, splits it,
//! and compares the two sets **in both directions** — so a class with no
//! sentence fails, a sentence with no class fails, and a document that stops
//! saying it fails. Two of the seven are spelled differently there than in
//! t068 section 5 and in `P2-K5`: the design document writes `PDF` where they
//! write `document`, and `sync replica` where they write `replica`. That is
//! recorded rather than silently normalised, because a reader who greps for
//! `DOCUMENT` in section 32.10 finds nothing and needs to know why.
//!
//! # A node per class, and the proof is not a list
//!
//! [`DeletionDryRun::of`] walks `academic_retention::DERIVATIVE_CLASSES` and
//! there is no other constructor. `dry_run_enumerates_every_derivative_class`
//! does not assert against a hand-written list of seven names: it drives a
//! resolver through **every** assignment of the three `ClassResolution` shapes
//! to the seven classes — all 2187 of them — and requires the node list to be
//! the registry, in registry order, every time. A build that dropped an empty
//! class, or reordered on one resolver answer and not another, fails on the
//! case that reaches it rather than on a name somebody remembered to list.

use academic_retention::{
    ClassResolution, DERIVATIVE_CLASSES, DerivativeClass, DerivativeResolver, RetentionSubject,
};

use crate::{
    protection::{ProtectionDecision, ProtectionRegistry},
    target::DeletionTarget,
};

/// The phrase section 32.10 uses for each derivative class.
///
/// Order is `academic_retention::DERIVATIVE_CLASSES`'s. The right-hand side is
/// the design document's spelling, which is not always t068's: see the module
/// documentation.
pub const SPEC_DERIVATIVE_WORDS: [(DerivativeClass, &str); 7] = [
    (DerivativeClass::Transcript, "transcript"),
    (DerivativeClass::Embedding, "embedding"),
    (DerivativeClass::GraphClaim, "graph claim"),
    (DerivativeClass::Document, "PDF"),
    (DerivativeClass::Cache, "cache"),
    (DerivativeClass::Replica, "sync replica"),
    (DerivativeClass::BackupExpiry, "backup expiry"),
];

/// The clause the enumeration is read out of, up to its list.
///
/// Held here so a scan looks for the specification's own sentence rather than
/// for a heading number that could move.
pub const SPEC_DERIVATIVE_SENTENCE_HEAD: &str = "artifact 삭제 요청은 파생 ";

/// The clause that closes it.
pub const SPEC_DERIVATIVE_SENTENCE_TAIL: &str = "까지 dependency plan을 보여준다.";

/// One class's line in a dry run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DryRunNode {
    class: DerivativeClass,
    resolution: ClassResolution,
    targets: Vec<DeletionTarget>,
}

impl DryRunNode {
    /// Which class.
    #[must_use]
    pub const fn class(&self) -> DerivativeClass {
        self.class
    }

    /// What the resolver said, in `P2-K5`'s own vocabulary.
    #[must_use]
    pub const fn resolution(&self) -> &ClassResolution {
        &self.resolution
    }

    /// The artifacts this class contributes, each named by artifact and locator.
    #[must_use]
    pub fn targets(&self) -> &[DeletionTarget] {
        &self.targets
    }

    /// Whether the resolver could not answer for this class (`RB03`).
    #[must_use]
    pub const fn is_unresolved(&self) -> bool {
        matches!(self.resolution, ClassResolution::Unresolved { .. })
    }
}

/// What a [`DerivativeIndex`] can say about one class.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClassTargets {
    /// These exact artifacts, each at the locator it is reachable under.
    Targets(Vec<DeletionTarget>),
    /// The class holds nothing here, for a stated reason.
    NothingToDelete {
        /// Why the class is empty for this subject.
        reason: String,
    },
    /// The class could not be answered for (`RB03`).
    Unresolved {
        /// Why.
        reason: String,
    },
}

/// Resolves one derivative class into the artifacts it holds.
///
/// This is `academic_retention::DerivativeResolver` with the identity fixed:
/// that trait answers in locators, and `P3-G10` of the rotation contract
/// records that a locator does not name an artifact. An implementation here
/// answers in [`DeletionTarget`]s, and [`DeletionDryRun::of`] projects them
/// down to the locators `P2-K5`'s plan takes.
pub trait DerivativeIndex {
    /// Every artifact of `class` derived from `subject`, or why the class could
    /// not be answered for.
    fn resolve(&self, class: DerivativeClass, subject: &DeletionTarget) -> ClassTargets;
}

/// What a deletion would reach, before anything is shown to anyone.
///
/// One node per class, in registry order, always.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletionDryRun {
    subject: DeletionTarget,
    protection: ProtectionDecision,
    nodes: Vec<DryRunNode>,
}

impl DeletionDryRun {
    /// Walks every derivative class for one subject.
    ///
    /// A protected subject is still walked. The refusal is what the caller
    /// reads and it carries the policy reason; what it does not do is hide
    /// which derivatives exist, because a user told "this cannot be deleted"
    /// and shown nothing has been told less than the previous screen showed.
    pub fn of<I, P>(subject: DeletionTarget, index: &I, protection: &P) -> Self
    where
        I: DerivativeIndex + ?Sized,
        P: ProtectionRegistry + ?Sized,
    {
        let nodes = DERIVATIVE_CLASSES
            .iter()
            .map(|class| {
                let (resolution, targets) = match index.resolve(*class, &subject) {
                    ClassTargets::Targets(targets) => (
                        ClassResolution::Locators(
                            targets.iter().map(|target| *target.locator()).collect(),
                        ),
                        targets,
                    ),
                    ClassTargets::NothingToDelete { reason } => {
                        (ClassResolution::NothingToDelete { reason }, Vec::new())
                    }
                    ClassTargets::Unresolved { reason } => {
                        (ClassResolution::Unresolved { reason }, Vec::new())
                    }
                };
                DryRunNode {
                    class: *class,
                    resolution,
                    targets,
                }
            })
            .collect();
        Self {
            subject,
            protection: protection.decide(&subject),
            nodes,
        }
    }

    /// The artifact this dry run is about.
    #[must_use]
    pub const fn subject(&self) -> &DeletionTarget {
        &self.subject
    }

    /// Whether a policy refuses this deletion, and which.
    #[must_use]
    pub const fn protection(&self) -> &ProtectionDecision {
        &self.protection
    }

    /// One node per class, in registry order.
    #[must_use]
    pub fn nodes(&self) -> &[DryRunNode] {
        &self.nodes
    }

    /// Every class the dry run enumerated, in registry order.
    #[must_use]
    pub fn enumerated_classes(&self) -> Vec<DerivativeClass> {
        self.nodes.iter().map(DryRunNode::class).collect()
    }

    /// Every artifact this deletion would reach, in class order.
    ///
    /// The subject is first, because it is what the user asked to delete and a
    /// preview that listed only the derivatives would omit the one thing the
    /// user named.
    #[must_use]
    pub fn reached(&self) -> Vec<DeletionTarget> {
        let mut reached = vec![self.subject];
        for node in &self.nodes {
            reached.extend(node.targets.iter().copied());
        }
        reached
    }

    /// `P2-K5`'s plan over the same nodes.
    ///
    /// The plan is built from this dry run rather than beside it, so the class
    /// answers a user is shown and the class answers an executor runs are one
    /// list. `academic_retention::DeletionPlan::build` asks a resolver; the
    /// resolver it is given here is this dry run, replaying what it already
    /// asked.
    #[must_use]
    pub fn plan(&self) -> academic_retention::DeletionPlan {
        academic_retention::DeletionPlan::build(
            RetentionSubject::whole_object(*self.subject.locator()),
            self,
        )
    }

    /// The nodes the resolver could not answer for (`RB03`), in registry order.
    #[must_use]
    pub fn unresolved_classes(&self) -> Vec<DerivativeClass> {
        self.nodes
            .iter()
            .filter(|node| node.is_unresolved())
            .map(DryRunNode::class)
            .collect()
    }
}

impl DerivativeResolver for DeletionDryRun {
    fn resolve(&self, class: DerivativeClass, _subject: &RetentionSubject) -> ClassResolution {
        self.nodes
            .iter()
            .find(|node| node.class == class)
            .map_or_else(
                || ClassResolution::Unresolved {
                    reason: format!("the dry run holds no node for {}", class.as_str()),
                },
                |node| node.resolution.clone(),
            )
    }
}
