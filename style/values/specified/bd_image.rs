/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe image-recompression / DPI control surface (F5).
//!
//! Six properties governing how embedded images are processed
//! when emitting PDF. Split between standard `image-resolution`
//! (CSS Images Level 4 §6) and PDFreactor / Prince proprietary
//! companions for recompression policy and resampling kernel.
//!
//! | Property | Source |
//! |----------|--------|
//! | `image-resolution` | CSS Images 4 §6 (standard) |
//! | `-bd-image-resolution` | Prince `-prince-image-resolution` 7807; PDFreactor `-ro-image-resolution` 15787 |
//! | `-bd-image-recompression` | PDFreactor `-ro-image-recompression` 15707 |
//! | `-bd-image-resampling` | PDFreactor `-ro-image-resampling` 15767 |
//! | `-bd-image-magic` | Prince `-prince-image-magic` 7768 |
//! | `-bd-image-clip-path` | PDFreactor `-ro-image-clip-path` 15645 |
//!
//! `image-resolution` and `-bd-image-resolution` are aliases at the
//! cascade level; both compute into the same `ImageResolution`
//! value. The audit plan flags `image-resolution` as ungated for
//! Servo upstream; here it is ungated explicitly via this
//! moegoe-controlled longhand.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::basic_shape::{AllowedBasicShapes, BasicShape, ShapeType};
use crate::values::specified::url::SpecifiedUrl;
use crate::values::specified::{Number, Resolution};
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// Value of `image-resolution` and `-bd-image-resolution`.
///
/// Syntax `[ from-image || <resolution> ] && snap?`.
///
/// `from-image` instructs the renderer to read intrinsic DPI from
/// image metadata; `<resolution>` overrides it. `snap` flags that
/// the renderer should snap the chosen dpi to a power-of-two
/// fraction of the device pixel density (CSS Images 4 §6.2).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdImageResolution {
    /// Whether `from-image` was set.
    pub from_image: bool,
    /// Optional explicit resolution.
    pub resolution: Option<Resolution>,
    /// Whether `snap` was set.
    pub snap: bool,
}

impl BdImageResolution {
    /// Initial value (`1dppx`).
    #[inline]
    pub fn one_dppx() -> Self {
        Self {
            from_image: false,
            resolution: Some(Resolution::from_dppx(1.0)),
            snap: false,
        }
    }
}

impl ToCss for BdImageResolution {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        let mut wrote = false;
        if self.from_image {
            dest.write_str("from-image")?;
            wrote = true;
        }
        if let Some(ref res) = self.resolution {
            if wrote {
                dest.write_char(' ')?;
            }
            res.to_css(dest)?;
            wrote = true;
        }
        if self.snap {
            if wrote {
                dest.write_char(' ')?;
            }
            dest.write_str("snap")?;
        }
        Ok(())
    }
}

impl Parse for BdImageResolution {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let mut from_image = false;
        let mut resolution: Option<Resolution> = None;
        let mut snap = false;
        loop {
            if !from_image
                && input
                    .try_parse(|i| i.expect_ident_matching("from-image"))
                    .is_ok()
            {
                from_image = true;
                continue;
            }
            if !snap
                && input
                    .try_parse(|i| i.expect_ident_matching("snap"))
                    .is_ok()
            {
                snap = true;
                continue;
            }
            if resolution.is_none() {
                if let Ok(r) = input.try_parse(|i| Resolution::parse(context, i)) {
                    resolution = Some(r);
                    continue;
                }
            }
            break;
        }
        if !from_image && resolution.is_none() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self {
            from_image,
            resolution,
            snap,
        })
    }
}

/// `-bd-image-recompression`.
///
/// `auto` (initial) lets the renderer choose; `none` disables
/// recompression entirely; `lossless` re-encodes lossily-supplied
/// images into lossless containers; `Quality(q)` re-encodes
/// raster images as JPEG with the given quality (0–100).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdImageRecompression {
    /// `auto`
    Auto,
    /// `none`
    None,
    /// `lossless`
    Lossless,
    /// `<number>` — JPEG quality.
    Quality(Number),
}

impl BdImageRecompression {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl ToCss for BdImageRecompression {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::None => dest.write_str("none"),
            Self::Lossless => dest.write_str("lossless"),
            Self::Quality(q) => q.to_css(dest),
        }
    }
}

impl Parse for BdImageRecompression {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(n) = input.try_parse(|i| Number::parse(context, i)) {
            return Ok(Self::Quality(n));
        }
        let location = input.current_source_location();
        let ident = input.expect_ident()?.clone();
        Ok(match_ignore_ascii_case! { &*ident,
            "auto" => Self::Auto,
            "none" => Self::None,
            "lossless" => Self::Lossless,
            _ => return Err(location.new_custom_error(
                StyleParseErrorKind::UnspecifiedError,
            )),
        })
    }
}

/// `-bd-image-resampling`.
///
/// `auto` defers to the renderer's resampling heuristic;
/// `none` skips resampling and emits the source raster verbatim;
/// the named kernels select nearest-neighbour / bilinear / bicubic.
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
pub enum BdImageResampling {
    #[default]
    Auto,
    None,
    Nearest,
    Linear,
    Cubic,
}

/// `-bd-image-magic`.
///
/// Prince's bespoke alpha + CMYK pre-processing knob. Parser-only
/// pass-through for Prince stylesheet compat — moegoe currently
/// no-ops the property.
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
pub enum BdImageMagic {
    #[default]
    None,
    Auto,
}

/// `-bd-image-clip-path`.
///
/// `none` (initial) — no per-image clip. Otherwise applies a
/// shape (`<basic-shape>`) or external SVG path (`<url>`).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdImageClipPath {
    /// `none`
    None,
    /// `<basic-shape>`
    Shape(Box<BasicShape>),
    /// `<url>`
    Url(SpecifiedUrl),
}

impl BdImageClipPath {
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

impl ToCss for BdImageClipPath {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Shape(s) => s.to_css(dest),
            Self::Url(u) => u.to_css(dest),
        }
    }
}

impl Parse for BdImageClipPath {
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
        if let Ok(url) = input.try_parse(|i| SpecifiedUrl::parse(context, i)) {
            return Ok(Self::Url(url));
        }
        Ok(Self::Shape(Box::new(BasicShape::parse(
            context,
            input,
            AllowedBasicShapes::ALL,
            ShapeType::Filled,
        )?)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::ParsingMode;

    fn parse_resampling(css: &str) -> BdImageResampling {
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
            .parse_entirely(|input| BdImageResampling::parse(input))
            .expect("resampling value should parse")
    }

    #[test]
    fn resampling_round_trips() {
        for css in ["auto", "none", "nearest", "linear", "cubic"] {
            let value = parse_resampling(css);
            assert_eq!(value.to_css_string(), css);
        }
    }
}
