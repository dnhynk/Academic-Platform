//! Section 25.6's course detail: six blocks, four disjoint coverage tabs.
//!
//! ```text
//! Official identity
//! course code · revision · credits · category · source · valid dates
//!
//! Offerings
//! semester · section · instructor · schedule · capacity · syllabus · status
//!
//! Coverage
//! DESIGNED / TAUGHT / PRACTICED / ASSESSED (겹치지 않는 탭)
//!
//! My record
//! attempts · grade · notes · questions · actual evidence
//!
//! Connections
//! prerequisites · follow-on courses · projects · competencies · roles
//!
//! Reviews
//! offering/instructor/semester scoped · raw provenance · bias indicators
//! ```
//!
//! > Course catalog 정보와 특정 Offering review를 같은 속성처럼 보이지 않게 한다.
//!
//! # The tabs partition, and the partition comes from section 7.2
//!
//! Each tab is one §7.2 predicate — `DESIGNED_TO_TEACH`, `TAUGHT_IN`,
//! `PRACTICED_IN`, `ASSESSED_IN` — and [`CoverageTab::of`] is the inverse. The
//! four predicates are four distinct arms of
//! `academic_domain::predicates::PredicateName`, so one entry is on exactly one
//! tab by construction rather than by a rule somebody remembered to apply.
//! `coverage_tabs_are_non_overlapping` measures the partition itself: the union
//! of the four tabs is the whole report, every pairwise intersection is empty,
//! and the same concept appearing under three predicates appears once on each
//! of three tabs and not at all on the fourth.
//!
//! # Catalog and review are two blocks, and the line is `P2-U8`'s
//!
//! [`CatalogIdentity`] reads `P2-U1`'s own `Course` and `CourseRevision` and
//! holds no review type: no `ReviewScope`, no `ReviewDimension`, no
//! `DimensionBand`, no aggregate. [`ReviewSection`] holds a `P2-U8`
//! `ReviewScope` and a `P2-U8` `OfferingAggregate` and holds **no `CourseId`**,
//! because `ReviewScope` has none — section 34's own failure row is *Course와
//! Offering 혼동 — catalog row에 교수·학기 속성을 덮어씀*.
//!
//! `P2-U8` drew the same line one crate over with
//! `scalar_is_not_a_course_property`: a course reading is a distribution and
//! there is no value it reduces to. Nothing here reduces one either — this
//! module declares no conversion from a band to a number and no accessor that
//! returns one — and `catalog_and_review_are_separate_sections` checks both
//! directions against the whole field list of each type rather than against a
//! list of spellings.

use academic_curriculum::{
    Course, CourseCode, CourseOffering, CourseRevision, Credits, CurriculumCategory,
    InstructorName, SectionCode, TermCode,
};
use academic_domain::{
    ContentDigest, CourseId, CourseRevisionId, EntityId, OfferingId, ValidInterval,
    predicates::PredicateName,
};
use academic_review::{BiasDisclosure, OfferingAggregate, ReviewScope};

use crate::{DashboardError, TimelineEntry};

/// One of section 25.6's six blocks.
///
/// The block a value belongs to is a property of its type, not a label a caller
/// chooses: [`CourseDetail`] has one field per block and
/// [`CourseSection::ALL`] is the order they are shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CourseSection {
    /// `Official identity`.
    OfficialIdentity,
    /// `Offerings`.
    Offerings,
    /// `Coverage`.
    Coverage,
    /// `My record`.
    MyRecord,
    /// `Connections`.
    Connections,
    /// `Reviews`.
    Reviews,
}

impl CourseSection {
    /// Every block, in section 25.6's own order.
    pub const ALL: [Self; 6] = [
        Self::OfficialIdentity,
        Self::Offerings,
        Self::Coverage,
        Self::MyRecord,
        Self::Connections,
        Self::Reviews,
    ];

    /// The heading section 25.6 spells this block with, verbatim.
    #[must_use]
    pub const fn spec_heading(self) -> &'static str {
        match self {
            Self::OfficialIdentity => "Official identity",
            Self::Offerings => "Offerings",
            Self::Coverage => "Coverage",
            Self::MyRecord => "My record",
            Self::Connections => "Connections",
            Self::Reviews => "Reviews",
        }
    }

    /// The identifier `packages/ui`'s shell half shows this block under.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::OfficialIdentity => "OFFICIAL_IDENTITY",
            Self::Offerings => "OFFERINGS",
            Self::Coverage => "COVERAGE",
            Self::MyRecord => "MY_RECORD",
            Self::Connections => "CONNECTIONS",
            Self::Reviews => "REVIEWS",
        }
    }

    /// Section 25.6's own position for this block, counting from one.
    #[must_use]
    pub const fn position(self) -> usize {
        match self {
            Self::OfficialIdentity => 1,
            Self::Offerings => 2,
            Self::Coverage => 3,
            Self::MyRecord => 4,
            Self::Connections => 5,
            Self::Reviews => 6,
        }
    }
}

