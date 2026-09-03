//! Publishing a curriculum, and the ledger a publication appends to.
//!
//! # A publication is founded on a dated official source, by construction
//!
//! [`CurriculumPublication::from_official_source`] takes an
//! `academic_ingestion::PublishedRules`. That is what `P2-U6`'s stage nine
//! produces, its fields are private, and its only producer is that crate's
//! `publish`, which takes a `PublishableRules` that `Reconciled::publishable`
//! returns `None` for on an undated document. So a curriculum version founded
//! on an `UNSCOPED_OFFICIAL_SOURCE` is not a value that exists — not because a
//! check here refuses it, but because there is nothing to call this with.
//! `tests/compile_fail/a_curriculum_cannot_be_published_from_an_unscoped_source.rs`
//! observes the second half: the reconciled state does not coerce into the
//! published one.
//!
//! # What atomic means here
//!
//! [`CurriculumPublisher::publish`] appends into a live [`CurriculumLedger`],
//! one aggregate at a time, so a partial publication is a state this code can
//! physically reach. What makes it unreachable is that every append is taken
//! from a recorded mark and rewound as one on any error, including an injected
//! one. The evidence is not "no partial rows were observed": it is that the
//! ledger after a failed publication is *the same value* as before it, compared
//! whole.
//!
//! Validation runs before the first append, so a publication that names a
//! parent it does not carry fails with nothing written. The injected faults are
//! what cover the other half — a failure that arrives after writing has already
//! started.

use std::collections::BTreeMap;

use academic_domain::{
    CourseId, CourseRevisionId, CurriculumVersionId, OfferingId, TimestampMillis,
};
use academic_ingestion::{
    ConnectorId, EffectiveDate, ParserVersion, PublishedRules, RetrievalInstant,
};

use crate::{
    course::Course,
    error::CurriculumError,
    fault::{NoFault, PublishCheckpoint, PublishFaultInjector},
    offering::CourseOffering,
    relation::{
        CourseRelations, EquivalenceRelation, IdentityDecision, ReplacementRelation,
        RetirementRelation,
    },
    revision::CourseRevision,
    version::CurriculumVersion,
};

/// Where a publication's authority comes from: one completed ingestion run.
///
/// Carries identifiers, dates and a parser version. No document text: section
/// 29.1's bytes stay behind `academic_ingestion::RawSnapshot`'s sealed route
/// and nothing here can reach them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficialSourceBinding {
    connector: ConnectorId,
    effective: EffectiveDate,
    retrieved_at: RetrievalInstant,
    parser_version: ParserVersion,
}

impl OfficialSourceBinding {
    /// Which connector's document.
    #[must_use]
    pub const fn connector(&self) -> &ConnectorId {
        &self.connector
    }

    /// When the source's rules start to apply.
    #[must_use]
    pub const fn effective(&self) -> EffectiveDate {
        self.effective
    }

    /// When the source was retrieved.
    #[must_use]
    pub const fn retrieved_at(&self) -> RetrievalInstant {
        self.retrieved_at
    }

    /// Which parser read it.
    #[must_use]
    pub const fn parser_version(&self) -> ParserVersion {
        self.parser_version
    }
}

/// One curriculum version and everything published under it, staged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurriculumPublication {
    source: OfficialSourceBinding,
    version: CurriculumVersion,
    courses: Vec<Course>,
    revisions: Vec<CourseRevision>,
    offerings: Vec<CourseOffering>,
    identities: Vec<IdentityDecision>,
    equivalences: Vec<EquivalenceRelation>,
    replacements: Vec<ReplacementRelation>,
    retirements: Vec<RetirementRelation>,
}

impl CurriculumPublication {
    /// The only constructor, and its argument is the ingestion run's output.
    #[must_use]
    pub fn from_official_source(published: &PublishedRules, version: CurriculumVersion) -> Self {
        Self {
            source: OfficialSourceBinding {
                connector: published.connector().clone(),
                effective: published.effective(),
                retrieved_at: published.retrieved_at(),
                parser_version: published.parser_version(),
            },
            version,
            courses: Vec::new(),
            revisions: Vec::new(),
            offerings: Vec::new(),
            identities: Vec::new(),
            equivalences: Vec::new(),
            replacements: Vec::new(),
            retirements: Vec::new(),
        }
    }

    /// Stages one course.
    #[must_use]
    pub fn with_course(mut self, course: Course) -> Self {
        self.courses.push(course);
        self
    }

