/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-float-*` Prince-style page-float modifier properties
//! (Family 18).
//!
//! Native moegoe fork-extension surface mirroring Prince's
//! `float-policy`, `float-tail`, `float-modifier`,
//! `float-defer-column`, and `float-defer-page` (the page-defer
//! property is already covered by the existing `float-defer`
//! alias). Each tunes Prince's page-floats placement behaviour
//! (see `docs/reference-manuals/prince.md:3236–3530`).
//!
//! All properties apply to floated boxes only; they are not
//! inherited. The renderer consumes them via the page-floats
//! placement pass in `moegoe-page`.

use crate::derives::*;

/// Specified value of `-bd-float-policy`.
///
/// Prince's `float-policy` controls when a floated box that does
/// not fit on the current page is released to a deferred slot vs.
/// allowed to bleed into the next page.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
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
pub enum BdFloatPolicy {
    #[default]
    Normal,
    KeepWithText,
    KeepInline,
    KeepInlineAndText,
}

/// Specified value of `-bd-float-tail`.
///
/// `<integer>` — minimum number of trailing-text characters that
/// must follow a floated reference before the float is released.
/// `auto` (initial) — Prince default behaviour.
#[derive(
    Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdFloatTail {
    /// Prince default behaviour (no explicit tail).
    Auto,
    /// `<integer>` — explicit non-negative tail.
    Length(u32),
}

impl Default for BdFloatTail {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

impl BdFloatTail {
    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl crate::parser::Parse for BdFloatTail {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        let n = input.expect_integer()?;
        if n < 0 {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Length(n as u32))
    }
}

/// Specified value of `-bd-float-modifier`.
///
/// Prince's `float-modifier` tweaks how the float interacts with
/// adjacent floats and inline content. The keyword set tracks
/// Prince's documented values (`docs/reference-manuals/prince.md`
/// §`float-modifier`).
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
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
pub enum BdFloatModifier {
    #[default]
    None,
    NextPage,
    NextColumn,
    LeftAlternating,
    RightAlternating,
    InsideAlternating,
    OutsideAlternating,
}

/// Specified value of `-bd-float-defer-column`.
///
/// `none | <integer>` — column-defer counterpart to
/// `float-defer-page`. `<integer>` defers the float by N columns;
/// `last` releases the float to the last column of the multicol.
#[derive(
    Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdFloatDeferColumn {
    /// Do not defer.
    None,
    /// Release to the last column.
    Last,
    /// Defer by N columns.
    Columns(i32),
}

impl Default for BdFloatDeferColumn {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdFloatDeferColumn {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl crate::parser::Parse for BdFloatDeferColumn {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        if input
            .try_parse(|i| i.expect_ident_matching("last"))
            .is_ok()
        {
            return Ok(Self::Last);
        }
        let n = input.expect_integer()?;
        Ok(Self::Columns(n))
    }
}
