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

/// An `Elsewhere` basis names a package that exists.
///
/// `P2-A4`'s second audit's F5: the only thing checked about a reason was
/// `reason.len() >= 40`, so "a forty-character sentence naming a crate that is
/// not linked passes, which is what is happening". A length is not a review.
///
/// `Elsewhere` is the one answer that points at somebody else, so it is the one
/// that has to say who. The token is resolved against the workspace: the
/// package directory must exist and its manifest must declare that name, so a
/// reason naming a crate that was renamed or removed fails here rather than
/// reading as a review of a mechanism nobody has.
///
/// Whether the class that *declares* the capability is joined to that package
/// by anything but this sentence is the other half, and it needs the resolved
/// dependency graph. `an_elsewhere_basis_is_linked_or_unreachable` in
/// `tools/phase1-scaffold-policy.test.mjs` is that half.
/// A class whose own declaration is defined in terms of a capability its
/// boundary would refuse it.
///
/// `P2-A4`'s second audit's F7 is that three classes still compute their
/// capability set and drop it, and that
/// `the_unenforced_process_classes_are_named` "carries no obligation on the
/// remainder". This is that obligation, and it is what says flipping those
/// three flags is not the mechanical edit it looks like.
///
/// `refusals(class)` is the enforced subset less what the class declares, so
/// entering the sandbox as `Indexer` refuses `WRITE_STAGED_ARTIFACT`. The
/// `BrokerOnly` reason for `WriteSearchIndex` — which `Indexer` **declares** —
/// says a search projection *is* a staged write. The two sentences are about
/// the same syscall and they disagree about whether that class may make it.
/// The same holds for `Connector` and `StageExternalPayload`, whose reason
/// says staging "is governed by `WriteStagedArtifact`".
///
/// Nothing observes that today because neither binary calls `enter`. Closing
/// those boundaries would make both processes refuse the write their own
/// declaration is written in terms of — and because both binaries are still
/// the four-line stub, the refusal would be **vacuously satisfied** and the
/// disagreement would read as resolved. That is the argument for measuring it
/// here instead of flipping the flags.
///
/// `ExportJob` is the third unenforced class and it has no pair: nothing it
/// declares is defined in terms of a capability its boundary would refuse. It
/// is the one of the three whose enforcement is the mechanical edit.
///
/// The set is compared whole, so a class that gains such a pair — or one whose
/// pair is resolved by editing either sentence — fails here.
#[test]
fn a_declared_capability_is_not_defined_by_one_the_boundary_would_refuse() {
    let mut tensions: Vec<String> = Vec::new();
    for class in ProcessClass::ALL {
        let refused = refusals(class);
        for declared in class.capabilities() {
            let EnforcementBasis::BrokerOnly(reason) = basis(*declared) else {
                continue;
            };
            for other in ProcessCapability::ALL {
                if other == *declared || !refused.contains(&other) {
                    continue;
                }
                // The reasons spell a capability by its variant name, which is
                // what `Debug` prints; `as_str` is the wire spelling and is not
                // what the prose uses.
                let variant = format!("{other:?}");
                if reason.contains(&variant) {
                    let name = format!("{declared:?}");
                    tensions.push(format!(
                        "{} declares {name} whose basis is written in terms of {variant}, \
                         and its boundary would refuse {variant}",
                        class.as_str()
                    ));
                }
            }
        }
    }
    tensions.sort();
    assert_eq!(
        tensions,
        vec![
            "CONNECTOR declares StageExternalPayload whose basis is written in terms of \
             WriteStagedArtifact, and its boundary would refuse WriteStagedArtifact"
                .to_owned(),
            "INDEXER declares WriteSearchIndex whose basis is written in terms of \
             WriteStagedArtifact, and its boundary would refuse WriteStagedArtifact"
                .to_owned(),
        ],
        "the set of classes whose declaration argues with their own boundary changed"
    );

    // The check is not vacuous: every class was read, and the scan found a
    // capability name inside a reason at all.
    assert_eq!(ProcessClass::ALL.len(), 6);
    assert!(
        ProcessCapability::ALL
            .into_iter()
            .filter_map(|capability| match basis(capability) {
                EnforcementBasis::BrokerOnly(reason) => Some(reason),
                _ => None,
            })
            .any(|reason| reason.contains("WriteStagedArtifact")),
        "no broker-only reason names another capability, so this rule reads nothing"
    );
}

#[test]
fn an_elsewhere_basis_names_a_package_that_exists() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    let mut named: BTreeMap<&'static str, String> = BTreeMap::new();
    for capability in ProcessCapability::ALL {
        let EnforcementBasis::Elsewhere(reason) = basis(capability) else {
            continue;
        };
        let package = reason
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '-'))
            .find(|word| word.starts_with("academic-"))
            .unwrap_or_default();
        assert!(
            !package.is_empty(),
            "{} says its enforcement is elsewhere and does not say where: {reason:?}",
            capability.as_str()
        );
        let directory = package.strip_prefix("academic-").unwrap_or(package);
        let manifest = root.join("crates").join(directory).join("Cargo.toml");
        let text = std::fs::read_to_string(&manifest)
            .unwrap_or_else(|_| String::from("<no manifest at that path>"));
        assert!(
            text.contains(&format!("name = \"{package}\"")),
            "{} names {package}, and {} does not declare it",
            capability.as_str(),
            manifest.display()
        );
        named.insert(capability.as_str(), package.to_owned());
    }
    // The whole set, so a capability that becomes `Elsewhere` arrives here.
    assert_eq!(
        named,
        BTreeMap::from([("CAPTURE_DEVICE", "academic-capture-gate".to_owned())]),
        "the set of capabilities enforced elsewhere changed"
    );
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
