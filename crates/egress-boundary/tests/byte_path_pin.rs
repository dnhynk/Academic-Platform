//! The byte path, pinned as whole text, and the exception-path inventory.
//!
//! `preview_bytes_equal_transmitted_bytes` in `egress_boundary.rs` observes
//! that one run wrote the previewed bytes. That is a fact about a run. What
//! makes it an invariant is that there is only one derivation: the preview owns
//! the buffer, the runtime call is built from it, and the transport is written
//! from the buffer the broker just verified. A second derivation would satisfy
//! the observation on the day it was added and drift later, so the functions
//! that carry the payload are pinned here as whitespace-collapsed whole text,
//! the way `crates/retention/tests/rotation_gate.rs` pins the rotation gate and
//! `crates/cli/src/main.rs` pins the compiled-key check.
//!
//! A forbidden-token list cannot do this job. Reading the source a second time,
//! calling a helper that re-runs the redactor, or copying the buffer and
//! mutating the copy all leave every token list untouched.
//!
//! The second half is the exception-path inventory that
//! `scanner_error_emits_zero_bytes` rests on. Every fallback in this crate's
//! product half is counted per file, and each count carries the reason its
//! sites are safe. A new fallback fails the count, which is the point: the
//! judgement about whether it fails open has to be written down.

use std::{
    collections::BTreeSet,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

type TestResult = Result<(), Box<dyn Error>>;

/// The crate root.
fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Reads one named file under this crate's `src`.
///
/// The whole-text pins below name the item they pin, so they name the file that
/// holds it; a rename fails here loudly because the read fails. What a named
/// read cannot see is a file that was *added*, which is what the walk is for.
fn source(relative: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(crate_root().join("src").join(relative))?)
}

/// Every `.rs` file anywhere under this crate's package directory.
///
/// The package rather than a list of names. `T149` added `mod relay;` and one
/// new file to this crate: it reached the transport through the broker without
/// binding a grant, wrote 178 bytes for a payload `transmit` refused with zero,
/// left no journal row, and passed this file, the egress suite,
/// `cargo test --workspace --all-targets` and both JS scans -- because the
/// three counts read `lib.rs` and the fallback inventory read six names, and
/// nothing in this crate had ever asked what files existed. Copied from
/// `crate_all_sources` in `crates/untrusted-content/tests/trust_scans.rs`,
/// which is where this repository's package walk lives.
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
            !path
                .strip_prefix(&root)
                .unwrap_or(path)
                .starts_with("tests")
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

/// The path of `path` under this crate's `src`, with forward slashes.
///
/// The fallback inventory is keyed on this rather than on a file stem, so a
/// module added as `sub/mod.rs` cannot be mistaken for a `mod.rs` elsewhere.
fn under_src(path: &Path) -> String {
    path.strip_prefix(crate_root().join("src"))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The comment-stripped code of every product file, in walk order.
fn product_code() -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut read = Vec::new();
    for path in crate_product_sources()? {
        read.push((under_src(&path), code_only(&fs::read_to_string(&path)?)));
    }
    Ok(read)
}

