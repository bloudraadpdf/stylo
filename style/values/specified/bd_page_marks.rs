/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe page-mark + print-shop properties (Families F4 and F20).
//!
//! F4 — per-mark tuning. Replaces the hardcoded crop / cross /
//! registration-mark constants in
//! `crates/moegoe-ir/src/page.rs` (`CROP_MARK_LENGTH_PT = 12pt`,
//! `CROP_MARK_OFFSET_PT = 3pt`, `CROSS_MARK_SIZE_PT = 6pt`) with
//! CSS-driven @page descriptors. Print shops use these knobs to
//! line up registration marks against their plate-imposition rig.
//!
//! All nine F4 longhands are `@page`-only descriptors:
//!
//! | Property | PDFreactor source |
//! |----------|--------------------|
//! | `-bd-page-crop-mark-length` | `-ro-crop-mark-length` 14572 |
//! | `-bd-page-crop-mark-offset` | `-ro-crop-mark-offset` 14589 |
//! | `-bd-page-bleed-mark-length` | `-ro-bleed-mark-length` 13234 |
//! | `-bd-page-bleed-mark-offset` | `-ro-bleed-mark-offset` 13251 |
//! | `-bd-page-registration-mark-offset` | `-ro-registration-mark-offset` 17865 |
//! | `-bd-page-registration-mark-size` | `-ro-registration-mark-size` 17882 |
//! | `-bd-page-marks-colour` | `-ro-marks-color` 16345 |
//! | `-bd-page-marks-offset` | `-ro-marks-offset` 16362 |
//! | `-bd-page-marks-width` | `-ro-marks-width` 16371 |
//!
//! Prince spellings (`-prince-mark-offset` / `-prince-mark-width`)
//! map onto the corresponding `marks-*` properties via the moegoe
//! compat translator.
//!
//! F20 — `-bd-page-colorbar-*` / `-bd-page-print-mark-set` page-margin
//! print-shop tooling. Native moegoe fork-extension surface for
//! PDFreactor's `-ro-colorbar-*` and `-ro-marks` properties (see
//! `docs/reference-manuals/pdfreactor.md:14026–14043, 16279`). Eight
//! positional colour-bar slots plus an offset, a marks shorthand,
//! and a print-mark-set synthesised property. All F20 longhands are
//! `@page` descriptors (`rule_types_allowed = ["page"]`).

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::{LengthPercentage, NonNegativeLength};
use crate::values::specified::url::UrlOrNone;
use crate::values::specified::Color;
use crate::OwnedSlice;
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// `<non-negative-length>` value for a mark dimension or offset.
///
/// Newtype so each per-mark property has its own type identity in
/// the cascade reader.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPageMarkLength(pub NonNegativeLength);

impl BdPageMarkLength {
    /// Build a value from a points constant.
    #[inline]
    pub fn from_pt(pt: f32) -> Self {
        // 1pt = 1.333… px in CSS px terms — but here we want the
        // initial values to express the historic moegoe constants
        // as authored. The renderer resolves these against the
        // page-box geometry.
        Self(NonNegativeLength::from_px(pt * 96.0 / 72.0))
    }
}

impl Parse for BdPageMarkLength {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(NonNegativeLength::parse(context, input)?))
    }
}

/// `-bd-page-marks-colour`.
///
/// Colour applied to crop / cross / registration marks (PDFreactor
/// `-ro-marks-color`). `auto` (initial) defers to the renderer's
/// default — typically the "registration" colour space (100% of
/// every separation channel).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPageMarksColour {
    /// `auto` — defer to the renderer default.
    Auto,
    /// `<color>` — explicit colour value.
    Colour(Color),
}

impl BdPageMarksColour {
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

impl Parse for BdPageMarksColour {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        Ok(Self::Colour(Color::parse(context, input)?))
    }
}

// ===== F20 — colour bar / print-mark-set =================================

/// Specified value of a `-bd-page-colorbar-*` slot.
///
/// `none` (initial) — slot empty. `auto` — engine default colour
/// bar (per ISO 12647). `<url>` — explicit colour-bar artwork.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdColorBarPosition {
    /// `none`.
    None,
    /// `auto`.
    Auto,
    /// `<url>`.
    Url(UrlOrNone),
}

impl Default for BdColorBarPosition {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdColorBarPosition {
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

impl Parse for BdColorBarPosition {
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
        if input
            .try_parse(|i| i.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        Ok(Self::Url(UrlOrNone::parse(context, input)?))
    }
}

// `-bd-page-colorbar-offset` uses the predefined `Length` type
// directly.

/// Specified value of `-bd-page-print-mark-set`.
///
/// Selects one of the well-known print-mark presets.
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
pub enum BdPrintMarkSet {
    #[default]
    Auto,
    None,
    Default,
    Iso12647,
    Pdfx,
    Custom,
}

/// G10 — per-mark `enabled` flag (`yes` / `no`).
///
/// Selects whether a given print-mark family (crop / registration /
/// colour-bar / page-info) participates in the painted print-mark
/// envelope. `auto` (initial) defers to the page-rule cascade — the
/// historic crop/cross flag on `marks` plus the F20 print-mark-set
/// keyword decide which families are active. Authors flip to `no`
/// to suppress a single family explicitly without unsetting the
/// shorthand, or to `yes` to force a family on regardless of the
/// shorthand.
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
pub enum BdPageMarkEnabled {
    #[default]
    Auto,
    Yes,
    No,
}

impl BdPageMarkEnabled {
    /// Whether the value is at its initial `auto`.
    #[inline]
    pub fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }
}

// ===== Tier 4 §A.4.7 — marker variants ==================================

/// Specified value of `-bd-pdf-mark-registration-color`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdRegistrationColour {
    /// `auto`.
    Auto,
    /// `<color>`.
    Colour(Color),
}

