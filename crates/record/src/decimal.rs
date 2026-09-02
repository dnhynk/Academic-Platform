//! Exact base-10 arithmetic over the canonical [`Decimal`].
//!
//! `academic_domain::Decimal` is this repository's one exact numeric type: a
//! signed `i128` coefficient and a base-10 scale. It carries no arithmetic, and
//! this module supplies the operations a grade-point average needs **without
//! introducing a second numeric type**. Every function here takes `Decimal` and
//! returns `Decimal`; no value in this crate is held in any other numeric
//! representation, and `no_float_reaches_the_gpa_path` reads the sources to say
//! so.
//!
//! Binary floating point is what this exists to avoid. A grade-point average is
//! a quotient of sums of products of exact tenths — the one shape where `f64`
//! error is guaranteed rather than unlikely.
//!
//! Every operation is checked. An overflow, a scale past the canonical
//! eighteen, or a division by zero is a typed error, never a panic and never a
//! silently wrapped value.

use core::cmp::Ordering;

use academic_domain::Decimal;

use crate::RecordError;

/// The canonical maximum scale, fixed by [`Decimal::new`].
pub const MAX_SCALE: u8 = 18;

/// Builds an exact integer decimal.
///
/// This is the crate's zero as well as its one, and it returns a `Result` for
/// the same reason every other operation here does: the workspace denies
/// `unwrap_used`, `expect_used`, and `panic`, so a construction that cannot
/// fail in practice still travels as a value rather than as an assumption.
pub fn integer(value: i128) -> Result<Decimal, RecordError> {
    Ok(Decimal::new(value, 0)?)
}

/// Returns the exact zero at scale zero.
pub fn zero() -> Result<Decimal, RecordError> {
    integer(0)
}

/// Raises ten to `exponent`, refusing an exponent that would overflow `i128`.
fn pow10(exponent: u32) -> Result<i128, RecordError> {
    10_i128
        .checked_pow(exponent)
        .ok_or(RecordError::DecimalOverflow)
}

/// Restates `value` at `target` scale without changing the quantity.
///
/// Widening always succeeds within `i128`. Narrowing succeeds only when every
/// digit dropped is a zero, so a rescale never quietly rounds — rounding has
/// exactly one entry point, [`div_round_half_up`], and it is reached only with
/// a scale the versioned grading scheme supplied.
pub fn rescale(value: Decimal, target: u8) -> Result<Decimal, RecordError> {
    if target > MAX_SCALE {
        return Err(RecordError::DecimalScaleTooLarge(target));
    }
    match target.cmp(&value.scale()) {
        Ordering::Equal => Ok(value),
        Ordering::Greater => {
            let factor = pow10(u32::from(target - value.scale()))?;
            let coefficient = value
                .coefficient()
                .checked_mul(factor)
                .ok_or(RecordError::DecimalOverflow)?;
            Ok(Decimal::new(coefficient, target)?)
        }
        Ordering::Less => {
            let factor = pow10(u32::from(value.scale() - target))?;
            if value.coefficient() % factor != 0 {
                return Err(RecordError::DecimalNotExactlyRepresentable { scale: target });
            }
            Ok(Decimal::new(value.coefficient() / factor, target)?)
        }
    }
}

/// Aligns two decimals to their common (larger) scale.
fn align(left: Decimal, right: Decimal) -> Result<(i128, i128, u8), RecordError> {
    let scale = left.scale().max(right.scale());
    let left = rescale(left, scale)?;
    let right = rescale(right, scale)?;
    Ok((left.coefficient(), right.coefficient(), scale))
}

/// Returns `left + right`, exactly.
pub fn add(left: Decimal, right: Decimal) -> Result<Decimal, RecordError> {
    let (left, right, scale) = align(left, right)?;
    let total = left
        .checked_add(right)
        .ok_or(RecordError::DecimalOverflow)?;
    Ok(Decimal::new(total, scale)?)
}

/// Returns `left - right`, exactly.
pub fn sub(left: Decimal, right: Decimal) -> Result<Decimal, RecordError> {
    let (left, right, scale) = align(left, right)?;
    let difference = left
        .checked_sub(right)
        .ok_or(RecordError::DecimalOverflow)?;
    Ok(Decimal::new(difference, scale)?)
}