/// Drops every comment line, so a count reads code and not prose.
///
/// The whole-text pins below read the source verbatim; only the occurrence
/// counts use this, because a doc comment that names the call it documents
/// would otherwise be counted as a second call site.
fn code_only(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Reads one item out of a source file, comment lines dropped and whitespace
/// collapsed.
///
/// Comment lines are dropped so a pin fixes code and not prose, which is what
/// `docs/contracts/policy-source-scans.md` says every pin in this repository
/// does and what `declared_item` in
/// `crates/untrusted-content/tests/trust_scans.rs` already did. It was untested
/// here until `WHOLE_TRANSPORT_TRAIT`: none of the other eight items has a
/// comment inside it, so none of them ever asked.
fn whole(text: &str, signature: &str, terminator: &str) -> Result<String, Box<dyn Error>> {
    let (_, rest) = text
        .split_once(signature)
        .ok_or_else(|| format!("{signature} is gone"))?;
    let (body, _) = rest
        .split_once(terminator)
        .ok_or_else(|| format!("{signature} has no terminator"))?;
    let joined = format!("{signature}{body}{terminator}");
    Ok(code_only(&joined)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

/// The only place a payload argument is built. It reads the preview.
const WHOLE_RUNTIME_CALL: &str = concat!(
    "pub(crate) fn staged_runtime_call<'a>( staged: &'a StagedPayload, ",
    "plan: &TransmissionPlan<'_>, ) -> Result<RuntimeToolCall<'a>, BrokerError> { ",
    "RuntimeToolCall::new( plan.actor_id, plan.process_class, plan.operation, ",
    "plan.purpose_id, plan.destination_id, vec![staged.object_range()?], ",
    "staged.preview().bytes(), ) }"
);

/// The only place a transport is written. It reads the authorized buffer.
const WHOLE_EMIT: &str = concat!(
    "pub(crate) fn write_authorized_bytes<T: OutboundTransport>( ",
    "authorized: &AuthorizedToolCall<'_>, transport: &mut T, chunk_bytes: usize, ",
    "now: &dyn Fn() -> u64, expires_at: u64, ) -> Result<usize, TransportError> { ",
    "let bytes = authorized.payload(); let mut sent = 0_usize; ",
    "for chunk in bytes.chunks(chunk_bytes.max(1)) { if now() >= expires_at { ",
    "return Err(TransportError::GrantExpiredMidTransfer { sent }); } ",
    "transport.send_chunk(chunk)?; sent = sent.saturating_add(chunk.len()); } Ok(sent) }"
);

/// The whole transport trait. One method, so there is one way to write bytes.
///
/// The call-site counts below read `send_chunk`, which is worth counting only
/// while it is the only writer. `T151` added a second *required* method to this
/// trait and reached it from a new module: no count moved, because the new name
/// is not one of the counted ones and a required method has no body to call
/// anything from. A second method with a default body is caught by the
/// `send_chunk` count, because its body calls it; one without a body was caught
/// by nothing until this pin.
const WHOLE_TRANSPORT_TRAIT: &str = concat!(
    "pub trait OutboundTransport { ",
    "fn send_chunk(&mut self, chunk: &[u8]) -> Result<(), TransportError>; }"
);

/// The preview's byte accessor. It returns the field and computes nothing.
const WHOLE_PREVIEW_BYTES: &str = "pub fn bytes(&self) -> &[u8] { &self.payload }";

/// The staged payload's preview accessor. Same reason.
const WHOLE_STAGED_PREVIEW: &str = "pub const fn preview(&self) -> &Preview { &self.preview }";

/// The scan-and-deny step. Both arms refuse; neither continues.
const WHOLE_DENY_ON_FINDINGS: &str = concat!(
    "fn deny_on_findings(rulepack: &Rulepack, text: &str, stage: &str) ",
    "-> Result<(), EgressDenial> { let findings = rulepack.scan(text)",
    ".map_err(|error: ScanError| { EgressDenial::new( ReasonCode::ScannerError, ",
    "format!( \"{stage}: rule {} could not complete: {}\", error.rule_id, error.detail ), ) })?; ",
    "let Some(first) = findings.first() else { return Ok(()); }; ",
    "Err(EgressDenial::with_findings( first.reason(), ",
    "format!( \"{stage}: rule {} matched bytes [{}, {}) in a {} span\", first.rule_id(), ",
    "first.start(), first.end(), first.span_kind().as_str() ), findings, )) }"
);

/// The whole staging pipeline, whitespace-collapsed.
///
/// The fallback inventory below counts sites; it does not read what a site
/// falls back *to*. Changing `unwrap_or(u64::MAX)` to `unwrap_or(0)` keeps the
/// count and turns an unrepresentable length from something that trips the
/// oversize bound into something that clears it. So the pipeline itself is
/// pinned: the order of its steps, the reason code each one denies with, and
/// every default it takes.
const WHOLE_STAGE: &str = concat!(
    "pub(crate) fn stage( rulepack: &Rulepack, request: &StagingRequest<'_>, ) -> ",
    "Result<StagedPayload, EgressDenial> { let source_len = ",
    "u64::try_from(request.document.payload.len()).unwrap_or(u64::MAX); if source_len > ",
    "request.max_bytes { return Err(EgressDenial::new( ReasonCode::Oversize, format!( ",
    "\"source is {source_len} bytes, over the {} byte destination bound\", ",
    "request.max_bytes ), )); } let text = ",
    "minimize::classify(&request.document.payload).map_err(|error| { let detail = match ",
    "error { ClassificationError::NotUtf8 => \"payload is not UTF-8 text\".to_owned(), ",
    "ClassificationError::ContainerMagic(name) => format!(\"payload is a {name}\"), ",
    "ClassificationError::ControlByte => { \"payload holds a control byte no source text ",
    "uses\".to_owned() } }; EgressDenial::new(ReasonCode::UnknownBinary, detail) })?; ",
    "let ranges = minimize::minimal_ranges(text, request.focus).ok_or_else(|| { ",
    "EgressDenial::new( ReasonCode::ScopeMismatch, \"the document declares no item for a ",
    "requested symbol\", ) })?; let selected = concatenate(text, ",
    "&ranges).ok_or_else(range_off_document)?; deny_on_findings(rulepack, &selected, ",
    "\"source slice\")?; let redaction = substitute_identifiers(text, &ranges, ",
    "request.identifier_policy) .ok_or_else(range_off_document)?; ",
    "meaning_check(&selected, &redaction, request)?; let staged_text = ",
    "String::from_utf8(redaction.payload).map_err(|_| { EgressDenial::new( ",
    "ReasonCode::UnknownBinary, \"redaction produced bytes that are not UTF-8 text\", ) ",
    "})?; deny_on_findings(rulepack, &staged_text, \"redacted slice\")?; let staged_len = ",
    "u64::try_from(staged_text.len()).unwrap_or(u64::MAX); if staged_len > ",
    "request.max_bytes { return Err(EgressDenial::new( ReasonCode::Oversize, format!( ",
    "\"redacted payload is {staged_len} bytes, over the {} byte destination bound\", ",
    "request.max_bytes ), )); } let rulepack_id = rulepack.id(); let preview = Preview ",
    "{ payload: staged_text.into_bytes(), source_ranges: ranges, substitutions: ",
    "redaction.substitutions, rulepack: rulepack_id.clone(), }; let staged_object_id = ",
    "staged_object_id(request.document.object_id(), &preview, &rulepack_id); ",
    "Ok(StagedPayload { staged_object_id, preview, }) }",
);

/// The grant binding both transmit paths run before they build a byte.
///
/// `T146` measured what a second public path costs when it does not:
/// `transmit_without_completion` skipped the grant read and the rulepack
/// comparison and wrote 180 bytes to a transport for a payload `transmit`
/// refused with zero. Counting the call sites below is what keeps a third path
/// from doing it again; pinning the body is what keeps the two comparisons
/// inside it from being deleted, which `T146` also did -- the whole workspace
/// suite passed with them gone.
const WHOLE_BIND_GRANT: &str = concat!(
    "fn bind_grant( &self, plan: &TransmissionPlan<'_>, capability: &CapabilityToken, ",
    "staged: &StagedPayload, ) -> Result<GrantRow, EgressError> { ",
    "if plan.grant_id != capability.grant_id() { ",
    "return Err(EgressError::Denied(EgressDenial::new( ReasonCode::ScopeMismatch, format!( ",
    "\"the plan names grant {} but the capability consumes {}\", plan.grant_id, ",
    "capability.grant_id() ), ))); } let grant = self .broker .grant_row(plan.grant_id) ",
    ".map_err(EgressError::Broker)? .ok_or_else(|| { EgressError::Denied(EgressDenial::new( ",
    "ReasonCode::NoGrant, format!(\"no grant row for {}\", plan.grant_id), )) })?; ",
    "let recorded = staged.preview().rulepack().redaction_policy_hash().as_str(); ",
    "if grant.redaction_policy_hash != recorded { ",
    "return Err(EgressError::Denied(EgressDenial::new( ReasonCode::ScopeMismatch, format!( ",
    "\"grant records redaction policy {} but the payload was produced by {recorded}\", ",
    "grant.redaction_policy_hash ), ))); } Ok(grant) }"
);

/// `GATE-38-028`'s default, whole. It takes no argument and reads nothing.
const WHOLE_CLOUD_DEFAULT: &str =
    "pub const fn cloud_egress_default() -> Route { Route::LocalOnlyOrStop }";

/// The preview and the transmission come from one buffer by construction.
#[test]
fn the_byte_path_has_one_derivation() -> TestResult {
    let transport = source("transport.rs")?;
    let stage = source("stage.rs")?;
    let lib = source("lib.rs")?;

    assert_eq!(
        whole(&transport, "pub(crate) fn staged_runtime_call", "\n}")?,
        WHOLE_RUNTIME_CALL,
        "the runtime call takes its payload from somewhere other than the preview"
    );
    assert_eq!(
        whole(&transport, "pub(crate) fn write_authorized_bytes", "\n}")?,
        WHOLE_EMIT,
        "the transport is written from something other than the authorized buffer"
    );
    assert_eq!(
        whole(&transport, "pub trait OutboundTransport", "\n}")?,
        WHOLE_TRANSPORT_TRAIT,
        "the transport trait gained a second way to write bytes"
    );
    assert_eq!(
        whole(&stage, "pub fn bytes(&self)", "\n    }")?,
        WHOLE_PREVIEW_BYTES,
        "the preview computes its bytes instead of holding them"
    );
    assert_eq!(
        whole(&stage, "pub const fn preview(&self)", "\n    }")?,
        WHOLE_STAGED_PREVIEW,
        "the staged payload builds a preview instead of holding one"
    );
    assert_eq!(
        whole(&stage, "fn deny_on_findings", "\n}")?,
        WHOLE_DENY_ON_FINDINGS,
        "the scan-and-deny step gained an arm that is not a refusal"
    );
    assert_eq!(
        whole(&stage, "pub(crate) fn stage(", "\n}")?,
        WHOLE_STAGE,
        "the staging pipeline changed a step, a reason code, or a default"
    );
    assert_eq!(
        whole(&lib, "pub const fn cloud_egress_default", "\n}")?,
        WHOLE_CLOUD_DEFAULT,
        "GATE-38-028's default is decided by something other than the constant"
    );
    assert_eq!(
        whole(&lib, "fn bind_grant(", "\n    }")?,
        WHOLE_BIND_GRANT,
        "the grant binding every transmit path runs first lost a comparison"
    );

    // One construction site for the staged bytes, one accessor, one writer.
    let stage_code = code_only(&stage);
    let transport_code = code_only(&transport);
    let lib_code = code_only(&lib);
    assert_eq!(
        stage_code
            .matches("payload: staged_text.into_bytes()")
            .count(),
        1,
        "the staged buffer is built in more than one place"
    );
    assert_eq!(
        stage_code.matches("fn substitute_identifiers").count(),
        1,
        "the redaction pass has more than one definition"
    );
    assert_eq!(
        identifier_uses(&transport_code, "preview"),
        1,
        "the preview buffer is read in more than one place on the transport path"
    );

    // Both transmit paths reach the transport only through the broker's
    // capability boundary. A direct call would be a send with no grant.
    //
    // The counts read identifiers, not spellings. A count of `.execute(` sees a
    // method call and not `PermissionBroker::execute(self.broker, ..)`, and a
    // count of `transport::write_authorized_bytes(` sees a path-qualified call
    // and not one made through a `use`. Both are the same call, and `T146`
    // reached a guarded function by exactly that substitution one file over.
    for site in [
        "transport::write_authorized_bytes(",
        ".execute(capability, call, started_at, |authorized| {",
    ] {
        assert!(lib_code.contains(site), "{site} is gone from the proxy");
    }

    // And they are counted over every file the walk read, not over `lib.rs`.
    // `T149` put the third path in a new module: the counts here were `lib.rs`
    // counts, so `mod relay;` -- which adds no call site to `lib.rs` -- moved
    // none of them, and 178 bytes reached a transport under a grant reviewed
    // by another rulepack. `the_transport_is_reached_from_no_module_but_the_proxy`
    // is what says the walk saw every module; these are what it counts.
    //
    // Each declaration is subtracted from its own use count, and a declaration
    // is read as a whole name rather than as a spelling: `fn bind_grant_later(`
    // is not `fn bind_grant`. `T149`'s other finding was a count one crate over
    // that subtracted the spelling, where `pub fn expose_rendered(` cancelled
    // its own call to `expose`.
    let product = product_code()?;
    for (name, sites, _, why) in CALL_SITE_COUNTS {
        let mut uses = 0_usize;
        let mut declarations = 0_usize;
        for (_, code) in &product {
            uses += identifier_uses(code, name);
            declarations += declarations_of(code, name);
        }
        assert!(
            uses >= declarations,
            "{name} is declared {declarations} times and named {uses}, which cannot happen"
        );
        assert_eq!(uses - declarations, sites, "{why}");
    }
    Ok(())
}

/// Call-site counts over every product file the walk reads, and the one file
/// each name may be called from.
///
/// A count is a claim that the crate has exactly this many ways to do the thing
/// named, and the walk is what makes "the crate" mean the package rather than
/// one file. The caller column is the second half: a count of two is still two
/// if one of them moves into a new module, and a module that reaches the
/// transport directly is the shape `T149` used.
const CALL_SITE_COUNTS: [(&str, usize, &str, &str); 4] = [
    (
        "execute",
        2,
        "lib.rs",
        "a transmit path bypasses the capability boundary",
    ),
    (
        "write_authorized_bytes",
        2,
        "lib.rs",
        "the emit helper is called from an unexpected number of places",
    ),
    (
        "bind_grant",
        2,
        "lib.rs",
        "a transmit path reaches the transport without binding its grant first",
    ),
    (
        "send_chunk",
        1,
        "transport.rs",
        "the transport is written from somewhere besides write_authorized_bytes",
    ),
];

/// Counts whole-identifier occurrences of `name` in already-stripped code.
///
/// Whole-identifier, so `bind_grant` does not match inside `bind_grant_later`,
/// and every spelling of one call counts the same: the receiver form, the
/// type-path form, and a bare reference taken without calling it. Copied from
/// `names_unsafe` in `crates/worker/tests/capability.rs`, which is where this
/// repository's identifier counter lives.
fn identifier_uses(code: &str, name: &str) -> usize {
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
/// A declaration is subtracted from a use count, so reading it as a spelling
/// lets a longer name cancel its own call: `T149` walked past the inventory one
/// crate over with `pub fn expose_rendered(`, which `occurrences(code, "fn
/// expose")` counted as a declaration of `expose` while `uses_of` did not count
/// it as a use. What follows the name here has to open a parameter list or a
/// generic list and nothing else, so `fn bind_grant_later(` is not `bind_grant`
/// and `fn write_authorized_bytes<T: OutboundTransport>(` still is.
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

/// Every fallback in the product half, counted, with the reason it is safe.
///
/// The tokens are the shapes that can swallow an outcome. A count that no
/// longer matches means a new one was added, and the review it needs is the
/// sentence beside it.
///
/// A count cannot see a fallback whose *value* changed, which is why the
/// staging pipeline is pinned as whole text above. Outside `stage`, the
/// remaining sites encode a length or an offset and are listed here.
const FALLBACK_INVENTORY: [(&str, &str, usize, &str); 6] = [
    (
        "lib.rs",
        "unwrap_or",
        0,
        "the proxy maps every broker and transport result explicitly",
    ),
    (
        "transport.rs",
        "unwrap_or",
        0,
        "the emit path has no fallback at all: every write returns a result the caller reads",
    ),
    (
        "minimize.rs",
        "map_or",
        2,
        "an unterminated comment runs to the end of the text, so nothing is skipped",
    ),
    (
        "rulepack.rs",
        "map_or",
        4,
        "three run an unterminated comment or authority to the end of the text; the fourth \
         reports an offset outside every span as code, which reports a finding rather than \
         suppressing one",
    ),
    (
        "rulepack.rs",
        "unwrap_or",
        3,
        "two saturate a length being encoded into a digest; one treats an all-whitespace \
         assignment key as empty, which cannot end with a needle either way",
    ),
    (
        "stage.rs",
        "unwrap_or",
        5,
        "three saturate a byte length towards the oversize bound, which refuses; one is the \
         unreachable divide-by-zero guard that yields a hundred percent substituted; one is \
         the meaning bound, which falls to zero and therefore refuses",
    ),
];

/// Shapes that must not appear at all in the product half.
const FORBIDDEN_SHAPES: [(&str, &str); 6] = [
    (
        "catch_unwind",
        "a swallowed panic is a send with no scan behind it",
    ),
    (
        ".is_ok()",
        "a boolean drops the reason a decision was refused",
    ),
    ("let _ =", "a discarded result is a decision nobody read"),
    ("if let Ok(", "an Ok-only arm continues past a failure"),
    ("unwrap()", "a panic is not a refusal"),
    (".expect(", "a panic is not a refusal"),
];

/// No path in this crate turns a failed decision into a permitted one.
#[test]
fn no_exception_path_fails_open() -> TestResult {
    // The walk, not a list of six names. A name list says nothing about a
    // seventh file, and `T149` added one.
    let sources = crate_product_sources()?;
    assert!(
        sources.len() >= 6,
        "the walk found only {} product files under the package",
        sources.len()
    );
    let mut inventory: Vec<(String, &str, usize)> = Vec::new();
    for path in &sources {
        let name = under_src(path);
        let text = fs::read_to_string(path)?;
        assert!(
            !text.contains("#[cfg(test)]"),
            "{name} gained a test module, so the product half is no longer the whole file"
        );
        let product = code_only(&text);
        for (shape, why) in FORBIDDEN_SHAPES {
            assert!(!product.contains(shape), "{name} uses {shape}: {why}");
        }
        for token in ["unwrap_or", "map_or"] {
            let count = product.matches(token).count()
                - if token == "unwrap_or" {
                    product.matches("unwrap_or_default").count()
                        + product.matches("unwrap_or_else").count()
                } else {
                    0
                };
            if count > 0 {
                inventory.push((name.clone(), token, count));
            }
        }
        // `unwrap_or_default` and `unwrap_or_else` are counted separately so a
        // new one cannot hide inside the `unwrap_or` total.
        assert_eq!(
            product.matches("unwrap_or_else").count(),
            0,
            "{name} gained an unwrap_or_else, which needs its own review"
        );
        let defaults = product.matches("unwrap_or_default").count();
        let expected_defaults = usize::from(name == "rulepack.rs");
        assert_eq!(
            defaults, expected_defaults,
            "{name} has {defaults} unwrap_or_default uses, not {expected_defaults}"
        );
    }

    let declared: Vec<(String, &str, usize)> = FALLBACK_INVENTORY
        .iter()
        .filter(|(_, _, count, _)| *count > 0)
        .map(|(file, token, count, _)| ((*file).to_owned(), *token, *count))
        .collect();
    inventory.sort_unstable();
    let mut declared = declared;
    declared.sort_unstable();
    assert_eq!(
        inventory, declared,
        "the fallback inventory and the source disagree; review the new site and record why it refuses"
    );

    // Every declared entry carries a reason, including the two zero rows that
    // say the byte path itself has no fallback.
    for (file, token, _, why) in FALLBACK_INVENTORY {
        assert!(why.len() >= 40, "{file}/{token} has no written reason");
    }
    Ok(())
}

/// The walk reads every module, and only the proxy reaches the transport.
///
/// The counts in `the_byte_path_has_one_derivation` are sums over the files
/// this walk returns, so they are worth exactly what the walk is worth. `T149`
/// wrote `mod relay;` into `lib.rs` and a new `src/relay.rs` beside it: the
/// module reached `broker.execute` and `write_authorized_bytes` directly, sent
/// 178 bytes under a grant reviewed by a different rulepack, wrote no journal
/// row, and passed every check this crate had -- because this crate had no walk
/// and no module inventory, and every count read `lib.rs`.
///
/// Two halves, and each catches what the other cannot. The tripwire fails if a
/// module is declared that the walk never read, so the walk cannot be narrowed
/// out from under the counts. The per-file rule fails if any file but `lib.rs`
/// names one of the three, so a new module that *is* walked still cannot reach
/// a transport: it would have to route through the proxy, where the grant is
/// bound. `transport.rs` declares two of the three and calls neither.
#[test]
fn the_transport_is_reached_from_no_module_but_the_proxy() -> TestResult {
    let sources = crate_all_sources()?;
    // The floor. A walk that returned nothing would satisfy every count above.
    assert!(
        sources.len() >= 6,
        "the walk found only {} files under the package",
        sources.len()
    );

    // Product source lives under `src` and nowhere else, which is what makes
    // "the files the walk read, less `tests`" mean "everything that ships".
    // `S-12` in `docs/contracts/policy-source-scans.md` is the shape this
    // refuses: `P2-G4` put a `[[bin]]` with an explicit `path` outside `src`,
    // and two scans stopped seeing that crate's product code.
    let root = crate_root();
    let outside: Vec<String> = crate_product_sources()?
        .iter()
        .filter(|path| !path.strip_prefix(&root).unwrap_or(path).starts_with("src"))
        .map(|path| under_src(path))
        .collect();
    assert_eq!(
        outside,
        Vec::<String>::new(),
        "this crate has product source outside src; every count that reads it has to widen"
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

    // The tripwire. Every `mod name;` and every `#[path = "…"]` in the crate
    // has to name a file the walk read. It fails the day the walk is narrowed,
    // and the day a module is added somewhere the walk does not descend into.
    let mut declared = 0_usize;
    for path in &sources {
        let text = fs::read_to_string(path)?;
        for line in text.lines() {
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
                    under_src(path)
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
                    under_src(path)
                );
            }
        }
    }
    assert!(declared >= 5, "the crate declares only {declared} modules");

    // Each of the four names may be called only from the file that owns it:
    // the proxy for the three that reach the broker and the emit helper, and
    // `transport.rs` for the trait method the emit helper writes through. A
    // declaration is not a call, so `transport.rs` declaring the emit helper
    // does not count as calling it. Every other file must name none of them.
    for (file, code) in product_code()? {
        for (name, _, caller, _) in CALL_SITE_COUNTS {
            if file == caller {
                continue;
            }
            let calls = identifier_uses(&code, name) - declarations_of(&code, name);
            assert_eq!(
                calls, 0,
                "{file} calls {name} {calls} time(s); only {caller} may call it"
            );
        }
    }
    Ok(())
}

/// The refusal type cannot carry bytes, so a denial cannot be transmitted.
#[test]
fn a_denial_has_no_payload_field() -> TestResult {
    let stage = source("stage.rs")?;
    let (_, body) = stage
        .split_once("pub struct EgressDenial {")
        .ok_or("EgressDenial is gone")?;
    let (fields, _) = body.split_once('}').ok_or("EgressDenial has no body")?;
    let names: Vec<&str> = fields
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with("//"))
        .filter_map(|line| line.split(':').next())
        .collect();
    assert_eq!(
        names,
        vec!["reason", "detail", "findings", "bytes_transmitted"],
        "EgressDenial gained or lost a field; a refusal may carry no payload"
    );

    // The staged payload is the only thing a transmission accepts, and it is
    // constructed in exactly one place.
    assert_eq!(
        stage.matches("Ok(StagedPayload {").count(),
        1,
        "StagedPayload is constructed somewhere besides the end of stage()"
    );
    assert_eq!(
        stage.matches("pub struct StagedPayload").count(),
        1,
        "StagedPayload is declared more than once"
    );
    Ok(())
}
