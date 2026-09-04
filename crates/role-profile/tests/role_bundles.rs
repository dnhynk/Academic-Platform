//! `P2-Y2`'s acceptance suite: section 24.2's versioned competency bundle.
//!
//! Six tests carry the task, and each one is named in the execution plan. The
//! rest are the measurements those six rest on — the identity, the registry
//! qualifier, and the specification readings — kept separate so a failure says
//! which fact moved.
//!
//! # Nothing here fabricates a competency
//!
//! Every `CompetencyId` in this file is `P2-Y1`'s own identity, built through
//! `CompetencyId::new`, which is the same door the competency crate uses. A
//! bundle entry cannot name a concept: `ConceptRef` and `CompetencyId` have no
//! conversion in either direction and this crate adds none.
//!
//! # The specification is read, not restated
//!
//! Four readings come out of `PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md`
//! at run time rather than being copied into this file: section 24.2's twelve
//! direction names, its YAML block's key set, its `importance` values, and
//! sections 25.11 and 37's two refusals. Each is compared **in both
//! directions**, so a specification that renames, adds or drops one fails this
//! suite instead of drifting past it. `P2-N6` set that pattern and this file
//! follows it.

use std::{error::Error, fs, path::PathBuf};

use academic_competency::CompetencyId;
use academic_domain::predicates::{NodeType, PredicateName, QualifierKind};
use academic_ingestion::Date;
use academic_role_profile::{
    Adjustment, AdjustmentLayer, BundleEntry, BundleImportance, BundleOrigin, BundleScope,
    BundleShelf, BundleSource, DirectionName, InterestStanding, NO_SHIPPED_BUNDLES,
    REFUSED_STANDINGS, RecordedOn, RoleDirection, RoleError, RoleInterest, RoleLabel, RoleProfile,
    RoleProfileId, RoleProfileRef, RoleProfileVersion, UserAdjustment, declare, fork,
    identity::VERSION_QUALIFIER, revise,
};

type TestResult = Result<(), Box<dyn Error>>;

/// The design document, read from the workspace root.
fn specification() -> Result<String, Box<dyn Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|crates| crates.parent())
        .ok_or("the crate has no workspace root")?
        .join("PERSONAL_ACADEMIC_CS_PROJECT_OS_END_STATE_DESIGN.md");
    Ok(fs::read_to_string(root)?)
}

/// One section's body, up to the next heading at or above `terminator`.
fn section_until(text: &str, heading: &str, terminator: &str) -> Result<String, Box<dyn Error>> {
    let start = text
        .find(heading)
        .ok_or_else(|| format!("the specification no longer holds {heading:?}"))?;
    let body = &text[start + heading.len()..];
    let end = body.find(terminator).unwrap_or(body.len());
    Ok(body[..end].to_owned())
}

/// One `###` section's body, up to the next heading of the same level.
fn section(text: &str, heading: &str) -> Result<String, Box<dyn Error>> {
    section_until(text, heading, "\n### ")
}

/// The fenced YAML block inside a section body.
fn yaml_block(body: &str) -> Result<String, Box<dyn Error>> {
    let open = body
        .find("```yaml")
        .ok_or("the section holds no yaml block")?;
    let rest = &body[open + "```yaml".len()..];
    let close = rest.find("```").ok_or("the yaml block is not closed")?;
    Ok(rest[..close].to_owned())
}

/// Section 24.2's twelve direction names, read out of its own sentence.
fn spec_direction_names(body: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let marker = "등을 지원하되";
    let line = body
        .lines()
        .find(|line| line.contains(marker))
        .ok_or("section 24.2 no longer says 등을 지원하되")?;
    let listed = line
        .split(marker)
        .next()
        .ok_or("the direction sentence has no prefix")?;
    Ok(listed
        .split(',')
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect())
}

/// Keys at one exact indent depth of a YAML block, in order.
fn yaml_keys(block: &str, indent: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in block.lines() {
        let Some(rest) = line.strip_prefix(indent) else {
            continue;
        };
        if rest.starts_with(' ') || rest.starts_with('-') {
            continue;
        }
        let Some((key, _)) = rest.split_once(':') else {
            continue;
        };
        if key.is_empty() || key.contains(' ') {
            continue;
        }
        if !found.iter().any(|seen| seen == key) {
            found.push(key.to_owned());
        }
    }
    found
}

/// The keys of the **first** list item in a YAML block, in order.
///
/// A second item ends the read, so what comes back is one entry's shape rather
/// than the union of every entry's.
fn first_list_item_keys(block: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let mut inside = false;
    for line in block.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some(item) = trimmed.strip_prefix("- ") {
            if inside {
                break;
            }
            inside = true;
            if let Some((key, _)) = item.split_once(':') {
                found.push(key.to_owned());
            }
            continue;
        }
        if inside {
            if indent < 4 {
                break;
            }
            if let Some((key, _)) = trimmed.split_once(':')
                && !key.is_empty()
                && !key.contains(' ')
                && !found.iter().any(|seen| seen == key)
            {
                found.push(key.to_owned());
            }
        }
    }
    found
}

fn competency(id: &str) -> Result<CompetencyId, Box<dyn Error>> {
    Ok(CompetencyId::new(id)?)
}

fn day(year: u16, month: u8, of_month: u8) -> Result<RecordedOn, Box<dyn Error>> {
    Ok(RecordedOn::on(Date::new(year, month, of_month)?))
}

fn source(cited: &str) -> Result<BundleSource, Box<dyn Error>> {
    Ok(BundleSource::cited(cited, day(2026, 8, 20)?)?)
}

