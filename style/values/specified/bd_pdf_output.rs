/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe PDF output tuning properties (F27).
//!
//! Per-element / inherited knobs that govern how the PDF backend
//! emits content. Most are keyword enums backed by trivial Stylo
//! types; `-bd-rasterization-max-size` carries `auto | none | <number>`
//! (megapixels), and `-bd-rasterization-supersampling` carries a
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
//! | `-bd-pdf-raster-accessibility` | moegoe-native (F27 R6b ActualText overlay) |
//! | `-bd-pdf-shape-optimization` | `-ro-pdf-shape-optimization` 17259 |
//! | `-bd-pdf-passdown-styles` | `-ro-passdown-styles` 17038 |
//! | `-bd-pdf-bookmarks-enabled` | `-ro-bookmarks-enabled` 13330 |

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::NonNegative;
use crate::values::specified::{NonNegativeNumber, Number};
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
/// Per the PDFreactor manual §`-ro-paint-reordering`, this property
/// controls the **CSS painting-layer assignment** of floated,
/// positioned, and transformed elements. The value space is
/// `normal | avoid` (mirrored from PDFreactor's `-ro-paint-reordering`).
///
/// - `normal` (initial): default CSS 2.1 / CSS Position stacking rules
///   apply. Floats without stacking-context styles paint in layer 4;
///   positioned/transformed elements with `z-index: 0|auto` paint in
///   layer 6.
/// - `avoid`: those same elements paint in their natural in-flow
///   layer (layer 3 for non-inline parts, layer 5 for inline parts)
///   when no higher-priority stacking-context style is present
///   (explicit `z-index`, CSS filter, opacity < 1, etc.). The
///   intent is to keep text in its natural layer so PDF viewer
///   text selection / extraction reads correctly.
///
/// `text-last` (a moegoe-invented variant) was removed in the
/// 2026-05-17 spec correction — it had no PDFreactor counterpart
/// and implied a content-stream reordering pass that does not exist.
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
    Normal,
    Avoid,
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

/// `-bd-pdf-raster-accessibility`.
///
/// Controls whether captured text under a force-rasterised element
/// (cascade resolved `-bd-rasterization: always`) is re-emitted as
/// an invisible ActualText overlay after the rasterised image (PDF
/// text rendering mode 3 — `Tr 3`, ISO 32000-2 §9.3.6 Table 105).
///
/// - `none` (initial): the rasterised image is the sole output for
///   the element subtree — text is pixels-only, not selectable /
///   extractable / searchable. Mirrors PDFreactor's default raster
///   behaviour.
/// - `actual-text`: after the rasterised image is emitted, every
///   captured `DrawText` command from the element subtree is
///   re-played onto the PDF surface with `TextRendering::Invisible`
///   (krilla 71e71db59). The glyphs remain in the content stream
///   for accessibility tools, text selection, and search per
///   ISO 32000-2 §14.9.4 ("ActualText" accessibility overlay).
///
/// No PDFreactor counterpart — this is a moegoe-native fork
/// extension (F27 R6b).
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
pub enum BdPdfRasterAccessibility {
    #[default]
    None,
    ActualText,
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
/// Per PDFreactor §`pdf-rasterization-max-size` (`-ro-rasterization-max-size`
/// id 17824) the value space is `auto | none | <number>` where `<number>`
/// is a megapixel ceiling clamping the rasterised pixmap. The unit is
/// **megapixels** — a bare dimensionless number, *not* a CSS `<length>`.
///
/// - `auto` (initial): the renderer chooses (defers to its built-in
///   default — 2 megapixels in moegoe).
/// - `none`: no megapixel clamp — the renderer never reduces a raster
///   buffer for size reasons (a hard pixel-dimension backstop in the
///   renderer still applies).
/// - `<number>`: clamp each pixmap side so the resulting buffer holds
///   at most `<number>` megapixels.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdRasterizationMaxSize {
    /// `auto` — defer to the renderer's built-in megapixel default.
    Auto,
    /// `none` — never apply a megapixel ceiling.
    None,
    /// `<number>` — megapixel ceiling.
    Megapixels(NonNegativeNumber),
}

impl BdRasterizationMaxSize {
    /// Initial value (`auto`).
    #[inline]
    pub fn initial() -> Self {
        BdRasterizationMaxSize::Auto
    }
}

impl Parse for BdRasterizationMaxSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        // `auto | none | <number>`. The number form must come last so
        // identifiers parse as keywords first.
        if let Ok(value) = input.try_parse(|input| {
            input.expect_ident_matching("auto")?;
            Ok::<_, ParseError<'i>>(BdRasterizationMaxSize::Auto)
        }) {
            return Ok(value);
        }
        if let Ok(value) = input.try_parse(|input| {
            input.expect_ident_matching("none")?;
            Ok::<_, ParseError<'i>>(BdRasterizationMaxSize::None)
        }) {
            return Ok(value);
        }
        Ok(BdRasterizationMaxSize::Megapixels(
            NonNegativeNumber::parse(context, input)?,
        ))
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

    fn parse_rasterization_max_size(css: &str) -> BdRasterizationMaxSize {
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
            .parse_entirely(|input| BdRasterizationMaxSize::parse(&context, input))
            .expect("rasterization-max-size value should parse")
    }

    #[test]
    fn rasterization_max_size_accepts_auto_none_and_number() {
        assert!(matches!(
            parse_rasterization_max_size("auto"),
            BdRasterizationMaxSize::Auto
        ));
        assert!(matches!(
            parse_rasterization_max_size("none"),
            BdRasterizationMaxSize::None
        ));
        match parse_rasterization_max_size("2") {
            BdRasterizationMaxSize::Megapixels(n) => {
                assert!((n.0.get() - 2.0).abs() < f32::EPSILON);
            }
            other => panic!("expected Megapixels(2), got {other:?}"),
        }
        match parse_rasterization_max_size("0.5") {
            BdRasterizationMaxSize::Megapixels(n) => {
                assert!((n.0.get() - 0.5).abs() < f32::EPSILON);
            }
            other => panic!("expected Megapixels(0.5), got {other:?}"),
        }
    }
}
