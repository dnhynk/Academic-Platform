//! Four scans over this crate's own structure, and one vacuity control for the
//! lexer all four share.
//!
//! They are here rather than in `tools/` for the reason `P2-G4`'s are: what
//! they read is where `unsafe` may appear in this crate, which targets a
//! default build links, that the one file allowed a syscall names only the
//! syscalls it installs with, and what this crate's declared surface is.
//! [policy source scans](../../../docs/contracts/policy-source-scans.md) is
//! where they are registered.

use std::path::{Path, PathBuf};

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Removes `//` and `/* */` comments and the contents of every literal.
///
/// The raw-string and character-literal arms are not decoration, and `P2-G4`
/// recorded why: a lexer that does not model `r#"..."#` leaves the quote count
/// odd from the first raw string onward, and one that does not model `'"'`
/// inverts it from that character on; in both cases every later literal in the
/// file is read as code and every stretch of code as a literal. This file's own
/// vacuity samples below quote the word this scan looks for, so a stripper that
/// kept literals would report this file, which is exactly the false alarm the
/// four arms exist to avoid. Each arm has a sample in
/// `the_stripper_reads_comments_strings_raw_strings_and_characters`.
fn code_only(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut out = String::new();
    let mut cursor = 0_usize;
    while cursor < bytes.len() {
        let two = &bytes[cursor..(cursor + 2).min(bytes.len())];
        if two == b"//" {
            while cursor < bytes.len() && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            continue;
        }
        if two == b"/*" {
            cursor += 2;
            while cursor + 1 < bytes.len() && !(bytes[cursor] == b'*' && bytes[cursor + 1] == b'/')
            {
                cursor += 1;
            }
            cursor = (cursor + 2).min(bytes.len());
            out.push(' ');
            continue;
        }
        // A raw string: `r`, any run of `#`, a quote, then everything up to the
        // matching quote-and-run.
        if bytes[cursor] == b'r' {
            let mut hashes = 0_usize;
            while bytes.get(cursor + 1 + hashes) == Some(&b'#') {
                hashes += 1;
            }
            if bytes.get(cursor + 1 + hashes) == Some(&b'"') {
                let closing: Vec<u8> = std::iter::once(b'"')
                    .chain(std::iter::repeat_n(b'#', hashes))
                    .collect();
                let mut end = cursor + 2 + hashes;
                while end < bytes.len()
                    && !bytes[end..(end + closing.len()).min(bytes.len())].starts_with(&closing)
                {
                    end += 1;
                }
                cursor = (end + closing.len()).min(bytes.len());
                out.push(' ');
                continue;
            }
        }
        if bytes[cursor] == b'"' {
            cursor += 1;
            while cursor < bytes.len() && bytes[cursor] != b'"' {
                cursor += if bytes[cursor] == b'\\' { 2 } else { 1 };
            }
            cursor = (cursor + 1).min(bytes.len());
            out.push(' ');
            continue;
        }
        // A character literal, which is what makes `'"'` stop inverting the
        // quote count. A lifetime — `'a` with no closing quote — is left alone.
        if bytes[cursor] == b'\'' {
            let escaped = bytes.get(cursor + 1) == Some(&b'\\');
            let end = if escaped {
                (cursor + 2..bytes.len()).find(|at| bytes[*at] == b'\'')
            } else {
                (bytes.get(cursor + 2) == Some(&b'\'')).then_some(cursor + 2)
            };
            if let Some(end) = end {
                cursor = end + 1;
                out.push(' ');
                continue;
            }
        }
        out.push(char::from(bytes[cursor]));
        cursor += 1;
    }
    out
}

/// `unsafe` as a whole word.
fn names_unsafe(code: &str) -> bool {
    let bytes = code.as_bytes();
    code.match_indices("unsafe").any(|(at, _)| {
        let before_ok =
            at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        let after = bytes.get(at + 6).copied().unwrap_or(b' ');
        before_ok && !(after.is_ascii_alphanumeric() || after == b'_')
    })
}

fn rust_sources(root: &Path) -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|extension| extension == "rs")
                && let Ok(source) = std::fs::read_to_string(&path)
            {
                found.push((path, source));
            }
        }
    }
    found.sort_by(|left, right| left.0.cmp(&right.0));
    found
}

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn the_stripper_reads_comments_strings_raw_strings_and_characters() {
    // One sample per arm, and the last two are the ones that invert a naive
    // lexer: text after a raw string or after a quoted character has to still
    // be read as code.
    assert!(!names_unsafe(&code_only("// unsafe\n")));
    assert!(!names_unsafe(&code_only("/* unsafe */ let x = 1;")));
    assert!(!names_unsafe(&code_only("let s = \"unsafe\";")));
    assert!(!names_unsafe(&code_only("let s = r#\"unsafe\"#;")));
    assert!(!names_unsafe(&code_only("let s = \"a\\\"unsafe\";")));
    assert!(names_unsafe(&code_only("unsafe { libc::close(0) };")));
    assert!(names_unsafe(&code_only(
        "let s = r#\"x\"#; unsafe { libc::close(0) };"
    )));
    assert!(names_unsafe(&code_only(
        "let c = '\\\"'; unsafe { libc::close(0) };"
    )));
    assert!(names_unsafe(&code_only(
        "let c = '\\n'; unsafe { libc::close(0) };"
    )));
    // A lifetime is not a character literal.
    assert!(names_unsafe(&code_only(
        "fn f<'a>(x: &'a str) { unsafe { libc::close(0) }; }"
    )));
    assert!(!names_unsafe(&code_only("let unsafely = 1;")));
}