/// Section 24.2's own example bundle, at its first version.
fn backend_bundle() -> Result<RoleProfile, Box<dyn Error>> {
    Ok(declare(
        RoleProfileId::new("backend_engineer_profile")?,
        RoleLabel::new("Backend Engineer")?,
        RoleDirection::Backend,
        day(2026, 8, 26)?,
        BundleScope::new(BundleScope::USER_CURATED_GENERAL)?,
        vec![
            BundleEntry::of(competency("API_ARCHITECTURE")?, BundleImportance::Core),
            BundleEntry::of(
                competency("RELATIONAL_DATABASE_DIAGNOSIS")?,
                BundleImportance::Core,
            ),
            BundleEntry::of(competency("CACHING_TRADEOFFS")?, BundleImportance::Common),
            BundleEntry::of(
                competency("DISTRIBUTED_FAILURE_REASONING")?,
                BundleImportance::ContextDependent,
            ),
            BundleEntry::of(competency("PRODUCTION_DEBUGGING")?, BundleImportance::Core),
        ],
        vec![source("the user's own reading of two job descriptions")?],
    )?)
}

// ---------------------------------------------------------------------------
// The six named acceptance tests
// ---------------------------------------------------------------------------

/// Section 24.2's schema, both ways, against the specification's own key set.
///
/// The document this crate writes is compared with the YAML block in the
/// specification. Three differences are deliberate and each is named in the
/// assertion below; every other key must agree in both directions.
#[test]
fn role_profile_schema_round_trip() -> TestResult {
    let text = specification()?;
    let body = section(&text, "### 24.2 Role은 versioned competency bundle")?;
    let block = yaml_block(&body)?;
    let spec_keys = yaml_keys(&block, "  ");
    assert_eq!(
        spec_keys,
        vec![
            "id".to_owned(),
            "label".to_owned(),
            "validAt".to_owned(),
            "scope".to_owned(),
            "competencies".to_owned(),
            "sources".to_owned(),
            "userAdjustments".to_owned(),
        ],
        "section 24.2's key set moved; reconcile the wire before the read"
    );
    assert_eq!(
        first_list_item_keys(&block),
        vec!["competency".to_owned(), "importance".to_owned()],
        "section 24.2's competency entry moved"
    );

    let profile = backend_bundle()?;
    let document = serde_json::to_value(&profile)?;
    let object = document
        .as_object()
        .ok_or("a role profile is not a JSON object")?;
    let mut wire_keys: Vec<String> = object.keys().cloned().collect();
    wire_keys.sort();

    // The three deliberate differences, each with the sentence that makes it.
    //
    // `id` is split into `id` and `version`, because section 24.2's
    // `backend_engineer_profile_v4` folds a lineage and a version into one
    // string and `P2-R4` measured what a folded identity collides.
    //
    // `direction` is added, because section 24.2's twelve names are in its
    // prose rather than its YAML and reading them out of `label` would be the
    // market truth the same paragraph refuses.
    //
    // `origin` is added, because `fork_preserves_base_and_records_lineage`
    // needs the lineage on the record.
    //
    // `userAdjustments` is removed, because it is a second document.
    let mut expected: Vec<String> = spec_keys
        .iter()
        .filter(|key| key.as_str() != "userAdjustments")
        .cloned()
        .chain([
            "version".to_owned(),
            "direction".to_owned(),
            "origin".to_owned(),
        ])
        .collect();
    expected.sort();
    assert_eq!(
        wire_keys, expected,
        "the wire key set is section 24.2's minus userAdjustments plus version, direction and origin"
    );

    assert_eq!(document["id"], "backend_engineer_profile");
    assert_eq!(document["version"], 1);
    assert_eq!(document["label"], "Backend Engineer");
    assert_eq!(document["validAt"], "2026-08-26");
    assert_eq!(document["scope"], "user_curated_general_profile");
    assert_eq!(
        document["competencies"][0]["competency"],
        "API_ARCHITECTURE"
    );
    assert_eq!(document["competencies"][0]["importance"], "CORE");
    assert_eq!(document["origin"]["kind"], "AUTHORED");

    let read: RoleProfile = serde_json::from_value(document.clone())?;
    assert_eq!(read, profile, "the round trip is not the identity");

    // Every key is required: removing any one refuses the document.
    for key in object.keys() {
        let mut broken = object.clone();
        broken.remove(key);
        assert!(
            serde_json::from_value::<RoleProfile>(serde_json::Value::Object(broken)).is_err(),
            "a document with no {key} was still read as a role profile"
        );
    }

    // `userAdjustments` cannot ride back in through the base document.
    let mut merged = object.clone();
    merged.insert("userAdjustments".to_owned(), serde_json::json!([]));
    assert!(
        serde_json::from_value::<RoleProfile>(serde_json::Value::Object(merged)).is_err(),
        "a base bundle accepted an adjustment key"
    );

    // The adjustments are their own document, and it round trips too.
    let layer = AdjustmentLayer::over(
        profile.reference(),
        vec![UserAdjustment::of(
            Adjustment::Reweight {
                competency: competency("CACHING_TRADEOFFS")?,
                importance: BundleImportance::Core,
            },
            "the target team runs its own cache tier",
        )?],
    )?;
    let layer_document = serde_json::to_value(&layer)?;
    let layer_keys: Vec<&String> = layer_document
        .as_object()
        .ok_or("a layer is not a JSON object")?
        .keys()
        .collect();
    assert_eq!(
        layer_keys,
        vec!["base", "userAdjustments"],
        "the layer document is the base it adjusts and section 24.2's own key"
    );
    assert_eq!(layer_document["base"]["version"], 1);
    assert_eq!(
        layer_document["userAdjustments"][0]["adjustment"]["kind"],
        "REWEIGHT"
    );
    assert_eq!(
        layer_document["userAdjustments"][0]["because"],
        "the target team runs its own cache tier"
    );
    let read_layer: AdjustmentLayer = serde_json::from_value(layer_document)?;
    assert_eq!(read_layer, layer);
    Ok(())
}