    /// Stages one revision.
    #[must_use]
    pub fn with_revision(mut self, revision: CourseRevision) -> Self {
        self.revisions.push(revision);
        self
    }

    /// Stages one offering.
    #[must_use]
    pub fn with_offering(mut self, offering: CourseOffering) -> Self {
        self.offerings.push(offering);
        self
    }

    /// Stages one identity decision.
    #[must_use]
    pub fn with_identity(mut self, decision: IdentityDecision) -> Self {
        self.identities.push(decision);
        self
    }

    /// Stages one equivalence.
    #[must_use]
    pub fn with_equivalence(mut self, relation: EquivalenceRelation) -> Self {
        self.equivalences.push(relation);
        self
    }

    /// Stages one replacement.
    #[must_use]
    pub fn with_replacement(mut self, relation: ReplacementRelation) -> Self {
        self.replacements.push(relation);
        self
    }

    /// Stages one retirement.
    #[must_use]
    pub fn with_retirement(mut self, relation: RetirementRelation) -> Self {
        self.retirements.push(relation);
        self
    }

    /// Where this publication's authority comes from.
    #[must_use]
    pub const fn source(&self) -> &OfficialSourceBinding {
        &self.source
    }

    /// The version being published.
    #[must_use]
    pub const fn version(&self) -> &CurriculumVersion {
        &self.version
    }
}

/// What one completed publication put into the ledger.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishReceipt {
    version: CurriculumVersionId,
    courses: Vec<CourseId>,
    revisions: Vec<CourseRevisionId>,
    offerings: Vec<OfferingId>,
}

impl PublishReceipt {
    /// The version that was published.
    #[must_use]
    pub const fn version(&self) -> CurriculumVersionId {
        self.version
    }

    /// The courses that were published.
    #[must_use]
    pub fn courses(&self) -> &[CourseId] {
        &self.courses
    }

    /// The revisions that were published.
    #[must_use]
    pub fn revisions(&self) -> &[CourseRevisionId] {
        &self.revisions
    }

    /// The offerings that were published.
    #[must_use]
    pub fn offerings(&self) -> &[OfferingId] {
        &self.offerings
    }
}

/// Everything published so far, and the reads over it.
///
/// Each read answers from one aggregate map. There is no read here that merges
/// two of them into one value, which is what
/// `one_course_two_revisions_three_offerings_do_not_leak` walks exhaustively.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CurriculumLedger {
    versions: Vec<CurriculumVersion>,
    courses: Vec<Course>,
    revisions: Vec<CourseRevision>,
    offerings: Vec<CourseOffering>,
    relations: CourseRelations,
    sources: Vec<OfficialSourceBinding>,
}

/// The lengths every append is taken from, so a failed publication rewinds as
/// one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LedgerMark {
    versions: usize,
    courses: usize,
    revisions: usize,
    offerings: usize,
    identities: usize,
    equivalences: usize,
    replacements: usize,
    retirements: usize,
    sources: usize,
}