/// One of section 25.6's four non-overlapping coverage tabs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CoverageTab {
    /// `DESIGNED` — the curriculum's stated intent.
    Designed,
    /// `TAUGHT` — what a lecture actually explained.
    Taught,
    /// `PRACTICED` — where there was an opportunity to use it.
    Practiced,
    /// `ASSESSED` — what was actually examined.
    Assessed,
}

impl CoverageTab {
    /// Every tab, in section 25.6's own order.
    pub const ALL: [Self; 4] = [
        Self::Designed,
        Self::Taught,
        Self::Practiced,
        Self::Assessed,
    ];

    /// The word section 25.6 spells this tab with.
    #[must_use]
    pub const fn spec_word(self) -> &'static str {
        match self {
            Self::Designed => "DESIGNED",
            Self::Taught => "TAUGHT",
            Self::Practiced => "PRACTICED",
            Self::Assessed => "ASSESSED",
        }
    }

    /// The section 7.2 predicate this tab is the whole of.
    #[must_use]
    pub const fn predicate(self) -> PredicateName {
        match self {
            Self::Designed => PredicateName::DesignedToTeach,
            Self::Taught => PredicateName::TaughtIn,
            Self::Practiced => PredicateName::PracticedIn,
            Self::Assessed => PredicateName::AssessedIn,
        }
    }

    /// The tab one predicate belongs to, or `None` for the other sixteen.
    ///
    /// A total function on the four and undefined on the rest, which is what
    /// makes the tabs a partition: an entry whose predicate is not one of these
    /// is refused by [`CoverageEntry::of`] rather than landing on a default
    /// tab.
    #[must_use]
    pub fn of(predicate: PredicateName) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|tab| tab.predicate() == predicate)
    }
}

/// One piece of coverage evidence.
///
/// The tab is derived from the predicate and is not a field: there is no state
/// in which an entry is on a tab its predicate does not belong to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageEntry {
    predicate: PredicateName,
    subject: EntityId,
    evidence: String,
}

impl CoverageEntry {
    /// Records one coverage relation, refusing a predicate no tab answers for.
    pub fn of(
        predicate: PredicateName,
        subject: EntityId,
        evidence: impl Into<String>,
    ) -> Result<Self, DashboardError> {
        if CoverageTab::of(predicate).is_none() {
            return Err(DashboardError::PredicateIsNotACoverageTab(
                predicate.as_str(),
            ));
        }
        let evidence = evidence.into();
        if evidence.trim().is_empty() {
            return Err(DashboardError::EmptyField("coverage evidence"));
        }
        Ok(Self {
            predicate,
            subject,
            evidence,
        })
    }

    /// The section 7.2 edge this entry is an instance of.
    #[must_use]
    pub const fn predicate(&self) -> PredicateName {
        self.predicate
    }

    /// The concept or competency the entry is about.
    #[must_use]
    pub const fn subject(&self) -> EntityId {
        self.subject
    }

    /// Where the entry was read from.
    #[must_use]
    pub fn evidence(&self) -> &str {
        &self.evidence
    }

    /// The one tab this entry is on.
    ///
    /// Returns a tab rather than an `Option` because [`CoverageEntry::of`] is
    /// the only producer and refuses anything else; the `unwrap_or` below is
    /// unreachable and is written rather than a panic because this crate denies
    /// panicking paths.
    #[must_use]
    pub fn tab(&self) -> CoverageTab {
        CoverageTab::of(self.predicate).unwrap_or(CoverageTab::Designed)
    }
}

/// Section 25.6's `Coverage` block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CoverageReport {
    entries: Vec<CoverageEntry>,
}

impl CoverageReport {
    /// Collects the entries. There is no `push` and no `&mut` accessor.
    #[must_use]
    pub fn over(entries: Vec<CoverageEntry>) -> Self {
        Self { entries }
    }

    /// Every entry, in the order it was collected in.
    #[must_use]
    pub fn entries(&self) -> &[CoverageEntry] {
        &self.entries
    }

    /// The entries on one tab.
    #[must_use]
    pub fn tab(&self, tab: CoverageTab) -> Vec<&CoverageEntry> {
        self.entries
            .iter()
            .filter(|entry| entry.tab() == tab)
            .collect()
    }
}

