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
use std::fmt::Write as _;
use style_traits::ParseError;

pub use generics::page::PageMarks;
pub use generics::page::PageOrientation;
pub use generics::page::PageSizeOrientation;
pub use generics::page::PaperSize;

/// Specified value of the `bleed` page descriptor.
///
/// CSS Paged Media 3 defines exactly `auto | <length>`. The signed scalar is
/// applied uniformly to every edge. Native asymmetric bleed is represented by
/// the independent `-bd-page-bleed-*` longhands, not by this standards type.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum Bleed {
    /// `auto`
    Auto,
    /// `<length>` — single value applied to all four edges.
    Length(Length),
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
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Length(l) => l.to_css(dest),
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

        Length::parse(context, input).map(Self::Length)
    }
}
/// Specified value of the @page size descriptor
pub type PageSize = generics::page::PageSize<Size2D<NonNegativeLength>>;

/// Parse a `<page-size>` keyword, accepting the Prince vendor
/// aliases `US-Letter`, `US-Legal`, and `US-Ledger` on top of the
/// css-page-3 set. Prince documents the aliases as interchangeable
/// with `letter` / `legal` / `ledger`; real-world Prince stylesheets
/// (e.g. the vendor's published sample projects) use the aliased
/// spelling, and rejecting it invalidates the whole `size`
/// declaration, silently falling back to the UA default sheet.
fn parse_paper_size_with_vendor_aliases<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<PaperSize, ParseError<'i>> {
    if let Ok(paper_size) = input.try_parse(PaperSize::parse) {
        return Ok(paper_size);
    }
    let ident = input.expect_ident()?.clone();
    match_ignore_ascii_case! { &ident,
        "us-letter" => Ok(PaperSize::Letter),
        "us-legal" => Ok(PaperSize::Legal),
        "us-ledger" => Ok(PaperSize::Ledger),
        _ => Err(input.new_custom_error(
            style_traits::StyleParseErrorKind::UnexpectedIdent(ident.clone()),
        )),
    }
}

impl Parse for PageSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        // Try to parse as <page-size> [ <orientation> ]
        if let Ok(paper_size) = input.try_parse(parse_paper_size_with_vendor_aliases) {
            let orientation = input
                .try_parse(PageSizeOrientation::parse)
                .unwrap_or(PageSizeOrientation::Portrait);
            return Ok(PageSize::PaperSize(paper_size, orientation));
        }
        // Try to parse as <orientation> [ <page-size> ]
        if let Ok(orientation) = input.try_parse(PageSizeOrientation::parse) {
            if let Ok(paper_size) = input.try_parse(parse_paper_size_with_vendor_aliases) {
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

        // moegoe extension: accept the PDFreactor-aligned vocabulary
        // (`crop | cross | bleed | registration`, any combination) in
        // addition to the standard CSS Paged Media Level 3 keywords.
        // The order of keywords does not affect the computed value;
        // any keyword may appear at most once.
        let mut crop = false;
        let mut cross = false;
        let mut bleed = false;
        let mut registration = false;
        while !input.is_exhausted() {
            let ident = input.expect_ident()?;
            match_ignore_ascii_case! { ident,
                "crop" => {
                    if crop {
                        return Err(input.new_custom_error(
                            style_traits::StyleParseErrorKind::UnspecifiedError,
                        ));
                    }
                    crop = true;
                },
                "cross" => {
                    if cross {
                        return Err(input.new_custom_error(
                            style_traits::StyleParseErrorKind::UnspecifiedError,
                        ));
                    }
                    cross = true;
                },
                "bleed" => {
                    if bleed {
                        return Err(input.new_custom_error(
                            style_traits::StyleParseErrorKind::UnspecifiedError,
                        ));
                    }
                    bleed = true;
                },
                "registration" => {
                    if registration {
                        return Err(input.new_custom_error(
                            style_traits::StyleParseErrorKind::UnspecifiedError,
                        ));
                    }
                    registration = true;
                },
                _ => {
                    let ident = ident.clone();
                    return Err(input.new_custom_error(
                        style_traits::StyleParseErrorKind::UnexpectedIdent(ident)
                    ));
                }
            }
        }

        if !crop && !cross && !bleed && !registration {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }

        Ok(PageMarks {
            crop,
            cross,
            bleed,
            registration,
        })
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
