/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe sidenote-area styling (F7).
//!
//! Native fork-extension surface for sidenote regions. Foreign syntax is
//! translated before declarations reach this parser.

use std::fmt::{self, Write};

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::LengthPercentage;
use crate::values::CustomIdent;
use crate::OwnedSlice;
use cssparser::Parser;
use style_traits::{CssWriter, ParseError, ToCss};

/// Specified value of `float-reference` extensions for sidenotes.
///
/// CSS Page Floats 3 already provides `inline | column | region |
/// page`; the sidenote extensions live on a sibling longhand
/// `-bd-float-reference-sidenote` to avoid widening the standard
/// enum surface that downstream consumers may match exhaustively.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BdFloatReferenceSidenote {
    #[default]
    None,
    Sidenote,
    Leftnote,
    Rightnote,
    Insidenote,
    Outsidenote,
}

/// Specified value of `-bd-sidenote-side`.
///
/// Which physical or spread-relative page side contains the sidenote area.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BdSidenoteSide {
    #[default]
    Auto,
    Inside,
    Outside,
    Left,
    Right,
}

/// Vertical reference used by `-bd-sidenote-align`.
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
pub enum BdSidenoteAlignment {
    Start,
    End,
    Stack,
    #[default]
    Baseline,
    ContainerStart,
    ContainerEnd,
}

/// Specified value of `-bd-sidenote-align`.
///
/// Alignment is vertical and independent from [`BdSidenoteSide`]. `strict`
/// is meaningful only for alignments with a specific originating position.
#[derive(
    Clone,
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
#[repr(C)]
pub struct BdSidenoteAlign {
    /// Vertical alignment reference.
    pub alignment: BdSidenoteAlignment,
    /// Whether the originating position must be preserved during stacking.
    pub strict: bool,
}

impl BdSidenoteAlign {
    /// Initial value (`baseline`).
    #[inline]
    pub fn baseline() -> Self {
        Self {
            alignment: BdSidenoteAlignment::Baseline,
            strict: false,
        }
    }
}

impl Parse for BdSidenoteAlign {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let alignment = BdSidenoteAlignment::parse(input)?;
        let strict = input
            .try_parse(|input| input.expect_ident_matching("strict"))
            .is_ok();
        if strict
            && !matches!(
                alignment,
                BdSidenoteAlignment::Baseline
                    | BdSidenoteAlignment::ContainerStart
                    | BdSidenoteAlignment::ContainerEnd
            )
        {
            return Err(input.new_custom_error(
                style_traits::StyleParseErrorKind::UnspecifiedError,
            ));
        }
        Ok(Self { alignment, strict })
    }
}

impl ToCss for BdSidenoteAlign {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        self.alignment.to_css(dest)?;
        if self.strict {
            dest.write_str(" strict")?;
        }
        Ok(())
    }
}

/// Specified value of `-bd-sidenote-avoid`.
///
/// `none` or a list of region names to avoid collisions against.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdSidenoteAvoid {
    /// `none` — no collision avoidance.
    None,
    /// `<custom-ident>+` — list of avoid targets.
    Names(#[css(iterable)] OwnedSlice<CustomIdent>),
}

impl BdSidenoteAvoid {
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

impl Parse for BdSidenoteAvoid {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let mut names: Vec<CustomIdent> = Vec::new();
        loop {
            let result = input.try_parse(|i| -> Result<CustomIdent, ParseError<'i>> {
                let location = i.current_source_location();
                let ident = i.expect_ident()?.clone();
                CustomIdent::from_ident(location, &ident, &["none"])
            });
            match result {
                Ok(name) => names.push(name),
                Err(_) => break,
            }
        }
        if names.is_empty() {
            return Err(input.new_custom_error(
                style_traits::StyleParseErrorKind::UnspecifiedError,
            ));
        }
        Ok(Self::Names(OwnedSlice::from(names)))
    }
}

/// Specified value of `-bd-sidenote-offset`.
///
/// Distance the sidenote anchor is shifted from its callout point.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped, Parse)]
#[repr(C)]
pub struct BdSidenoteOffset(pub LengthPercentage);

impl BdSidenoteOffset {
    /// Initial value (`0`).
    #[inline]
    pub fn zero() -> Self {
        Self(LengthPercentage::zero_percent())
    }
}
