/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Overflow Module Level 4 longhands and CSS Inline 3 §6.
//!
//! Implements the `block-ellipsis`, `max-lines`, and `continue`
//! longhands of the `line-clamp` shorthand (Overflow 4 §5), plus
//! `text-box-trim` (Inline 3 §6, with the earlier `leading-trim` spelling):
//!
//! - <https://drafts.csswg.org/css-overflow-4/#line-clamp>
//! - <https://drafts.csswg.org/css-overflow-4/#block-ellipsis>
//! - <https://drafts.csswg.org/css-overflow-4/#max-lines>
//! - <https://drafts.csswg.org/css-overflow-4/#continue>
//! - <https://drafts.csswg.org/css-inline-3/#text-box-trim>
//!
//! These cap block-container line content (`line-clamp` shorthand triplet —
//! `block-ellipsis`, `max-lines`, `continue`) and trim the first/last line
//! leading respectively. Only `block-ellipsis` is inherited; `max-lines` and
//! `continue` are reset properties, as required by their definitions.
//!
//! The shorthand parser lives in `crate::properties::shorthands::line_clamp`.
//! There is deliberately no specified or computed `line-clamp` value:
//! successful parsing immediately produces the three typed longhands, so an
//! independent shorthand value cannot disagree with the cascade result.
//!
//! `MaxLines` stores a private [`PositiveLineCount`] rather than an unrefined
//! `specified::Integer`. Consequently, once parsing succeeds, later stages
//! cannot construct a non-positive line limit. Its computed-side counterpart
//! is declared in `crate::values::computed::overflow_4`, with a manual
//! `ToComputedValue` implementation preserving that proof.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::{Integer, PositiveInteger};
use crate::OwnedStr;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value of the `block-ellipsis` property
/// (<https://drafts.csswg.org/css-overflow-4/#block-ellipsis>).
///
/// Grammar: `none | auto | <string>`. Selects the ellipsis glyph
/// inserted at the truncation boundary. `auto` defers to the UA's
/// content-language-aware default.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BlockEllipsis {
    /// `none` — no glyph is inserted at truncation.
    None,
    /// `auto` — UA-selected default glyph.
    Auto,
    /// `<string>` — author-supplied ellipsis glyph string.
    String(OwnedStr),
}

impl BlockEllipsis {
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

impl Parse for BlockEllipsis {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let s = input.expect_string()?;
        Ok(Self::String(s.as_ref().to_owned().into()))
    }
}

/// A parser-validated positive specified line count.
#[derive(
    Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(transparent)]
pub struct PositiveLineCount(Integer);

impl PositiveLineCount {
    /// Retains the positive-integer proof established by the CSS parser.
    #[inline]
    pub(crate) fn from_positive(value: PositiveInteger) -> Self {
        Self(value.0)
    }

    /// Returns the underlying specified integer for computed-value conversion.
    #[inline]
    pub(crate) fn integer(&self) -> &Integer {
        &self.0
    }

    /// Reconstructs a specified value from its proof-carrying computed value.
    #[inline]
    pub(crate) fn from_computed(
        value: crate::values::computed::overflow_4::PositiveLineCount,
    ) -> Self {
        Self(Integer::new(value.raw()))
    }
}

impl Parse for PositiveLineCount {
    #[inline]
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        PositiveInteger::parse(context, input).map(Self::from_positive)
    }
}

/// Specified value of the `max-lines` property
/// (<https://drafts.csswg.org/css-overflow-4/#max-lines>).
///
/// Grammar: `none | <integer>`. The integer is the maximum number of
/// lines a fragmentation root may produce before triggering the
/// `continue` policy.
#[derive(
    Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum MaxLines {
    /// `none` — no cap.
    None,
    /// `<integer>` — line cap; must be positive.
    Lines(PositiveLineCount),
}

impl MaxLines {
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

impl Parse for MaxLines {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        Ok(Self::Lines(PositiveLineCount::parse(context, input)?))
    }
}

/// Specified value of the `continue` property
/// (<https://drafts.csswg.org/css-overflow-4/#continue>).
///
/// Grammar: `auto | discard`. Selects whether content overflowing the
/// `max-lines` cap is preserved on subsequent fragments (`auto`) or
/// silently discarded (`discard`).
///
/// The Rust type is named [`Continue`] but the longhand keyword
/// `continue` is a Rust reserved word; the generated property module
/// is renamed to `continue_` by `data.py::to_rust_ident`.
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
pub enum Continue {
    /// `auto` — overflow continues into subsequent fragments.
    #[default]
    Auto,
    /// `discard` — overflow is dropped at the truncation boundary.
    Discard,
}

/// Specified value of the `text-box-trim` property
/// (<https://drafts.csswg.org/css-inline-3/#text-box-trim>).
///
/// Current grammar: `none | trim-start | trim-end | trim-both`. The earlier
/// `normal | start | end | both` keywords remain accepted for compatibility.
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
pub enum LeadingTrim {
    /// `none` — preserve leading on both edges (default).
    #[default]
    #[parse(aliases = "none")]
    Normal,
    /// `trim-start` — trim leading from the block-start edge.
    #[parse(aliases = "trim-start")]
    Start,
    /// `trim-end` — trim leading from the block-end edge.
    #[parse(aliases = "trim-end")]
    End,
    /// `trim-both` — trim leading from both edges.
    #[parse(aliases = "trim-both")]
    Both,
}

#[cfg(test)]
mod leading_trim_tests {
    use super::*;
    use cssparser::{Parser, ParserInput};

    fn parse_leading_trim(css: &str) -> LeadingTrim {
        let mut input = ParserInput::new(css);
        Parser::new(&mut input)
            .parse_entirely(LeadingTrim::parse)
            .expect("text-box-trim value should parse")
    }

    #[test]
    fn current_text_box_trim_keywords_map_to_the_existing_typed_states() {
        for (css, expected) in [
            ("none", LeadingTrim::Normal),
            ("trim-start", LeadingTrim::Start),
            ("trim-end", LeadingTrim::End),
            ("trim-both", LeadingTrim::Both),
        ] {
            assert_eq!(parse_leading_trim(css), expected);
        }
    }
}
