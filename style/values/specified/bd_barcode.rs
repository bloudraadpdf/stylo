/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-barcode-*` properties (Family 15).
//!
//! Native moegoe fork-extension surface for declarative barcodes.
//! The CSS surface lands here and the rendering backend consumes the
//! computed values without exposing backend-specific types.
//!
//! Each longhand is non-inherited. The `-bd-barcode` shorthand
//! lives in `style/properties/shorthands.toml`; the initial surface is
//! longhand-only.

use crate::derives::*;
use crate::values::specified::length::NonNegativeLengthPercentage;
use crate::values::specified::url::SpecifiedUrl;
use crate::OwnedSlice;
use crate::OwnedStr;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

/// Specified value of `-bd-barcode-type`.
///
/// Selects the encoding family. Backend support is gated on the
/// availability of an encoder for the selected symbology.
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
pub enum BdBarcodeType {
    #[default]
    None,
    QrCode,
    DataMatrix,
    Pdf417,
    Aztec,
    // Accept both compact and hyphenated symbology spellings.
    #[parse(aliases = "code-39")]
    Code39,
    /// LOGMARS (MIL-STD-1189), including its required modulo-43 check
    /// character and 3:1 wide-to-narrow module ratio.
    Logmars,
    #[parse(aliases = "code-93")]
    Code93,
    #[parse(aliases = "code-128")]
    Code128,
    #[parse(aliases = "ean-8")]
    Ean8,
    #[parse(aliases = "ean-13")]
    Ean13,
    #[parse(aliases = "upc-a")]
    Upca,
    #[parse(aliases = "upc-e")]
    Upce,
    Itf,
    /// ITF-14 with its GS1 bearer frame and modulo-10 GTIN check digit.
    Itf14,
    Codabar,
    MaxiCode,
    Telepen,
    MicroQr,
    GridMatrix,
    CodeOne,
    CodablockF,
    DataBarLimited,
    DataBarStacked,
    Pharmacode,
    Postnet,
    Kix,
    UspsIntelligentMail,
    KoreaPost,
    DeutschePostLeitcode,
    AustraliaPost,
}

/// Specified value of `-bd-barcode-content`.
///
/// `none` clears; `<string>+` provides literal data (joined by the
/// renderer per symbology rules); `url(...)` provides a URL payload
/// resolved against the stylesheet base URL.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdBarcodeContent {
    /// `none`.
    None,
    /// `<string>+`.
    Strings(OwnedSlice<OwnedStr>),
    /// `url(...)` resolved against the stylesheet base URL.
    Url(SpecifiedUrl),
}

impl Default for BdBarcodeContent {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdBarcodeContent {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for BdBarcodeContent {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Strings(strings) => {
                for (index, string) in strings.iter().enumerate() {
                    if index != 0 {
                        dest.write_char(' ')?;
                    }
                    string.to_css(dest)?;
                }
                Ok(())
            },
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl crate::parser::Parse for BdBarcodeContent {
    fn parse<'i, 't>(
        context: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        if let Ok(url) =
            input.try_parse(|i| <SpecifiedUrl as crate::parser::Parse>::parse(context, i))
        {
            input.expect_exhausted()?;
            return Ok(Self::Url(url));
        }
        let mut strings: Vec<OwnedStr> = Vec::new();
        loop {
            match input.try_parse(|i| -> Result<OwnedStr, style_traits::ParseError<'i>> {
                let s = i.expect_string()?;
                Ok(s.as_ref().to_owned().into())
            }) {
                Ok(s) => strings.push(s),
                Err(_) => break,
            }
        }
        if strings.is_empty() {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Strings(OwnedSlice::from(strings)))
    }
}

/// Specified value of `-bd-barcode-checkdigit-mode`.
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
pub enum BdBarcodeCheckDigitMode {
    #[default]
    Auto,
    None,
    Add,
    Check,
}

// `-bd-barcode-composite-content` reuses the
// `BdBarcodeContent` longhand type directly in `longhands.toml`.

/// Specified value of `-bd-barcode-composite-type`.
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
pub enum BdBarcodeCompositeType {
    #[default]
    None,
    CcA,
    CcB,
    CcC,
}

/// Specified value of `-bd-barcode-ecc-level`.
///
/// `<integer>` percentage-of-modules ECC for QR / DataMatrix.
#[derive(
    Clone,
    Copy,
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
pub enum BdBarcodeEccLevel {
    /// `auto`.
    Auto,
    /// QR ECC keyword (`L`/`M`/`Q`/`H`).
    Letter(BdQrEccLetter),
    /// Numeric percent.
    Percent(u32),
}

/// QR ECC keyword.
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
pub enum BdQrEccLetter {
    #[default]
    L,
    M,
    Q,
    H,
}

impl Default for BdBarcodeEccLevel {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

impl crate::parser::Parse for BdBarcodeEccLevel {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        if let Ok(letter) = input.try_parse(BdQrEccLetter::parse) {
            return Ok(Self::Letter(letter));
        }
        let n = input.expect_integer()?;
        if !(0..=100).contains(&n) {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Percent(n as u32))
    }
}

/// Specified value of `-bd-barcode-encoding`.
///
/// Selects the semantic data convention applied before a symbology encodes
/// its payload.
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
pub enum BdBarcodeEncoding {
    #[default]
    Auto,
    Eci,
    Hibc,
    Gs1,
}

/// Specified value of `-bd-barcode-font-family`.
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
pub enum BdBarcodeFontFamily {
    /// `auto` — engine default.
    Auto,
    /// `<string>` — explicit family.
    Name(OwnedStr),
}

impl Default for BdBarcodeFontFamily {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

impl crate::parser::Parse for BdBarcodeFontFamily {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        if let Ok(s) = input.try_parse(|i| -> Result<String, style_traits::ParseError<'i>> {
            Ok(i.expect_string()?.as_ref().to_owned())
        }) {
            return Ok(Self::Name(s.into()));
        }
        let ident = input.expect_ident()?;
        Ok(Self::Name(ident.as_ref().to_owned().into()))
    }
}

// `-bd-barcode-font-size` uses the predefined
// `NonNegativeLength` longhand type directly.

/// Specified value of `-bd-barcode-human-readable-affix`.
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
pub enum BdBarcodeAffix {
    /// `none`.
    None,
    /// `auto`.
    Auto,
    /// `<string>`.
    Literal(OwnedStr),
}

impl Default for BdBarcodeAffix {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

impl crate::parser::Parse for BdBarcodeAffix {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-barcode-human-readable-position`.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdBarcodeHrPosition {
    None,
    TopLeft,
    TopCenter,
    TopRight,
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
}

