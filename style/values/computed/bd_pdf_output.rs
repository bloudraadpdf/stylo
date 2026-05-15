/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for the F27 PDF output tuning properties.
//!
//! The keyword enums are identity-computed (re-exported). The two
//! length / number-bearing properties compute their inner value
//! through the standard length / number walk via a derive.

use crate::derives::*;
use crate::values::computed::length::NonNegativeLength;
use crate::values::computed::{Context, NonNegativeNumber, ToComputedValue};
use crate::values::specified::bd_pdf_output as specified;
use crate::Zero;

pub use crate::values::specified::bd_pdf_output::{
    BdFontEmbeddingType, BdGlyphLayoutMode, BdPaintReordering, BdPdfBookmarksEnabled,
    BdPdfPassdownStyles, BdPdfShapeOptimization, BdPdfTextRendering, BdRasterization,
};

/// Computed value of `-bd-rasterization-max-size`.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdRasterizationMaxSize(pub NonNegativeLength);

impl BdRasterizationMaxSize {
    /// Initial value (zero — no threshold).
    #[inline]
    pub fn zero() -> Self {
        Self(NonNegativeLength::zero())
    }
}

impl ToComputedValue for specified::BdRasterizationMaxSize {
    type ComputedValue = BdRasterizationMaxSize;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdRasterizationMaxSize(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self(ToComputedValue::from_computed_value(&computed.0))
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
