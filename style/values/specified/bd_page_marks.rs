/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe per-mark tuning properties (F4).
//!
//! Replaces the hardcoded crop / cross / registration-mark constants
//! in `crates/moegoe-ir/src/page.rs` (`CROP_MARK_LENGTH_PT = 12pt`,
//! `CROP_MARK_OFFSET_PT = 3pt`, `CROSS_MARK_SIZE_PT = 6pt`) with
//! CSS-driven @page descriptors. Print shops use these knobs to
//! line up registration marks against their plate-imposition rig.
//!
//! All nine are `@page`-only descriptors:
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

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::NonNegativeLength;
use crate::values::specified::Color;
use cssparser::Parser;
use style_traits::ParseError;

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
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
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

impl style_traits::ToCss for BdPageMarksColour {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> std::fmt::Result
    where
        W: std::fmt::Write,
    {
        use std::fmt::Write as _;
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Colour(c) => c.to_css(dest),
        }
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
