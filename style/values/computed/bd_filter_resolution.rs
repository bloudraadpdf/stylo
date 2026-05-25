/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed value of `-bd-filter-resolution`.
//!
//! The contained `<resolution>` is mapped through the standard
//! specified-to-computed `Resolution::to_computed_value` collapse to
//! dppx; `auto` is identity-computed.

use crate::derives::*;
use crate::values::computed::{Context, Resolution, ToComputedValue};
use crate::values::specified::bd_filter_resolution as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

/// Computed value of `-bd-filter-resolution`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdFilterResolution {
    /// `auto` — defer to the backend default raster density.
    Auto,
    /// `<resolution>` — explicit per-element density (computed to dppx).
    Resolution(Resolution),
}

impl BdFilterResolution {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto` (initial — no cascade override).
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToCss for BdFilterResolution {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Resolution(r) => r.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdFilterResolution {
    type ComputedValue = BdFilterResolution;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdFilterResolution::Auto,
            Self::Resolution(r) => BdFilterResolution::Resolution(r.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdFilterResolution::Auto => Self::Auto,
            BdFilterResolution::Resolution(r) => {
                Self::Resolution(ToComputedValue::from_computed_value(r))
            },
        }
    }
}
