/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-change-bar-*` revision-mark properties (Family 16).
//!
//! Native moegoe fork-extension surface mirroring PDFreactor's
//! `-ro-change-bar*` family for pre-press change-mark rules
//! (`docs/reference-manuals/pdfreactor.md:13805–13937`). The
//! paginator allocates a margin-side track, the paint stage draws
//! a vertical bar covering the fragment's vertical extent.
//!
//! All longhands apply to all elements; they are not inherited.

use std::fmt::{self, Write};

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::Percentage;
use crate::values::specified::border::LineWidth;
use crate::values::specified::color::Color;
use crate::values::specified::length::LengthPercentage;
use crate::OwnedStr;
use cssparser::Parser;
use style_traits::{CssWriter, ParseError, ToCss};

/// Specified value of `-bd-change-bar-align`.
///
/// Controls which side of the column / page the change bar sits on.
///
/// This is deliberately structured rather than a flat keyword enum:
/// the side and reference box are independent components, and
/// `distribute-column` modifies page-relative placement.  Keeping all
/// three components preserves declarations such as `outside
/// distribute-column` through the cascade instead of silently dropping
/// the later, more specific declaration.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdChangeBarSide {
    #[default]
    Start,
    End,
    Inside,
    Outside,
    Left,
    Right,
}

/// Reference box used to position a change bar.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdChangeBarReference {
    #[default]
    Page,
    Column,
}

/// Fully resolved specified value of `-bd-change-bar-align`.
#[repr(C)]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
pub struct BdChangeBarAlign {
    /// Logical or physical side used for the selected reference box.
    pub side: BdChangeBarSide,
    /// Whether the base position is the page or the containing column.
    pub reference: BdChangeBarReference,
    /// Route page-relative bars to the page edge nearest each column.
    pub distribute_column: bool,
}

impl BdChangeBarAlign {
    /// Initial value (`start page`).
    #[inline]
    pub fn start_page() -> Self {
        Self {
            side: BdChangeBarSide::Start,
            reference: BdChangeBarReference::Page,
            distribute_column: false,
        }
    }
}

impl Parse for BdChangeBarAlign {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let mut side = None;
        let mut saw_page = false;
        let mut saw_column = false;
        let mut distribute_column = false;
        let mut consumed = false;

        while !input.is_exhausted() {
            if side.is_none() {
                if let Ok(value) = input.try_parse(BdChangeBarSide::parse) {
                    side = Some(value);
                    consumed = true;
                    continue;
                }
            }
            if !saw_page
                && input
                    .try_parse(|input| input.expect_ident_matching("page"))
                    .is_ok()
            {
                saw_page = true;
                consumed = true;
                continue;
            }
            if !saw_column
                && input
                    .try_parse(|input| input.expect_ident_matching("column"))
                    .is_ok()
            {
                saw_column = true;
                consumed = true;
                continue;
            }
            if !distribute_column
                && input
                    .try_parse(|input| input.expect_ident_matching("distribute-column"))
                    .is_ok()
            {
                distribute_column = true;
                consumed = true;
                continue;
            }
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }

        if !consumed || (saw_column && (saw_page || distribute_column)) {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }

        Ok(Self {
            side: side.unwrap_or_default(),
            reference: if saw_column {
                BdChangeBarReference::Column
            } else {
                BdChangeBarReference::Page
            },
            distribute_column,
        })
    }
}

impl ToCss for BdChangeBarAlign {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        self.side.to_css(dest)?;
        match self.reference {
            BdChangeBarReference::Page if self.distribute_column => {
                dest.write_str(" distribute-column")?;
            },
            BdChangeBarReference::Page => {},
            BdChangeBarReference::Column => dest.write_str(" column")?,
        }
        Ok(())
    }
}

/// Specified value of `-bd-change-bar-exclusion`.
///
/// Controls which kinds of content the change bar suppresses
/// itself over (typically `none` or `headings`).
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
pub enum BdChangeBarExclusion {
    #[default]
    None,
    Headings,
}

/// Specified value of `-bd-change-bar-colour`.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdChangeBarColour {
    /// `auto` — fall back to `currentcolor`.
    Auto,
    /// Explicit colour.
    Colour(Color),
}

impl BdChangeBarColour {
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

impl crate::parser::Parse for BdChangeBarColour {
    fn parse<'i, 't>(
        context: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        Ok(Self::Colour(Color::parse(context, input)?))
    }
}

/// Specified value of `-bd-change-bar-name`.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdChangeBarName {
    /// `none` — the element does not contribute to any change-bar group.
    None,
    /// `<custom-ident>` — group identifier.
    Ident(OwnedStr),
}

impl Default for BdChangeBarName {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdChangeBarName {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl crate::parser::Parse for BdChangeBarName {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let ident = input.expect_ident()?;
        Ok(Self::Ident(ident.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-change-bar-offset`.
///
/// Signed distance from the column / page edge expressed as a length
/// or a percentage. Per PDFreactor's reference manual the initial
/// value is `25%` of the page-margin width (or, for column-anchored
/// bars, of the column gap). Negative values pull the bar inside the
/// margin track.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, Parse, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C)]
pub struct BdChangeBarOffset(pub LengthPercentage);

impl BdChangeBarOffset {
    /// Initial value (`25%`).
    ///
    /// Resolved against the page-margin width (or column gap) at
    /// allocation time in `moegoe_page::change_bars`.
    #[inline]
    pub fn quarter_percent() -> Self {
        Self(LengthPercentage::Percentage(Percentage(0.25)))
    }
}

/// Specified value of `-bd-change-bar-width`.
///
/// Mirrors the CSS Backgrounds & Borders `<line-width>` grammar:
/// the `thin` / `medium` / `thick` keywords (1px / 3px / 5px per
/// CSS-Backgrounds-3 §3.1) or an explicit `<length>`. PDFreactor's
/// `-ro-change-bar-width` accepts the same set
/// (`docs/reference-manuals/pdfreactor.md:13921`).
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, Parse, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C)]
pub struct BdChangeBarWidth(pub LineWidth);

impl BdChangeBarWidth {
    /// Initial value (`medium`).
    #[inline]
    pub fn medium() -> Self {
        Self(LineWidth::Medium)
    }
}
