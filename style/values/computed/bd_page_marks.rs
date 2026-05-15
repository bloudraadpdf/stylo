/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-page-colorbar-*` /
//! `-bd-page-print-mark-set` (Family 20).
//!
//! Keyword-only specified types reuse the specified module's
//! enums. The URL-bearing `BdColorBarPosition` lifts through a
//! manual `ToComputedValue` so the inner URL is converted to the
//! computed `CssUrl`. `BdColorBarOffset` is the computed
//! `Length`.

use crate::derives::*;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::generics::url::GenericUrlOrNone;
use crate::values::specified::bd_page_marks as specified;

pub use specified::BdPrintMarkSet;

/// Computed value of `-bd-page-colorbar-*`.
///
/// Note: `ComputedUrl` is not `ToShmem` (it carries an `Arc`); the
/// derive is therefore omitted here.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped,
)]
#[repr(C, u8)]
pub enum BdColorBarPosition {
    /// `none`.
    None,
    /// `auto`.
    Auto,
    /// `<url>`.
    Url(GenericUrlOrNone<ComputedUrl>),
}

impl BdColorBarPosition {
    /// Initial value.
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

impl ToComputedValue for specified::BdColorBarPosition {
    type ComputedValue = BdColorBarPosition;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdColorBarPosition::None => BdColorBarPosition::None,
            specified::BdColorBarPosition::Auto => BdColorBarPosition::Auto,
            specified::BdColorBarPosition::Url(u) => {
                BdColorBarPosition::Url(u.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(c: &Self::ComputedValue) -> Self {
        match c {
            BdColorBarPosition::None => specified::BdColorBarPosition::None,
            BdColorBarPosition::Auto => specified::BdColorBarPosition::Auto,
            BdColorBarPosition::Url(u) => specified::BdColorBarPosition::Url(
                ToComputedValue::from_computed_value(u),
            ),
        }
    }
}

// `-bd-page-colorbar-offset` resolves to the predefined
// `computed::Length` type directly.
