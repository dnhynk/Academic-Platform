//! `GATE-38-024`: the speed of personalization is a configuration decision with
//! no shipped value.
//!
//! `PersonalizationSpeed` implements no `Default`, so a caller cannot reach one
//! without naming a minimum sample count and a step.

use academic_freshness::PersonalizationSpeed;

fn main() {
    let _speed = PersonalizationSpeed::default();
}
