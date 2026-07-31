/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for the CSS Overflow 4 `line-clamp` longhands
//! (`block-ellipsis`, `max-lines`, `continue`) and CSS Inline 3
//! (`leading-trim`).
//!
//! Most types pass through via `pub use` of the specified-side
//! re-exports. [`MaxLines`] needs a bespoke computed counterpart because its
//! inner `Integer` field is `specified::Integer` (which doesn't satisfy
//! `ToResolvedValue` / `ToTyped`); the computed mirror stores a plain `i32`
//! instead. `line-clamp` itself has no computed value because it is a
//! shorthand and is expanded before cascade.

use crate::derives::*;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::overflow_4 as specified;

pub use specified::{BlockEllipsis, Continue, LeadingTrim};

/// Computed value of the `max-lines` property.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum MaxLines {
    /// `none` — no cap.
    None,
    /// `<integer>` — line cap.
    Integer(i32),
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
            specified::MaxLines::Integer(i) => MaxLines::Integer(i.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            MaxLines::None => specified::MaxLines::None,
            MaxLines::Integer(i) => {
                specified::MaxLines::Integer(ToComputedValue::from_computed_value(i))
            }
        }
    }
}
