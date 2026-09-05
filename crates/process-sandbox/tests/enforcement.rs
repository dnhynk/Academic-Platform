//! What this crate claims about every class and every capability.
//!
//! A successful `enter` is irreversible and applies to the thread that calls
//! it, so the lane that measures a real installation is `native.rs`, which
//! launches the probe. What is measured here is the part that is a function
//! rather than a syscall — which capabilities this crate claims to enforce,
//! which refusals each class gets, and that a build with no backend refuses to
//! start rather than running unenforced.
//!
//! The one exception is the last test, which does call `enter` where a backend
//! exists, off the main thread, and requires it to fail.

use std::collections::BTreeMap;

use academic_policy::{ProcessCapability, ProcessClass};
use academic_process_sandbox::{EnforcementBasis, EnforcementError, basis, enter, refusals};

/// The whole vocabulary, classified, written out here rather than derived.
///
/// The point of restating it is that `basis` and this table are two
/// independent statements of the same fact: a `basis` arm edited on its own
/// fails here, and a capability added to `ProcessCapability::ALL` fails here
/// too because the two key sets are compared whole.
fn expected_bases() -> BTreeMap<&'static str, &'static str> {
    BTreeMap::from([
        ("CAPTURE_DEVICE", "ELSEWHERE"),
        ("WRITE_STAGED_ARTIFACT", "PROCESS_BOUNDARY"),
        ("READ_ARTIFACT_RANGE", "BROKER_ONLY"),
        ("WRITE_SEARCH_INDEX", "BROKER_ONLY"),
        ("ANALYZE_REPOSITORY", "BROKER_ONLY"),
        ("BORROW_CONNECTOR_CREDENTIAL", "BROKER_ONLY"),
        ("STAGE_EXTERNAL_PAYLOAD", "BROKER_ONLY"),
        ("OPEN_OUTBOUND_SOCKET", "PROCESS_BOUNDARY"),
        ("CREATE_CLAIM", "BROKER_ONLY"),
        ("ASSEMBLE_EXPORT", "BROKER_ONLY"),
        ("READ_KEY_MATERIAL", "BROKER_ONLY"),
    ])
}

/// The refusal each class gets, written out here rather than derived.
///
/// `refusals` computes the complement of the declaration inside the enforced
/// subset. This table is what that complement has to come out as, spelled from
/// `P2-G7`'s matrix rather than from the function, so a change to either side
/// is a failure and not a silent agreement.
fn expected_refusals() -> BTreeMap<&'static str, Vec<&'static str>> {
    BTreeMap::from([
        // Declares WriteStagedArtifact; declares no socket.
        ("CAPTURE_CLIENT", vec!["OPEN_OUTBOUND_SOCKET"]),
        (
            "INDEXER",
            vec!["WRITE_STAGED_ARTIFACT", "OPEN_OUTBOUND_SOCKET"],
        ),
        (
            "REPOSITORY_ANALYZER",
            vec!["WRITE_STAGED_ARTIFACT", "OPEN_OUTBOUND_SOCKET"],
        ),
        (
            "CONNECTOR",
            vec!["WRITE_STAGED_ARTIFACT", "OPEN_OUTBOUND_SOCKET"],
        ),
        // The one class that declares a socket, and the only empty socket row.
        ("EGRESS_PROXY", vec!["WRITE_STAGED_ARTIFACT"]),
        (
            "EXPORT_JOB",
            vec!["WRITE_STAGED_ARTIFACT", "OPEN_OUTBOUND_SOCKET"],
        ),
    ])
}

#[test]
fn every_capability_in_the_vocabulary_has_exactly_one_basis() {
    let observed: BTreeMap<&'static str, &'static str> = ProcessCapability::ALL
        .into_iter()
        .map(|capability| (capability.as_str(), basis(capability).as_str()))
        .collect();
    assert_eq!(
        observed.len(),
        ProcessCapability::ALL.len(),
        "two capabilities share a wire spelling, so this table lost a row"
    );
    assert_eq!(
        observed,
        expected_bases(),
        "the enforcement basis of a capability changed, or the vocabulary gained one"
    );
}

#[test]
fn a_basis_that_is_not_the_process_boundary_says_who_or_why() {
    // The classification is only worth having if the two non-enforcing answers
    // carry their reason. An empty string would classify a capability as
    // reviewed while saying nothing.
    for capability in ProcessCapability::ALL {
        match basis(capability) {
            EnforcementBasis::ProcessBoundary => {}
            EnforcementBasis::Elsewhere(reason) | EnforcementBasis::BrokerOnly(reason) => {
                assert!(
                    reason.len() >= 40,
                    "{} carries a {} of {reason:?}, which reviews nothing",
                    capability.as_str(),
                    basis(capability).as_str()
                );
            }
        }
    }
}

#[test]
fn the_enforced_subset_is_exactly_the_two_the_operating_system_can_refuse() {
    let mut enforced: Vec<&'static str> = ProcessCapability::ALL
        .into_iter()
        .filter(|capability| basis(*capability) == EnforcementBasis::ProcessBoundary)
        .map(ProcessCapability::as_str)
        .collect();
    enforced.sort_unstable();
    assert_eq!(
        enforced,
        vec!["OPEN_OUTBOUND_SOCKET", "WRITE_STAGED_ARTIFACT"]
    );
}

