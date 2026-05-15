/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe hyphenation extensions (F31).
//!
//! - `-bd-hyphenate-limit-lines`: max number of consecutive
//!   hyphenated lines.
//! - `-bd-hyphenate-patterns`: URL to TeX-style patterns dictionary.
//! - `-bd-hyphenate-lines`: alternating no-hyphenate / hyphenate counts.
//! - `-bd-hyphenate-word-length`: deprecated alias of
//!   `hyphenate-limit-chars` (PDFreactor compat).
//! - `-bd-linebreak-magic`: Prince typographic-quality knob.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::url::SpecifiedUrl;
use crate::values::specified::Integer;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value of `-bd-hyphenate-limit-lines`.
///
/// `no-limit | <integer>`. Default `no-limit`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdHyphenateLimitLines {
    /// `no-limit` (initial).
    NoLimit,
    /// `<integer>` cap on consecutive hyphenated lines.
    Count(Integer),
}

impl BdHyphenateLimitLines {
    /// Initial value (`no-limit`).
    #[inline]
    pub fn no_limit() -> Self {
        Self::NoLimit
    }

    /// Whether the value is `no-limit`.
    #[inline]
    pub fn is_no_limit(&self) -> bool {
        matches!(self, Self::NoLimit)
    }
}

impl Parse for BdHyphenateLimitLines {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("no-limit"))
            .is_ok()
        {
            return Ok(Self::NoLimit);
        }
        Ok(Self::Count(Integer::parse(context, input)?))
    }
}

/// Specified value of `-bd-hyphenate-patterns`.
///
/// `none | url(...)`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdHyphenatePatterns {
    /// `none` — bundled built-in patterns only.
    None,
    /// `url(...)` — explicit patterns dictionary.
    Url(SpecifiedUrl),
}

impl BdHyphenatePatterns {
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

impl Parse for BdHyphenatePatterns {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        Ok(Self::Url(SpecifiedUrl::parse(context, input)?))
    }
}

/// Specified value of `-bd-hyphenate-lines`.
///
/// `<integer>+` — alternating no-hyphen / hyphen run lengths.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdHyphenateLines {
    /// `auto` (initial).
    Auto,
    /// `<integer>+` line-count alternation.
    Counts(#[css(iterable)] crate::OwnedSlice<Integer>),
}

impl BdHyphenateLines {
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

impl Parse for BdHyphenateLines {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let mut counts: Vec<Integer> = Vec::new();
        loop {
            match input.try_parse(|i| Integer::parse(context, i)) {
                Ok(v) => counts.push(v),
                Err(_) => break,
            }
        }
        if counts.is_empty() {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Counts(crate::OwnedSlice::from(counts)))
    }
}

/// Specified value of `-bd-hyphenate-word-length`.
///
/// PDFreactor compat — deprecated alias of `hyphenate-limit-chars`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
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

impl Parse for BdHyphenateWordLength {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        Ok(Self::Length(Integer::parse(context, input)?))
    }
}

/// Specified value of `-bd-linebreak-magic`.
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
pub enum BdLinebreakMagic {
    #[default]
    Auto,
    None,
    All,
}
