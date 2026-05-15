/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-change-bar-*` revision-mark
//! properties (Family 16).

use crate::derives::*;
use crate::values::computed::color::Color;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_change_bar as specified;

pub use specified::{BdChangeBarAlign, BdChangeBarExclusion, BdChangeBarName};

/// Computed value of `-bd-change-bar-colour`.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdChangeBarColour {
    /// `auto` — fall back to `currentcolor`.
    Auto,
    /// Explicit colour.
    Colour(Color),
}

impl BdChangeBarColour {
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

impl ToComputedValue for specified::BdChangeBarColour {
    type ComputedValue = BdChangeBarColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdChangeBarColour::Auto => BdChangeBarColour::Auto,
            specified::BdChangeBarColour::Colour(c) => {
                BdChangeBarColour::Colour(c.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdChangeBarColour::Auto => specified::BdChangeBarColour::Auto,
            BdChangeBarColour::Colour(c) => specified::BdChangeBarColour::Colour(
                ToComputedValue::from_computed_value(c),
            ),
        }
    }
}

// `-bd-change-bar-offset` / `-bd-change-bar-width` use the
// predefined `Length` / `NonNegativeLength` types directly.
