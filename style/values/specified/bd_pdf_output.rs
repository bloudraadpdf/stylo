/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe PDF output tuning properties (F27).
//!
//! Per-element / inherited knobs that govern how the PDF backend
//! emits content. Most are keyword enums backed by trivial Stylo
//! types; one (`-bd-rasterization-max-size`) carries a positive
//! `<length>` and one (`-bd-rasterization-supersampling`) carries a
//! positive `<number>`. The renderer surface (krilla) consumes them
//! at the paint-to-PDF boundary.
//!
//! All values are PDFreactor-derived per
//! `pdfreactor-inventory.md:386-427`:
//!
//! | Property | PDFreactor source |
//! |----------|--------------------|
//! | `-bd-pdf-text-rendering` | `-ro-pdf-text-rendering` 17569 |
//! | `-bd-paint-reordering` | `-ro-paint-reordering` 17021 |
//! | `-bd-font-embedding-type` | `-ro-font-embedding-type` 15054 |
//! | `-bd-glyph-layout-mode` | `-ro-glyph-layout-mode` 15269 |
//! | `-bd-rasterization` | `-ro-rasterization` 17799 |
//! | `-bd-rasterization-max-size` | `-ro-rasterization-max-size` 17824 |
//! | `-bd-rasterization-supersampling` | `-ro-rasterization-supersampling` 17848 |
//! | `-bd-pdf-shape-optimization` | `-ro-pdf-shape-optimization` 17259 |
//! | `-bd-pdf-passdown-styles` | `-ro-passdown-styles` 17038 |
//! | `-bd-pdf-bookmarks-enabled` | `-ro-bookmarks-enabled` 13330 |

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::NonNegative;
use crate::values::specified::length::NonNegativeLength;
use crate::values::specified::{NonNegativeNumber, Number};
use crate::Zero;
use cssparser::Parser;
use style_traits::ParseError;

/// `-bd-pdf-text-rendering`.
///
/// `auto` defers to the PDF text-rendering default (use glyphs);
/// `text-as-glyphs` emits TJ / Tj operators against the embedded
/// font; `text-as-vector` traces every glyph outline so the
/// document carries no embedded fonts. Vector emission inflates
/// content streams but is required when accessibility is off and
/// the embed surface is constrained.
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
pub enum BdPdfTextRendering {
    #[default]
    Auto,
    TextAsGlyphs,
    TextAsVector,
}

/// `-bd-paint-reordering`.
///
/// PDFreactor reorders draw calls so that text always paints
/// last; `none` disables the reordering and emits content in
/// source order.
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
pub enum BdPaintReordering {
    #[default]
    Auto,
    None,
    /// `text-last` — paint text on top of everything else.
    TextLast,
}

/// `-bd-font-embedding-type`.
///
/// `auto` (initial) defers to the conformance-driven default;
/// `embed` always embeds the whole font; `subset` embeds only the
/// glyphs referenced from the document (default for most flavours);
/// `reference` registers the font in the document but does not
/// embed it (the viewer is expected to find the font locally);
/// `none` opts out of embedding entirely. Strictly conforming
/// PDF/A and PDF/X workflows reject anything except `embed` or
/// `subset`.
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
pub enum BdFontEmbeddingType {
    #[default]
    Auto,
    Embed,
    Subset,
    Reference,
    None,
}

/// `-bd-glyph-layout-mode`.
///
/// `auto` (initial) lets the layout decide; `optical` enables
/// metric-and-kerning-driven layout; `metric` uses only advance
/// widths (faster but worse spacing).
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
pub enum BdGlyphLayoutMode {
    #[default]
    Auto,
    Optical,
    Metric,
}

/// `-bd-rasterization`.
///
/// Forces rasterisation of an element to a bitmap. `auto` defers
/// to the renderer's heuristic (rasterise if the vector content
/// would exceed `-bd-rasterization-max-size`); `always` forces
/// raster output; `never` disables the rasterisation fallback.
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
pub enum BdRasterization {
    #[default]
    Auto,
    Always,
    Never,
}

/// `-bd-pdf-shape-optimization`.
///
/// `auto` (initial) lets the renderer simplify long sequences of
/// straight-line segments where it can; `none` disables the
/// optimisation; `full` forces it even for short sequences.
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
pub enum BdPdfShapeOptimization {
    #[default]
    Auto,
    None,
    Full,
}

/// `-bd-pdf-passdown-styles`.
///
/// PDFreactor proprietary: forces specific style declarations to
/// "pass down" to the PDF emission layer (used for shape merging
/// in their renderer). `auto` (initial) is the safe default; the
/// other keywords are pass-throughs preserved for compatibility.
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
pub enum BdPdfPassdownStyles {
    #[default]
    Auto,
    None,
    All,
}

/// `-bd-pdf-bookmarks-enabled`.
///
/// `auto` (initial) — bookmarks emit when `bookmark-label` is set
/// on the element; `none` disables bookmark emission for the
/// element subtree (PDFreactor uses this on iframes).
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
pub enum BdPdfBookmarksEnabled {
    #[default]
    Auto,
    None,
}

/// `-bd-rasterization-max-size`.
///
/// Length threshold above which the renderer prefers rasterising
/// to vector emission. Zero (initial) disables the threshold —
/// the renderer never rasterises for size reasons.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdRasterizationMaxSize(pub NonNegativeLength);

impl BdRasterizationMaxSize {
    /// Initial value (zero — no threshold).
    #[inline]
    pub fn zero() -> Self {
        Self(NonNegativeLength::zero())
    }
}

impl Parse for BdRasterizationMaxSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(NonNegativeLength::parse(context, input)?))
    }
}

/// `-bd-rasterization-supersampling`.
///
/// Multiplier applied to the raster sample grid when an element
/// falls back to raster output. `1` (initial) disables
/// supersampling; values `>= 1` produce smoother edges at the
/// cost of memory.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdRasterizationSupersampling(pub NonNegativeNumber);

impl BdRasterizationSupersampling {
    /// Initial value (`1`).
    #[inline]
    pub fn one() -> Self {
        Self(NonNegative(Number::new(1.0)))
    }
}

impl Parse for BdRasterizationSupersampling {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(NonNegativeNumber::parse(context, input)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::{ParsingMode, ToCss};

    fn parse_text_rendering(css: &str) -> BdPdfTextRendering {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = ParserContext::new(
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
            .parse_entirely(|input| BdPdfTextRendering::parse(input))
            .expect("text-rendering value should parse")
    }

    #[test]
    fn text_rendering_round_trips() {
        for css in ["auto", "text-as-glyphs", "text-as-vector"] {
            let value = parse_text_rendering(css);
            assert_eq!(value.to_css_string(), css);
        }
    }
}
