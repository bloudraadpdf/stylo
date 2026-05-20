/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Overflow Module Level 4 longhands and CSS Inline 3 §6.
//!
//! Implements `line-clamp`, `block-ellipsis`, `max-lines`,
//! `continue` (Overflow 4 §5) and `leading-trim` (Inline 3 §6):
//!
//! - <https://drafts.csswg.org/css-overflow-4/#line-clamp>
//! - <https://drafts.csswg.org/css-overflow-4/#block-ellipsis>
//! - <https://drafts.csswg.org/css-overflow-4/#max-lines>
//! - <https://drafts.csswg.org/css-overflow-4/#continue>
//! - <https://drafts.csswg.org/css-inline-3/#leading-trim>
//!
//! These cap block-container line content (line-clamp shorthand
//! triplet — block-ellipsis, max-lines, continue) and trim the
//! first/last line leading respectively. They cascade through
//! `inherited_text` so descendants inherit the cap policy.
//!
//! The standardised `line-clamp` is distinct from the WebKit-prefixed
//! `-webkit-line-clamp`: the standardised property carries an optional
//! ellipsis `<string>`; the legacy WebKit variant carries only an
//! integer.
//!
//! `StandardLineClamp` and `MaxLines` both store `specified::Integer`
//! which does not derive `ToResolvedValue` / `ToTyped`; consequently
//! their computed-side counterparts are bespoke types declared in
//! `crate::values::computed::overflow_4`, with manual `ToComputedValue`
//! impls bridging the two.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::Integer;
use crate::OwnedStr;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value of the standardised `line-clamp` property
/// (<https://drafts.csswg.org/css-overflow-4/#line-clamp>).
///
/// Grammar: `none | <integer> <string>?`. The integer must be
/// positive; the optional string overrides the block-ellipsis glyph.
/// Setting `line-clamp` is equivalent to setting the triplet
/// `max-lines`, `continue: discard`, and `block-ellipsis`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum StandardLineClamp {
    /// `none` — no cap; default.
    None,
    /// `<integer> <string>?` — cap at N lines with optional ellipsis.
    Lines {
        /// Maximum number of lines retained.
        count: Integer,
        /// Optional `<string>` overriding the block-ellipsis glyph.
        ellipsis: Option<OwnedStr>,
    },
}

impl StandardLineClamp {
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

impl Parse for StandardLineClamp {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let count = crate::values::specified::PositiveInteger::parse(context, input)?.0;
        let ellipsis = input
            .try_parse(|i| {
                let s = i.expect_string()?;
                Ok::<OwnedStr, ParseError<'i>>(s.as_ref().to_owned().into())
            })
            .ok();
        Ok(Self::Lines { count, ellipsis })
    }
}

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
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        if input
            .try_parse(|i| i.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        let s = input.expect_string()?;
        Ok(Self::String(s.as_ref().to_owned().into()))
    }
}

/// Specified value of the `max-lines` property
/// (<https://drafts.csswg.org/css-overflow-4/#max-lines>).
///
/// Grammar: `none | <integer>`. The integer is the maximum number of
/// lines a fragmentation root may produce before triggering the
/// `continue` policy.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum MaxLines {
    /// `none` — no cap.
    None,
    /// `<integer>` — line cap; must be positive.
    Integer(Integer),
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
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let i = crate::values::specified::PositiveInteger::parse(context, input)?.0;
        Ok(Self::Integer(i))
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

/// Specified value of the `leading-trim` property
/// (<https://drafts.csswg.org/css-inline-3/#leading-trim>).
///
/// Grammar: `normal | start | end | both`. Controls whether the
/// half-leading on the first / last line of a block container is
/// trimmed against the cap-height / x-height baseline.
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
    /// `normal` — preserve leading on both edges (default).
    #[default]
    Normal,
    /// `start` — trim leading from the block-start edge.
    Start,
    /// `end` — trim leading from the block-end edge.
    End,
    /// `both` — trim leading from both edges.
    Both,
}
