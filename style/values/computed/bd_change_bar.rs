/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-change-bar-*` revision-mark
//! properties (Family 16).

use crate::derives::*;
use crate::values::computed::color::Color;
use crate::values::computed::length::{LengthPercentage, NonNegativeLength};
use crate::values::computed::{Context, Percentage, ToComputedValue};
use crate::values::specified::bd_change_bar as specified;
use crate::values::specified::border::LineWidth;
use app_units::Au;

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

/// Computed value of `-bd-change-bar-offset`.
///
/// Retains the percentage form when the author wrote one — the
/// percentage is resolved against the page-margin width / column
/// gap in `moegoe_page::change_bars`, which has the page geometry
/// the cascade does not.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdChangeBarOffset(pub LengthPercentage);

impl BdChangeBarOffset {
    /// Initial value (`25%`).
    #[inline]
    pub fn quarter_percent() -> Self {
        Self(LengthPercentage::new_percent(Percentage(0.25)))
    }
}

impl ToComputedValue for specified::BdChangeBarOffset {
    type ComputedValue = BdChangeBarOffset;

    #[inline]
    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdChangeBarOffset(self.0.to_computed_value(ctx))
    }

    #[inline]
    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdChangeBarOffset(ToComputedValue::from_computed_value(&computed.0))
    }
}

/// Computed value of `-bd-change-bar-width`.
///
/// `thin` / `medium` / `thick` resolve to the same px values as
/// `<border-width>` (1 / 3 / 5 px per CSS Backgrounds-3 §3.1);
/// explicit lengths pass through. The resolved value is an absolute
/// non-negative length in CSS px.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdChangeBarWidth(pub NonNegativeLength);

impl BdChangeBarWidth {
    /// Initial value (`medium`, 3 CSS px).
    #[inline]
    pub fn medium() -> Self {
        Self(NonNegativeLength::new(3.0))
    }
}

impl ToComputedValue for specified::BdChangeBarWidth {
    type ComputedValue = BdChangeBarWidth;

    #[inline]
    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        let au: Au = self.0.to_computed_value(ctx);
        BdChangeBarWidth(NonNegativeLength::new(au.to_f32_px()))
    }

    #[inline]
    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdChangeBarWidth(LineWidth::Length(ToComputedValue::from_computed_value(
            &computed.0,
        )))
    }
}
