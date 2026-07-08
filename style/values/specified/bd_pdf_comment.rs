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
//! gated on bladsy upstream support and emits a
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

/// Specified value of `-bd-pdf-comment-open` (formerly
/// `-bd-pdf-comment-state`).
///
/// Drives the PDF `/Open` flag on a Text annotation — whether the
/// pop-up is initially shown when the page is opened. The original
/// `-bd-pdf-comment-state` name has been reclaimed for the
/// review-state model (`/State` + `/StateModel`); the open/closed
/// keyword pair lives on the renamed longhand.
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
pub enum BdPdfCommentOpen {
    #[default]
    Closed,
    Open,
}

/// Specified value of `-bd-pdf-comment-state` (PDF `/State`,
/// ISO 32000-2 §12.5.6.4). Together with [`BdPdfCommentStateModel`]
/// drives the review-state markup on a comment annotation.
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
    None,
    Marked,
    Unmarked,
    Accepted,
    Rejected,
    Cancelled,
    Completed,
}

/// Specified value of `-bd-pdf-comment-state-model` (PDF
/// `/StateModel`, ISO 32000-2 §12.5.6.4).
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
pub enum BdPdfCommentStateModel {
    #[default]
    Marked,
    Review,
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

/// Specified value of `-bd-pdf-comment-author` (F22).
///
/// `auto | <string>`. `auto` defers to the render-time signed-in user
/// (mirroring Prince `-prince-pdf-annotation-author` behaviour).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfCommentAuthor {
    /// `auto` — defer to render-time author.
    Auto,
    /// `<string>` — explicit author name.
    Literal(OwnedStr),
}

