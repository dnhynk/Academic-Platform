//! Section 24.4: no direction ends nowhere.
//!
//! `Termination` holds its first terminus as a field taken by value and has
//! private fields, so a walk with no path is not a value that exists. `traverse`
//! is the one producer, and its fallback names the direction and the starting
//! point when no row was reached.

use academic_readiness::{NavigationDirection, StartingPoint, Termination};

fn shape(direction: NavigationDirection, start: StartingPoint) -> Termination {
    Termination {
        direction,
        start,
        rest: Vec::new(),
    }
}

fn main() {
    let _ = shape;
}
