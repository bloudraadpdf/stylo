/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for CSS Rhythmic Sizing Module Level 1 longhands.
//!
//! All four keyword properties (`block-step-insert`, `block-step-align`,
//! `block-step-round`) are identity-computed; `block-step-size` swaps
//! the specified `NonNegativeLength` for its computed equivalent via a
//! manual `ToComputedValue` walk.

use crate::derives::*;
use crate::values::computed::length::NonNegativeLength;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::rhythm_1 as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use specified::{BlockStepAlign, BlockStepInsert, BlockStepRound};

/// Computed value of `block-step-size`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BlockStepSize {
    /// `none` — initial; opt-out of block-step sizing.
    None,
    /// Computed length quantum.
    Length(NonNegativeLength),
}

impl BlockStepSize {
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

impl ToCss for BlockStepSize {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Length(l) => l.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BlockStepSize {
    type ComputedValue = BlockStepSize;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BlockStepSize::None => BlockStepSize::None,
            specified::BlockStepSize::Length(l) => {
                BlockStepSize::Length(l.to_computed_value(ctx))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BlockStepSize::None => specified::BlockStepSize::None,
            BlockStepSize::Length(l) => {
                specified::BlockStepSize::Length(ToComputedValue::from_computed_value(l))
            }
        }
    }
}