/// An edit produces the next version and leaves the base exactly where it was.
#[test]
fn role_edit_creates_a_new_version() -> TestResult {
    let base = backend_bundle()?;
    let before = serde_json::to_string(&base)?;

    let layer = AdjustmentLayer::over(
        base.reference(),
        vec![
            UserAdjustment::of(
                Adjustment::Add {
                    competency: competency("MESSAGE_QUEUE_DELIVERY_SEMANTICS")?,
                    importance: BundleImportance::Common,
                },
                "the two services the user reads talk over a queue",
            )?,
            UserAdjustment::of(
                Adjustment::Reweight {
                    competency: competency("CACHING_TRADEOFFS")?,
                    importance: BundleImportance::Core,
                },
                "the target team runs its own cache tier",
            )?,
            UserAdjustment::of(
                Adjustment::Remove {
                    competency: competency("DISTRIBUTED_FAILURE_REASONING")?,
                },
                "out of scope for this bundle; it has its own",
            )?,
        ],
    )?;
    let revised = revise(&base, &layer, day(2026, 9, 1)?)?;

    assert_eq!(
        serde_json::to_string(&base)?,
        before,
        "the base changed while it was being revised"
    );
    assert_eq!(revised.id(), base.id(), "a revision stays in its lineage");
    assert_eq!(revised.version().get(), base.version().get() + 1);
    assert_eq!(
        revised.origin(),
        &BundleOrigin::Revised(base.reference()),
        "the revision does not name the version it came from"
    );
    assert_ne!(
        revised.reference(),
        base.reference(),
        "the edit did not take a new identity"
    );

    assert_eq!(
        revised
            .entry(&competency("MESSAGE_QUEUE_DELIVERY_SEMANTICS")?)
            .map(BundleEntry::importance),
        Some(BundleImportance::Common)
    );
    assert_eq!(
        revised
            .entry(&competency("CACHING_TRADEOFFS")?)
            .map(BundleEntry::importance),
        Some(BundleImportance::Core)
    );
    assert!(
        revised
            .entry(&competency("DISTRIBUTED_FAILURE_REASONING")?)
            .is_none()
    );
    assert_eq!(
        base.entry(&competency("CACHING_TRADEOFFS")?)
            .map(BundleEntry::importance),
        Some(BundleImportance::Common),
        "the base's own entry moved"
    );

    // Both versions coexist, and neither replaces the other on the shelf.
    let shelf = BundleShelf::empty()
        .shelve(base.clone())?
        .shelve(revised.clone())?;
    assert_eq!(shelf.versions_of(base.id()).len(), 2);
    assert_eq!(
        shelf.get(&base.reference()),
        Some(&base),
        "the earlier version is not what it was"
    );

    // A second edit that landed on the same version is refused rather than
    // overwriting: an edit that wants to be stored takes a version it does not
    // hold.
    assert_eq!(
        shelf.clone().shelve(revised.clone()),
        Err(RoleError::VersionAlreadyShelved(
            revised.reference().rendered()
        )),
        "a second bundle at an occupied pair was shelved instead of refused"
    );

    // A layer written over version one is not applied to version two.
    assert_eq!(
        revise(&revised, &layer, day(2026, 9, 2)?),
        Err(RoleError::LayerIsForAnotherVersion {
            layer_base: base.reference().rendered(),
            profile: revised.reference().rendered(),
        }),
        "a layer written over version one was applied to version two"
    );
    Ok(())
}

/// Section 24.2's twelve names, read out of the specification, in both
/// directions — and the ten this build ships nothing for, by name.
#[test]
fn twelve_role_directions_are_representable_or_explicitly_absent() -> TestResult {
    let text = specification()?;
    let body = section(&text, "### 24.2 Role은 versioned competency bundle")?;
    let spec_names = spec_direction_names(&body)?;
    assert_eq!(
        spec_names,
        vec![
            "Backend",
            "Systems",
            "Database",
            "Distributed Systems",
            "Infrastructure/Platform",
            "SRE",
            "Cloud",
            "Security",
            "ML/AI",
            "Data",
            "Compiler/PL",
            "Research",
        ],
        "section 24.2's direction sentence moved"
    );

    // Both directions: every specification name is an arm, and every arm is a
    // specification name.
    let arm_names: Vec<&str> = RoleDirection::NAMED
        .iter()
        .filter_map(RoleDirection::spec_name)
        .collect();
    assert_eq!(
        arm_names.len(),
        RoleDirection::NAMED.len(),
        "an arm of NAMED carries no specification spelling"
    );
    assert_eq!(arm_names, spec_names, "the arms are not the twelve names");
    for name in &spec_names {
        assert!(
            arm_names.contains(&name.as_str()),
            "section 24.2 names {name}, and no arm carries it"
        );
    }

    // The `등` is represented and is not a thirteenth name.
    assert!(
        body.contains("등을 지원하되"),
        "section 24.2 stopped saying its list is open"
    );
    let user_named = RoleDirection::UserNamed(DirectionName::new("quantitative_research_infra")?);
    assert_eq!(user_named.spec_name(), None);
    assert!(!user_named.is_named_by_the_specification());
    assert!(
        !RoleDirection::NAMED.contains(&user_named),
        "the open arm was counted among the twelve"
    );

    // Absence is named, not silent. This build ships nothing for any of them.
    assert!(
        NO_SHIPPED_BUNDLES.contains("GATE-38-029"),
        "the absence sentence does not name the gate it is open on"
    );
    let empty = BundleShelf::empty();
    let coverage = empty.directions_covered();
    assert_eq!(
        coverage.len(),
        RoleDirection::NAMED.len(),
        "an empty shelf did not report every named direction"
    );
    for row in &coverage {
        assert!(
            !row.is_covered(),
            "{:?} is covered on an empty shelf",
            row.direction()
        );
        assert!(row.held().is_empty());
    }

    // Two of the twelve curated by the user; the other ten still appear, and
    // they appear as themselves rather than as a gap in the map.
    let shelf = BundleShelf::empty()
        .shelve(backend_bundle()?)?
        .shelve(declare(
            RoleProfileId::new("systems_profile")?,
            RoleLabel::new("Systems")?,
            RoleDirection::Systems,
            day(2026, 8, 26)?,
            BundleScope::new(BundleScope::USER_CURATED_GENERAL)?,
            vec![BundleEntry::of(
                competency("KERNEL_MEMORY_BEHAVIOUR")?,
                BundleImportance::Core,
            )],
            vec![source(
                "the user's own notes from an operating systems course",
            )?],
        )?)?;
    let coverage = shelf.directions_covered();
    let covered: Vec<&RoleDirection> = coverage
        .iter()
        .filter(|row| row.is_covered())
        .map(academic_role_profile::DirectionCoverage::direction)
        .collect();
    assert_eq!(
        covered,
        vec![&RoleDirection::Backend, &RoleDirection::Systems]
    );
    let uncovered: Vec<&RoleDirection> = coverage
        .iter()
        .filter(|row| !row.is_covered())
        .map(academic_role_profile::DirectionCoverage::direction)
        .collect();
    assert_eq!(
        uncovered.len(),
        RoleDirection::NAMED.len() - 2,
        "the directions nothing covers were not all reported"
    );
    for direction in &RoleDirection::NAMED {
        assert!(
            coverage.iter().any(|row| row.direction() == direction),
            "{direction:?} is missing from the coverage report"
        );
    }
    Ok(())
}

