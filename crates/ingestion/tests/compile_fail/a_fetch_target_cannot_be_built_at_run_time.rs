//! A fetch target is declared in a manifest, not discovered in a page.
//!
//! `DeclaredTarget::declared` takes `&'static str`. Bytes that arrive at run
//! time are owned, so a link read out of a fetched document is not a value this
//! constructor accepts. That is what "this is not a crawler" means here.

use academic_ingestion::DeclaredTarget;

fn main() {
    let discovered: String = String::from("official/cse/found-in-the-page");
    let _target = DeclaredTarget::declared(&discovered);
}
