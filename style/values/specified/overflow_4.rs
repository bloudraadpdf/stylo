/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Overflow Module Level 4 longhands and CSS Inline 3 §6.
//!
//! Implements the `block-ellipsis`, `max-lines`, and `continue`
//! longhands of the `line-clamp` shorthand (Overflow 4 §5), plus
//! `leading-trim` (Inline 3 §6):
//!
//! - <https://drafts.csswg.org/css-overflow-4/#line-clamp>
//! - <https://drafts.csswg.org/css-overflow-4/#block-ellipsis>
//! - <https://drafts.csswg.org/css-overflow-4/#max-lines>
//! - <https://drafts.csswg.org/css-overflow-4/#continue>
//! - <https://drafts.csswg.org/css-inline-3/#leading-trim>
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
//! `MaxLines` stores `specified::Integer`, which does not derive
//! `ToResolvedValue` / `ToTyped`; consequently its computed-side counterpart
//! is declared in `crate::values::computed::overflow_4`, with a manual
//! `ToComputedValue` implementation bridging the two.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::Integer;
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