impl BdRegistrationColour {
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

impl Parse for BdRegistrationColour {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        Ok(Self::Colour(Color::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-mark-registration-position`.
#[repr(u8)]
#[derive(
    Clone, Copy, Debug, Default, Eq, MallocSizeOf, Parse, PartialEq, SpecifiedValueInfo,
    ToCss, ToComputedValue, ToResolvedValue, ToShmem, ToTyped,
)]
#[allow(missing_docs)]
pub enum BdRegistrationPosition {
    #[default]
    AllCorners,
    Top,
    Bottom,
    TopAndBottom,
}

/// Specified value of `-bd-pdf-mark-colour-bar-swatches`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdColourBarSwatches {
    /// `none`.
    None,
    /// One or more `<color>` swatches.
    Colours(OwnedSlice<Color>),
}

impl Default for BdColourBarSwatches {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdColourBarSwatches {
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

impl ToCss for BdColourBarSwatches {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Colours(list) => {
                let mut first = true;
                for c in list.iter() {
                    if !first {
                        dest.write_str(" ")?;
                    }
                    first = false;
                    c.to_css(dest)?;
                }
                Ok(())
            }
        }
    }
}

impl Parse for BdColourBarSwatches {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let mut colours: Vec<Color> = Vec::new();
        let first = Color::parse(context, input)?;
        colours.push(first);
        while let Ok(c) = input.try_parse(|i| Color::parse(context, i)) {
            colours.push(c);
        }
        Ok(Self::Colours(OwnedSlice::from(colours)))
    }
}

/// Specified value of `-bd-pdf-mark-colour-bar-position`.
#[repr(u8)]
#[derive(
    Clone, Copy, Debug, Default, Eq, MallocSizeOf, Parse, PartialEq, SpecifiedValueInfo,
    ToCss, ToComputedValue, ToResolvedValue, ToShmem, ToTyped,
)]
#[allow(missing_docs)]
pub enum BdColourBarPositionSide {
    #[default]
    Top,
    Bottom,
    Left,
    Right,
}

/// Specified value of `-bd-pdf-mark-sidenote-glyph`.
///
/// The computed value equals the specified value: keyword variants
/// are `Copy` data, the `Literal(String)` variant inherits its data
/// directly into computed style without further resolution.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdSidenoteGlyph {
    /// `asterisk`.
    Asterisk,
    /// `dagger` (U+2020).
    Dagger,
    /// `double-dagger` (U+2021).
    DoubleDagger,
    /// `section` (U+00A7).
    Section,
    /// `numeric` — page-local note counter.
    Numeric,
    /// Authored literal string.
    Literal(crate::OwnedStr),
}

impl Default for BdSidenoteGlyph {
    #[inline]
    fn default() -> Self {
        Self::Numeric
    }
}

impl Parse for BdSidenoteGlyph {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(s) = input.try_parse(|i| i.expect_string().map(|s| s.as_ref().to_owned())) {
            return Ok(Self::Literal(crate::OwnedStr::from(s)));
        }
        let location = input.current_source_location();
        let ident = input.expect_ident()?.clone();
        match_ignore_ascii_case! { &ident,
            "asterisk" => Ok(Self::Asterisk),
            "dagger" => Ok(Self::Dagger),
            "double-dagger" => Ok(Self::DoubleDagger),
            "section" => Ok(Self::Section),
            "numeric" => Ok(Self::Numeric),
            _ => Err(location.new_custom_error(
                style_traits::StyleParseErrorKind::UnspecifiedError,
            )),
        }
    }
}

/// Specified value of `-bd-pdf-mark-sidenote-offset`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped, Parse)]
#[repr(C)]
pub struct BdSidenoteMarkerOffset(pub LengthPercentage);

impl BdSidenoteMarkerOffset {
    /// Initial value (`0`).
    #[inline]
    pub fn zero() -> Self {
        Self(LengthPercentage::zero_percent())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::{ParsingMode, ToCss};

    fn parse_length(css: &str) -> BdPageMarkLength {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Page),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPageMarkLength::parse(&context, input))
            .expect("mark length should parse")
    }

    #[test]
    fn mark_length_round_trips() {
        // Numbers serialise canonically; check parse + serialise consistency.
        let value = parse_length("12pt");
        assert!(value.to_css_string().contains("pt"));
    }
}
