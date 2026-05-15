/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe sidenote-area styling (F7).
//!
//! Native fork-extension surface for sidenote regions per
//! PDFreactor `-ro-sidenote-*` and Prince `float-reference:
//! sidenote|leftnote|rightnote|insidenote|outsidenote`. The
//! audit `docs/audits/CSS-COVERAGE-AUDIT-2026-05-14/stylo-push-plan.md`
//! family 7 enumerates source vendors.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::LengthPercentage;
use crate::values::CustomIdent;
use crate::OwnedSlice;
use cssparser::Parser;
use style_traits::ParseError;

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

/// Specified value of `-bd-sidenote-align`.
///
/// Which side of the sidenote area the call/note anchors to.
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
pub enum BdSidenoteAlign {
    #[default]
    Auto,
    Inside,
    Outside,
    Left,
    Right,
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
