/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for the CSS Overflow 4 `line-clamp` longhands
//! (`block-ellipsis`, `max-lines`, `continue`) and CSS Inline 3
//! (`text-box-trim`).
//!
//! Most types pass through via `pub use` of the specified-side
//! re-exports. [`MaxLines`] needs a bespoke computed counterpart because its
//! specified line count carries a parser-established positivity proof. The
//! computed mirror keeps the integer private and only exposes it as
//! `NonZeroU32`, so downstream code cannot represent or handle a non-positive
//! non-`none` value. `line-clamp` itself has no computed value because it is a
//! shorthand and is expanded before cascade.

use crate::derives::*;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::overflow_4 as specified;
use std::num::NonZeroU32;

pub use specified::{
    BlockEllipsis, Continue, LeadingTrim, TextBoxEdge, TextBoxEdgeOver, TextBoxEdgeUnder,
};

/// A computed line count whose private representation is always positive.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(transparent)]
pub struct PositiveLineCount(i32);

impl PositiveLineCount {
    #[inline]
    fn from_specified(value: &specified::PositiveLineCount, ctx: &Context) -> Self {
        let value = value.integer().to_computed_value(ctx);
        debug_assert!(
            value > 0,
            "specified PositiveLineCount must remain positive"
        );
        Self(value)
    }

    /// Returns the positive line count without exposing an unrefined integer.
    #[inline]
    pub fn get(self) -> NonZeroU32 {
        // SAFETY: the field is private and every constructor consumes a
        // parser-proven positive specified value.
        unsafe { NonZeroU32::new_unchecked(self.0 as u32) }
    }

    #[inline]
    pub(crate) fn raw(self) -> i32 {
        self.0
    }

    /// Converts the legacy zero-or-positive line-clamp representation into a
    /// standard proof-carrying count after `none` has been excluded.
    #[inline]
    pub(crate) fn from_legacy(value: &crate::values::computed::box_::LineClamp) -> Self {
        debug_assert!(!value.is_none());
        let value = *value.value();
        debug_assert!(value > 0, "legacy LineClamp must be positive or none");
        Self(value)
    }
}

/// Computed value of the `max-lines` property.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum MaxLines {
    /// `none` — no cap.
    None,
    /// `<integer>` — line cap.
    Lines(PositiveLineCount),
}

impl MaxLines {
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

impl ToComputedValue for specified::MaxLines {
    type ComputedValue = MaxLines;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::MaxLines::None => MaxLines::None,
            specified::MaxLines::Lines(i) => {
                MaxLines::Lines(PositiveLineCount::from_specified(i, ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            MaxLines::None => specified::MaxLines::None,
            MaxLines::Lines(i) => {
                specified::MaxLines::Lines(specified::PositiveLineCount::from_computed(*i))
            },
        }
    }
}