#[test]
fn unsafe_is_confined_to_the_linux_backend() -> TestResult {
    let root = crate_root();
    let mut with_unsafe = Vec::new();
    let mut scanned = 0_usize;
    for directory in ["src", "probes", "tests"] {
        for (path, source) in rust_sources(&root.join(directory)) {
            scanned += 1;
            let relative = path
                .strip_prefix(&root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if names_unsafe(&code_only(&source)) {
                with_unsafe.push(relative);
            }
        }
    }
    assert!(
        scanned >= 6,
        "the walk found only {scanned} files, so it proved nothing"
    );
    with_unsafe.sort();
    assert_eq!(
        with_unsafe,
        vec!["src/linux.rs"],
        "an `unsafe` block appeared outside the one backend"
    );
    Ok(())
}

#[test]
fn the_probe_target_is_not_in_any_default_build() -> TestResult {
    let manifest = std::fs::read_to_string(crate_root().join("Cargo.toml"))?;
    let normalized = manifest.replace("\r\n", "\n");
    assert!(
        normalized.contains("default = []"),
        "the default feature set is no longer empty"
    );
    let mut binaries = 0_usize;
    for section in normalized.split("[[bin]]").skip(1) {
        binaries += 1;
        assert!(
            section.contains("path = \"probes/"),
            "a binary target was added outside probes/: {section}"
        );
        assert!(
            section.contains("required-features = [\"native-enforcement\"]"),
            "a probe binary is buildable without the native-enforcement feature: {section}"
        );
    }
    assert_eq!(binaries, 1, "the probe binary inventory changed");
    Ok(())
}

#[test]
fn the_backend_names_only_the_syscalls_it_installs_with_outside_its_deny_list() -> TestResult {
    // The same bargain `P2-G4`'s backend makes, checked inside this crate as
    // well as in `tools/phase1-scaffold-policy.test.mjs`, because a rule that
    // lives in one file only is a rule one merge can drop. Four syscalls are
    // made — the three the backend installs with, and the `getpid` it makes on
    // the x32 ABI in order to be refused; every other `SYS_` name in the file
    // has to be inside the function that builds the deny list.
    let called = [
        "SYS_getpid",
        "SYS_landlock_create_ruleset",
        "SYS_landlock_restrict_self",
        "SYS_seccomp",
    ];
    let source = std::fs::read_to_string(crate_root().join("src").join("linux.rs"))?;
    let whole = code_only(&source);
    let start = whole
        .find("fn denied_syscalls() -> Vec<i64> {")
        .ok_or("src/linux.rs no longer has a denied_syscalls function")?;
    let end = whole[start..]
        .find("\n}")
        .ok_or("denied_syscalls has no closing brace")?;
    let denied = &whole[start..start + end];

    let mut named: Vec<String> = Vec::new();
    let bytes = whole.as_bytes();
    for (at, _) in whole.match_indices("SYS_") {
        if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
            continue;
        }
        let rest: String = whole[at..]
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect();
        if !named.contains(&rest) {
            named.push(rest);
        }
    }
    assert!(
        named.len() >= 14,
        "src/linux.rs names only {} syscalls, so this scan read almost nothing",
        named.len()
    );
    for name in &named {
        if called.contains(&name.as_str()) {
            assert!(
                !denied.contains(name.as_str()),
                "{name} is both installed with and denied"
            );
            continue;
        }
        let inside = denied.matches(name.as_str()).count();
        let anywhere = whole.matches(name.as_str()).count();
        assert_eq!(
            anywhere,
            inside,
            "src/linux.rs names {name} {} time(s) outside its seccomp deny list",
            anywhere - inside
        );
    }
    for name in called {
        assert!(
            named.iter().any(|found| found == name),
            "src/linux.rs no longer names {name}"
        );
    }
    Ok(())
}

