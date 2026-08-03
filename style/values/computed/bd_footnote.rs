/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe footnote-area extensions (F6).

use crate::derives::*;
use crate::values::computed::length::LengthPercentage;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_footnote as specified;

pub use specified::{BdFootnoteFragmentation, FloatPlacement, FootnoteStylePosition};

/// Computed value of `-bd-footnote-rule-length`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdFootnoteRuleLength(pub LengthPercentage);

impl BdFootnoteRuleLength {
    /// Initial value (`100%`).
    #[inline]
    pub fn full() -> Self {
        Self(LengthPercentage::new_percent(
            crate::values::computed::Percentage::hundred(),
        ))
    }
}

impl ToComputedValue for specified::BdFootnoteRuleLength {
    type ComputedValue = BdFootnoteRuleLength;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdFootnoteRuleLength(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdFootnoteRuleLength(ToComputedValue::from_computed_value(&computed.0))
    }
}
