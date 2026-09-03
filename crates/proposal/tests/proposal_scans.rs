//! Source scans for the `P2-M2` proposal boundary.
//!
//! What this crate claims is partly a shape of the source rather than a
//! behaviour: the payload comes out at three named places, the tier comparison
//! runs at every door, and the boundary implements nothing that would take the
//! label off. None of those has a run-time observation that would notice the
//! day it stops being true, which is what
//! `docs/contracts/policy-source-scans.md` says a policy source scan is for.
//!
//! That page names three shapes that make a scan empty and two more about what
//! a scan concludes. This file is written against all five.
//!
//! **The walk does not stop short.** [`crate_all_sources`] descends into every
//! subdirectory of the whole package rather than into `src` by name -- `S-12`
//! on that page is the row about a walk that reads `<crate>/src` and stops
//! seeing a target whose `path` is outside it. There is a floor under it, a
//! tripwire requiring every `mod name;` and every `#[path = "…"]` target in the
//! crate to be a file the walk read, and a rule that this crate's product
//! source is under `src` and nowhere else.
//!
//! **The checks are not token lists.** The release inventory compares the whole
//! set of call sites of the one crate-private accessor against three written
//! reasons; the trait rule compares the crate's whole set of `impl` blocks
//! naming `Proposed<` against a pinned list, so an implementation nobody
//! predicted fails as an extra key; and the door rule compares the whole public
//! settlement surface of the queue against a table naming the workflow each
//! door serves.
//!
//! **The pins fix their callers too.** [`WHOLE_REQUIRE`] pins the one place a
//! tier is compared against a workflow, and pinning it alone would say nothing
//! about whether it runs -- which is the `T141` hole. So [`DOOR_GUARDS`] pins
//! the first statement of every door beside it, and the call-site count holds
//! the number of places that comparison is reached.
//!
//! **Every inventory counts a name, not a spelling.** `uses_of` reads whole
//! identifiers, so `Proposed::release(taken)` is the same call as
//! `taken.release()`, and `declarations_of` requires a `(` or `<` after the
//! name so `fn release_now(` is not read as a declaration of `release`.
//! `P2-RF10` and `P2-RF11` repaired both holes in the untrusted-content
//! inventory and this file is written to the repaired shape.

use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

/// The crate root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` file anywhere under this crate's package directory.
///
/// The package directory rather than `src`, for the `S-12` reason
/// `crates/untrusted-content/tests/trust_scans.rs` gives: a walk that reads
/// `<crate>/src` and nothing else stops seeing an `examples/`, a `benches/`, or
/// a `[[bin]]` whose `path` is outside it. Injection `M2-I3` is the observation
/// that this walk does not -- a `#[path = "../extra/side.rs"]` module holding a
/// release site is refused by the inventory below.
fn crate_all_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut found = Vec::new();
    walk(&crate_root(), &mut found)?;
    found.sort();
    Ok(found)
}

/// Every `.rs` file that ships, which is every one outside `tests`.
fn crate_product_sources() -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let root = crate_root();
    Ok(crate_all_sources()?
        .into_iter()
        .filter(|path| {
            let relative = path.strip_prefix(&root).unwrap_or(path);
            !relative.starts_with("tests")
        })
        .collect())
}

