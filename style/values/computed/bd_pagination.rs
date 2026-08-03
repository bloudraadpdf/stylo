/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe Prince-for-Books pagination tuning (F28).

use crate::derives::*;
use crate::values::computed::{Context, Integer, ToComputedValue};
use crate::values::specified::bd_pagination as specified;

pub use specified::{
    BdChangeLineBreaksForPagination, BdForcedBreaks, BdKeepWithPrevious, BdLineBreakChoices,
    BdPageFill, BdResizeAdjust, BdResizeOptions, BdSpreadLengthOptions, BdTextWrap, BdWrapInside,
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
            BdNLines::Count(i) => {
                specified::BdNLines::Count(ToComputedValue::from_computed_value(i))
            },
        }
    }
}

/// Computed value of `-bd-pdf-signature`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfSignature {
    /// `auto` (initial) — no signature padding.
    Auto,
    /// `<integer>` — pages per signature fold.
    Count(Integer),
}

impl BdPdfSignature {
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

impl ToComputedValue for specified::BdPdfSignature {
    type ComputedValue = BdPdfSignature;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfSignature::Auto => BdPdfSignature::Auto,
            specified::BdPdfSignature::Count(i) => BdPdfSignature::Count(i.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfSignature::Auto => specified::BdPdfSignature::Auto,
            BdPdfSignature::Count(i) => {
                specified::BdPdfSignature::Count(ToComputedValue::from_computed_value(i))
            },
        }
    }
}

/// Computed value of `-bd-blank-page-content`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdBlankPageContent {
    /// `normal` (initial) — empty page.
    Normal,
    /// `<string>` — text marker.
    Text(crate::OwnedStr),
}

impl BdBlankPageContent {
    /// Initial value (`normal`).
    #[inline]
    pub fn normal() -> Self {
        Self::Normal
    }
}

impl ToComputedValue for specified::BdBlankPageContent {
    type ComputedValue = BdBlankPageContent;

    fn to_computed_value(&self, _ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdBlankPageContent::Normal => BdBlankPageContent::Normal,
            specified::BdBlankPageContent::Text(s) => BdBlankPageContent::Text(s.clone()),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdBlankPageContent::Normal => specified::BdBlankPageContent::Normal,
            BdBlankPageContent::Text(s) => specified::BdBlankPageContent::Text(s.clone()),
        }
    }
}

/// Computed value of `-bd-orphans-fragments`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdOrphansFragments {
    /// `auto` (initial) — defer to `orphans`.
    Auto,
    /// `<integer>` — minimum lines on previous page.
    Count(Integer),
}

impl BdOrphansFragments {
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

impl ToComputedValue for specified::BdOrphansFragments {
    type ComputedValue = BdOrphansFragments;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdOrphansFragments::Auto => BdOrphansFragments::Auto,
            specified::BdOrphansFragments::Count(i) => {
                BdOrphansFragments::Count(i.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdOrphansFragments::Auto => specified::BdOrphansFragments::Auto,
            BdOrphansFragments::Count(i) => {
                specified::BdOrphansFragments::Count(ToComputedValue::from_computed_value(i))
            },
        }
    }
}