impl CurriculumLedger {
    /// An empty ledger.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            versions: Vec::new(),
            courses: Vec::new(),
            revisions: Vec::new(),
            offerings: Vec::new(),
            relations: CourseRelations::new(),
            sources: Vec::new(),
        }
    }

    /// Every published curriculum version.
    #[must_use]
    pub fn versions(&self) -> &[CurriculumVersion] {
        &self.versions
    }

    /// Every published course.
    #[must_use]
    pub fn courses(&self) -> &[Course] {
        &self.courses
    }

    /// Every published revision.
    #[must_use]
    pub fn revisions(&self) -> &[CourseRevision] {
        &self.revisions
    }

    /// Every published offering.
    #[must_use]
    pub fn offerings(&self) -> &[CourseOffering] {
        &self.offerings
    }

    /// The four recorded relation sets.
    #[must_use]
    pub const fn relations(&self) -> &CourseRelations {
        &self.relations
    }

    /// Every official source a publication was founded on.
    #[must_use]
    pub fn sources(&self) -> &[OfficialSourceBinding] {
        &self.sources
    }

    /// One curriculum version by identity.
    #[must_use]
    pub fn version(&self, id: CurriculumVersionId) -> Option<&CurriculumVersion> {
        self.versions.iter().find(|value| value.id() == id)
    }

    /// One course by identity.
    #[must_use]
    pub fn course(&self, id: CourseId) -> Option<&Course> {
        self.courses.iter().find(|value| value.id() == id)
    }

    /// One revision by identity.
    #[must_use]
    pub fn revision(&self, id: CourseRevisionId) -> Option<&CourseRevision> {
        self.revisions.iter().find(|value| value.id() == id)
    }

    /// One offering by identity.
    #[must_use]
    pub fn offering(&self, id: OfferingId) -> Option<&CourseOffering> {
        self.offerings.iter().find(|value| value.id() == id)
    }

    /// The revisions of one course, and no other course's.
    #[must_use]
    pub fn revisions_of(&self, course: CourseId) -> Vec<&CourseRevision> {
        self.revisions
            .iter()
            .filter(|value| value.course() == course)
            .collect()
    }

    /// The offerings of one revision, and no other revision's.
    #[must_use]
    pub fn offerings_of(&self, revision: CourseRevisionId) -> Vec<&CourseOffering> {
        self.offerings
            .iter()
            .filter(|value| value.course_revision() == revision)
            .collect()
    }

    /// The offerings that ran against any revision of one course.
    ///
    /// Two hops, each through its own map, and the intermediate revision set is
    /// the one [`Self::revisions_of`] returns. A course with no revision has no
    /// offering here even if an offering names a revision of another course.
    #[must_use]
    pub fn offerings_for_course(&self, course: CourseId) -> Vec<&CourseOffering> {
        let revisions: Vec<CourseRevisionId> = self
            .revisions_of(course)
            .into_iter()
            .map(CourseRevision::id)
            .collect();
        self.offerings
            .iter()
            .filter(|value| revisions.contains(&value.course_revision()))
            .collect()
    }

    fn mark(&self) -> LedgerMark {
        LedgerMark {
            versions: self.versions.len(),
            courses: self.courses.len(),
            revisions: self.revisions.len(),
            offerings: self.offerings.len(),
            identities: self.relations.identities().len(),
            equivalences: self.relations.equivalences().len(),
            replacements: self.relations.replacements().len(),
            retirements: self.relations.retirements().len(),
            sources: self.sources.len(),
        }
    }

    fn rewind_to(&mut self, mark: LedgerMark) {
        self.versions.truncate(mark.versions);
        self.courses.truncate(mark.courses);
        self.revisions.truncate(mark.revisions);
        self.offerings.truncate(mark.offerings);
        self.sources.truncate(mark.sources);
        self.relations.truncate_to(
            mark.identities,
            mark.equivalences,
            mark.replacements,
            mark.retirements,
        );
    }
}

/// The publisher. Holds the injector; owns no state of its own.
#[derive(Debug)]
pub struct CurriculumPublisher<'injector> {
    faults: &'injector dyn PublishFaultInjector,
}

impl Default for CurriculumPublisher<'static> {
    fn default() -> Self {
        Self::new()
    }
}

impl CurriculumPublisher<'static> {
    /// A publisher whose every checkpoint is a no-op.
    #[must_use]
    pub const fn new() -> Self {
        Self { faults: &NoFault }
    }
}

impl<'injector> CurriculumPublisher<'injector> {
    /// A publisher an explicitly supplied harness may fail.
    #[must_use]
    pub const fn with_faults(faults: &'injector dyn PublishFaultInjector) -> Self {
        Self { faults }
    }

    /// Publishes one version and everything under it, or changes nothing.
    ///
    /// # Errors
    ///
    /// [`CurriculumError::Dangling`] when a staged child names a parent the
    /// publication does not carry, [`CurriculumError::Duplicate`] when one
    /// identity is staged twice or is already in the ledger, and whatever the
    /// injector returns. In every case the ledger is left equal to the value it
    /// had on entry.
    pub fn publish(
        &self,
        ledger: &mut CurriculumLedger,
        publication: CurriculumPublication,
    ) -> Result<PublishReceipt, CurriculumError> {
        let mark = ledger.mark();
        match self.append(ledger, publication) {
            Ok(receipt) => Ok(receipt),
            Err(failure) => {
                ledger.rewind_to(mark);
                Err(failure)
            }
        }
    }