fn walk(directory: &Path, found: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            walk(&path, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Removes comments, string literals, and character literals.
///
/// Copied from `crates/untrusted-content/tests/trust_scans.rs`, which copied it
/// from `crates/record/tests/record_scans.rs`, where this repository's Rust-side
/// stripper lives -- raw strings and nested block comments included. `P2-G4`
/// found that a lexer without raw strings desynchronizes and reads every
/// literal after one as code, so the copy is deliberate rather than a
/// simplification.
fn strip_non_code(source: &str) -> String {
    let bytes: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut index = 0;
    while index < bytes.len() {
        let current = bytes[index];
        let next = bytes.get(index + 1).copied();

        if current == '/' && next == Some('/') {
            while index < bytes.len() && bytes[index] != '\n' {
                index += 1;
            }
            out.push('\n');
            continue;
        }
        if current == '/' && next == Some('*') {
            let mut depth = 1_usize;
            index += 2;
            while index < bytes.len() && depth > 0 {
                if bytes[index] == '/' && bytes.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if bytes[index] == '*' && bytes.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            out.push(' ');
            continue;
        }
        if current == 'r' && matches!(next, Some('"') | Some('#')) {
            let mut hashes = 0_usize;
            let mut cursor = index + 1;
            while bytes.get(cursor) == Some(&'#') {
                hashes += 1;
                cursor += 1;
            }
            if bytes.get(cursor) == Some(&'"') {
                cursor += 1;
                let closing: String = core::iter::once('"')
                    .chain(core::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = bytes[cursor..].iter().collect();
                if let Some(end) = rest.find(&closing) {
                    index = cursor + rest[..end].chars().count() + closing.chars().count();
                    out.push(' ');
                    continue;
                }
            }
        }
        if current == '"' {
            index += 1;
            while index < bytes.len() {
                if bytes[index] == '\\' {
                    index += 2;
                    continue;
                }
                if bytes[index] == '"' {
                    index += 1;
                    break;
                }
                index += 1;
            }
            out.push(' ');
            continue;
        }
        if current == '\'' {
            let closes = if bytes.get(index + 1) == Some(&'\\') {
                bytes[index + 2..]
                    .iter()
                    .position(|character| *character == '\'')
                    .map(|offset| index + 2 + offset)
            } else {
                (bytes.get(index + 2) == Some(&'\'')).then_some(index + 2)
            };
            if let Some(end) = closes {
                index = end + 1;
                out.push(' ');
                continue;
            }
        }
        out.push(current);
        index += 1;
    }
    out
}

/// Extracts one item's text, comment lines dropped and whitespace collapsed.
fn declared_item(source: &str, signature: &str) -> Result<String, Box<dyn Error>> {
    let start = source
        .find(signature)
        .ok_or_else(|| format!("{signature} is not in the source"))?;
    let end = source[start..]
        .find("\n    }")
        .ok_or_else(|| format!("{signature} has no closing brace at method indentation"))?;
    let body = &source[start..start + end + 6];
    let kept: Vec<&str> = body
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect();
    Ok(kept
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

/// Counts whole-identifier occurrences of `name` in already-stripped code.
///
/// `T146` reached a fourth exposure site in `academic-untrusted-content` by
/// writing the call through the type path instead of on the receiver, which an
/// inventory counting the spelling `.expose()` did not see. A name has no such
/// freedom: the call has to spell it, whether it is written as a method,
/// through the type path, or taken as a function value. Injection `M2-I1` is
/// that same shape applied here.
fn uses_of(code: &str, name: &str) -> usize {
    let bytes = code.as_bytes();
    code.match_indices(name)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + name.len()).copied().unwrap_or(b' ');
            before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
        })
        .count()
}

/// Counts declarations of a function whose name is exactly `name`.
///
/// `T149` walked past a version that subtracted the *spelling* `fn expose`,
/// which counts `pub fn expose_rendered(` and lets one function cancel its own
/// call. What follows the name has to open a parameter list or a generic list
/// and nothing else, so `fn release_now(` is not `release`. Injection `M2-I2`
/// is that shape applied here.
fn declarations_of(code: &str, name: &str) -> usize {
    let needle = format!("fn {name}");
    let bytes = code.as_bytes();
    code.match_indices(&needle)
        .filter(|(at, _)| {
            let before_ok =
                *at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
            let after = bytes.get(at + needle.len()).copied().unwrap_or(b' ');
            before_ok && (after == b'(' || after == b'<')
        })
        .count()
}

/// The use count of `name` less its declarations, which cannot go negative.
fn calls_of(code: &str, name: &str) -> usize {
    let uses = uses_of(code, name);
    let declarations = declarations_of(code, name);
    assert!(
        uses >= declarations,
        "{name} is declared {declarations} times and named {uses}; the two counts disagree"
    );
    uses - declarations
}

/// Drops every `use` item, so a re-export is not counted as a caller.
fn without_use_items(code: &str) -> String {
    let mut kept = String::with_capacity(code.len());
    let mut inside = false;
    for line in code.lines() {
        let trimmed = line.trim_start();
        let opens = trimmed.starts_with("use ")
            || (trimmed.starts_with("pub") && trimmed.contains(" use "));
        if inside || opens {
            inside = !line.trim_end().ends_with(';');
            continue;
        }
        kept.push_str(line);
        kept.push('\n');
    }
    kept
}

fn code_of(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(without_use_items(&strip_non_code(&fs::read_to_string(
        path,
    )?)))
}

fn relative(path: &Path) -> String {
    path.strip_prefix(crate_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

#[test]
fn the_walk_reads_every_module_in_this_crate() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every assertion the
    // rest of this file makes over its result.
    assert!(
        sources.len() >= 8,
        "the walk found only {} files under the package",
        sources.len()
    );

    // Product source lives under `src` and nowhere else. That is what makes the
    // per-file rules below cover everything that ships, and it is the condition
    // `S-12` says a crate has to keep if it does not want to widen every scan
    // that reads it.
    let root = crate_root();
    let outside: Vec<String> = crate_product_sources()?
        .iter()
        .filter(|path| !path.strip_prefix(&root).unwrap_or(path).starts_with("src"))
        .map(|path| relative(path))
        .collect();
    assert_eq!(
        outside,
        Vec::<String>::new(),
        "this crate has product source outside src; every scan that reads it has to widen"
    );

    // A module is either `<name>.rs` or `<name>/mod.rs`, so both spellings are
    // collected: a tripwire that only knew the first would fire on every
    // directory module and be turned off rather than fixed.
    let mut read: BTreeSet<String> = BTreeSet::new();
    for path in &sources {
        if let Some(stem) = path.file_stem() {
            let stem = stem.to_string_lossy().into_owned();
            if stem == "mod" {
                if let Some(parent) = path.parent().and_then(Path::file_name) {
                    read.insert(parent.to_string_lossy().into_owned());
                }
            } else {
                read.insert(stem);
            }
        }
    }

    // The tripwire. Every `mod name;` and every `#[path = "…"]` in the crate has
    // to name a file the walk read. It fails the day the walk is narrowed, and
    // the day a module is added somewhere the walk does not descend into.
    let mut declared = 0_usize;
    for path in &sources {
        let source = fs::read_to_string(path)?;
        for line in source.lines() {
            let trimmed = line.trim();
            if let Some(name) = trimmed
                .strip_prefix("pub mod ")
                .or_else(|| trimmed.strip_prefix("mod "))
                .and_then(|rest| rest.strip_suffix(';'))
            {
                declared += 1;
                assert!(
                    read.contains(name),
                    "`{name}` is declared in {} and the walk never read it",
                    relative(path)
                );
            }
            if let Some(rest) = trimmed.strip_prefix("#[path = \"") {
                let target = rest.split('"').next().unwrap_or_default();
                let resolved = path
                    .parent()
                    .map_or_else(|| PathBuf::from(target), |parent| parent.join(target));
                assert!(
                    sources.iter().any(|read_path| read_path == &resolved),
                    "{} includes {target}, which the walk never read",
                    relative(path)
                );
            }
        }
    }
    assert!(declared >= 5, "the crate declares only {declared} modules");
    Ok(())
}

// ---------------------------------------------------------------------------
// The release inventory
// ---------------------------------------------------------------------------

/// The three places the payload comes out of `Proposed<T>`, and why each is
/// allowed.
///
/// Compared against the whole inventory of the accessor's call sites, counted
/// by name. A fourth fails as an extra key however it is spelled; a removed one
/// fails as a missing key.
const RELEASE_SITES: [(&str, &str); 3] = [
    (
        "ReviewQueue::autosave",
        "section 27.4's low-risk row saves without a human, and what leaves is an \
         Autosaved whose epistemic status is a constant equal to AI_INFERRED",
    ),
    (
        "ReviewQueue::approve",
        "the high-risk row needs an explicit approval, and the approval that named \
         this exact proposal is already in the history when the value leaves",
    ),
    (
        "ReviewQueue::commit",
        "the two queued rows release only after a user CONFIRM for this exact \
         proposal is in the history, which the call checks immediately above",
    ),
];

#[test]
fn every_release_site_is_named_and_justified() -> TestResult {
    // The accessor is declared once, and this is where that is established: an
    // inventory over an accessor that had grown a twin would be counting one of
    // two ways out.
    let boundary = code_of(&crate_root().join("src/proposed.rs"))?;
    assert_eq!(
        declarations_of(&boundary, "release"),
        1,
        "the payload accessor is declared more than once"
    );

    let mut found: BTreeMap<String, usize> = BTreeMap::new();
    let mut total = 0_usize;
    for path in crate_product_sources()? {
        let code = code_of(&path)?;
        let calls = calls_of(&code, "release");
        if calls == 0 {
            continue;
        }
        total += calls;
        // Attribute each call to the function it sits in, so the inventory is a
        // list of sites rather than a list of files.
        let mut enclosing = String::from("<crate root>");
        for line in code.lines() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("pub fn ").or_else(|| {
                trimmed
                    .strip_prefix("fn ")
                    .or_else(|| trimmed.strip_prefix("pub(crate) fn "))
            }) {
                let name = rest.split(['(', '<']).next().unwrap_or_default();
                enclosing = format!("ReviewQueue::{name}");
            }
            if uses_of(line, "release") > 0 && !line.contains("fn release") {
                *found.entry(enclosing.clone()).or_default() += 1;
            }
        }
    }

    let named: BTreeSet<&str> = RELEASE_SITES.iter().map(|(site, _)| *site).collect();
    let observed: BTreeSet<&str> = found.keys().map(String::as_str).collect();
    assert_eq!(
        observed, named,
        "the inventory of payload release sites is not the three named ones"
    );
    assert_eq!(
        total,
        RELEASE_SITES.len(),
        "the accessor is called {total} times and {} sites are named",
        RELEASE_SITES.len()
    );
    for (_, reason) in RELEASE_SITES {
        assert!(reason.len() > 40, "a release site has no written reason");
    }
    Ok(())
}

#[test]
fn no_public_signature_hands_out_a_proposed_payload() -> TestResult {
    // The complement of the inventory above. A `pub fn` that takes a
    // `Proposed<T>` and returns a `T` would be a fourth way out that calls the
    // accessor once inside a function the inventory would then have to name --
    // this refuses the shape rather than waiting for the count to move.
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for signature in public_signatures(&code) {
            let Some((parameters, returns)) = parameters_and_return(&signature) else {
                continue;
            };
            if !parameters.contains("Proposed<") {
                continue;
            }
            assert!(
                !returns.trim().ends_with("T") && !returns.contains("-> T"),
                "{}: `{signature}` takes a Proposed and returns its payload",
                relative(&path)
            );
        }
    }
    Ok(())
}

/// Every `pub` function signature in `code`, whitespace-collapsed.
fn public_signatures(code: &str) -> Vec<String> {
    let lines: Vec<&str> = code.lines().collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if !line.trim_start().starts_with("pub fn ")
            && !line.trim_start().starts_with("pub const fn ")
        {
            continue;
        }
        let mut signature = String::new();
        for candidate in &lines[index..] {
            signature.push(' ');
            signature.push_str(candidate.trim());
            if candidate.contains('{') || candidate.trim_end().ends_with(';') {
                break;
            }
        }
        found.push(signature.split_whitespace().collect::<Vec<_>>().join(" "));
    }
    found
}

/// Splits a signature into its parameter list and its return type.
fn parameters_and_return(signature: &str) -> Option<(&str, &str)> {
    let open = signature.find('(')?;
    let close = signature.rfind(')')?;
    if close <= open {
        return None;
    }
    Some((&signature[open + 1..close], &signature[close + 1..]))
}

// ---------------------------------------------------------------------------
// The traits the boundary does not implement
// ---------------------------------------------------------------------------

/// The whole set of `impl` block headers in this crate that name `Proposed<`.
///
/// A written list rather than a search for forbidden traits: an implementation
/// of something nobody predicted fails as an extra key, which a list of
/// forbidden spellings cannot do. The `compile_fail` case
/// `proposed_has_no_unwrapping_trait` is the behaviour half; this is the half
/// that notices an implementation of a trait that case does not name.
const PROPOSED_IMPL_HEADERS: [&str; 2] = [
    "impl<T> Proposed<T> {",
    "impl<T> fmt::Debug for Proposed<T> {",
];

#[test]
fn the_boundary_has_no_unwrapping_trait_impl() -> TestResult {
    let mut found: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl") && trimmed.contains("Proposed<") {
                found.push(trimmed.to_owned());
            }
        }
    }
    found.sort();
    let mut expected = PROPOSED_IMPL_HEADERS.map(str::to_owned).to_vec();
    expected.sort();
    assert_eq!(
        found, expected,
        "the set of impl blocks naming Proposed< is not the pinned pair"
    );

    // The orphan rule refuses the same implementation written in another crate,
    // because both the trait and the type would be foreign there. That is the
    // one half nothing in this repository needs to check.
    Ok(())
}

