mod support;

use academic_projections::generation::ProjectionKind;

use support::{RankSkewCorpus, RankedIsolationFixture, TestResult};

#[test]
fn inactive_historical_generation_cannot_change_selected_generation_ranks() -> TestResult {
    for kind in [ProjectionKind::Unicode61, ProjectionKind::Trigram] {
        let case = RankedIsolationFixture::new(
            &format!("inactive-history-{kind}"),
            kind,
            RankSkewCorpus::InactiveHistorical,
            false,
        )?;
        let before = case.snapshot()?;
        let receipt = case.fixture.runner()?.build_inactive_at(
            kind,
            case.selected_domain,
            case.noise_coordinates,
            &case.policies,
        )?;
        assert!(!receipt.activated);
        let after = case.snapshot()?;
        assert_eq!(after, before);
    }
    Ok(())
}

#[test]
fn another_academic_domain_cannot_change_selected_generation_ranks() -> TestResult {
    for kind in [ProjectionKind::Unicode61, ProjectionKind::Trigram] {
        let case = RankedIsolationFixture::new(
            &format!("other-domain-{kind}"),
            kind,
            RankSkewCorpus::FailedOrOtherDomain,
            true,
        )?;
        let before = case.snapshot()?;
        let receipt = case.fixture.runner()?.rebuild_at(
            kind,
            case.noise_domain,
            case.noise_coordinates,
            &case.policies,
        )?;
        assert!(receipt.activated);
        let after = case.snapshot()?;
        assert_eq!(after, before);
    }
    Ok(())
}