/// Section 25.6's `Official identity` block.
///
/// Read from `P2-U1`'s own `Course` and `CourseRevision`. It holds no review
/// type of any kind, which is half of what section 25.6's last sentence asks
/// for; the other half is that [`ReviewSection`] holds no course identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogIdentity {
    course: CourseId,
    revision: CourseRevisionId,
    code: CourseCode,
    credits: Credits,
    category: CurriculumCategory,
    source_snapshot: Option<ContentDigest>,
    valid_time: ValidInterval,
}

impl CatalogIdentity {
    /// Reads the catalogue's own record.
    ///
    /// Takes both, and compares them: a revision of a different course is not
    /// this course's identity.
    pub fn of(course: &Course, revision: &CourseRevision) -> Result<Self, DashboardError> {
        if revision.course() != course.id() {
            return Err(DashboardError::EmptyField("course revision identity"));
        }
        Ok(Self {
            course: course.id(),
            revision: revision.id(),
            code: revision.code().clone(),
            credits: revision.credits(),
            category: revision.curriculum_category(),
            source_snapshot: revision.source_snapshot().cloned(),
            valid_time: revision.valid_time(),
        })
    }

    /// The durable course identity.
    #[must_use]
    pub const fn course(&self) -> CourseId {
        self.course
    }

    /// The revision this identity is of.
    #[must_use]
    pub const fn revision(&self) -> CourseRevisionId {
        self.revision
    }

    /// `course code`.
    #[must_use]
    pub const fn code(&self) -> &CourseCode {
        &self.code
    }

    /// `credits`.
    #[must_use]
    pub const fn credits(&self) -> Credits {
        self.credits
    }

    /// `category`.
    #[must_use]
    pub const fn category(&self) -> CurriculumCategory {
        self.category
    }

    /// `source` — the digest of the official document the revision was read from.
    #[must_use]
    pub const fn source_snapshot(&self) -> Option<&ContentDigest> {
        self.source_snapshot.as_ref()
    }

    /// `valid dates`.
    #[must_use]
    pub const fn valid_time(&self) -> ValidInterval {
        self.valid_time
    }
}

/// One row of section 25.6's `Offerings` block.
///
/// Read from `P2-U1`'s own `CourseOffering`. It carries no review reading:
/// section 25.6's last sentence is about exactly this row, which is where a
/// review's instructor or term would look like a catalogue property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferingRow {
    offering: OfferingId,
    term: TermCode,
    section: SectionCode,
    instructors: Vec<InstructorName>,
    meeting_count: usize,
    capacity: Option<u16>,
    has_syllabus: bool,
    status: String,
}

impl OfferingRow {
    /// Reads one published offering.
    #[must_use]
    pub fn of(offering: &CourseOffering) -> Self {
        Self {
            offering: offering.id(),
            term: offering.term().clone(),
            section: offering.section().clone(),
            instructors: offering.instructors().to_vec(),
            meeting_count: offering.meetings().len(),
            capacity: offering.capacity().map(|capacity| capacity.seats()),
            has_syllabus: offering.syllabus_artifact().is_some(),
            status: offering.official_status().as_str().to_owned(),
        }
    }

    /// The offering identity.
    #[must_use]
    pub const fn offering(&self) -> OfferingId {
        self.offering
    }

    /// `semester`.
    #[must_use]
    pub const fn term(&self) -> &TermCode {
        &self.term
    }

    /// `section`.
    #[must_use]
    pub const fn section(&self) -> &SectionCode {
        &self.section
    }

    /// `instructor`.
    #[must_use]
    pub fn instructors(&self) -> &[InstructorName] {
        &self.instructors
    }

    /// `schedule`, as the number of weekly meetings the official reading holds.
    #[must_use]
    pub const fn meeting_count(&self) -> usize {
        self.meeting_count
    }

    /// `capacity`, when the official reading recorded one.
    #[must_use]
    pub const fn capacity(&self) -> Option<u16> {
        self.capacity
    }

    /// `syllabus` — whether one is attached, not its bytes.
    #[must_use]
    pub const fn has_syllabus(&self) -> bool {
        self.has_syllabus
    }

    /// `status`.
    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }
}

/// Section 25.6's `Connections` block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Connections {
    prerequisites: Vec<CourseCode>,
    follow_on: Vec<CourseCode>,
    projects: Vec<EntityId>,
    competencies: Vec<EntityId>,
    roles: Vec<EntityId>,
}

impl Connections {
    /// Records the five kinds of connection section 25.6 lists.
    #[must_use]
    pub fn linking(
        prerequisites: Vec<CourseCode>,
        follow_on: Vec<CourseCode>,
        projects: Vec<EntityId>,
        competencies: Vec<EntityId>,
        roles: Vec<EntityId>,
    ) -> Self {
        Self {
            prerequisites,
            follow_on,
            projects,
            competencies,
            roles,
        }
    }