#[test]
fn each_class_refuses_exactly_what_it_does_not_declare() {
    let observed: BTreeMap<&'static str, Vec<&'static str>> = ProcessClass::ALL
        .into_iter()
        .map(|class| {
            (
                class.as_str(),
                refusals(class)
                    .into_iter()
                    .map(ProcessCapability::as_str)
                    .collect(),
            )
        })
        .collect();
    assert_eq!(
        observed,
        expected_refusals(),
        "a class's refusal set is no longer the complement of its declaration"
    );

    // And the same fact read the other way, from the declaration rather than
    // from the table above: nothing enforced is both declared and refused, and
    // nothing enforced is neither.
    for class in ProcessClass::ALL {
        let refused = refusals(class);
        for capability in ProcessCapability::ALL {
            if basis(capability) != EnforcementBasis::ProcessBoundary {
                assert!(
                    !refused.contains(&capability),
                    "{} refuses {}, which it does not enforce",
                    class.as_str(),
                    capability.as_str()
                );
                continue;
            }
            assert_eq!(
                refused.contains(&capability),
                !class.allows(capability),
                "{} declares {} = {} and refuses it = {}",
                class.as_str(),
                capability.as_str(),
                class.allows(capability),
                refused.contains(&capability)
            );
        }
    }
}

#[test]
fn a_refusal_is_in_the_vocabulary_order_and_never_repeats() {
    for class in ProcessClass::ALL {
        let refused = refusals(class);
        let mut deduplicated = refused.clone();
        deduplicated.dedup();
        assert_eq!(
            refused,
            deduplicated,
            "{} repeats a refusal",
            class.as_str()
        );
        let positions: Vec<usize> = refused
            .iter()
            .filter_map(|capability| {
                ProcessCapability::ALL
                    .iter()
                    .position(|candidate| candidate == capability)
            })
            .collect();
        let mut sorted = positions.clone();
        sorted.sort_unstable();
        assert_eq!(
            positions,
            sorted,
            "{}'s refusals are not in ProcessCapability::ALL order",
            class.as_str()
        );
    }
}

/// Every lane except the one that has a backend: the default build on both
/// platforms, and the feature build on Windows.
#[cfg(not(all(feature = "native-enforcement", target_os = "linux")))]
#[test]
fn a_build_with_no_backend_refuses_every_class() {
    // This is the lane the whole workspace builds by default, on every
    // platform. What it must never do is return an `Enforcement`: a build that
    // installed nothing has to be indistinguishable, to its caller, from a
    // platform that cannot install anything.
    for class in ProcessClass::ALL {
        let outcome = enter(class);
        assert!(
            outcome.is_err(),
            "{} entered with no backend compiled: {outcome:?}",
            class.as_str()
        );
        let Err(error) = outcome else {
            continue;
        };
        assert!(
            matches!(error, EnforcementError::Unavailable { .. }),
            "{} failed with {error:?} rather than Unavailable",
            class.as_str()
        );
        let line = academic_process_sandbox::refusal_line(class, &error);
        assert!(
            line.starts_with(class.as_str()),
            "the refusal line does not name the class: {line}"
        );
        assert!(
            line.contains("refuses to start"),
            "the refusal line does not say the process refused: {line}"
        );
    }
}

#[test]
fn the_windows_reason_names_the_parent_that_would_have_to_apply_it() {
    // The Windows refusal is a claim about this repository as well as about the
    // platform: there is no launcher for a process class. If one is ever added,
    // this sentence stops being true and this is where that shows up.
    let reason = academic_process_sandbox::WINDOWS_HAS_NO_SELF_APPLIED_MECHANISM;
    for fragment in [
        "cannot replace its own primary token",
        "AppContainer",
        "CreateProcessW",
        "No launcher in this repository launches a process class",
    ] {
        assert!(
            reason.contains(fragment),
            "the Windows refusal no longer says {fragment:?}"
        );
    }
}

/// The one lane that has a backend, where `enter` really installs something.
///
/// It is called here from a spawned thread on purpose. Both mechanisms are
/// applied to the *calling thread* and inherited by threads created after it,
/// and the verification reads the thread group leader's status, so an `enter`
/// anywhere but at the top of `main` cannot be confirmed and fails closed. That
/// is the requirement stated as an observation, and it is also this suite's
/// proof that the verification is able to fail at all: a `verify` that always
/// said yes would return `Ok` here.
#[cfg(all(feature = "native-enforcement", target_os = "linux"))]
#[test]
fn entering_off_the_main_thread_is_not_confirmed_by_the_kernel() {
    let joined = std::thread::spawn(|| enter(ProcessClass::CaptureClient)).join();
    assert!(joined.is_ok(), "the thread that entered did not join");
    let Ok(outcome) = joined else {
        return;
    };
    assert!(
        outcome.is_err(),
        "enter reported success from a thread that is not the group leader: {outcome:?}"
    );
    let Err(error) = outcome else {
        return;
    };
    assert!(
        matches!(error, EnforcementError::NotVerified { .. }),
        "enter failed with {error:?} rather than NotVerified"
    );
}