// ---------------------------------------------------------------------------
// The workflow comparison, and the doors that reach it
// ---------------------------------------------------------------------------

/// The one place a tier is compared against the workflow a caller reached for.
const WHOLE_REQUIRE: &str = "fn require(&self, id: ProposalId, attempted: Workflow) -> Result<&Entry<T>, WorkflowError> { let entry = self .entries .get(&id) .ok_or(WorkflowError::NoSuchProposal(id))?; let required = entry.tier.workflow(); if required == attempted { Ok(entry) } else { Err(WorkflowError::WrongWorkflow { tier: entry.tier, required, attempted, }) } }";

/// Every door, the workflow it serves, and the first statement that enforces it.
///
/// `T141` left a pinned check byte-identical and wrapped the *call* to it in a
/// condition, so verification was skipped whenever a marker file existed. A pin
/// on the comparison alone would carry that hole, so the first statement of
/// each door is pinned beside it: a door that stopped calling the comparison,
/// or that called it behind an `if`, fails here.
const DOOR_GUARDS: [(&str, &str, &str); 4] = [
    (
        "autosave",
        "AutosaveAsAiInferred",
        "self.require(id, Workflow::AutosaveAsAiInferred)?;",
    ),
    (
        "review",
        "QueueAndUndo",
        "self.require(id, Workflow::QueueAndUndo)?;",
    ),
    (
        "approve",
        "ExplicitApproval",
        "self.require(id, Workflow::ExplicitApproval)?;",
    ),
    (
        "decide",
        "UserOnly",
        "self.require(id, Workflow::UserOnly)?;",
    ),
];