/// Every `impl` header, `#[derive]` list and public item in `src`, whichever
/// file it is in, whitespace-collapsed.
///
/// The three are collected together because they are three ways to reach the
/// same thing and a scan that reads one of them is blind to the other two.
/// `P2-A5` and `P2-A4` each measured a trait `impl` handing out a value the
/// crate's own guards refused, five times in this run between them, precisely
/// because the inventories they were checked against read only `pub fn `
/// headers. A `impl From<ProcessClass> for Enforcement` declares no `pub fn`
/// and no `#[derive]`; a `#[derive(Default)]` on `Enforcement` declares
/// neither of the other two; and a second constructor inside the existing
/// `impl Enforcement` block declares no new header. One of the three sees each.
fn declared_surface(code: &str) -> Vec<String> {
    let collapse = |text: &str| text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut found = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("impl ") {
            found.push(format!(
                "impl {}",
                collapse(rest.trim_end_matches('{').trim())
            ));
        } else if trimmed.starts_with("#[derive(") {
            found.push(collapse(trimmed));
        } else if let Some(rest) = trimmed.strip_prefix("macro_rules! ") {
            // An exported macro is a public item of this crate and the five
            // `pub …` prefixes below do not begin with `pub`. `P2-RF31` added
            // `class_main!` and this whole set passed unchanged, which is the
            // same line-prefix hole `P2-A5` recorded three times elsewhere in
            // this run; the name is enough here because the transcriber's own
            // text is pinned by `crates/contracts/tests/pinned-items`.
            found.push(format!(
                "macro_rules! {}",
                collapse(rest.trim_end_matches('{').trim())
            ));
        } else if trimmed.starts_with("pub fn ")
            || trimmed.starts_with("pub const fn ")
            || trimmed.starts_with("pub const ")
            || trimmed.starts_with("pub struct ")
            || trimmed.starts_with("pub enum ")
        {
            let head = trimmed
                .split_once(" {")
                .map_or(trimmed, |(head, _)| head)
                .split_once(" =")
                .map_or_else(
                    || trimmed.split_once(" {").map_or(trimmed, |(head, _)| head),
                    |(head, _)| head,
                );
            found.push(collapse(head.trim_end_matches(';')));
        }
    }
    found.sort();
    found
}

#[test]
fn the_declared_surface_of_this_crate_is_reviewed() -> TestResult {
    let root = crate_root();
    let mut observed = Vec::new();
    let mut scanned = 0_usize;
    for (_, source) in rust_sources(&root.join("src")) {
        scanned += 1;
        observed.extend(declared_surface(&code_only(&source)));
    }
    assert!(scanned >= 2, "the walk read only {scanned} files in src");
    observed.sort();
    // Whole set, both directions. A trait `impl`, a derive, a public
    // constructor or a public constant that is not here fails, and one removed
    // from the crate without being removed here fails too.
    assert_eq!(
        observed,
        vec![
            "#[derive(Clone, Copy)]",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]",
            "#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]",
            "#[derive(Debug, Clone, PartialEq, Eq)]",
            "#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]",
            "impl BackendId",
            "impl Enforcement",
            "impl EnforcementBasis",
            "impl fmt::Display for BackendId",
            "macro_rules! class_main",
            "pub const NO_BACKEND_COMPILED: &str",
            "pub const UNSUPPORTED_PLATFORM: &str",
            "pub const WINDOWS_HAS_NO_SELF_APPLIED_MECHANISM: &str",
            "pub const fn as_str(self) -> &'static str",
            "pub const fn as_str(self) -> &'static str",
            "pub const fn backend(&self) -> BackendId",
            "pub const fn basis(capability: ProcessCapability) -> EnforcementBasis",
            "pub const fn class(&self) -> ProcessClass",
            "pub enum BackendId",
            "pub enum EnforcementBasis",
            "pub enum EnforcementError",
            "pub fn enter(class: ProcessClass) -> Result<Enforcement, EnforcementError>",
            "pub fn receipt_line(&self) -> String",
            "pub fn refusal_line(class: ProcessClass, error: &EnforcementError) -> String",
            "pub fn refusals(class: ProcessClass) -> Vec<ProcessCapability>",
            "pub fn refused(&self) -> &[ProcessCapability]",
            "pub fn verification(&self) -> &str",
            "pub struct Enforcement",
        ],
        "this crate's declared surface changed: a trait impl, a derive, a public \
         constructor or a public constant was added or removed"
    );

    // The reader is exercised against a sample of every arm it has, so an arm
    // that matched nothing would make the whole set above pass over a smaller
    // set than the crate declares. The macro arm is the one `P2-RF31` added
    // after measuring that `class_main!` joined this crate's public surface
    // with this comparison unchanged.
    assert_eq!(
        declared_surface(
            "impl A for B {\n#[derive(Debug)]\npub struct C;\npub enum D {}\n\
             pub const E: u8 = 1;\npub fn f() {}\npub const fn g() {}\n\
             macro_rules! h {\n"
        ),
        vec![
            "#[derive(Debug)]",
            "impl A for B",
            "macro_rules! h",
            "pub const E: u8",
            "pub const fn g()",
            "pub enum D",
            "pub fn f()",
            "pub struct C",
        ],
        "the surface reader lost an arm"
    );
    Ok(())
}