    fn append(
        &self,
        ledger: &mut CurriculumLedger,
        publication: CurriculumPublication,
    ) -> Result<PublishReceipt, CurriculumError> {
        validate(ledger, &publication)?;
        self.faults.hit(PublishCheckpoint::BeforeAnything)?;

        let version = publication.version.id();
        ledger.sources.push(publication.source);
        ledger.versions.push(publication.version);
        self.faults.hit(PublishCheckpoint::AfterCurriculumVersion)?;

        let mut courses = Vec::new();
        for course in publication.courses {
            courses.push(course.id());
            ledger.courses.push(course);
            self.faults.hit(PublishCheckpoint::AfterCourse)?;
        }

        let mut revisions = Vec::new();
        for revision in publication.revisions {
            revisions.push(revision.id());
            ledger.revisions.push(revision);
            self.faults.hit(PublishCheckpoint::AfterRevision)?;
        }

        let mut offerings = Vec::new();
        for offering in publication.offerings {
            offerings.push(offering.id());
            ledger.offerings.push(offering);
            self.faults.hit(PublishCheckpoint::AfterOffering)?;
        }

        for decision in publication.identities {
            ledger.relations.record_identity(decision);
            self.faults.hit(PublishCheckpoint::AfterRelation)?;
        }
        for relation in publication.equivalences {
            ledger.relations.record_equivalence(relation);
            self.faults.hit(PublishCheckpoint::AfterRelation)?;
        }
        for relation in publication.replacements {
            ledger.relations.record_replacement(relation);
            self.faults.hit(PublishCheckpoint::AfterRelation)?;
        }
        for relation in publication.retirements {
            ledger.relations.record_retirement(relation);
            self.faults.hit(PublishCheckpoint::AfterRelation)?;
        }

        self.faults.hit(PublishCheckpoint::BeforeReceipt)?;
        Ok(PublishReceipt {
            version,
            courses,
            revisions,
            offerings,
        })
    }
}

/// Refuses a publication whose parents are not present, before anything is
/// appended.
fn validate(
    ledger: &CurriculumLedger,
    publication: &CurriculumPublication,
) -> Result<(), CurriculumError> {
    if ledger.version(publication.version.id()).is_some() {
        return Err(CurriculumError::Duplicate {
            aggregate: "curriculum version",
        });
    }

    let mut courses: BTreeMap<CourseId, ()> = BTreeMap::new();
    for course in &publication.courses {
        if ledger.course(course.id()).is_some() || courses.insert(course.id(), ()).is_some() {
            return Err(CurriculumError::Duplicate {
                aggregate: "course",
            });
        }
    }

    let mut revisions: BTreeMap<CourseRevisionId, ()> = BTreeMap::new();
    for revision in &publication.revisions {
        if ledger.revision(revision.id()).is_some() || revisions.insert(revision.id(), ()).is_some()
        {
            return Err(CurriculumError::Duplicate {
                aggregate: "course revision",
            });
        }
        if revision.curriculum_version() != publication.version.id() {
            return Err(CurriculumError::Dangling {
                child: "course revision",
                parent: "curriculum version",
            });
        }
        if !courses.contains_key(&revision.course()) && ledger.course(revision.course()).is_none() {
            return Err(CurriculumError::Dangling {
                child: "course revision",
                parent: "course",
            });
        }
    }

    let mut offerings: BTreeMap<OfferingId, ()> = BTreeMap::new();
    for offering in &publication.offerings {
        if ledger.offering(offering.id()).is_some() || offerings.insert(offering.id(), ()).is_some()
        {
            return Err(CurriculumError::Duplicate {
                aggregate: "offering",
            });
        }
        if !revisions.contains_key(&offering.course_revision())
            && ledger.revision(offering.course_revision()).is_none()
        {
            return Err(CurriculumError::Dangling {
                child: "offering",
                parent: "course revision",
            });
        }
    }

    Ok(())
}

/// Reads one relation question at one instant, as the ledger holds them.
///
/// A convenience over [`CurriculumLedger::relations`] that changes nothing
/// about which set each question reads.
impl CurriculumLedger {
    /// See [`CourseRelations::same_course`].
    #[must_use]
    pub fn same_course(
        &self,
        earlier: CourseId,
        later: CourseId,
        instant: TimestampMillis,
    ) -> crate::relation::CourseCodeReuse {
        self.relations.same_course(earlier, later, instant)
    }

    /// See [`CourseRelations::equivalent`].
    #[must_use]
    pub fn equivalent(&self, source: CourseId, target: CourseId, instant: TimestampMillis) -> bool {
        self.relations.equivalent(source, target, instant)
    }

    /// See [`CourseRelations::replacements_for`].
    #[must_use]
    pub fn replacements_for(
        &self,
        retired: CourseId,
        instant: TimestampMillis,
    ) -> std::collections::BTreeSet<CourseId> {
        self.relations.replacements_for(retired, instant)
    }

    /// See [`CourseRelations::retired`].
    #[must_use]
    pub fn retired(&self, course: CourseId, instant: TimestampMillis) -> bool {
        self.relations.retired(course, instant)
    }
}