#[test]
fn every_door_reaches_the_workflow_comparison() -> TestResult {
    let queue = fs::read_to_string(crate_root().join("src/queue.rs"))?;
    let pinned = declared_item(&queue, "fn require(&self")?;
    assert_eq!(
        pinned, WHOLE_REQUIRE,
        "the workflow comparison is not the pinned text"
    );

    let code = code_of(&crate_root().join("src/queue.rs"))?;
    // The call-site count. Four doors, one call each. A fifth call is a door
    // this file has not named; a fourth would mean one door stopped comparing.
    assert_eq!(
        calls_of(&code, "require"),
        DOOR_GUARDS.len(),
        "the workflow comparison is reached from a number of places other than the four doors"
    );

    for (door, workflow, first_statement) in DOOR_GUARDS {
        let body = declared_item(&queue, &format!("pub fn {door}("))?;
        let after_brace = body
            .split_once("{ ")
            .map(|(_, rest)| rest)
            .ok_or_else(|| format!("{door} has no body"))?;
        assert!(
            after_brace.starts_with(first_statement),
            "the first statement of {door} is not its workflow guard; it starts `{}`",
            &after_brace[..after_brace.len().min(80)]
        );
        assert!(
            body.contains(&format!("Workflow::{workflow}")),
            "{door} does not name the workflow it serves"
        );
    }
    Ok(())
}

