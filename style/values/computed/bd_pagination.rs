/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe Prince-for-Books pagination tuning (F28).

use crate::derives::*;
use crate::values::computed::{Context, Integer, ToComputedValue};
use crate::values::specified::bd_pagination as specified;

pub use specified::{
    BdChangeLineBreaksForPagination, BdForcedBreaks, BdLineBreakChoices, BdPageFill,
    BdResizeAdjust, BdResizeOptions, BdSpreadLengthOptions, BdTextWrap, BdWrapInside,
};

/// Computed value of `-bd-n-lines`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdNLines {
    /// `auto` (initial).
    Auto,
    /// `<integer>` line count.
    Count(Integer),
}

impl BdNLines {
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

impl ToComputedValue for specified::BdNLines {
    type ComputedValue = BdNLines;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdNLines::Auto => BdNLines::Auto,
            specified::BdNLines::Count(i) => BdNLines::Count(i.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdNLines::Auto => specified::BdNLines::Auto,
            BdNLines::Count(i) => specified::BdNLines::Count(ToComputedValue::from_computed_value(i)),
        }
    }
}
