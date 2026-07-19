/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe sidenote extensions (F7).

use crate::derives::*;
use crate::values::computed::length::LengthPercentage;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_sidenote as specified;

pub use specified::{
    BdFloatReferenceSidenote, BdSidenoteAlign, BdSidenoteAlignment,
    BdSidenoteAvoid, BdSidenoteSide,
};

/// Computed value of `-bd-sidenote-offset`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdSidenoteOffset(pub LengthPercentage);

impl BdSidenoteOffset {
    /// Initial value (`0`).
    #[inline]
    pub fn zero() -> Self {
        use crate::Zero;
        Self(LengthPercentage::new_percent(crate::values::computed::Percentage::zero()))
    }
}

impl ToComputedValue for specified::BdSidenoteOffset {
    type ComputedValue = BdSidenoteOffset;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdSidenoteOffset(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdSidenoteOffset(ToComputedValue::from_computed_value(&computed.0))
    }
}