/// The whole surface that can move a proposal, and what stops an automatic
/// actor reaching each one.
///
/// Compared against the queue's whole set of public methods that take `&mut
/// self`, so a fifth door fails as an extra key. `NON_DELEGABLE` is user-only,
/// and this is the executed form of that: every door is named here with the
/// reason an automatic actor cannot use it, and
/// `non_delegable_has_no_automatic_actor_path` in `tests/proposals.rs` runs
/// each one.
const SETTLEMENT_DOORS: [(&str, &str); 7] = [
    (
        "admit",
        "takes no actor and records nothing; admission is not a disposition",
    ),
    (
        "autosave",
        "refused for NON_DELEGABLE by the workflow comparison",
    ),
    (
        "review",
        "refused for NON_DELEGABLE by the workflow comparison",
    ),
    (
        "approve",
        "refused for NON_DELEGABLE by the workflow comparison",
    ),
    (
        "decide",
        "takes a UserDecision, which only Actor::User mints",
    ),
    (
        "undo",
        "takes a UserDecision, and needs an open record only a user can have made",
    ),
    (
        "commit",
        "needs a recorded CONFIRM only a user decision can have put there",
    ),
];

#[test]
fn every_settlement_door_is_named() -> TestResult {
    let code = strip_non_code(&fs::read_to_string(crate_root().join("src/queue.rs"))?);
    let mut observed: BTreeSet<String> = BTreeSet::new();
    let lines: Vec<&str> = code.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("pub fn ") else {
            continue;
        };
        let name = rest.split(['(', '<']).next().unwrap_or_default().to_owned();
        // A door is a public method that can change the queue.
        let takes_mut = lines[index..]
            .iter()
            .take(4)
            .any(|candidate| candidate.contains("&mut self"));
        if takes_mut {
            observed.insert(name);
        }
    }
    let named: BTreeSet<String> = SETTLEMENT_DOORS
        .iter()
        .map(|(door, _)| (*door).to_owned())
        .collect();
    assert_eq!(
        observed, named,
        "the queue's public mutating surface is not the doors this file names"
    );
    for (door, reason) in SETTLEMENT_DOORS {
        assert!(
            reason.len() > 30,
            "{door} is named with no reason an automatic actor cannot use it"
        );
    }
    Ok(())
}