/// Two organisations, one label, two bundles, and neither one wins.
#[test]
fn two_org_bundles_coexist_with_scope_and_source() -> TestResult {
    let label = RoleLabel::new("Backend Engineer")?;
    let north = declare(
        RoleProfileId::new("north_org_backend")?,
        label.clone(),
        RoleDirection::Backend,
        day(2026, 8, 26)?,
        BundleScope::new("north_org_platform_team")?,
        vec![
            BundleEntry::of(competency("API_ARCHITECTURE")?, BundleImportance::Core),
            BundleEntry::of(
                competency("RELATIONAL_DATABASE_DIAGNOSIS")?,
                BundleImportance::Core,
            ),
        ],
        vec![source(
            "north org's published engineering ladder, 2026 edition",
        )?],
    )?;
    let south = declare(
        RoleProfileId::new("south_lab_backend")?,
        label.clone(),
        RoleDirection::Backend,
        day(2026, 8, 26)?,
        BundleScope::new("south_lab_research_infrastructure")?,
        vec![
            BundleEntry::of(competency("API_ARCHITECTURE")?, BundleImportance::Common),
            BundleEntry::of(
                competency("DISTRIBUTED_FAILURE_REASONING")?,
                BundleImportance::Core,
            ),
        ],
        vec![source(
            "a conversation with the laboratory's second-year student",
        )?],
    )?;

    let north_before = serde_json::to_string(&north)?;
    let shelf = BundleShelf::empty()
        .shelve(north.clone())?
        .shelve(south.clone())?;
    assert_eq!(shelf.len(), 2, "one bundle displaced the other");
    assert_eq!(
        serde_json::to_string(shelf.get(&north.reference()).ok_or("north is gone")?)?,
        north_before,
        "shelving the second bundle changed the first"
    );

    // The label reaches both, and reaching both is reported rather than
    // resolved.
    let reading = shelf.by_label(&label);
    assert_eq!(reading.reached().len(), 2);
    let ambiguity = reading
        .ambiguity()
        .ok_or("one label over two bundles carried no diagnostic")?;
    assert_eq!(
        ambiguity.lineages(),
        [
            RoleProfileId::new("north_org_backend")?,
            RoleProfileId::new("south_lab_backend")?
        ]
    );
    assert_eq!(
        ambiguity.scopes(),
        [
            "north_org_platform_team".to_owned(),
            "south_lab_research_infrastructure".to_owned()
        ],
        "the diagnostic does not say which scopes disagreed"
    );

    // Scope and source travel with each bundle and are not merged.
    let north_on_shelf = shelf.get(&north.reference()).ok_or("north is gone")?;
    let south_on_shelf = shelf.get(&south.reference()).ok_or("south is gone")?;
    assert_ne!(north_on_shelf.scope(), south_on_shelf.scope());
    assert_ne!(
        north_on_shelf.sources()[0].cited_as(),
        south_on_shelf.sources()[0].cited_as()
    );
    assert_eq!(
        north_on_shelf
            .entry(&competency("API_ARCHITECTURE")?)
            .map(BundleEntry::importance),
        Some(BundleImportance::Core)
    );
    assert_eq!(
        south_on_shelf
            .entry(&competency("API_ARCHITECTURE")?)
            .map(BundleEntry::importance),
        Some(BundleImportance::Common),
        "the two organisations were made to agree about one competency"
    );

    // A label with one bundle behind it carries no diagnostic, so the
    // diagnostic above is a reading and not a constant.
    let alone = shelf.by_label(&RoleLabel::new("Systems")?);
    assert!(alone.reached().is_empty());
    assert!(alone.ambiguity().is_none());
    let one = BundleShelf::empty().shelve(north.clone())?.by_label(&label);
    assert_eq!(one.reached().len(), 1);
    assert!(one.ambiguity().is_none());
    Ok(())
}

