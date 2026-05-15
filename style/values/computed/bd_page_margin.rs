/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe asymmetric page margins (F29).

use crate::derives::*;
use crate::values::computed::length::LengthPercentageOrAuto;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_page_margin as specified;

/// Computed value of `-bd-margin-{inside,outside,alt}` /
/// `-bd-inset-{inside,outside}`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdPageMarginEdge(pub LengthPercentageOrAuto);

impl BdPageMarginEdge {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(LengthPercentageOrAuto::Auto)
    }
}

impl ToComputedValue for specified::BdPageMarginEdge {
    type ComputedValue = BdPageMarginEdge;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdPageMarginEdge(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdPageMarginEdge(ToComputedValue::from_computed_value(&computed.0))
    }
}
