//! Section 34.6's first principle, one stage over.
//!
//! A correction is a new `take` over new inputs. `ReadinessRow` has no public
//! field, no setter and no `&mut self` method, and nothing in this crate takes
//! `&mut self` at all.

use academic_domain::FreshnessBand;
use academic_readiness::ReadinessRow;

fn shape(row: &mut ReadinessRow) {
    row.set_freshness(FreshnessBand::VeryHigh);
}

fn main() {
    let _ = shape;
}
