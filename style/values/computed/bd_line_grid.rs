/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe baseline-grid / line-snap (F8).

use crate::derives::*;
use crate::values::computed::length::Length;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_line_grid as specified;

pub use specified::{BdLineGrid, BdLineSnap, BdLineStackingStrategy};

/// Computed value of `-bd-baseline-grid`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdBaselineGrid {
    /// `none` — no baseline grid.
    None,
    /// `auto` — derive grid step from `line-height`.
    Auto,
    /// `<length>` — explicit grid step.
    Length(Length),
}

impl BdBaselineGrid {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToComputedValue for specified::BdBaselineGrid {
    type ComputedValue = BdBaselineGrid;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdBaselineGrid::None => BdBaselineGrid::None,
            specified::BdBaselineGrid::Auto => BdBaselineGrid::Auto,
            specified::BdBaselineGrid::Length(l) => {
                BdBaselineGrid::Length(l.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdBaselineGrid::None => specified::BdBaselineGrid::None,
            BdBaselineGrid::Auto => specified::BdBaselineGrid::Auto,
            BdBaselineGrid::Length(l) => {
                specified::BdBaselineGrid::Length(ToComputedValue::from_computed_value(l))
            },
        }
    }
}