/// Returns `left * right`, exactly.
///
/// Scales add, which is what makes the product exact: three credits at scale
/// one times a grade point at scale one is a quality point at scale two, with
/// no rounding step anywhere in the multiplication.
pub fn mul(left: Decimal, right: Decimal) -> Result<Decimal, RecordError> {
    let scale = left
        .scale()
        .checked_add(right.scale())
        .ok_or(RecordError::DecimalOverflow)?;
    if scale > MAX_SCALE {
        return Err(RecordError::DecimalScaleTooLarge(scale));
    }
    let coefficient = left
        .coefficient()
        .checked_mul(right.coefficient())
        .ok_or(RecordError::DecimalOverflow)?;
    Ok(Decimal::new(coefficient, scale)?)
}

/// Compares two decimals by quantity rather than by spelling.
///
/// `3` at scale zero and `3.0` at scale one are the same quantity and compare
/// [`Ordering::Equal`]. `Decimal`'s derived `PartialEq` calls them different,
/// which is right for a byte encoding and wrong for arithmetic.
pub fn compare(left: Decimal, right: Decimal) -> Result<Ordering, RecordError> {
    let (left, right, _) = align(left, right)?;
    Ok(left.cmp(&right))
}

/// Whether the value is exactly zero, at any scale.
#[must_use]
pub fn is_zero(value: Decimal) -> bool {
    value.coefficient() == 0
}

/// Divides `numerator` by `denominator` to exactly `scale` digits, rounding
/// half away from zero.
///
/// A grade-point average is a quotient that usually does not terminate, so the
/// scale and the rounding rule are part of the answer rather than an
/// afterthought. Both are carried by the versioned `GradingScheme` and passed
/// in; neither is hard-coded here, which is what lets
/// `gpa_policy_version_matrix` change a scheme's published scale and observe
/// the average change.
///
/// Half away from zero, not banker's rounding: a published academic average is
/// rounded the way a registrar's table is, and a rule that depends on the
/// parity of the last kept digit is not one any official source states.
pub fn div_round_half_up(
    numerator: Decimal,
    denominator: Decimal,
    scale: u8,
) -> Result<Decimal, RecordError> {
    if scale > MAX_SCALE {
        return Err(RecordError::DecimalScaleTooLarge(scale));
    }
    if is_zero(denominator) {
        return Err(RecordError::DivisionByZero);
    }

    // value = (nc / 10^ns) / (dc / 10^ds) = nc * 10^ds / (dc * 10^ns)
    // want r / 10^scale, so r = nc * 10^(ds + scale) / (dc * 10^ns). The net
    // exponent is applied to whichever side keeps it non-negative.
    let net = i32::from(denominator.scale()) + i32::from(scale) - i32::from(numerator.scale());

    let (mut top, mut bottom) = (numerator.coefficient(), denominator.coefficient());
    if net >= 0 {
        let factor = pow10(u32::try_from(net).map_err(|_| RecordError::DecimalOverflow)?)?;
        top = top
            .checked_mul(factor)
            .ok_or(RecordError::DecimalOverflow)?;
    } else {
        let factor = pow10(u32::try_from(-net).map_err(|_| RecordError::DecimalOverflow)?)?;
        bottom = bottom
            .checked_mul(factor)
            .ok_or(RecordError::DecimalOverflow)?;
    }

    let negative = (top < 0) != (bottom < 0);
    let top_magnitude = top.checked_abs().ok_or(RecordError::DecimalOverflow)?;
    let bottom_magnitude = bottom.checked_abs().ok_or(RecordError::DecimalOverflow)?;
    let quotient = top_magnitude / bottom_magnitude;
    let remainder = top_magnitude % bottom_magnitude;
    let doubled = remainder
        .checked_mul(2)
        .ok_or(RecordError::DecimalOverflow)?;
    let rounded = if doubled >= bottom_magnitude {
        quotient
            .checked_add(1)
            .ok_or(RecordError::DecimalOverflow)?
    } else {
        quotient
    };
    let signed = if negative {
        rounded.checked_neg().ok_or(RecordError::DecimalOverflow)?
    } else {
        rounded
    };
    Ok(Decimal::new(signed, scale)?)
}

/// Parses a fixed-point spelling into the canonical decimal.
///
/// This is the crate's one text entry point for a number, and it reuses
/// `academic_transcript::record::parse_decimal` rather than adding a second parser: a
/// credit value that reconciled on one spelling during import must not parse
/// differently here.
pub fn parse(text: &str) -> Result<Decimal, RecordError> {
    Ok(academic_transcript::record::parse_decimal(text)?)
}

/// Renders a decimal in the one spelling this repository hashes.
///
/// Delegates to `academic_transcript::record::canonical_decimal` for the reason
/// [`parse`] delegates: two spellings of one quantity must not reach a digest.
#[must_use]
pub fn render(value: Decimal) -> String {
    academic_transcript::record::canonical_decimal(value)
}
