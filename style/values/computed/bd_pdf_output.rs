/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for the F27 PDF output tuning properties.
//!
//! The keyword enums are identity-computed (re-exported). The two
//! number-bearing properties compute their inner value through the
//! standard number walk.

use crate::derives::*;
use crate::values::computed::{Context, NonNegativeNumber, ToComputedValue};
use crate::values::specified::bd_pdf_output as specified;

pub use crate::values::specified::bd_pdf_output::{
    BdFontEmbeddingType, BdGlyphLayoutMode, BdPaintReordering, BdPdfBookmarksEnabled,
    BdPdfPassdownStyles, BdPdfRasterAccessibility, BdPdfShapeOptimization, BdPdfTextRendering,
    BdRasterization,
};

/// Computed value of `-bd-rasterization-max-size`.
///
/// Mirrors the specified `auto | none | <number>` value space; the
/// number is megapixels (PDFreactor `pdf-rasterization-max-size`).
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdRasterizationMaxSize {
    /// `auto` — defer to the renderer's built-in megapixel default.
    Auto,
    /// `none` — never apply a megapixel ceiling.
    None,
    /// `<number>` — megapixel ceiling.
    Megapixels(NonNegativeNumber),
}

impl BdRasterizationMaxSize {
    /// Initial value (`auto`).
    #[inline]
    pub fn initial() -> Self {
        BdRasterizationMaxSize::Auto
    }
}

impl ToComputedValue for specified::BdRasterizationMaxSize {
    type ComputedValue = BdRasterizationMaxSize;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdRasterizationMaxSize::Auto => BdRasterizationMaxSize::Auto,
            specified::BdRasterizationMaxSize::None => BdRasterizationMaxSize::None,
            specified::BdRasterizationMaxSize::Megapixels(n) => {
                BdRasterizationMaxSize::Megapixels(n.to_computed_value(ctx))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdRasterizationMaxSize::Auto => specified::BdRasterizationMaxSize::Auto,
            BdRasterizationMaxSize::None => specified::BdRasterizationMaxSize::None,
            BdRasterizationMaxSize::Megapixels(n) => specified::BdRasterizationMaxSize::Megapixels(
                ToComputedValue::from_computed_value(n),
            ),
        }
    }
}

/// Computed value of `-bd-rasterization-supersampling`.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdRasterizationSupersampling(pub NonNegativeNumber);

impl BdRasterizationSupersampling {
    /// Initial value (`1`).
    #[inline]
    pub fn one() -> Self {
        Self(NonNegativeNumber::from(1.0))
    }
}

impl ToComputedValue for specified::BdRasterizationSupersampling {
    type ComputedValue = BdRasterizationSupersampling;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdRasterizationSupersampling(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self(ToComputedValue::from_computed_value(&computed.0))
    }
}