#[derive(Clone, Copy)]
enum HrBlockPosition {
    Top,
    Bottom,
}

#[derive(Clone, Copy)]
enum HrInlineAlignment {
    Left,
    Center,
    Right,
}

impl crate::parser::Parse for BdBarcodeHrPosition {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            input.expect_exhausted()?;
            return Ok(Self::None);
        }

        let mut block = None;
        let mut alignment = None;
        let mut saw_keyword = false;
        while !input.is_exhausted() {
            let ident = input.expect_ident()?;
            saw_keyword = true;
            if ident.eq_ignore_ascii_case("top") || ident.eq_ignore_ascii_case("above") {
                if block.replace(HrBlockPosition::Top).is_some() {
                    return Err(
                        input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)
                    );
                }
            } else if ident.eq_ignore_ascii_case("bottom") || ident.eq_ignore_ascii_case("below") {
                if block.replace(HrBlockPosition::Bottom).is_some() {
                    return Err(
                        input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)
                    );
                }
            } else if ident.eq_ignore_ascii_case("left") {
                if alignment.replace(HrInlineAlignment::Left).is_some() {
                    return Err(
                        input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)
                    );
                }
            } else if ident.eq_ignore_ascii_case("center") {
                if alignment.replace(HrInlineAlignment::Center).is_some() {
                    return Err(
                        input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)
                    );
                }
            } else if ident.eq_ignore_ascii_case("right") {
                if alignment.replace(HrInlineAlignment::Right).is_some() {
                    return Err(
                        input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)
                    );
                }
            } else {
                return Err(
                    input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)
                );
            }
        }

        if !saw_keyword {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }

        match (
            block.unwrap_or(HrBlockPosition::Bottom),
            alignment.unwrap_or(HrInlineAlignment::Center),
        ) {
            (HrBlockPosition::Top, HrInlineAlignment::Left) => Ok(Self::TopLeft),
            (HrBlockPosition::Top, HrInlineAlignment::Center) => Ok(Self::TopCenter),
            (HrBlockPosition::Top, HrInlineAlignment::Right) => Ok(Self::TopRight),
            (HrBlockPosition::Bottom, HrInlineAlignment::Left) => Ok(Self::BottomLeft),
            (HrBlockPosition::Bottom, HrInlineAlignment::Center) => Ok(Self::BottomCenter),
            (HrBlockPosition::Bottom, HrInlineAlignment::Right) => Ok(Self::BottomRight),
        }
    }
}

