/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe hyphenation extensions (F31).

use crate::derives::*;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, Integer, ToComputedValue};
use crate::values::specified::bd_hyphenation as specified;
use crate::OwnedSlice;

pub use specified::BdLinebreakMagic;

/// Computed value of `-bd-hyphenate-limit-lines`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdHyphenateLimitLines {
    /// `no-limit` (initial).
    NoLimit,
    /// `<integer>` cap on consecutive hyphenated lines.
    Count(Integer),
}

impl BdHyphenateLimitLines {
    /// Initial value.
    #[inline]
    pub fn no_limit() -> Self {
        Self::NoLimit
    }
}

impl ToComputedValue for specified::BdHyphenateLimitLines {
    type ComputedValue = BdHyphenateLimitLines;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdHyphenateLimitLines::NoLimit => BdHyphenateLimitLines::NoLimit,
            specified::BdHyphenateLimitLines::Count(i) => {
                BdHyphenateLimitLines::Count(i.to_computed_value(ctx))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdHyphenateLimitLines::NoLimit => specified::BdHyphenateLimitLines::NoLimit,
            BdHyphenateLimitLines::Count(i) => {
                specified::BdHyphenateLimitLines::Count(ToComputedValue::from_computed_value(i))
            }
        }
    }
}

/// Computed value of `-bd-hyphenate-patterns`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdHyphenatePatterns {
    /// `none` — bundled built-in patterns only.
    None,
    /// `url(...)` — explicit patterns dictionary.
    Url(ComputedUrl),
}

impl BdHyphenatePatterns {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

impl ToComputedValue for specified::BdHyphenatePatterns {
    type ComputedValue = BdHyphenatePatterns;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdHyphenatePatterns::None => BdHyphenatePatterns::None,
            specified::BdHyphenatePatterns::Url(u) => {
                BdHyphenatePatterns::Url(u.to_computed_value(ctx))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdHyphenatePatterns::None => specified::BdHyphenatePatterns::None,
            BdHyphenatePatterns::Url(u) => {
                specified::BdHyphenatePatterns::Url(ToComputedValue::from_computed_value(u))
            }
        }
    }
}

/// Computed value of `-bd-hyphenate-lines`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdHyphenateLines {
    /// `auto` (initial).
    Auto,
    /// `<integer>+` line-count alternation.
    Counts(#[css(iterable)] OwnedSlice<Integer>),
}

impl BdHyphenateLines {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl ToComputedValue for specified::BdHyphenateLines {
    type ComputedValue = BdHyphenateLines;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdHyphenateLines::Auto => BdHyphenateLines::Auto,
            specified::BdHyphenateLines::Counts(counts) => BdHyphenateLines::Counts(
                OwnedSlice::from(
                    counts
                        .iter()
                        .map(|c| c.to_computed_value(ctx))
                        .collect::<Vec<_>>(),
                ),
            ),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdHyphenateLines::Auto => specified::BdHyphenateLines::Auto,
            BdHyphenateLines::Counts(counts) => specified::BdHyphenateLines::Counts(
                OwnedSlice::from(
                    counts
                        .iter()
                        .map(ToComputedValue::from_computed_value)
                        .collect::<Vec<_>>(),
                ),
            ),
        }
    }
}

/// Computed value of `-bd-hyphenate-word-length`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdHyphenateWordLength {
    /// `auto` (initial).
    Auto,
    /// `<integer>` minimum word length to hyphenate.
    Length(Integer),
}

impl BdHyphenateWordLength {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl ToComputedValue for specified::BdHyphenateWordLength {
    type ComputedValue = BdHyphenateWordLength;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdHyphenateWordLength::Auto => BdHyphenateWordLength::Auto,
            specified::BdHyphenateWordLength::Length(i) => {
                BdHyphenateWordLength::Length(i.to_computed_value(ctx))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdHyphenateWordLength::Auto => specified::BdHyphenateWordLength::Auto,
            BdHyphenateWordLength::Length(i) => specified::BdHyphenateWordLength::Length(
                ToComputedValue::from_computed_value(i),
            ),
        }
    }
}