/// A fork keeps its base intact and records which version it came from.
#[test]
fn fork_preserves_base_and_records_lineage() -> TestResult {
    let base = backend_bundle()?;
    let before = serde_json::to_string(&base)?;

    let forked = fork(
        &base,
        RoleProfileId::new("north_org_backend")?,
        RoleLabel::new("Backend Engineer, North Org")?,
        day(2026, 9, 3)?,
        BundleScope::new("north_org_platform_team")?,
        vec![source(
            "north org's published engineering ladder, 2026 edition",
        )?],
    )?;

    assert_eq!(
        serde_json::to_string(&base)?,
        before,
        "the base changed while it was being forked"
    );
    assert_ne!(forked.id(), base.id(), "a fork is a different lineage");
    assert_eq!(forked.version(), RoleProfileVersion::FIRST);
    assert_eq!(
        forked.origin(),
        &BundleOrigin::Forked(base.reference()),
        "the fork does not record what it came from"
    );
    assert_eq!(
        forked.origin().base().map(RoleProfileRef::version),
        Some(base.version()),
        "the lineage does not name the base's version"
    );
    assert_eq!(
        forked.competencies(),
        base.competencies(),
        "a fork starts from the base's entries"
    );
    assert_eq!(forked.direction(), base.direction());

    // The fork states its own scope, label and citations rather than claiming
    // the base's.
    assert_ne!(forked.scope(), base.scope());
    assert_ne!(forked.label(), base.label());
    assert_ne!(forked.sources(), base.sources());

    // Forking two different versions records two different things.
    let layer = AdjustmentLayer::over(
        base.reference(),
        vec![UserAdjustment::of(
            Adjustment::Remove {
                competency: competency("DISTRIBUTED_FAILURE_REASONING")?,
            },
            "it has a bundle of its own",
        )?],
    )?;
    let second = revise(&base, &layer, day(2026, 9, 1)?)?;
    let from_second = fork(
        &second,
        RoleProfileId::new("north_org_backend")?,
        RoleLabel::new("Backend Engineer, North Org")?,
        day(2026, 9, 3)?,
        BundleScope::new("north_org_platform_team")?,
        vec![source(
            "north org's published engineering ladder, 2026 edition",
        )?],
    )?;
    assert_ne!(
        forked.origin(),
        from_second.origin(),
        "forking version one and version two recorded the same lineage"
    );

    // A fork into the base's own lineage is a revision and is refused.
    assert_eq!(
        fork(
            &base,
            base.id().clone(),
            RoleLabel::new("Backend Engineer")?,
            day(2026, 9, 3)?,
            BundleScope::new("north_org_platform_team")?,
            vec![source(
                "north org's published engineering ladder, 2026 edition",
            )?],
        ),
        Err(RoleError::ForkIntoTheSameLineage(
            base.id().as_str().to_owned()
        )),
        "a fork into the base's own lineage was allowed"
    );

    // The base and both forks live together, and the base is unchanged after.
    let shelf = BundleShelf::empty()
        .shelve(base.clone())?
        .shelve(second)?
        .shelve(forked)?;
    assert_eq!(shelf.len(), 3);
    assert_eq!(
        serde_json::to_string(shelf.get(&base.reference()).ok_or("the base is gone")?)?,
        before
    );
    Ok(())
}