impl ToCss for BdBarcodeHrPosition {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        dest.write_str(match self {
            Self::None => "none",
            Self::TopLeft => "top left",
            Self::TopCenter => "top center",
            Self::TopRight => "top right",
            Self::BottomLeft => "bottom left",
            Self::BottomCenter => "bottom center",
            Self::BottomRight => "bottom right",
        })
    }
}

// `-bd-barcode-letter-spacing` uses the predefined `Length` type
// directly.

/// Specified value of `-bd-barcode-reader-initialization`.
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
pub enum BdBarcodeReaderInit {
    #[default]
    None,
    True,
    False,
}

/// Specified value of `-bd-barcode-size`.
///
/// Computed value lifts to `computed::bd_barcode::BdBarcodeSize`
/// (manual `ToComputedValue` impl); the `NonNegativeLengthPercentage`
/// inner field is not specified-to-computed identity, so the
/// derive can't be used.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdBarcodeSize {
    /// `auto`.
    Auto,
    /// Explicit (square) edge length.
    Square(NonNegativeLengthPercentage),
}

impl Default for BdBarcodeSize {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

impl BdBarcodeSize {
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

impl crate::parser::Parse for BdBarcodeSize {
    fn parse<'i, 't>(
        context: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        Ok(Self::Square(NonNegativeLengthPercentage::parse(
            context, input,
        )?))
    }
}

/// Specified value of `-bd-barcode-structured-append`.
///
/// `<integer> of <integer>` shape, e.g. `2 of 4`. `none` initial.
#[derive(
    Clone,
    Copy,
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
pub enum BdBarcodeStructuredAppend {
    /// `none`.
    None,
    /// `<index> of <total>`.
    Pair {
        /// One-based index.
        index: u32,
        /// Total in the sequence.
        total: u32,
    },
}

impl Default for BdBarcodeStructuredAppend {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl crate::parser::Parse for BdBarcodeStructuredAppend {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let index = input.expect_integer()?;
        input.expect_ident_matching("of")?;
        let total = input.expect_integer()?;
        if index < 1 || total < 1 || index > total {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Pair {
            index: index as u32,
            total: total as u32,
        })
    }
}

// `-bd-barcode-structured-append-position` reuses the
// `BdBarcodeStructuredAppend` longhand type directly in
// `longhands.toml`. `-bd-barcode-symbol-width` uses the
// predefined `NonNegativeLength` type directly.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::parser::Parse;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::{ParsingMode, ToCss};

    fn parse_hrt_position(css: &str) -> Result<BdBarcodeHrPosition, ()> {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = crate::parser::ParserContext::new(
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
        Parser::new(&mut input)
            .parse_entirely(|input| BdBarcodeHrPosition::parse(&context, input))
            .map_err(|_| ())
    }

    #[test]
    fn barcode_hrt_position_parses_both_axes_in_either_order() {
        assert_eq!(
            parse_hrt_position("top left"),
            Ok(BdBarcodeHrPosition::TopLeft),
        );
        assert_eq!(
            parse_hrt_position("right top"),
            Ok(BdBarcodeHrPosition::TopRight),
        );
        assert_eq!(
            parse_hrt_position("left"),
            Ok(BdBarcodeHrPosition::BottomLeft)
        );
        assert_eq!(
            parse_hrt_position("top"),
            Ok(BdBarcodeHrPosition::TopCenter)
        );
    }

    #[test]
    fn barcode_hrt_position_uses_canonical_serialisation_and_accepts_legacy_vertical_aliases() {
        assert_eq!(
            parse_hrt_position("above").unwrap().to_css_string(),
            "top center",
        );
        assert_eq!(
            parse_hrt_position("below right").unwrap().to_css_string(),
            "bottom right",
        );
        assert_eq!(
            BdBarcodeHrPosition::default(),
            BdBarcodeHrPosition::BottomCenter
        );
    }

    #[test]
    fn barcode_hrt_position_rejects_conflicting_or_duplicate_axes() {
        for invalid in ["", "top bottom", "left right", "top top", "sideways"] {
            assert!(parse_hrt_position(invalid).is_err(), "accepted {invalid:?}");
        }
    }
}
