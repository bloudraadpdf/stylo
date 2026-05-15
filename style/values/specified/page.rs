/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified @page at-rule properties and named-page style properties

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::size::Size2D;
use crate::values::specified::length::Length;
use crate::values::specified::length::NonNegativeLength;
use crate::values::{generics, CustomIdent};
use cssparser::{match_ignore_ascii_case, Parser};
use style_traits::ParseError;

pub use generics::page::PageMarks;
pub use generics::page::PageOrientation;
pub use generics::page::PageSizeOrientation;
pub use generics::page::PaperSize;

/// Per-side bleed lengths (top, right, bottom, left).
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToShmem,
    ToTyped,
)]
#[repr(C)]
pub struct BleedSides {
    /// Top edge.
    pub top: Length,
    /// Right edge.
    pub right: Length,
    /// Bottom edge.
    pub bottom: Length,
    /// Left edge.
    pub left: Length,
}

/// Specified value of the `bleed` page descriptor.
///
/// The standard CSS Paged Media grammar admits `auto | <length>`.
/// moegoe extends this with the 2/3/4-length shorthand syntax
/// shared by `margin` / `padding` (F4 per the audit), so authors
/// can declare asymmetric bleed without dropping into the per-side
/// `-bd-page-bleed-*` longhands. Two/three-length expansion follows
/// the CSS shorthand rules (top, right/left = right, bottom = top;
/// top, right/left = right, bottom).
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum Bleed {
    /// `auto`
    Auto,
    /// `<length>` — single value applied to all four edges.
    Length(Length),
    /// `<length>{2..=4}` — explicit per-side values after CSS
    /// shorthand expansion.
    Sides(BleedSides),
}

impl Bleed {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether this is the `auto` value.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl style_traits::ToCss for Bleed {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> std::fmt::Result
    where
        W: std::fmt::Write,
    {
        use std::fmt::Write as _;
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Length(l) => l.to_css(dest),
            Self::Sides(BleedSides {
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

impl Parse for Bleed {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }

        let first = Length::parse(context, input)?;
        // Try to parse up to three additional lengths.
        let second = match input.try_parse(|i| Length::parse(context, i)) {
            Ok(l) => l,
            Err(_) => return Ok(Self::Length(first)),
        };
        let third = match input.try_parse(|i| Length::parse(context, i)) {
            Ok(l) => Some(l),
            Err(_) => None,
        };
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
        Ok(Self::Sides(BleedSides {
            top,
            right,
            bottom,
            left,
        }))
    }
}
/// Specified value of the @page size descriptor
pub type PageSize = generics::page::PageSize<Size2D<NonNegativeLength>>;

impl Parse for PageSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        // Try to parse as <page-size> [ <orientation> ]
        if let Ok(paper_size) = input.try_parse(PaperSize::parse) {
            let orientation = input
                .try_parse(PageSizeOrientation::parse)
                .unwrap_or(PageSizeOrientation::Portrait);
            return Ok(PageSize::PaperSize(paper_size, orientation));
        }
        // Try to parse as <orientation> [ <page-size> ]
        if let Ok(orientation) = input.try_parse(PageSizeOrientation::parse) {
            if let Ok(paper_size) = input.try_parse(PaperSize::parse) {
                return Ok(PageSize::PaperSize(paper_size, orientation));
            }
            return Ok(PageSize::Orientation(orientation));
        }
        // Try to parse dimensions
        if let Ok(size) =
            input.try_parse(|i| Size2D::parse_with(context, i, NonNegativeLength::parse))
        {
            return Ok(PageSize::Size(size));
        }
        // auto value
        input.expect_ident_matching("auto")?;
        Ok(PageSize::Auto)
    }
}

impl Parse for PageMarks {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("none"))
            .is_ok()
        {
            input.expect_exhausted()?;
            return Ok(PageMarks::none());
        }

        let mut crop = false;
        let mut cross = false;
        while !input.is_exhausted() {
            let ident = input.expect_ident()?;
            match_ignore_ascii_case! { ident,
                "crop" => crop = true,
                "cross" => cross = true,
                _ => {
                    let ident = ident.clone();
                    return Err(input.new_custom_error(
                        style_traits::StyleParseErrorKind::UnexpectedIdent(ident)
                    ));
                }
            }
        }

        if !crop && !cross {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }

        Ok(PageMarks { crop, cross })
    }
}

/// Page name value.
///
/// https://drafts.csswg.org/css-page-3/#using-named-pages
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
pub enum PageName {
    /// `auto` value.
    Auto,
    /// Page name value
    PageName(CustomIdent),
}

impl Parse for PageName {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        Ok(match_ignore_ascii_case! { ident,
            "auto" => PageName::auto(),
            _ => PageName::PageName(CustomIdent::from_ident(location, ident, &[])?),
        })
    }
}

impl PageName {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        PageName::Auto
    }

    /// Whether this is the `auto` value.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(*self, PageName::Auto)
    }
}
