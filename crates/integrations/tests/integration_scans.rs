//! What `academic-integrations` may reach, hold and hand out.
//!
//! `crates/integrations/tests/integrations.rs` carries `P2-P3`'s ten acceptance
//! rows. This file carries the structural claims those rows rest on, and the
//! controls that stop the claims being vacuous:
//!
//! * the walk actually reads every module of this package, so a comparison over
//!   its result is a comparison over the crate rather than over nothing;
//! * `IntegrationSurface::read_core` and `PrivateBlobEgress::bind_disclosure`
//!   are pinned as whole text and their call sites counted, because a
//!   behavioural test says a path exists and a count says there is no second
//!   one -- and neither says what the other does;
//! * the whole set of paths, imports and macros the product source spells is
//!   compared in both directions, so a reach for a clock, a socket, a
//!   filesystem or a competency appears as an extra key;
//! * every public signature is inventoried, so a second entry point of any kind
//!   is an added line;
//! * every extractor is re-run against a sample it must match, because one that
//!   answered the empty set would satisfy every comparison above.

#![allow(clippy::items_after_statements)]

mod support;

use std::{collections::BTreeSet, fs};

use support::{
    TestResult, absolute_paths, all_array, collapse, crate_product_sources, crate_root,
    enum_variants, macros_spelled, product_code, public_enums, public_signatures,
    public_signatures_with_owner, read_module, relative, strip_non_code, use_items, uses_of,
    whole_block, workspace_root,
};

/// Every module of this package, relative to the workspace root.
const REACHED_FILES: [&str; 6] = [
    "crates/integrations/src/assistant.rs",
    "crates/integrations/src/calendar.rs",
    "crates/integrations/src/github.rs",
    "crates/integrations/src/ide.rs",
    "crates/integrations/src/identity.rs",
    "crates/integrations/src/lib.rs",
];

#[test]
fn the_walk_reads_every_module_in_this_package() -> TestResult {
    let found: Vec<String> = crate_product_sources()?
        .iter()
        .map(|path| relative(path))
        .collect();
    assert_eq!(
        found,
        REACHED_FILES
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "the walk and REACHED_FILES disagree"
    );

    // The tripwire: every `mod name;` in the crate root must be a file the walk
    // read. A module moved out of `src` would otherwise leave the walk passing
    // over a smaller crate.
    let root = read_module("lib.rs")?;
    for line in root.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("mod ") else {
            continue;
        };
        let Some(name) = rest.strip_suffix(';') else {
            continue;
        };
        assert!(
            found.contains(&format!("crates/integrations/src/{name}.rs")),
            "{name} is declared but was not read by the walk"
        );
    }
    assert!(
        !root.contains("#[path"),
        "a #[path] attribute would point the compiler at a file this walk does not read"
    );
    Ok(())
}

