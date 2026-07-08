/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-source` / `-bd-source-page` / `-bd-source-area` cluster
//! (F30) — embed an external PDF page inside a CSS box.
//!
//! PDFreactor matrix lines 18133/18149/18161 expose `-ro-source`,
//! `-ro-source-page`, and `-ro-source-area`; the cluster turns a
//! styled element into a PDF-replaced surface that renders one page
//! of a referenced external PDF over the element's content box.
//!
//! * `-bd-source: none | url(<pdf-url>)` — the source PDF. `none`
//!   (initial) suppresses embedding; the element renders as a normal
//!   styled box.
//! * `-bd-source-page: <integer>` — 1-based index of the page inside
//!   the source PDF to embed. Initial value `1`.
//! * `-bd-source-area: content-box | inset(<length>{4})` — clipping
//!   rectangle applied in the source PDF's coordinate space.
//!   `content-box` (initial) keeps the full source page;
//!   `inset(<top> <right> <bottom> <left>)` crops the page by the
//!   specified lengths on each side (lengths in CSS absolute units
//!   that resolve to PDF points at computation time).
//!
//! Per-element; not inherited. The renderer's paint pass walks the
//! cascade-resolved map (populated by
//! `StyleEngine::extract_bd_source_overrides`) and emits a
//! `PaintCommand::DrawPdfPage` over each bearing element's content
//! box. The PDF backend resolves the URL via the embedder's
//! `ResourceLoader` and forwards the page bytes through bladsy's
//! `Surface::draw_pdf_page` API.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::GreaterThanOrEqualToOne;
use crate::values::specified::length::NonNegativeLength;
use crate::values::specified::url::SpecifiedUrl;
use crate::values::specified::{Integer, PositiveInteger};
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// Specified value of `-bd-source`.
///
/// `none` (initial) — the element is not a PDF-source replaced
/// surface; cascade walks contribute no entry. `url(<pdf-url>)` —
/// the renderer fetches the URL via the embedder's `ResourceLoader`
/// and paints the selected page over the element's content box.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdSource {
    /// `none` (initial) — no source PDF.
    None,
    /// `url(<pdf-url>)` — embed the named PDF.
    Url(SpecifiedUrl),
}

impl BdSource {
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

impl ToCss for BdSource {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl Parse for BdSource {
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
        Ok(Self::Url(SpecifiedUrl::parse(context, input)?))
    }
}

/// Specified value of `-bd-source-page`.
///
/// Wraps a `PositiveInteger` (>= 1). Initial value is `1` per the
/// PDFreactor semantics on `-ro-source-page` — when no page is
/// authored the renderer embeds page one.
///
/// `ToComputedValue` and `ToResolvedValue` are implemented manually
/// because deriving them via the inner `PositiveInteger` flips the
/// associated `ComputedValue` over to the `GreaterThanOrEqualToOne<i32>`
/// shape parley/stylo derives produce, which fails the
/// "computed type matches specified type" round-trip the property
/// generator emits for identity-computed wrappers. Identity-compute
/// instead.
#[derive(
    Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped,
)]
#[repr(C)]
pub struct BdSourcePage(pub PositiveInteger);

impl crate::values::computed::ToComputedValue for BdSourcePage {
    type ComputedValue = Self;

    #[inline]
    fn to_computed_value(&self, _ctx: &crate::values::computed::Context) -> Self {
        *self
    }

    #[inline]
    fn from_computed_value(computed: &Self) -> Self {
        *computed
    }
}

impl crate::values::resolved::ToResolvedValue for BdSourcePage {
    type ResolvedValue = Self;

    #[inline]
    fn to_resolved_value(self, _ctx: &crate::values::resolved::Context) -> Self {
        self
    }

    #[inline]
    fn from_resolved_value(resolved: Self) -> Self {
        resolved
    }
}

impl BdSourcePage {
    /// Initial value (`1`).
    #[inline]
    pub fn one() -> Self {
        Self(GreaterThanOrEqualToOne(Integer::new(1)))
    }

    /// Returns the underlying 1-based page index.
    #[inline]
    pub fn value(&self) -> i32 {
        (self.0).0.value()
    }
}

impl ToCss for BdSourcePage {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        self.0.to_css(dest)
    }
}

impl Parse for BdSourcePage {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(PositiveInteger::parse(context, input)?))
    }
}

/// Specified value of `-bd-source-area`.
///
/// `content-box` (initial) — embed the full source page. `Inset` —
/// crop the source page by `top`, `right`, `bottom`, `left` lengths
/// expressed in the source PDF's coordinate space. The renderer
/// converts each length to PDF points at paint time.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdSourceArea {
    /// `content-box` (initial) — full source page, no cropping.
    ContentBox,
    /// `inset(<top> <right> <bottom> <left>)` — crop the source page
    /// by the four non-negative lengths.
    Inset {
        /// Top inset.
        top: NonNegativeLength,
        /// Right inset.
        right: NonNegativeLength,
        /// Bottom inset.
        bottom: NonNegativeLength,
        /// Left inset.
        left: NonNegativeLength,
    },
}

impl BdSourceArea {
    /// Initial value (`content-box`).
    #[inline]
    pub fn content_box() -> Self {
        Self::ContentBox
    }

    /// Whether the value is the initial `content-box`.
    #[inline]
    pub fn is_content_box(&self) -> bool {
        matches!(self, Self::ContentBox)
    }
}

impl ToCss for BdSourceArea {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::ContentBox => dest.write_str("content-box"),
            Self::Inset {
                top,
                right,
                bottom,
                left,
            } => {
                dest.write_str("inset(")?;
                top.to_css(dest)?;
                dest.write_char(' ')?;
                right.to_css(dest)?;
                dest.write_char(' ')?;
                bottom.to_css(dest)?;
                dest.write_char(' ')?;
                left.to_css(dest)?;
                dest.write_char(')')
            }
        }
    }
}

impl Parse for BdSourceArea {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("content-box"))
            .is_ok()
        {
            return Ok(Self::ContentBox);
        }
        input.expect_function_matching("inset")?;
        input.parse_nested_block(|input| {
            // Accept the CSS Shapes Level 1 1-to-4 length shorthand
            // pattern. `inset(<a>)` -> top=right=bottom=left=a;
            // `inset(<a> <b>)` -> top=bottom=a, right=left=b;
            // `inset(<a> <b> <c>)` -> top=a, right=left=b, bottom=c;
            // `inset(<a> <b> <c> <d>)` -> top=a, right=b, bottom=c,
            // left=d. Percentages and `calc()` are intentionally
            // rejected — the inset addresses the SOURCE PDF's
            // coordinate space, where percentages have no defined
            // containing block.
            let top = NonNegativeLength::parse(context, input)?;
            let right = input
                .try_parse(|i| NonNegativeLength::parse(context, i))
                .unwrap_or_else(|_| top.clone());
            let bottom = input
                .try_parse(|i| NonNegativeLength::parse(context, i))
                .unwrap_or_else(|_| top.clone());
            let left = input
                .try_parse(|i| NonNegativeLength::parse(context, i))
                .unwrap_or_else(|_| right.clone());
            Ok(Self::Inset {
                top,
                right,
                bottom,
                left,
            })
        })
    }
}