/// Favouriting a role confers nothing, and the two standings that would be a
/// decision are absent by name.
#[test]
fn favoriting_a_role_is_not_a_career_decision() -> TestResult {
    let text = specification()?;
    let explorer = section(&text, "### 25.11 Career Explorer")?;
    let scenario = section_until(
        &text,
        "## 37. Multi-year Scenario",
        "
## ",
    )?;

    // The two refusals are the specification's own sentences, read back.
    for (standing, sentence) in REFUSED_STANDINGS {
        assert!(
            explorer.contains(sentence) || scenario.contains(sentence),
            "no section still says {sentence:?}, which is why {standing} is absent"
        );
        assert!(
            !InterestStanding::ALL
                .iter()
                .any(|arm| arm.as_str() == standing),
            "{standing} is an arm of InterestStanding"
        );
    }
    assert!(
        scenario.contains("다시 neutral 상태로 둘 수 있다"),
        "section 37 stopped licensing a return to neutral"
    );
    assert_eq!(
        InterestStanding::ALL
            .iter()
            .map(|standing| standing.as_str())
            .collect::<Vec<_>>(),
        vec!["FAVORITED", "EXPLORING", "NEUTRAL"],
        "the standing vocabulary moved"
    );

    let base = backend_bundle()?;
    let shelf = BundleShelf::empty().shelve(base.clone())?;
    let before = serde_json::to_string(&base)?;
    let coverage_before = shelf.directions_covered();

    let favourite = RoleInterest::in_role(base.id().clone(), InterestStanding::Favorited);

    // It names a lineage and not a version, so it does not even select which
    // bundle is in force.
    assert_eq!(favourite.profile(), base.id());
    assert_eq!(favourite.standing(), InterestStanding::Favorited);

    // Nothing about the bundles moved.
    assert_eq!(
        serde_json::to_string(shelf.get(&base.reference()).ok_or("the base is gone")?)?,
        before,
        "favouriting a role changed the bundle"
    );
    assert_eq!(
        shelf.directions_covered(),
        coverage_before,
        "favouriting a role changed what the shelf covers"
    );
    assert_eq!(shelf.len(), 1);

    // Section 37: going back to neutral does not rewrite what came before, and
    // there is no failure to record.
    let now_neutral = favourite.clone().standing_now(InterestStanding::Neutral);
    assert_eq!(now_neutral.standing(), InterestStanding::Neutral);
    assert_eq!(
        favourite.standing(),
        InterestStanding::Favorited,
        "changing the standing rewrote the value it was made from"
    );
    assert_eq!(now_neutral.profile(), favourite.profile());

    // The interest document holds two keys and neither is a target.
    let document = serde_json::to_value(&favourite)?;
    let keys: Vec<&String> = document
        .as_object()
        .ok_or("an interest is not a JSON object")?
        .keys()
        .collect();
    assert_eq!(keys, vec!["profile", "standing"]);

    // An interest is not an input to anything. `fork` — the one act section
    // 24.2 says a user performs on a bundle — takes a bundle, and the
    // compile-fail suite holds the half that cannot be written here.
    let still_a_bundle = fork(
        &base,
        RoleProfileId::new("north_org_backend")?,
        RoleLabel::new("Backend Engineer, North Org")?,
        day(2026, 9, 3)?,
        BundleScope::new("north_org_platform_team")?,
        vec![source(
            "north org's published engineering ladder, 2026 edition",
        )?],
    )?;
    assert_eq!(
        still_a_bundle.origin(),
        &BundleOrigin::Forked(base.reference()),
        "the act that does commit something is still the one that names a bundle"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The measurements the six rest on
// ---------------------------------------------------------------------------

/// The identity is the pair, and the pair does not collide where the rendered
/// name does.
#[test]
fn an_identity_is_a_pair_and_not_a_rendered_name() -> TestResult {
    let folded = RoleProfileRef::of(
        RoleProfileId::new("backend_engineer_profile_v4")?,
        RoleProfileVersion::FIRST,
    );
    let split = RoleProfileRef::of(
        RoleProfileId::new("backend_engineer_profile_v4")?,
        RoleProfileVersion::new(1)?,
    );
    assert_eq!(folded, split, "one pair written twice is two values");

    // Two different bundles that render the same section 24.2 `id`.
    let lineage_v4 = RoleProfileRef::of(
        RoleProfileId::new("backend_engineer_profile")?,
        RoleProfileVersion::new(4)?,
    );
    let named_v4_first = RoleProfileRef::of(
        RoleProfileId::new("backend_engineer_profile_v4")?,
        RoleProfileVersion::new(1)?,
    );
    assert_ne!(
        lineage_v4, named_v4_first,
        "two bundles collided on their rendered name"
    );
    assert_eq!(
        lineage_v4.rendered(),
        "backend_engineer_profile_v4",
        "section 24.2's spelling is not what is rendered"
    );
    assert_eq!(named_v4_first.rendered(), "backend_engineer_profile_v4_v1");

    // A shelf keyed on the pair holds both.
    let held = BundleShelf::empty()
        .shelve(declare(
            lineage_v4.profile().clone(),
            RoleLabel::new("Backend Engineer")?,
            RoleDirection::Backend,
            day(2026, 8, 26)?,
            BundleScope::new(BundleScope::USER_CURATED_GENERAL)?,
            vec![BundleEntry::of(
                competency("API_ARCHITECTURE")?,
                BundleImportance::Core,
            )],
            vec![source("the user's own reading")?],
        )?)?
        .shelve(declare(
            named_v4_first.profile().clone(),
            RoleLabel::new("Backend Engineer")?,
            RoleDirection::Backend,
            day(2026, 8, 26)?,
            BundleScope::new(BundleScope::USER_CURATED_GENERAL)?,
            vec![BundleEntry::of(
                competency("API_ARCHITECTURE")?,
                BundleImportance::Common,
            )],
            vec![source("a second reading, six months later")?],
        )?)?;
    assert_eq!(held.len(), 2);
    Ok(())
}

/// The version's key and kind come from the predicate registry, both ways.
#[test]
fn the_version_qualifier_is_the_registry_s() -> TestResult {
    let descriptor = PredicateName::RelevantToRole.descriptor();
    let keys: Vec<&str> = descriptor
        .qualifiers
        .iter()
        .map(|schema| schema.key)
        .collect();
    assert_eq!(
        keys,
        vec![VERSION_QUALIFIER],
        "RELEVANT_TO_ROLE's qualifier set moved; section 24.2's importance is not one of them, \
         and a registry that grows one has to be reconciled with BundleImportance"
    );
    let schema = descriptor
        .qualifiers
        .iter()
        .find(|schema| schema.key == VERSION_QUALIFIER)
        .ok_or("the registry no longer carries the version qualifier")?;
    assert!(
        schema.required,
        "the version qualifier stopped being required"
    );
    assert_eq!(schema.kind, QualifierKind::PositiveInteger);
    assert_eq!(
        RoleProfileVersion::registry_kind(),
        Some(QualifierKind::PositiveInteger)
    );
    assert_eq!(RoleProfileVersion::qualifier_key(), VERSION_QUALIFIER);

    // A positive integer, at every door.
    assert_eq!(
        RoleProfileVersion::new(0),
        Err(RoleError::VersionIsNotPositive)
    );
    assert!(serde_json::from_str::<RoleProfileVersion>("0").is_err());
    assert_eq!(RoleProfileVersion::FIRST.get(), 1);
    assert_eq!(
        RoleProfileVersion::new(u32::MAX)?.next(),
        Err(RoleError::VersionWouldOverflow)
    );

    // Section 7.1 and 7.2 name this entity and this edge; both are read from
    // the shared registry rather than declared here.
    assert_eq!(RoleProfile::node_type(), NodeType::RoleProfile);
    assert_eq!(
        RoleProfile::entry_predicate(),
        PredicateName::RelevantToRole
    );
    assert!(
        descriptor.object_types.contains(&NodeType::RoleProfile),
        "RELEVANT_TO_ROLE no longer points at a role profile"
    );
    Ok(())
}

/// Section 24.2's `importance` values, read out of its own YAML block.
#[test]
fn the_importance_vocabulary_is_the_specification_s() -> TestResult {
    let text = specification()?;
    let block = yaml_block(&section(
        &text,
        "### 24.2 Role은 versioned competency bundle",
    )?)?;
    let mut spelled: Vec<String> = Vec::new();
    for line in block.lines() {
        if let Some((key, value)) = line.trim().split_once(": ")
            && key.trim_start_matches("- ") == "importance"
        {
            let value = value.trim().to_owned();
            if !spelled.contains(&value) {
                spelled.push(value);
            }
        }
    }
    let arms: Vec<&str> = BundleImportance::ALL
        .iter()
        .map(|importance| importance.as_str())
        .collect();
    assert_eq!(spelled, arms, "section 24.2's importance vocabulary moved");
    for value in &spelled {
        assert_eq!(
            serde_json::from_value::<BundleImportance>(serde_json::Value::String(value.clone()))?
                .as_str(),
            value
        );
    }
    Ok(())
}

/// The identifier rule is executed, not declared.
#[test]
fn an_identifier_is_classified_byte_by_byte() -> TestResult {
    assert!(RoleProfileId::new("backend_engineer_profile.v4-1").is_ok());
    assert!(RoleProfileId::new("a".repeat(64)).is_ok());
    for refused in [
        String::new(),
        "a".repeat(65),
        "north org".to_owned(),
        "north/org".to_owned(),
        "north\u{0}org".to_owned(),
        "북엔드".to_owned(),
        "north\norg".to_owned(),
        "north+org".to_owned(),
        "north\torg".to_owned(),
        "north:org".to_owned(),
        "\u{7f}".to_owned(),
        "\u{80}".to_owned(),
    ] {
        assert!(
            RoleProfileId::new(refused.clone()).is_err(),
            "{refused:?} was admitted as a role profile identifier"
        );
        assert!(BundleScope::new(refused.clone()).is_err());
        assert!(DirectionName::new(refused.clone()).is_err());
    }

    // Every byte outside the class is refused, not only the ones somebody
    // listed. This walks the whole single-byte range.
    for byte in 0u8..=255 {
        let admitted = RoleProfileId::new(String::from_utf8_lossy(&[byte]).into_owned()).is_ok();
        let legal = byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-');
        assert_eq!(
            admitted,
            legal,
            "byte {byte} was {} and the rule says {}",
            if admitted { "admitted" } else { "refused" },
            if legal { "legal" } else { "illegal" }
        );
    }

    // Prose is not an identifier, and the empty one is still refused.
    assert!(RoleLabel::new("Backend Engineer (North Org)").is_ok());
    assert!(RoleLabel::new("   ").is_err());
    assert!(BundleSource::cited("", day(2026, 8, 20)?).is_err());
    Ok(())
}

/// A document whose origin disagrees with its own version is refused.
#[test]
fn an_origin_that_disagrees_with_its_version_is_refused() -> TestResult {
    let base = backend_bundle()?;
    let document = serde_json::to_value(&base)?;
    let object = document
        .as_object()
        .ok_or("a role profile is not a JSON object")?
        .clone();

    // `AUTHORED` at a version nothing authored.
    let mut authored_late = object.clone();
    authored_late.insert("version".to_owned(), serde_json::json!(9));
    assert!(
        serde_json::from_value::<RoleProfile>(serde_json::Value::Object(authored_late)).is_err()
    );

    // `REVISED` that does not name its own predecessor.
    let mut wrong_predecessor = object.clone();
    wrong_predecessor.insert("version".to_owned(), serde_json::json!(2));
    wrong_predecessor.insert(
        "origin".to_owned(),
        serde_json::json!({
            "kind": "REVISED",
            "from": { "profile": "backend_engineer_profile", "version": 7 },
        }),
    );
    assert!(
        serde_json::from_value::<RoleProfile>(serde_json::Value::Object(wrong_predecessor))
            .is_err()
    );

    // `FORKED` that names its own lineage.
    let mut self_fork = object.clone();
    self_fork.insert(
        "origin".to_owned(),
        serde_json::json!({
            "kind": "FORKED",
            "from": { "profile": "backend_engineer_profile", "version": 1 },
        }),
    );
    assert!(serde_json::from_value::<RoleProfile>(serde_json::Value::Object(self_fork)).is_err());

    // The well-formed revision reads back.
    let mut good = object;
    good.insert("version".to_owned(), serde_json::json!(2));
    good.insert(
        "origin".to_owned(),
        serde_json::json!({
            "kind": "REVISED",
            "from": { "profile": "backend_engineer_profile", "version": 1 },
        }),
    );
    let read: RoleProfile = serde_json::from_value(serde_json::Value::Object(good))?;
    assert_eq!(read.version().get(), 2);
    Ok(())
}

/// The refusals that keep a bundle and a layer from being empty or ambiguous.
#[test]
fn a_bundle_and_a_layer_are_refused_when_they_say_nothing() -> TestResult {
    let id = RoleProfileId::new("backend_engineer_profile")?;
    let empty = declare(
        id.clone(),
        RoleLabel::new("Backend Engineer")?,
        RoleDirection::Backend,
        day(2026, 8, 26)?,
        BundleScope::new(BundleScope::USER_CURATED_GENERAL)?,
        Vec::new(),
        vec![source("the user's own reading")?],
    );
    assert_eq!(
        empty,
        Err(RoleError::BundleNamesNoCompetency(id.as_str().to_owned()))
    );

    let unsourced = declare(
        id.clone(),
        RoleLabel::new("Backend Engineer")?,
        RoleDirection::Backend,
        day(2026, 8, 26)?,
        BundleScope::new(BundleScope::USER_CURATED_GENERAL)?,
        vec![BundleEntry::of(
            competency("API_ARCHITECTURE")?,
            BundleImportance::Core,
        )],
        Vec::new(),
    );
    assert_eq!(
        unsourced,
        Err(RoleError::BundleRecordsNoSource(id.as_str().to_owned()))
    );

    let twice = declare(
        id.clone(),
        RoleLabel::new("Backend Engineer")?,
        RoleDirection::Backend,
        day(2026, 8, 26)?,
        BundleScope::new(BundleScope::USER_CURATED_GENERAL)?,
        vec![
            BundleEntry::of(competency("API_ARCHITECTURE")?, BundleImportance::Core),
            BundleEntry::of(competency("API_ARCHITECTURE")?, BundleImportance::Common),
        ],
        vec![source("the user's own reading")?],
    );
    assert_eq!(
        twice,
        Err(RoleError::DuplicateCompetency {
            profile: id.as_str().to_owned(),
            competency: "API_ARCHITECTURE".to_owned(),
        })
    );

    let base = backend_bundle()?;
    assert_eq!(
        AdjustmentLayer::over(base.reference(), Vec::new()),
        Err(RoleError::LayerAdjustsNothing(base.reference().rendered()))
    );

    let contradictory = AdjustmentLayer::over(
        base.reference(),
        vec![
            UserAdjustment::of(
                Adjustment::Remove {
                    competency: competency("CACHING_TRADEOFFS")?,
                },
                "first",
            )?,
            UserAdjustment::of(
                Adjustment::Reweight {
                    competency: competency("CACHING_TRADEOFFS")?,
                    importance: BundleImportance::Core,
                },
                "second",
            )?,
        ],
    );
    assert_eq!(
        contradictory,
        Err(RoleError::CompetencyAdjustedTwice(
            "CACHING_TRADEOFFS".to_owned()
        ))
    );

    assert!(
        UserAdjustment::of(
            Adjustment::Remove {
                competency: competency("CACHING_TRADEOFFS")?,
            },
            "  ",
        )
        .is_err()
    );

    // An adjustment that disagrees with the base about what is in it.
    let absent = AdjustmentLayer::over(
        base.reference(),
        vec![UserAdjustment::of(
            Adjustment::Remove {
                competency: competency("NOTHING_HERE")?,
            },
            "it is not there",
        )?],
    )?;
    assert_eq!(
        revise(&base, &absent, day(2026, 9, 1)?),
        Err(RoleError::AdjustedCompetencyIsNotInTheBundle {
            profile: base.id().as_str().to_owned(),
            competency: "NOTHING_HERE".to_owned(),
        })
    );
    let present = AdjustmentLayer::over(
        base.reference(),
        vec![UserAdjustment::of(
            Adjustment::Add {
                competency: competency("API_ARCHITECTURE")?,
                importance: BundleImportance::Common,
            },
            "again",
        )?],
    )?;
    assert_eq!(
        revise(&base, &present, day(2026, 9, 1)?),
        Err(RoleError::AddedCompetencyAlreadyPresent {
            profile: base.id().as_str().to_owned(),
            competency: "API_ARCHITECTURE".to_owned(),
        })
    );
    Ok(())
}

/// A new arm cannot be added without reaching the inventory it belongs in.
///
/// The three comparisons above — `RoleDirection::NAMED` against section 24.2's
/// sentence, `BundleImportance::ALL` against its YAML block, and
/// `InterestStanding::ALL` against sections 25.11 and 37 — all read a
/// **constant**. An arm added to one of these enumerations and left out of its
/// constant would be invisible to every one of them: `as_str` and `spec_name`
/// are exhaustive matches in the product code and would force an arm there, but
/// nothing would force an entry in `ALL` or `NAMED`.
///
/// Each block below closes that. The match is exhaustive over a value of the
/// type, so a new arm is a **compile error here**; and the list it is matched
/// against is compared with the constant, so an arm added to the match and not
/// to the constant is a failure. `P2-N5` measured why this is worth a test:
/// two guards there were removed and every test still passed.
#[test]
fn a_new_arm_cannot_be_added_without_reaching_its_inventory() -> TestResult {
    let standings = vec![
        InterestStanding::Favorited,
        InterestStanding::Exploring,
        InterestStanding::Neutral,
    ];
    for standing in &standings {
        match standing {
            InterestStanding::Favorited
            | InterestStanding::Exploring
            | InterestStanding::Neutral => {}
        }
    }
    assert_eq!(
        standings,
        InterestStanding::ALL.to_vec(),
        "a standing is an arm of the enumeration and not an entry of ALL"
    );

    let importances = vec![
        BundleImportance::Core,
        BundleImportance::Common,
        BundleImportance::ContextDependent,
    ];
    for importance in &importances {
        match importance {
            BundleImportance::Core
            | BundleImportance::Common
            | BundleImportance::ContextDependent => {}
        }
    }
    assert_eq!(
        importances,
        BundleImportance::ALL.to_vec(),
        "an importance is an arm of the enumeration and not an entry of ALL"
    );

    let named = vec![
        RoleDirection::Backend,
        RoleDirection::Systems,
        RoleDirection::Database,
        RoleDirection::DistributedSystems,
        RoleDirection::InfrastructurePlatform,
        RoleDirection::Sre,
        RoleDirection::Cloud,
        RoleDirection::Security,
        RoleDirection::MlAi,
        RoleDirection::Data,
        RoleDirection::CompilerPl,
        RoleDirection::Research,
    ];
    for direction in named
        .iter()
        .chain([RoleDirection::UserNamed(DirectionName::new("anything")?)].iter())
    {
        match direction {
            RoleDirection::Backend
            | RoleDirection::Systems
            | RoleDirection::Database
            | RoleDirection::DistributedSystems
            | RoleDirection::InfrastructurePlatform
            | RoleDirection::Sre
            | RoleDirection::Cloud
            | RoleDirection::Security
            | RoleDirection::MlAi
            | RoleDirection::Data
            | RoleDirection::CompilerPl
            | RoleDirection::Research
            | RoleDirection::UserNamed(_) => {}
        }
    }
    assert_eq!(
        named,
        RoleDirection::NAMED.to_vec(),
        "a direction is an arm of the enumeration and not an entry of NAMED"
    );

    // The origins and the adjustments are closed the same way. Neither has a
    // constant listing them, so what the match buys here is that a fourth arm
    // cannot arrive without somebody reading this test's reason for existing.
    for origin in [
        BundleOrigin::Authored,
        BundleOrigin::Revised(backend_bundle()?.reference()),
        BundleOrigin::Forked(backend_bundle()?.reference()),
    ] {
        match origin {
            BundleOrigin::Authored | BundleOrigin::Revised(_) | BundleOrigin::Forked(_) => {}
        }
    }
    for adjustment in [
        Adjustment::Add {
            competency: competency("A")?,
            importance: BundleImportance::Core,
        },
        Adjustment::Remove {
            competency: competency("A")?,
        },
        Adjustment::Reweight {
            competency: competency("A")?,
            importance: BundleImportance::Core,
        },
    ] {
        match adjustment {
            Adjustment::Add { .. } | Adjustment::Remove { .. } | Adjustment::Reweight { .. } => {}
        }
    }
    Ok(())
}

/// Section 24.2's `validAt` is a calendar date and not a clock reading.
#[test]
fn a_date_is_a_calendar_date_in_the_specification_s_spelling() -> TestResult {
    assert_eq!(
        RecordedOn::parse("2026-08-26")?.date(),
        Date::new(2026, 8, 26)?
    );
    assert_eq!(String::from(RecordedOn::parse("2026-08-26")?), "2026-08-26");
    for refused in [
        "2026-02-30",
        "2026-13-01",
        "2026-00-01",
        "2026-8-26",
        "26-08-26",
        "2026-08-26T00:00:00Z",
        "2026-08",
        "",
        "aaaa-bb-cc",
    ] {
        assert!(
            RecordedOn::parse(refused).is_err(),
            "{refused:?} was read as a calendar date"
        );
    }
    assert!(
        RecordedOn::parse("2028-02-29").is_ok(),
        "2028 is a leap year"
    );
    assert!(RecordedOn::parse("2027-02-29").is_err());
    Ok(())
}
