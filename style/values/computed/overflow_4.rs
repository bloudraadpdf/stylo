/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed positive line counts and text box edges.

use crate::derives::*;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::overflow_4 as specified;
use std::num::NonZeroU32;

pub use specified::{
    BlockEllipsis, Continue, LeadingTrim, TextBoxEdge, TextEdgeOver, TextEdgeUnder,
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
}

/// Computed maximum line count.
pub type MaxLines = crate::values::generics::box_::GenericMaxLines<PositiveLineCount>;

impl ToComputedValue for specified::PositiveLineCount {
    type ComputedValue = PositiveLineCount;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        PositiveLineCount::from_specified(self, ctx)
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self::from_computed(*computed)
    }
}
