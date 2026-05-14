/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-tab-stops` property.
//!
//! Positional tab-stop list as authored on a paragraph (per
//! `docs/DOCX-to-HTML-CSS.md` §6.2.5 in the moegoe repository).
//! Standard CSS `tab-size` defines the interval used between stops;
//! `-bd-tab-stops` defines explicit positions at which a tab character
//! lands first, with per-stop alignment and optional leader.
//!
//! Syntax:
//!
//! ```text
//! -bd-tab-stops: none
//!              | <length> <alignment> <leader>?, ...
//! ```
//!
//! `none` (initial) clears any inherited stops and the paragraph falls
//! back to interval tab-size behaviour. `<alignment>` is one of
//! `left | center | right | decimal | bar`; `<leader>` defaults to
//! `none` and is one of `none | dotted | hyphen | underscore | heavy |
//! middle-dot`. Leader values mirror the GCPM `leader()` vocabulary
//! where they overlap. The property inherits like `tab-size`.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::NonNegativeLength;
use crate::OwnedSlice;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// Alignment applied to text positioned at a tab stop.
///
/// `bar` is preserved from the OOXML / `<w:tabs>` vocabulary so the
/// converter can round-trip Word documents that declare bar tabs.
/// The renderer treats `bar` as a decoration hint (draw a vertical
/// bar at the position) rather than a positioning anchor.
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
pub enum BdTabStopAlignment {
    #[default]
    Left,
    Center,
    Right,
    Decimal,
    Bar,
}

/// Leader glyph repeated in the empty span before a tab stop.
///
/// Variants align with the GCPM `leader()` vocabulary where they
/// overlap (`dotted`, `hyphen`, `underscore`). `heavy` and
/// `middle-dot` round-trip the OOXML `<w:leader>` values that GCPM
/// does not name.
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
pub enum BdTabStopLeader {
    #[default]
    None,
    Dotted,
    Hyphen,
    Underscore,
    Heavy,
    MiddleDot,
}

impl BdTabStopLeader {
    /// Whether the value is the default (`none`).
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// One entry in a `-bd-tab-stops` list.
///
/// Serialised as `<length> <alignment>` followed by the leader only
/// when it differs from the default (`none`).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdTabStop {
    /// Position of the tab stop, measured from the inline start of
    /// the containing block.
    pub position: NonNegativeLength,
    /// Alignment applied to text at this stop.
    pub alignment: BdTabStopAlignment,
    /// Leader glyph repeated up to this stop.
    pub leader: BdTabStopLeader,
}

impl ToCss for BdTabStop {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        self.position.to_css(dest)?;
        dest.write_char(' ')?;
        self.alignment.to_css(dest)?;
        if !self.leader.is_none() {
            dest.write_char(' ')?;
            self.leader.to_css(dest)?;
        }
        Ok(())
    }
}

/// Specified value of the `-bd-tab-stops` property.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTabStops {
    /// `none` — clear inherited stops and fall back to `tab-size`.
    None,
    /// Comma-separated list of positional stops.
    Stops(OwnedSlice<BdTabStop>),
}

impl BdTabStops {
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

impl ToCss for BdTabStops {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Stops(stops) => {
                let mut first = true;
                for stop in stops.iter() {
                    if !first {
                        dest.write_str(", ")?;
                    }
                    stop.to_css(dest)?;
                    first = false;
                }
                Ok(())
            }
        }
    }
}

impl Parse for BdTabStop {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let position = NonNegativeLength::parse(context, input)?;
        let alignment = BdTabStopAlignment::parse(input)?;
        let leader = input
            .try_parse(BdTabStopLeader::parse)
            .unwrap_or(BdTabStopLeader::None);
        Ok(BdTabStop {
            position,
            alignment,
            leader,
        })
    }
}

impl Parse for BdTabStops {
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
        let stops = input.parse_comma_separated(|i| BdTabStop::parse(context, i))?;
        if stops.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Stops(OwnedSlice::from(stops)))
    }
}