/// The whole inherent surface of the user receipt.
///
/// The receipt is what separates a user from an automatic actor, so what can be
/// called on it is pinned as text: an inherent `pub fn forge` would name no
/// trait and would pass a rule that only looked at trait implementations.
const WHOLE_USER_DECISION: &str = "impl UserDecision { pub fn by(actor: &Actor) -> Result<Self, WorkflowError> { match actor { Actor::User { user_id } => Ok(Self { user_id: u128::from_be_bytes(*user_id.as_bytes()), }), Actor::DeterministicEngine { .. } | Actor::ModelRun { .. } | Actor::Importer { .. } => { Err(WorkflowError::AutomaticActor { actor: actor.kind_name(), }) } } } #[must_use] pub const fn user_id(&self) -> u128 { self.user_id } }";

#[test]
fn the_user_receipt_has_one_producer() -> TestResult {
    let source = fs::read_to_string(crate_root().join("src/disposition.rs"))?;
    let start = source
        .find("impl UserDecision {")
        .ok_or("the UserDecision impl block is not in the source")?;
    let end = source[start..]
        .find("\n}")
        .ok_or("the UserDecision impl block has no closing brace at column zero")?;
    let pinned = source[start..start + end + 2]
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        pinned, WHOLE_USER_DECISION,
        "the user receipt's inherent surface is not the pinned text"
    );

    // And there is no second block. A trait implementation for `UserDecision`
    // that produced one would not appear in the pin above.
    let mut headers: Vec<String> = Vec::new();
    for path in crate_product_sources()? {
        let code = strip_non_code(&fs::read_to_string(&path)?);
        for line in code.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("impl") && uses_of(trimmed, "UserDecision") > 0 {
                headers.push(trimmed.to_owned());
            }
        }
    }
    assert_eq!(
        headers,
        vec!["impl UserDecision {".to_owned()],
        "there is a second impl block naming UserDecision"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The absent dependency
// ---------------------------------------------------------------------------

#[test]
fn the_crate_has_no_writer_dependency() -> TestResult {
    // What makes `proposal_crate_cannot_name_the_canonical_writer` a compile
    // error is the manifest, so the manifest is what this reads. The
    // `compile_fail` case proves a compiler sees the absence; this proves the
    // absence is declared rather than incidental.
    // Comment lines are dropped first. The manifest's own prose says there is
    // deliberately no edge to the writer, and a check that read the prose would
    // report the sentence as the edge.
    let manifest: String = fs::read_to_string(crate_root().join("Cargo.toml"))?
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "academic-store",
        "academic-core",
        "academic-ledger",
        "academic-daemon",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "the manifest declares an edge to {forbidden}, which puts a canonical writer in reach"
        );
    }
    // The product edge set is exactly the domain crate plus two leaf libraries,
    // pinned whole so an addition fails here rather than only in the workspace
    // edge map. A dependency this crate does not need is how a writer, a
    // socket, or a filesystem path gets into reach without anyone deciding to
    // put it there.
    let declared: Vec<&str> = manifest
        .split("[dependencies]")
        .nth(1)
        .ok_or("the manifest has no [dependencies] section")?
        .split("\n[")
        .next()
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            line.split(['=', '.'])
                .next()
                .map(str::trim)
                .filter(|name| !name.is_empty())
        })
        .collect();
    assert_eq!(
        declared,
        vec!["academic-domain", "sha2", "thiserror"],
        "the product dependency set is not the pinned three"
    );
    Ok(())
}
