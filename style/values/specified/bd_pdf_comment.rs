/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-comment*` annotation properties and
//! `-bd-pdf-link-border` (G7).
//!
//! Native fork-extension surface for PDF annotations. The
//! `-bd-pdf-comment*` family declares per-element comment-style
//! annotations (Text / Highlight / Underline / Strikeout /
//! Squiggly). `-bd-pdf-link-border` styles the implicit Link
//! annotation produced by `<a href>`. v1 ships the parse surface
//! and IR plumbing; emission of non-link annotation subtypes is
//! gated on krilla upstream support and emits a
//! `RenderWarning::UnsupportedPdfFeature` until that lands.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::color::Color;
use crate::values::specified::length::NonNegativeLength;
use crate::OwnedStr;
use cssparser::Parser;

/// Specified value of `-bd-pdf-comment`.
///
/// Selects the annotation subtype the element opts into. `none`
/// (initial) — the element produces no comment annotation.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BdPdfCommentKind {
    #[default]
    None,
    Note,
    Highlight,
    Underline,
    Strikeout,
    Squiggly,
}

impl BdPdfCommentKind {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Specified value of `-bd-pdf-comment-icon`.
///
/// Maps to PDF Text annotation icon name (`/Name` entry).
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BdPdfCommentIcon {
    #[default]
    Note,
    Comment,
    Key,
    Help,
    NewParagraph,
    Paragraph,
    Insert,
}

/// Specified value of `-bd-pdf-comment-state`.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BdPdfCommentState {
    #[default]
    Closed,
    Open,
}

/// Specified value of `-bd-pdf-comment-contents` and
/// `-bd-pdf-comment-title`.
///
/// v1 grammar: `none | <string>`. `attr()` / `content()` /
/// concatenation are reserved for a follow-up — the IR layer
/// already supports richer content tokens, but the parse rule
/// stays minimal until renderer plumbing catches up.
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
pub enum BdPdfCommentString {
    /// `none` — the slot is empty.
    None,
    /// `<string>` — literal annotation text.
    Literal(OwnedStr),
}

impl BdPdfCommentString {
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

impl Parse for BdPdfCommentString {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-comment-colour`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfCommentColour {
    /// `auto` — viewer default.
    Auto,
    /// `<color>` — explicit annotation `/C` array.
    Colour(Color),
}

impl BdPdfCommentColour {
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

impl Parse for BdPdfCommentColour {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        Ok(Self::Colour(Color::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-link-border`.
///
/// `none` — no border on the implicit Link annotation.
/// `<length> <color>` — border with explicit width and colour.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfLinkBorder {
    /// `none` — no border.
    None,
    /// `<length> <color>` — explicit border.
    Border {
        /// Border width (non-negative).
        width: NonNegativeLength,
        /// Border colour.
        colour: Color,
    },
}

impl BdPdfLinkBorder {
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

impl Parse for BdPdfLinkBorder {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let width = NonNegativeLength::parse(context, input)?;
        let colour = Color::parse(context, input)?;
        Ok(Self::Border { width, colour })
    }
}
