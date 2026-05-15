/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe text-replace / tooltip CSS surface (F12).
//!
//! `-bd-text-replace`: `none | [ <string> <string> ]+` —
//! orthography substitution applied before line-breaking. Pairs
//! `(from, to)` are flattened into a single `OwnedSlice` (even
//! indices = source, odd = replacement) to keep the typed-value
//! shape diff-friendly across `ToShmem`.
//!
//! `-bd-tooltip`: `none | <string>` — PDF tooltip annotation.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::{OwnedSlice, OwnedStr};
use cssparser::Parser;
use style_traits::{ParseError, StyleParseErrorKind};

/// Specified value of `-bd-text-replace`.
///
/// `none | [ <string> <string> ]+`. The pairs are stored as a
/// flat `OwnedSlice<OwnedStr>` with `[from0, to0, from1, to1, ...]`
/// ordering — the slice length is therefore always even when
/// the value is non-empty.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTextReplace {
    /// `none` — no replacement.
    None,
    /// `[ <string> <string> ]+` — flat pairs.
    Pairs(#[css(iterable)] OwnedSlice<OwnedStr>),
}

impl BdTextReplace {
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

impl Parse for BdTextReplace {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let mut pairs: Vec<OwnedStr> = Vec::new();
        loop {
            // try to parse a pair (from, to)
            let result = input.try_parse(|i| -> Result<(OwnedStr, OwnedStr), ParseError<'i>> {
                let from = i.expect_string()?;
                let from_owned: OwnedStr = from.as_ref().to_owned().into();
                let to = i.expect_string()?;
                let to_owned: OwnedStr = to.as_ref().to_owned().into();
                Ok((from_owned, to_owned))
            });
            match result {
                Ok((from, to)) => {
                    pairs.push(from);
                    pairs.push(to);
                }
                Err(_) => break,
            }
        }
        if pairs.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Pairs(OwnedSlice::from(pairs)))
    }
}

/// Specified value of `-bd-tooltip`.
///
/// `none | <string>`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTooltip {
    /// `none` — no tooltip.
    None,
    /// `<string>` — tooltip text.
    Literal(OwnedStr),
}

impl BdTooltip {
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

impl Parse for BdTooltip {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}
