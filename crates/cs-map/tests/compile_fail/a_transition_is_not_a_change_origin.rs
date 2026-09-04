//! `P2-C6`'s refusal, one layer up.
//!
//! The bitemporal contract records that `user scope change` is deliberately not
//! a `ChangeOrigin`, because changing which scope is displayed changes what a
//! viewer is shown and not what the record says. A total conversion from a map
//! transition to a change origin would have to invent an origin for a display
//! setting, so there is none.

use academic_cs_map::MapTransition;
use academic_domain::temporal::ChangeOrigin;

fn record(transition: MapTransition) -> ChangeOrigin {
    transition.into()
}

fn main() {
    let _ = record;
}
