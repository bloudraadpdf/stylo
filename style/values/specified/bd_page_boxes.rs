/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe PDF page box descriptors (F3).
//!
//! PDF defines five page-box rectangles (ISO 32000-2 §14.11.2):
//! MediaBox (physical media), CropBox (clip-to area), BleedBox
//! (production bleed area), TrimBox (final trim), and ArtBox
//! (artistic area). CSS Paged Media exposes only `size` (which
//! moegoe currently maps to TrimBox) and `bleed` (which adds an
//! outset to BleedBox / MediaBox). The four `@page` descriptors
//! here let pre-press authors override the other boxes explicitly.
//!
//! `-bd-pdf-page-clip` controls which page box the paint pipeline
//! clips its display list against.
//!
//! All four are `@page`-only descriptors; the renderer reads them
//! from the resolved page-rule cascade.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::size::Size2D;
use crate::values::specified::length::{Length, NonNegativeLength};
use crate::values::specified::page::PageSize;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// Specified value of `-bd-pdf-media-size` / `-bd-pdf-crop-size` /
/// `-bd-pdf-art-size`.
///
/// Same value space as `@page { size: ... }` but with explicit
/// `auto` semantics (the page falls back to deriving the box from
/// `size` + `bleed` + marks per the existing moegoe defaults).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfPageBoxSize {
    /// `auto` — defer to the moegoe default.
    Auto,
    /// `<length>{1,2}` or `<page-size> [<orientation>]` — explicit box.
    Page(PageSize),
}

impl BdPdfPageBoxSize {
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

impl ToCss for BdPdfPageBoxSize {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Page(p) => p.to_css(dest),
        }
    }
}

impl Parse for BdPdfPageBoxSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        // Special-case `auto` so we keep distinct semantics from
        // `PageSize::Auto`. PageSize parses `auto` too but we want
        // the renderer to distinguish "page box explicitly auto"
        // from "box inherits via size".
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        // Re-use the existing PageSize grammar for the rest.
        Ok(Self::Page(PageSize::parse(context, input)?))
    }
}

// Newtype wrappers per property so the cascade reader can route them
// distinctly without `Box<dyn>` indirection. The inner type is
// shared.

/// Specified value of `-bd-pdf-media-size`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfMediaSize(pub BdPdfPageBoxSize);

impl BdPdfMediaSize {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(BdPdfPageBoxSize::auto())
    }
}

impl Parse for BdPdfMediaSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(BdPdfPageBoxSize::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-crop-size`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfCropSize(pub BdPdfPageBoxSize);

impl BdPdfCropSize {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(BdPdfPageBoxSize::auto())
    }
}

impl Parse for BdPdfCropSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(BdPdfPageBoxSize::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-art-size`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfArtSize(pub BdPdfPageBoxSize);

impl BdPdfArtSize {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(BdPdfPageBoxSize::auto())
    }
}

impl Parse for BdPdfArtSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(BdPdfPageBoxSize::parse(context, input)?))
    }
}

/// Per-edge insets (top, right, bottom, left) carried by the
/// `-bd-pdf-{crop,art,trim,bleed}-box` descriptors.
///
/// The descriptor defines the named PDF page box as a rectangle
/// inset from the MediaBox by the four edge values, mirroring the
/// CSS `margin` shorthand expansion rules. All four lengths must be
/// non-negative (a negative inset would push the box outside the
/// MediaBox, which the PDF specification forbids).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfPageBoxInsetsSides {
    /// Top edge inset.
    pub top: Length,
    /// Right edge inset.
    pub right: Length,
    /// Bottom edge inset.
    pub bottom: Length,
    /// Left edge inset.
    pub left: Length,
}

/// Specified value of `-bd-pdf-{crop,art,trim,bleed}-box`.
///
/// PDF defines five page-box rectangles (ISO 32000-2 §14.11.2). CSS
/// Paged Media exposes `size` (→ TrimBox) and `bleed` (→ outset
/// for BleedBox / MediaBox). The four box descriptors here let
/// authors override the remaining boxes explicitly as **insets**
/// from the MediaBox.
///
/// Grammar: `auto | <length>{1,4}`. The one-to-four length form
/// follows the CSS shorthand expansion shared by `margin` and
/// `padding`: one length applies to all four edges; two assign
/// top/bottom = first and right/left = second; three assign
/// top = first, right/left = second, bottom = third; four assign
/// top, right, bottom, left in source order.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfPageBoxInsets {
    /// `auto` — defer to the moegoe default for this box.
    Auto,
    /// Per-side insets after shorthand expansion.
    Sides(BdPdfPageBoxInsetsSides),
}

