/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for the F4 per-mark tuning properties.

use crate::derives::*;
use crate::values::computed::length::NonNegativeLength;
use crate::values::computed::{Color, Context, ToComputedValue};
use crate::values::specified::bd_page_marks as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

/// Computed value of a `-bd-page-*-mark-length` / `-offset` property.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPageMarkLength(pub NonNegativeLength);

impl BdPageMarkLength {
    /// Build a value from a points constant.
    #[inline]
    pub fn from_pt(pt: f32) -> Self {
        // CSS px = 1/96in, pt = 1/72in. Convert at the boundary.
        Self(NonNegativeLength::new(pt * 96.0 / 72.0))
    }
}

impl ToComputedValue for specified::BdPageMarkLength {
    type ComputedValue = BdPageMarkLength;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdPageMarkLength(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self(ToComputedValue::from_computed_value(&computed.0))
    }
}

/// Computed value of `-bd-page-marks-colour`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPageMarksColour {
    /// `auto`.
    Auto,
    /// Concrete computed colour.
    Colour(Color),
}

impl BdPageMarksColour {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToCss for BdPageMarksColour {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Colour(c) => c.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdPageMarksColour {
    type ComputedValue = BdPageMarksColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdPageMarksColour::Auto,
            Self::Colour(c) => BdPageMarksColour::Colour(c.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPageMarksColour::Auto => Self::Auto,
            BdPageMarksColour::Colour(c) => {
                Self::Colour(ToComputedValue::from_computed_value(c))
            },
        }
    }
}