    /// `prerequisites`.
    #[must_use]
    pub fn prerequisites(&self) -> &[CourseCode] {
        &self.prerequisites
    }

    /// `follow-on courses`.
    #[must_use]
    pub fn follow_on(&self) -> &[CourseCode] {
        &self.follow_on
    }

    /// `projects`.
    #[must_use]
    pub fn projects(&self) -> &[EntityId] {
        &self.projects
    }

    /// `competencies`.
    #[must_use]
    pub fn competencies(&self) -> &[EntityId] {
        &self.competencies
    }

    /// `roles`.
    #[must_use]
    pub fn roles(&self) -> &[EntityId] {
        &self.roles
    }
}

/// One entry of section 25.6's `Reviews` block.
///
/// Scoped, and the scope is `P2-U8`'s own `ReviewScope`, which has no
/// `CourseId` field, no constructor that takes one and no accessor that returns
/// one. The reading is a `P2-U8` `OfferingAggregate`, which is a distribution
/// per dimension; nothing here reduces one to a value, and nothing here
/// promotes one to the course.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewSection {
    scope: ReviewScope,
    aggregate: OfferingAggregate,
}

impl ReviewSection {
    /// Records one offering-scoped review reading.
    #[must_use]
    pub const fn scoped(scope: ReviewScope, aggregate: OfferingAggregate) -> Self {
        Self { scope, aggregate }
    }

    /// `offering/instructor/semester scoped` — section 29.5's four dimensions.
    #[must_use]
    pub const fn scope(&self) -> &ReviewScope {
        &self.scope
    }

    /// The distributions, one per review dimension.
    #[must_use]
    pub const fn aggregate(&self) -> &OfferingAggregate {
        &self.aggregate
    }

    /// `bias indicators` — the disclosure the aggregate travels with.
    #[must_use]
    pub const fn disclosure(&self) -> &BiasDisclosure {
        self.aggregate.disclosure()
    }
}

/// Section 25.6's course detail, one field per block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourseDetail {
    identity: CatalogIdentity,
    offerings: Vec<OfferingRow>,
    coverage: CoverageReport,
    my_record: Vec<TimelineEntry>,
    connections: Connections,
    reviews: Vec<ReviewSection>,
}

impl CourseDetail {
    /// Assembles the six blocks.
    ///
    /// Refuses a detail with no offering row: the `Offerings` block is one of
    /// the six and a course detail that cannot fill it is a frame rather than a
    /// detail.
    pub fn assemble(
        identity: CatalogIdentity,
        offerings: Vec<OfferingRow>,
        coverage: CoverageReport,
        my_record: Vec<TimelineEntry>,
        connections: Connections,
        reviews: Vec<ReviewSection>,
    ) -> Result<Self, DashboardError> {
        if offerings.is_empty() {
            return Err(DashboardError::CourseDetailWithoutAnOffering);
        }
        Ok(Self {
            identity,
            offerings,
            coverage,
            my_record,
            connections,
            reviews,
        })
    }

    /// `Official identity`.
    #[must_use]
    pub const fn identity(&self) -> &CatalogIdentity {
        &self.identity
    }

    /// `Offerings`.
    #[must_use]
    pub fn offerings(&self) -> &[OfferingRow] {
        &self.offerings
    }

    /// `Coverage`.
    #[must_use]
    pub const fn coverage(&self) -> &CoverageReport {
        &self.coverage
    }

    /// `My record`.
    #[must_use]
    pub fn my_record(&self) -> &[TimelineEntry] {
        &self.my_record
    }

    /// `Connections`.
    #[must_use]
    pub const fn connections(&self) -> &Connections {
        &self.connections
    }

    /// `Reviews`.
    #[must_use]
    pub fn reviews(&self) -> &[ReviewSection] {
        &self.reviews
    }

    /// Which block one section identifier names, and how many rows it holds.
    ///
    /// Total over [`CourseSection::ALL`] with no wildcard arm, so a seventh
    /// block stops this crate compiling rather than silently showing nothing.
    #[must_use]
    pub fn rows_in(&self, section: CourseSection) -> usize {
        match section {
            CourseSection::OfficialIdentity => 1,
            CourseSection::Offerings => self.offerings.len(),
            CourseSection::Coverage => self.coverage.entries().len(),
            CourseSection::MyRecord => self.my_record.len(),
            CourseSection::Connections => {
                self.connections.prerequisites().len()
                    + self.connections.follow_on().len()
                    + self.connections.projects().len()
                    + self.connections.competencies().len()
                    + self.connections.roles().len()
            }
            CourseSection::Reviews => self.reviews.len(),
        }
    }
}