impl BdPdfPageBoxInsets {
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

impl ToCss for BdPdfPageBoxInsets {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Sides(BdPdfPageBoxInsetsSides {
                top,
                right,
                bottom,
                left,
            }) => {
                top.to_css(dest)?;
                dest.write_char(' ')?;
                right.to_css(dest)?;
                if bottom != top || left != right {
                    dest.write_char(' ')?;
                    bottom.to_css(dest)?;
                    if left != right {
                        dest.write_char(' ')?;
                        left.to_css(dest)?;
                    }
                }
                Ok(())
            },
        }
    }
}

impl Parse for BdPdfPageBoxInsets {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }

        let first = Length::parse(context, input)?;
        let second = match input.try_parse(|i| Length::parse(context, i)) {
            Ok(l) => l,
            Err(_) => {
                // One length — apply to all four edges.
                return Ok(Self::Sides(BdPdfPageBoxInsetsSides {
                    top: first.clone(),
                    right: first.clone(),
                    bottom: first.clone(),
                    left: first,
                }));
            },
        };
        let third = input.try_parse(|i| Length::parse(context, i)).ok();
        let fourth = match third.as_ref() {
            Some(_) => input.try_parse(|i| Length::parse(context, i)).ok(),
            None => None,
        };
        let (top, right, bottom, left) = match (third, fourth) {
            (None, _) => {
                // `<top/bottom> <right/left>`
                (first.clone(), second.clone(), first, second)
            },
            (Some(third), None) => {
                // `<top> <right/left> <bottom>`
                (first, second.clone(), third, second)
            },
            (Some(third), Some(fourth)) => {
                // `<top> <right> <bottom> <left>`
                (first, second, third, fourth)
            },
        };
        Ok(Self::Sides(BdPdfPageBoxInsetsSides {
            top,
            right,
            bottom,
            left,
        }))
    }
}

// Newtype wrappers per property so the cascade reader routes each
// box descriptor distinctly. The inner inset payload is shared.

/// Specified value of `-bd-pdf-crop-box`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfCropBox(pub BdPdfPageBoxInsets);

impl BdPdfCropBox {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(BdPdfPageBoxInsets::auto())
    }
}

impl Parse for BdPdfCropBox {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(BdPdfPageBoxInsets::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-art-box`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfArtBox(pub BdPdfPageBoxInsets);

impl BdPdfArtBox {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(BdPdfPageBoxInsets::auto())
    }
}

impl Parse for BdPdfArtBox {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(BdPdfPageBoxInsets::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-trim-box`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfTrimBox(pub BdPdfPageBoxInsets);

impl BdPdfTrimBox {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(BdPdfPageBoxInsets::auto())
    }
}

impl Parse for BdPdfTrimBox {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(BdPdfPageBoxInsets::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-bleed-box`.
///
/// Overrides the F20 automatic BleedBox computation (BleedBox =
/// TrimBox + `bleed`) when authored. Useful for production
/// workflows where the bleed extent is uneven (e.g. extra grip on
/// the binding edge for trimming).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfBleedBox(pub BdPdfPageBoxInsets);

impl BdPdfBleedBox {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(BdPdfPageBoxInsets::auto())
    }
}

impl Parse for BdPdfBleedBox {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(BdPdfPageBoxInsets::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-page-clip`.
///
/// Selects which page box the paint pipeline clips the rendered
/// content against. `media` matches the historic non-clipping
/// behaviour; `none` opts out of any clip (rare; useful for
/// debug bleed-box overflow). The other values clip to the named
/// PDF page box.
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
pub enum BdPdfPageClip {
    #[default]
    Media,
    Crop,
    Trim,
    Bleed,
    Art,
    None,
}

// Force `Size2D<NonNegativeLength>` into scope for downstream
// callers — keeps the API surface symmetric with `PageSize`.
#[allow(dead_code)]
type _PageSize2D = Size2D<NonNegativeLength>;

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::{Parser, ParserInput};
    use style_traits::ToCss;

    fn parse_clip(css: &str) -> BdPdfPageClip {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfPageClip::parse(input))
            .expect("page-clip should parse")
    }

    #[test]
    fn page_clip_round_trips() {
        for css in ["media", "crop", "trim", "bleed", "art", "none"] {
            let value = parse_clip(css);
            assert_eq!(value.to_css_string(), css);
        }
    }
}
