//! A curriculum version is founded on a dated official source, by construction.
//!
//! Section 29.2: *a document whose effective date cannot be found is
//! `UNSCOPED_OFFICIAL_SOURCE` and is not automatically published as a rule.*
//! `P2-U6` executes that by giving `PublishableRules` no public constructor and
//! making `Reconciled::publishable` — which returns `None` for
//! `Dating::Unscoped` — its only producer.
//!
//! `CurriculumPublication::from_official_source` takes the `PublishedRules`
//! that comes out the far side of that pipeline. It takes no digest, no
//! connector identifier and no string, so there is no argument a caller can
//! assemble in place of a completed run.

use academic_curriculum::{
    AdmissionCohort, CurriculumPublication, CurriculumVersionDraft,
};
use academic_domain::{CurriculumVersionId, TimestampMillis, ValidInterval};
use academic_ingestion::{ConnectorId, PublishedRules};

fn main() {
    let id: CurriculumVersionId = "01900000-0000-7000-8000-000000000001".parse().unwrap();
    let cohort = AdmissionCohort::parse("2026").unwrap();
    let version = CurriculumVersionDraft::new(
        id,
        (cohort.clone(), cohort),
        ValidInterval::open_ended(TimestampMillis::new(0)),
    )
    .institution_segment("SNU")
    .build()
    .unwrap();

    // There is no struct literal for the argument: its fields are private to
    // `academic-ingestion`.
    let forged = PublishedRules {
        connector: ConnectorId::new("snu.cse.official").unwrap(),
        rules: Vec::new(),
    };
    let _publication = CurriculumPublication::from_official_source(&forged, version);

    // And nothing else coerces into the argument. A connector identifier is
    // what an undated document still has; it is not what publishes one.
    let connector = ConnectorId::new("snu.cse.official").unwrap();
    let _from_connector = CurriculumPublication::from_official_source(&connector, version);
}