impl BdPdfCommentAuthor {
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

impl Parse for BdPdfCommentAuthor {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-comment-{createdate,modifydate}` (F22).
///
/// `auto | <string>`. `auto` defers to the render timestamp. The
/// `<string>` form is parsed verbatim and validated downstream
/// (ISO 8601 / PDF D: format).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfCommentDate {
    /// `auto` — defer to render timestamp.
    Auto,
    /// `<string>` — literal timestamp.
    Literal(OwnedStr),
}

impl BdPdfCommentDate {
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

impl Parse for BdPdfCommentDate {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-comment-subject` (PDF `/Subj`,
/// ISO 32000-2 §12.5.6.4 Table 169). `auto` (initial) suppresses
/// `/Subj` so the viewer falls back to its default; a literal
/// `<string>` projects verbatim onto the annotation dictionary.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfCommentSubject {
    /// `auto` — no `/Subj` emission.
    Auto,
    /// `<string>` — explicit subject line.
    Literal(OwnedStr),
}

impl BdPdfCommentSubject {
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

impl Parse for BdPdfCommentSubject {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-comment-date-format`. Advisory format
/// string used when `-bd-pdf-comment-date` resolves to the render-time
/// timestamp (see PDFreactor `-ro-comment-dateformat`); mirrors
/// Java `SimpleDateFormat` syntax. `none` (initial) leaves the
/// renderer's default ISO 32000-2 §7.9.4 PDF date format untouched.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfCommentDateFormat {
    /// `none` — renderer default (`D:YYYYMMDDhhmmssZ`).
    None,
    /// `<string>` — explicit format spec.
    Literal(OwnedStr),
}

impl BdPdfCommentDateFormat {
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

impl Parse for BdPdfCommentDateFormat {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-comment-position` (F22).
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
pub enum BdPdfCommentPosition {
    #[default]
    Auto,
    Anchor,
    Body,
    Margin,
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

/// Tier 3 §A.3.4 — `-bd-pdf-link-border-style` longhand.
///
/// Selects the line shape PDF emits in the `/BS << /S … >>` sub-
/// dictionary on the resulting `/Link` annotation
/// (ISO 32000-2 §12.5.4 Table 165). `none` (initial) suppresses
/// the `/BS` slot entirely so the viewer falls back to the
/// legacy `/Border` array. `solid` writes `/S /S`; `dashed`
/// writes `/S /D` plus the default `/D [3 3]` dash pattern;
/// `underline` writes `/S /U`; `inset` writes `/S /I`.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
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
pub enum BdPdfLinkBorderStyle {
    #[default]
    None,
    Solid,
    Dashed,
    /// `dotted` — the renderer paints the link border as a tight dot
    /// pattern. Wired through to a PDF `/BS << /S /D /D [1 1] >>`
    /// border-style dictionary on the resulting `/Link` annotation
    /// (ISO 32000-2 §12.5.4); the moegoe-side wire-through owns the
    /// dash-array emission so this enum only carries the variant.
    Dotted,
    Underline,
    Inset,
}

impl BdPdfLinkBorderStyle {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(self) -> bool {
        matches!(self, Self::None)
    }
}

/// Tier 3 §A.3.4 — `-bd-pdf-link-area` longhand.
///
/// Selects which rectangle the renderer emits as the `/Rect`
/// (and `/QuadPoints`) of the resulting `/Link` annotation.
/// `border-box` (initial) covers the full border-box including
/// padding and borders; `content-box` shrinks to the content
/// rectangle; `text` emits one quadrilateral per visual line
/// (interaction with F23-3's per-line quad-points pipeline).
#[repr(u8)]
#[derive(
    Clone,
    Copy,
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
pub enum BdPdfLinkArea {
    #[default]
    BorderBox,
    ContentBox,
    Text,
}

impl BdPdfLinkArea {
    /// Whether the value is at its initial `border-box`.
    #[inline]
    pub fn is_border_box(self) -> bool {
        matches!(self, Self::BorderBox)
    }
}

/// Tier 3 §A.3.4 — `-bd-pdf-link-border-color` longhand.
///
/// Drives the `/C [r g b]` colour array on the `/Link`
/// annotation dictionary (ISO 32000-2 §12.5.6.5). `auto` (the
/// initial) leaves the slot empty so the renderer falls back to
/// either the `-bd-pdf-link-border` shorthand's colour or the
/// viewer default; an explicit `<color>` value flattens
/// `currentcolor` / `color-mix()` via the cascade reader.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfLinkBorderColor {
    /// `auto` — defer to the shorthand or viewer default.
    Auto,
    /// Explicit `<color>` value.
    Colour(Color),
}

impl BdPdfLinkBorderColor {
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

impl Parse for BdPdfLinkBorderColor {
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

/// Tier 3 §A.3.4 — `-bd-pdf-link-border-width` longhand.
///
/// Drives the third entry of the legacy `/Border [hr vr w]`
/// array plus the `/BS << /W … >>` sub-dictionary on the
/// `/Link` annotation (ISO 32000-2 §12.5.4 Table 165). `auto`
/// (the initial) leaves the slot empty so the renderer falls
/// back to the `-bd-pdf-link-border` shorthand's width or `0`;
/// an explicit `<length>` overrides at the longhand level.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfLinkBorderWidth {
    /// `auto` — defer to the shorthand or `0` default.
    Auto,
    /// Explicit `<length>` value (non-negative).
    Length(NonNegativeLength),
}

impl BdPdfLinkBorderWidth {
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

impl Parse for BdPdfLinkBorderWidth {
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
        Ok(Self::Length(NonNegativeLength::parse(context, input)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::parser::ParserContext;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::{ParsingMode, ToCss};

    fn parse_link_border_style(css: &str) -> BdPdfLinkBorderStyle {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let _context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfLinkBorderStyle::parse(input))
            .expect("link border style should parse")
    }

    #[test]
    fn link_border_style_dotted_round_trips() {
        let value = parse_link_border_style("dotted");
        assert_eq!(value, BdPdfLinkBorderStyle::Dotted);
        assert_eq!(value.to_css_string(), "dotted");
    }

    #[test]
    fn link_border_style_all_variants_round_trip() {
        for css in ["none", "solid", "dashed", "dotted", "underline", "inset"] {
            let value = parse_link_border_style(css);
            assert_eq!(value.to_css_string(), css);
        }
    }
}
