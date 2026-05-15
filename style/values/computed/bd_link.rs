/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe link-styling (F11).

use crate::derives::*;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_link as specified;

pub use specified::BdLinkArea;

/// Computed value of `-bd-link`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdLink {
    /// `none` — no link annotation.
    None,
    /// `auto` (initial) — defer to renderer.
    Auto,
    /// `url(...)` — explicit link target.
    Url(ComputedUrl),
}

impl BdLink {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToComputedValue for specified::BdLink {
    type ComputedValue = BdLink;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdLink::None => BdLink::None,
            specified::BdLink::Auto => BdLink::Auto,
            specified::BdLink::Url(u) => BdLink::Url(u.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdLink::None => specified::BdLink::None,
            BdLink::Auto => specified::BdLink::Auto,
            BdLink::Url(u) => specified::BdLink::Url(ToComputedValue::from_computed_value(u)),
        }
    }
}