#[test]
fn the_core_read_consults_no_connector() -> TestResult {
    let root = read_module("lib.rs")?;
    let read_core = whole_block(&root, "pub fn read_core(&self, view: CoreView) -> Vec<u8>")?;
    assert_eq!(
        read_core,
        "pub fn read_core(&self, view: CoreView) -> Vec<u8> { self.core.read_view(view) }",
        "read_core is no longer a forward to the core"
    );

    // The whole set of identifiers its body spells. `fleet` is not among them,
    // and a health check reached through any other spelling would still have to
    // name something that is not on this list.
    let body = read_core
        .split_once('{')
        .map_or("", |(_, tail)| tail)
        .trim_end_matches('}');
    let identifiers: BTreeSet<String> = body
        .split(|character: char| !(character.is_alphanumeric() || character == '_'))
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    assert_eq!(
        identifiers,
        ["self", "core", "read_view", "view"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        "read_core's body spells something other than the forward"
    );

    // And exactly one function reads the fleet at all.
    let readers: Vec<String> = public_signatures_with_owner(&root)
        .into_iter()
        .filter(|(owner, _, _)| owner == "IntegrationSurface")
        .map(|(_, name, _)| name)
        .collect();
    assert_eq!(
        readers,
        vec![
            "new".to_owned(),
            "read_core".to_owned(),
            "connector_health".to_owned()
        ],
        "IntegrationSurface's public method set changed"
    );
    assert_eq!(
        uses_of(&root, "fleet"),
        4,
        "the number of places the fleet is named changed"
    );
    Ok(())
}

#[test]
fn the_disclosure_is_bound_once() -> TestResult {
    let github = read_module("github.rs")?;

    // One declaration and one call site. A second path to the transport that
    // skipped this would be a private blob leaving under one grant, and a count
    // is what says there is no second path. `P2-G2`'s
    // `the_byte_path_has_one_derivation` is the same claim about its own two
    // paths, and this crate has one.
    let total = uses_of(&github, "bind_disclosure");
    assert_eq!(
        total, 2,
        "bind_disclosure has {} mentions rather than one declaration and one call",
        total
    );

    // And the call is the first statement of the one public transmit, so the
    // refusals it makes are made before a byte can reach `P2-G2`.
    let transmit = whole_block(&github, "pub fn transmit<T: OutboundTransport>")?;
    let after_signature = transmit
        .split_once('{')
        .map_or("", |(_, tail)| tail)
        .trim_start();
    assert!(
        after_signature.starts_with("let disclosure_grant_id = self.bind_disclosure("),
        "the disclosure binding is no longer the first statement: {after_signature}"
    );

    // The proxy is reached from exactly one place, so there is one byte path.
    assert_eq!(
        uses_of(&github, "proxy"),
        4,
        "the number of places the egress proxy is named changed"
    );
    assert_eq!(
        uses_of(&github, "transmit"),
        2,
        "the number of transmit mentions changed"
    );
    Ok(())
}

/// Every `academic_*`, `std`, `core` and `crate` path the product source spells.
const REACHED_PATHS: [&str; 3] = ["core::fmt", "std::mem", "std::process"];

#[test]
fn the_crate_reaches_only_the_declared_vocabulary() -> TestResult {
    let mut paths: BTreeSet<String> = BTreeSet::new();
    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut macros: BTreeSet<String> = BTreeSet::new();
    for (_, code) in product_code()? {
        paths.extend(absolute_paths(&code));
        imports.extend(use_items(&code));
        macros.extend(macros_spelled(&code));
    }

    // Whole sets, both directions. A clock, a socket, an environment read or a
    // process spawn is an extra key here rather than a token somebody forbade.
    assert_eq!(
        paths,
        [
            "academic_domain::TimestampMillis",
            "academic_egress_boundary::EgressError",
            "crate::ConnectorKind",
            "crate::identity",
            "std::collections",
            "thiserror::Error",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        "the set of absolute paths this crate spells changed"
    );
    assert_eq!(
        imports,
        [
            "academic_domain::ArtifactId",
            "academic_domain::ContentDigest",
            "academic_domain::CourseId",
            "academic_domain::EntityId",
            "academic_domain::OfferingId",
            "academic_domain::RepositoryId",
            "academic_domain::TimestampMillis",
            "academic_egress_boundary::EgressDenial",
            "academic_egress_boundary::EgressProxy",
            "academic_egress_boundary::IdentifierPolicy",
            "academic_egress_boundary::OutboundTransport",
            "academic_egress_boundary::SourceDocument",
            "academic_egress_boundary::StagedGrantJournal",
            "academic_egress_boundary::StagedPayload",
            "academic_egress_boundary::StagingRequest",
            "academic_egress_boundary::Transmission",
            "academic_egress_boundary::TransmissionPlan",
            "academic_model_run::Digest32",
            "academic_model_run::ModelRun",
            "academic_model_run::ModelRunId",
            "academic_policy::BrokerError",
            "academic_policy::CapabilityToken",
            "academic_policy::PermissionBroker",
            "academic_policy::ReasonCode",
            "academic_policy::RuntimeToolCall",
            "academic_repository::FineGrainedToken",
            "academic_repository::GitHubError",
            "academic_repository::GitHubRepository",
            "academic_repository::TokenPermission",
            "academic_untrusted_content::IngestError",
            "academic_untrusted_content::IngestedDocument",
            "academic_untrusted_content::SourceId",
            "academic_untrusted_content::SourceKind",
            "academic_untrusted_content::Untrusted",
            "academic_untrusted_content::ingest",
            "crate::ConnectorKind",
            "crate::identity::CanonicalRef",
            "crate::identity::ExternalId",
            "sha2::Digest as _",
            "sha2::Sha256",
            "std::collections::BTreeMap",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>(),
        "the set of names this crate imports changed"
    );
    assert_eq!(
        macros,
        ["format", "matches", "vec"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        "the set of macros this crate invokes changed"
    );

    // The named list above is a claim about reach; this is the claim that a
    // reach the list does not carry is impossible to spell without appearing in
    // it. `std::process` in `REACHED_PATHS` is a sentinel the sample below
    // exercises, not a path this crate takes.
    for forbidden in [
        "std::time",
        "std::net",
        "std::fs",
        "std::env",
        "std::process",
        "std::io",
        "std::thread",
    ] {
        assert!(
            !paths.iter().any(|path| path.starts_with(forbidden)),
            "this crate reaches {forbidden}"
        );
        assert!(
            !imports.iter().any(|item| item.starts_with(forbidden)),
            "this crate imports {forbidden}"
        );
    }
    Ok(())
}

/// Every public function this crate declares, as `module name -> return type`.
const PUBLIC_SIGNATURES: [&str; 120] = [
    "assistant.rs as_str -> &'static str",
    "assistant.rs as_str -> &'static str",
    "assistant.rs context_digest -> Digest32",
    "assistant.rs eligibility -> EvidenceEligibility",
    "assistant.rs eligibility -> EvidenceEligibility",
    "assistant.rs minimize -> Result<Self, EgressDenial>",
    "assistant.rs model_run -> ModelRunId",
    "assistant.rs new -> Result<Self, AssistantError>",
    "assistant.rs output_digest -> Digest32",
    "assistant.rs produced_at -> TimestampMillis",
    "assistant.rs record -> Self",
    "assistant.rs run_digest -> Digest32",
    "assistant.rs selection -> &AssistantSelection",
    "assistant.rs staged -> &StagedPayload",
    "assistant.rs symbols -> &[String]",
    "assistant.rs use_kind -> AssistantUse",
    "calendar.rs as_str -> &'static str",
    "calendar.rs digest -> [u8; 32]",
    "calendar.rs encode -> Vec<u8>",
    "calendar.rs ends_at -> TimestampMillis",
    "calendar.rs event_id -> &ExternalId",
    "calendar.rs kind -> CalendarEventKind",
    "calendar.rs new -> Result<Self, CalendarError>",
    "calendar.rs starts_at -> TimestampMillis",
    "calendar.rs subject -> CanonicalRef",
    "calendar.rs summary -> &'static str",
    "calendar.rs summary -> &'static str",
    "github.rs accept -> Result<Self, ConnectorError>",
    "github.rs as_str -> &'static str",
    "github.rs as_str -> &'static str",
    "github.rs as_str -> &'static str",
    "github.rs as_str -> &'static str",
    "github.rs body -> &Untrusted<IngestedDocument>",
    "github.rs bytes_transmitted -> usize",
    "github.rs delivery_id -> &SourceId",
    "github.rs detail -> &str",
    "github.rs disclosure_grant_id -> Option<&str>",
    "github.rs kind -> WebhookEventKind",
    "github.rs method -> HttpMethod",
    "github.rs method -> HttpMethod",
    "github.rs new -> Self",
    "github.rs new -> Self",
    "github.rs new -> Self",
    "github.rs operation -> GitHubOperation",
    "github.rs path -> &str",
    "github.rs path -> &str",
    "github.rs permission -> TokenPermission",
    "github.rs read -> Result<ReadRequest, ConnectorError>",
    "github.rs reason -> Option<ReasonCode>",
    "github.rs reason -> ReasonCode",
    "github.rs repository -> &GitHubRepository",
    "github.rs repository -> &GitHubRepository",
    "github.rs repository -> &GitHubRepository",
    "github.rs required_grants -> u8",
    "github.rs resource -> &'static str",
    "github.rs source_kind -> SourceKind",
    "github.rs transmission -> &Transmission",
    "github.rs transmit -> Result<BlobTransfer, ConnectorError>",
    "github.rs visibility -> BlobVisibility",
    "ide.rs actor_id -> &str",
    "ide.rs as_str -> &'static str",
    "ide.rs as_str -> &str",
    "ide.rs as_str -> &str",
    "ide.rs at -> TimestampMillis",
    "ide.rs attach -> Self",
    "ide.rs changed_scope -> Result<ChangedScope, IdeError>",
    "ide.rs confirmed_at -> TimestampMillis",
    "ide.rs confirmed_by -> &str",
    "ide.rs deep_link -> DeepLink",
    "ide.rs digest -> ContentDigest",
    "ide.rs end -> u32",
    "ide.rs name -> &str",
    "ide.rs new -> Result<Self, IdeError>",
    "ide.rs new -> Result<Self, IdeError>",
    "ide.rs new -> Self",
    "ide.rs open_paths -> Vec<WorkspacePath>",
    "ide.rs path -> &WorkspacePath",
    "ide.rs paths -> &[WorkspacePath]",
    "ide.rs record -> Self",
    "ide.rs request_snapshot -> Result<SnapshotRequest, IdeError>",
    "ide.rs scope -> &ChangedScope",
    "ide.rs scope_digest -> ContentDigest",
    "ide.rs since -> TimestampMillis",
    "ide.rs start -> u32",
    "ide.rs symbols -> Vec<SymbolRef>",
    "ide.rs watch -> WatchMode",
    "ide.rs with_watch -> Self",
    "identity.rs as_bytes -> &[u8; 16]",
    "identity.rs as_str -> &'static str",
    "identity.rs as_str -> &'static str",
    "identity.rs as_str -> &'static str",
    "identity.rs as_str -> &str",
    "identity.rs authority -> SourceAuthority",
    "identity.rs basis -> ConflictBasis",
    "identity.rs canonical -> CanonicalRef",
    "identity.rs conflicts -> &[SyncConflict]",
    "identity.rs external_id -> &ExternalId",
    "identity.rs held -> &ExternalIdentity",
    "identity.rs incoming -> &ExternalIdentity",
    "identity.rs kind -> CanonicalKind",
    "identity.rs map -> Self",
    "identity.rs mappings -> Vec<&ExternalIdentity>",
    "identity.rs mappings_for -> Vec<&ExternalIdentity>",
    "identity.rs new -> Result<Self, IdentityError>",
    "identity.rs new -> Self",
    "identity.rs preferred -> Option<&ExternalIdentity>",
    "identity.rs register -> Option<&SyncConflict>",
    "identity.rs resolve -> Option<&ExternalIdentity>",
    "identity.rs system -> ConnectorKind",
    "identity.rs valid_from -> TimestampMillis",
    "lib.rs all_down -> Self",
    "lib.rs all_up -> Self",
    "lib.rs as_str -> &'static str",
    "lib.rs as_str -> &'static str",
    "lib.rs as_str -> &'static str",
    "lib.rs connector_health -> ConnectorHealth",
    "lib.rs new -> Self",
    "lib.rs read_core -> Vec<u8>",
    "lib.rs unreachable -> Vec<ConnectorKind>",
    "lib.rs with -> Self",
];

#[test]
fn every_public_signature_is_in_the_inventory() -> TestResult {
    let mut found: Vec<String> = Vec::new();
    for (path, code) in product_code()? {
        let module = support::module_of(&path);
        for (name, signature) in public_signatures(&code) {
            // The *last* arrow: a `&dyn Fn() -> u64` parameter carries one
            // too, and reading the first turned `transmit`'s entry into half a
            // parameter list.
            let tail = signature
                .rsplit_once("->")
                .map_or("()", |(_, rest)| rest)
                .trim();
            let tail = tail.split_whitespace().collect::<Vec<_>>().join(" ");
            found.push(format!("{module} {name} -> {tail}"));
        }
    }
    found.sort();
    let mut declared: Vec<String> = PUBLIC_SIGNATURES
        .iter()
        .map(|item| (*item).to_owned())
        .collect();
    declared.sort();
    assert_eq!(
        found, declared,
        "this crate's public signatures and PUBLIC_SIGNATURES disagree"
    );
    assert!(
        found.len() >= 80,
        "the signature reader found only {} public functions",
        found.len()
    );

    // The control: the same reader is required to see a return type it is being
    // asked to notice, so an extractor that always answered `()` would not pass.
    assert!(
        found.iter().any(|entry| entry.ends_with("-> u8")),
        "the reader reports no numeric return type at all"
    );
    Ok(())
}

/// Every `pub enum` in this crate that declares a `pub const ALL`, with the
/// variant list its `ALL` names.
///
/// The comparison is against the variant list read out of the **enum body**, so
/// a variant added without an `ALL` entry fails here rather than passing every
/// walk in the suite that iterates the array. `P2-N6` measured what an
/// incomplete frozen input costs: an engine that is not a function of the
/// inputs it declares. An `ALL` that is not the enum is the same shape.
///
/// The set of enums *with* an `ALL` is pinned too, so removing an `ALL` to
/// escape the comparison is an extra key rather than a silent exemption.
const VOCABULARIES_WITH_ALL: [&str; 14] = [
    "AssistantUse",
    "BlobVisibility",
    "CalendarEventKind",
    "CanonicalKind",
    "ConflictBasis",
    "ConnectorHealth",
    "ConnectorKind",
    "CoreView",
    "EvidenceEligibility",
    "GitHubOperation",
    "HttpMethod",
    "SourceAuthority",
    "WatchMode",
    "WebhookEventKind",
];

#[test]
fn every_all_array_is_the_enum_it_names() -> TestResult {
    let mut declared: Vec<String> = Vec::new();
    for (path, code) in product_code()? {
        for name in public_enums(&code) {
            let Some(entries) = all_array(&code, &name) else {
                continue;
            };
            let variants = enum_variants(&code, &format!("pub enum {name}"))?;
            assert_eq!(
                entries, variants,
                "{path}: {name}::ALL and {name}'s variants disagree"
            );
            declared.push(name);
        }
    }
    declared.sort();
    assert_eq!(
        declared,
        VOCABULARIES_WITH_ALL
            .iter()
            .map(|item| (*item).to_owned())
            .collect::<Vec<_>>(),
        "the set of vocabularies declaring an ALL changed"
    );

    // The control: the reader is required to see a disagreement it is being
    // asked to notice, so an extractor that always answered the empty list
    // would not pass.
    let sample = concat!(
        "pub enum Sample {\n",
        "    One,\n",
        "    Two,\n",
        "}\n",
        "\n",
        "impl Sample {\n",
        "    pub const ALL: [Self; 1] = [Self::One];\n",
        "}\n",
    );
    assert_eq!(
        all_array(sample, "Sample"),
        Some(vec!["One".to_owned()]),
        "the ALL reader did not read the array"
    );
    assert_eq!(
        enum_variants(sample, "pub enum Sample")?,
        vec!["One".to_owned(), "Two".to_owned()],
        "the variant reader did not read the enum"
    );
    assert_ne!(
        all_array(sample, "Sample"),
        Some(enum_variants(sample, "pub enum Sample")?),
        "the two readers agree on a sample where they must not"
    );
    assert_eq!(all_array(sample, "Absent"), None);

    // And the reader must not reach past the block it was pointed at. `Bare`
    // declares no `ALL`; an unbounded reader reported the next type's, which is
    // how this scan first failed on `ConnectorError`.
    let neighbours = concat!(
        "pub enum Bare {\n",
        "    Only,\n",
        "}\n",
        "\n",
        "impl Bare {\n",
        "    pub const fn as_str(self) -> &'static str {\n",
        "        ONLY\n",
        "    }\n",
        "}\n",
        "\n",
        "pub enum Beside {\n",
        "    First,\n",
        "}\n",
        "\n",
        "impl Beside {\n",
        "    pub const ALL: [Self; 1] = [Self::First];\n",
        "}\n",
    );
    assert_eq!(all_array(neighbours, "Bare"), None);
    assert_eq!(
        all_array(neighbours, "Beside"),
        Some(vec!["First".to_owned()])
    );
    Ok(())
}

#[test]
fn the_helpers_are_not_vacuous() -> TestResult {
    // One sample exercising every extractor this suite depends on. Each
    // assertion below is a shape a real module has, so an extractor that
    // returned the empty set fails here rather than passing everything above.
    let sample = concat!(
        "//! A comment naming std::time::SystemTime, which is not a reach.\n",
        "use std::process::abort;\n",
        "use academic_domain::{ContentDigest, TimestampMillis};\n",
        "\n",
        "/// A doc comment.\n",
        "pub struct Sample {\n",
        "    first: ContentDigest,\n",
        "    second: TimestampMillis,\n",
        "}\n",
        "\n",
        "pub enum Kind {\n",
        "    One,\n",
        "    Two(u32),\n",
        "}\n",
        "\n",
        "impl<'a, T: Clone> Sample {\n",
        "    pub fn make(value: &'a T) -> Self {\n",
        "        let _ = format!(\"std::net::TcpStream in a literal is not a reach\");\n",
        "        std::mem::drop(abort);\n",
        "        Self { first: ContentDigest::sha256(b\"\"), second: TimestampMillis::new(0) }\n",
        "    }\n",
        "}\n",
    );
    let stripped = strip_non_code(sample);
    assert!(
        !stripped.contains("SystemTime"),
        "the comment stripper left a comment behind"
    );
    assert!(
        !stripped.contains("TcpStream"),
        "the literal stripper left a string literal behind"
    );
    assert_eq!(
        absolute_paths(&stripped),
        ["std::mem", "std::process"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>(),
        "the path reader missed a reach or invented one"
    );
    assert_eq!(
        use_items(&stripped),
        vec![
            "academic_domain::ContentDigest".to_owned(),
            "academic_domain::TimestampMillis".to_owned(),
            "std::process::abort".to_owned(),
        ],
        "the import reader did not expand the braced group"
    );
    assert_eq!(
        macros_spelled(&stripped),
        ["format"]
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(
        support::struct_fields(&stripped, "pub struct Sample")?,
        vec![
            ("first".to_owned(), "ContentDigest".to_owned()),
            ("second".to_owned(), "TimestampMillis".to_owned()),
        ]
    );
    assert_eq!(
        support::enum_variants(&stripped, "pub enum Kind")?,
        vec!["One".to_owned(), "Two".to_owned()]
    );
    let owners: Vec<(String, String)> = public_signatures_with_owner(&stripped)
        .into_iter()
        .map(|(owner, name, _)| (owner, name))
        .collect();
    assert_eq!(
        owners,
        vec![("Sample".to_owned(), "make".to_owned())],
        "the owner reader did not see through the generic impl header"
    );
    assert_eq!(uses_of(&stripped, "Sample"), 2);
    assert_eq!(
        uses_of("sample_prefixed Sample", "Sample"),
        1,
        "the identifier counter matched inside a longer identifier"
    );
    assert_eq!(
        collapse("a\n// dropped\n   b"),
        "a b",
        "collapse did not drop a comment line"
    );
    // `REACHED_PATHS` is the sentinel list the sample above is written against.
    assert!(REACHED_PATHS.contains(&"std::process"));
    Ok(())
}

#[test]
fn this_scan_is_in_the_inventory() -> TestResult {
    let page = fs::read_to_string(workspace_root().join("docs/contracts/policy-source-scans.md"))?;
    let mut declared: Vec<String> = Vec::new();
    for file in [
        "tests/integration_scans.rs",
        "tests/integrations.rs",
        "tests/support/mod.rs",
    ] {
        assert!(
            page.contains(&format!("crates/integrations/{file}")),
            "the inventory has no row naming crates/integrations/{file}"
        );
        let source = fs::read_to_string(crate_root().join(file))?;
        let mut previous_is_test = false;
        for line in source.lines() {
            let trimmed = line.trim();
            if previous_is_test
                && let Some(rest) = trimmed.strip_prefix("fn ")
                && let Some(name) = rest.split('(').next()
            {
                declared.push(name.to_owned());
            }
            previous_is_test = trimmed == "#[test]";
        }
    }
    declared.sort();
    assert!(
        declared.len() >= 20,
        "the scan reader found only {} tests across this crate's suites",
        declared.len()
    );
    for name in &declared {
        assert!(
            page.contains(name),
            "the inventory has no row naming {name}"
        );
    }
    Ok(())
}
