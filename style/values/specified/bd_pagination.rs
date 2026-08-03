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
//! Tier 4 §A.4.5 — Prince-for-Books pagination tuning additions:
//! - `-bd-pdf-signature` (document-level integer; pad blank pages
//!   to a multiple of N at end of pagination)
//! - `-bd-blank-page-content` (document-level string-or-`normal`;
//!   content emitted on signature padding blank pages)
//! - `-bd-keep-with-previous` (per-element; mirror of
//!   `prince-keep-with-next`)
//! - `-bd-orphans-fragments` (per-element; minimum lines on the
//!   previous page when a fragmentable element splits)

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

/// Specified value of `-bd-pdf-signature`.
///
/// `<integer>` — pages per signature fold for book imposition. The
/// paginator pads with blank pages at end of pagination so the page
/// count is a multiple of N. `auto` (initial) disables padding.
///
/// Per the Prince-for-Books extension, signature is a document-level
/// knob: only the value cascaded onto `:root` is consulted. The
/// property is parsed as a regular `style` rule longhand so authors
/// can write `:root { -bd-pdf-signature: 4; }`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfSignature {
    /// `auto` (initial) — no signature padding.
    Auto,
    /// `<integer>` — pages per signature fold (must be >= 1).
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

impl Parse for BdPdfSignature {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        Ok(Self::Count(Integer::parse_positive(context, input)?))
    }
}

/// Specified value of `-bd-blank-page-content`.
///
/// Controls what is emitted on the blank pages inserted by the
/// signature-padding pass. `normal` (initial) emits an empty page;
/// a `<string>` value is rendered as an explicit text marker so
/// authors can label the padding pages (e.g. `"This page
/// intentionally left blank."`).
///
/// Document-level: only the value cascaded onto `:root` is
/// consulted (mirrors `-bd-pdf-signature`).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdBlankPageContent {
    /// `normal` (initial) — empty blank page.
    Normal,
    /// `<string>` — text marker rendered on the blank page.
    Text(crate::OwnedStr),
}

impl BdBlankPageContent {
    /// Initial value (`normal`).
    #[inline]
    pub fn normal() -> Self {
        Self::Normal
    }
}

impl Parse for BdBlankPageContent {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("normal"))
            .is_ok()
        {
            return Ok(Self::Normal);
        }
        let text = input.expect_string()?.as_ref().to_owned();
        Ok(Self::Text(text.into()))
    }
}

/// Specified value of `-bd-keep-with-previous`.
///
/// Mirror of `prince-keep-with-next`: when set to `always`, the
/// paginator tries to keep this element on the same page as its
/// preceding in-flow sibling.
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
pub enum BdKeepWithPrevious {
    #[default]
    Auto,
    Always,
}

/// Specified value of `-bd-orphans-fragments`.
///
/// `<integer>` — minimum number of lines that must remain on the
/// previous page when a fragmentable element splits across pages.
/// Mirrors Prince's `prince-orphans-fragments`. `auto` (initial)
/// defers to the regular `orphans` value.
///
/// Inherits, like `orphans`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdOrphansFragments {
    /// `auto` (initial) — defer to `orphans`.
    Auto,
    /// `<integer>` — minimum lines on previous page (>= 1).
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

impl Parse for BdOrphansFragments {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        Ok(Self::Count(Integer::parse_positive(context, input)?))
    }
}
