//! A finding's scope has no repository-wide value to select.
//!
//! Section 34.4's prevention column for over-generalised snippets is *finding
//! scope를 symbol/component로 시작*. `FindingScope` has exactly two variants
//! and neither of them is the tree, so the widest scope is not something a
//! caller can name — including through a wildcard variant nobody wrote.

use academic_repository_analysis::FindingScope;

fn repository_wide() -> FindingScope {
    FindingScope::Repository
}

fn everything() -> FindingScope {
    FindingScope::All {
        component: "src".to_owned(),
    }
}

fn main() {
    let _wide = repository_wide();
    let _all = everything();
}
