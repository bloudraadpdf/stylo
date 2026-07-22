/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-barcode-*` (Family 15).
//!
//! Specified-to-computed is the identity for the keyword and
//! enum variants. Colour and length variants reuse the standard
//! computed types.

use crate::derives::*;
use crate::values::computed::length::NonNegativeLengthPercentage;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_barcode as specified;
use crate::{OwnedSlice, OwnedStr};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use specified::{
    BdBarcodeAffix, BdBarcodeCheckDigitMode, BdBarcodeCompositeType, BdBarcodeEccLevel,
    BdBarcodeEncoding, BdBarcodeFontFamily, BdBarcodeHrPosition, BdBarcodeReaderInit,
    BdBarcodeStructuredAppend, BdBarcodeType, BdQrEccLetter,
};

/// Computed `-bd-barcode-content`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdBarcodeContent {
    /// No authored payload; the renderer derives it from the host element.
    None,
    /// Literal string fragments.
    Strings(OwnedSlice<OwnedStr>),
    /// A URL resolved against the stylesheet base URL.
    Url(ComputedUrl),
}

impl Default for BdBarcodeContent {
    fn default() -> Self {
        Self::None
    }
}

impl BdBarcodeContent {
    /// Whether the value supplies no authored payload.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for BdBarcodeContent {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Strings(strings) => {
                for (index, string) in strings.iter().enumerate() {
                    if index != 0 {
                        dest.write_char(' ')?;
                    }
                    string.to_css(dest)?;
                }
                Ok(())
            },
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdBarcodeContent {
    type ComputedValue = BdBarcodeContent;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        match self {
            Self::None => BdBarcodeContent::None,
            Self::Strings(strings) => BdBarcodeContent::Strings(strings.clone()),
            Self::Url(url) => BdBarcodeContent::Url(url.to_computed_value(context)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdBarcodeContent::None => Self::None,
            BdBarcodeContent::Strings(strings) => Self::Strings(strings.clone()),
            BdBarcodeContent::Url(url) => Self::Url(ToComputedValue::from_computed_value(url)),
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
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
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
            specified::BdBarcodeSize::Square(l) => BdBarcodeSize::Square(l.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(c: &Self::ComputedValue) -> Self {
        match c {
            BdBarcodeSize::Auto => specified::BdBarcodeSize::Auto,
            BdBarcodeSize::Square(l) => {
                specified::BdBarcodeSize::Square(ToComputedValue::from_computed_value(l))
            },
        }
    }
}

// `-bd-barcode-structured-append-position`, `-bd-barcode-font-size`,
// `-bd-barcode-letter-spacing`, and `-bd-barcode-symbol-width`
// reuse their predefined longhand types directly.
