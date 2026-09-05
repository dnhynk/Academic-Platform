//! `a_percentage_is_not_built_from_a_number`.
//!
//! Section 25.4's last line: 상세 breakdown이 항상 붙는다. Every route from a
//! number into a percentage is tried here, and none of them exists.
//!
//! The percentage arrives as a parameter for the same reason the plan snapshot
//! case gives: an error reported against `Result<SecondaryPercentage, _>` would
//! say nothing about `SecondaryPercentage`. The private-field probe is in
//! `no_academic_surface_type_has_a_public_field` rather than here: E0451 is
//! reported by the privacy pass, which does not run once type checking has
//! failed, so a literal beside these probes produces no diagnostic.

use academic_dashboard::SecondaryPercentage;

fn routes(bar: &mut SecondaryPercentage) {
    // There is no constructor taking the number.
    let _from_number = SecondaryPercentage::over(720_u32);

    // Nor a `From<u32>`, nor a `TryFrom<u32>`.
    let _converted = SecondaryPercentage::from(720_u32);
    let _tried = SecondaryPercentage::try_from(720_u32);

    // Nor a `Default`, which would be a bar over nothing.
    let _default = SecondaryPercentage::default();

    // Nor a way to take the breakdown back off one.
    bar.clear_breakdown();

}

fn main() {}
