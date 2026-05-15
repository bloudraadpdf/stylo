/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for bookmark-target + `-bd-pdf-link-type` (F10).

use crate::derives::*;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_bookmark as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use specified::BdPdfLinkType;

/// Computed value of `bookmark-target`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue)]
#[repr(C, u8)]
pub enum BookmarkTarget {
    /// `none` — no link.
    None,
    /// `<url>` — link to an external or internal URL.
    Url(ComputedUrl),
    /// `<integer>` — counter index into a generated list.
    Counter(i32),
}

impl style_traits::ToTyped for BookmarkTarget {}

impl BookmarkTarget {
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

impl ToCss for BookmarkTarget {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::None => dest.write_str("none"),
            Self::Url(u) => u.to_css(dest),
            Self::Counter(i) => i.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BookmarkTarget {
    type ComputedValue = BookmarkTarget;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BookmarkTarget::None => BookmarkTarget::None,
            specified::BookmarkTarget::Url(u) => BookmarkTarget::Url(u.to_computed_value(ctx)),
            specified::BookmarkTarget::Counter(i) => BookmarkTarget::Counter(*i),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BookmarkTarget::None => specified::BookmarkTarget::None,
            BookmarkTarget::Url(u) => {
                specified::BookmarkTarget::Url(ToComputedValue::from_computed_value(u))
            }
            BookmarkTarget::Counter(i) => specified::BookmarkTarget::Counter(*i),
        }
    }
}
