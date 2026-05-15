/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-barcode-*` (Family 15).
//!
//! Specified-to-computed is the identity for the keyword and
//! enum variants. Colour and length variants reuse the standard
//! computed types.

use crate::derives::*;
use crate::values::computed::color::Color;
use crate::values::computed::length::NonNegativeLengthPercentage;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_barcode as specified;

pub use specified::{
    BdBarcodeAffix, BdBarcodeCheckDigitMode, BdBarcodeCompositeType, BdBarcodeContent,
    BdBarcodeEccLevel, BdBarcodeEncoding, BdBarcodeFontFamily, BdBarcodeHrPosition,
    BdBarcodeReaderInit, BdBarcodeStructuredAppend, BdBarcodeType, BdQrEccLetter,
};

/// Computed `-bd-barcode-colour`.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdBarcodeColour {
    /// `auto` — fall back to `currentcolor`.
    Auto,
    /// Explicit colour.
    Colour(Color),
}

impl BdBarcodeColour {
    /// Initial value.
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl ToComputedValue for specified::BdBarcodeColour {
    type ComputedValue = BdBarcodeColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdBarcodeColour::Auto => BdBarcodeColour::Auto,
            specified::BdBarcodeColour::Colour(c) => BdBarcodeColour::Colour(c.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(c: &Self::ComputedValue) -> Self {
        match c {
            BdBarcodeColour::Auto => specified::BdBarcodeColour::Auto,
            BdBarcodeColour::Colour(c) => specified::BdBarcodeColour::Colour(
                ToComputedValue::from_computed_value(c),
            ),
        }
    }
}

/// Computed `-bd-barcode-size`.
///
/// Note: computed `NonNegativeLengthPercentage` is a union over
/// `LengthPercentage` and does not implement `ToShmem` (the
/// underlying computed `LengthPercentage` is a `#[repr(transparent)]`
/// union); `ToShmem` is omitted here for that reason. The
/// renderer never round-trips computed `-bd-barcode-size` through
/// shared memory.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped,
)]
#[repr(C, u8)]
pub enum BdBarcodeSize {
    /// `auto`.
    Auto,
    /// Explicit edge length.
    Square(NonNegativeLengthPercentage),
}

impl BdBarcodeSize {
    /// Initial value.
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl ToComputedValue for specified::BdBarcodeSize {
    type ComputedValue = BdBarcodeSize;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdBarcodeSize::Auto => BdBarcodeSize::Auto,
            specified::BdBarcodeSize::Square(l) => {
                BdBarcodeSize::Square(l.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(c: &Self::ComputedValue) -> Self {
        match c {
            BdBarcodeSize::Auto => specified::BdBarcodeSize::Auto,
            BdBarcodeSize::Square(l) => specified::BdBarcodeSize::Square(
                ToComputedValue::from_computed_value(l),
            ),
        }
    }
}

// `-bd-barcode-structured-append-position`, `-bd-barcode-font-size`,
// `-bd-barcode-letter-spacing`, and `-bd-barcode-symbol-width`
// reuse their predefined longhand types directly.
