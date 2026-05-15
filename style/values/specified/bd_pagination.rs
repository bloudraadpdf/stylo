/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe Prince-for-Books pagination tuning surface (F28).
//!
//! These declarative knobs influence multi-pass pagination heuristics:
//! - `-bd-page-fill`, `-bd-change-line-breaks-for-pagination`
//! - `-bd-line-break-choices`, `-bd-forced-breaks`, `-bd-n-lines`
//! - `-bd-resize-adjust`, `-bd-resize-options`
//! - `-bd-spread-length-options`
//! - `-bd-text-wrap`, `-bd-wrap-inside`
//!
//! v1 ships the parse surface only. The paginator integration is
//! a separate workstream (per audit family 28 note).

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::Integer;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value of `-bd-page-fill`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPageFill {
    #[default]
    Auto,
    PreferFigures,
    PreferText,
}

/// Specified value of `-bd-change-line-breaks-for-pagination`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdChangeLineBreaksForPagination {
    #[default]
    Auto,
    Never,
    Within,
    Across,
}

/// Specified value of `-bd-line-break-choices`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdLineBreakChoices {
    #[default]
    Default,
    Greedy,
    Optimal,
}

/// Specified value of `-bd-forced-breaks`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdForcedBreaks {
    #[default]
    Auto,
    Honour,
    Ignore,
}

/// Specified value of `-bd-n-lines`.
///
/// `<integer>` count or `auto`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
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

impl Parse for BdNLines {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        Ok(Self::Count(Integer::parse(context, input)?))
    }
}

/// Specified value of `-bd-resize-adjust`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdResizeAdjust {
    #[default]
    Auto,
    Allow,
    Forbid,
}

/// Specified value of `-bd-resize-options`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdResizeOptions {
    #[default]
    Auto,
    Shrink,
    Grow,
    Both,
    None,
}

/// Specified value of `-bd-spread-length-options`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdSpreadLengthOptions {
    #[default]
    Auto,
    Match,
    Free,
}

/// Specified value of `-bd-text-wrap`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdTextWrap {
    #[default]
    Wrap,
    Nowrap,
    Balance,
    Pretty,
    Stable,
}

/// Specified value of `-bd-wrap-inside`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdWrapInside {
    #[default]
    Auto,
    Avoid,
}
