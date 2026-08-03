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
//!              | <length> <alignment>? <leader>?, ...
//! ```
//!
//! `none` (initial) clears any inherited stops and the paragraph falls
//! back to interval tab-size behaviour. `<alignment>` is one of
//! `left | center | right | decimal | bar` (defaults to `left` when
//! omitted, matching the OOXML `<w:tab>` default).
//!
//! `<leader>` defaults to `none` and is one of
//! `none | dotted | dashed | solid | double | hyphen | underscore |
//! heavy | middle-dot | <string>`. The keyword leaders mirror the
//! GCPM `leader()` vocabulary where they overlap; `dashed`, `solid`
//! and `double` are the moegoe additions needed to round-trip the
//! Word leader vocabulary the converter emits. `<string>` allows the
//! author to specify an arbitrary leader glyph (Unicode code-points
//! permitted). The property inherits like `tab-size`.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::NonNegativeLength;
use crate::OwnedSlice;
use crate::OwnedStr;
use cssparser::{match_ignore_ascii_case, Parser};
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
/// The keyword variants align with the GCPM `leader()` vocabulary
/// where they overlap (`dotted`, `dashed`, `solid`, `double`,
/// `hyphen`, `underscore`); `heavy` and `middle-dot` round-trip the
/// OOXML `<w:leader>` values that GCPM does not name. The `Custom`
/// variant carries an author-supplied `<string>` (e.g. `"·"`,
/// `"--"`) that the renderer repeats verbatim.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
#[allow(missing_docs)]
pub enum BdTabStopLeader {
    None,
    Dotted,
    Dashed,
    Solid,
    Double,
    Hyphen,
    Underscore,
    Heavy,
    MiddleDot,
    /// Author-supplied leader glyph (`<string>` form).
    Custom(OwnedStr),
}

impl Default for BdTabStopLeader {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdTabStopLeader {
    /// Whether the value is the default (`none`).
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for BdTabStopLeader {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Dotted => dest.write_str("dotted"),
            Self::Dashed => dest.write_str("dashed"),
            Self::Solid => dest.write_str("solid"),
            Self::Double => dest.write_str("double"),
            Self::Hyphen => dest.write_str("hyphen"),
            Self::Underscore => dest.write_str("underscore"),
            Self::Heavy => dest.write_str("heavy"),
            Self::MiddleDot => dest.write_str("middle-dot"),
            Self::Custom(s) => {
                // Serialise as a CSS string literal.
                dest.write_char('"')?;
                for c in s.chars() {
                    match c {
                        '"' => dest.write_str("\\\"")?,
                        '\\' => dest.write_str("\\\\")?,
                        _ => dest.write_char(c)?,
                    }
                }
                dest.write_char('"')
            },
        }
    }
}

impl Parse for BdTabStopLeader {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        // Try `<string>` first to avoid eating a bare ident that may
        // belong to a following comma-separated entry.
        if let Ok(s) = input.try_parse(|i| i.expect_string().map(|s| s.as_ref().to_owned())) {
            return Ok(Self::Custom(s.into()));
        }
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        Ok(match_ignore_ascii_case! { &ident,
            "none" => Self::None,
            "dotted" => Self::Dotted,
            "dashed" => Self::Dashed,
            "solid" => Self::Solid,
            "double" => Self::Double,
            "hyphen" => Self::Hyphen,
            "underscore" => Self::Underscore,
            "heavy" => Self::Heavy,
            "middle-dot" => Self::MiddleDot,
            _ => return Err(location.new_unexpected_token_error(
                cssparser::Token::Ident(ident.clone()),
            )),
        })
    }
}

/// One entry in a `-bd-tab-stops` list.
///
/// Serialised as `<length>`, followed by `<alignment>` only when it
/// differs from the default (`left`) and `<leader>` only when it
/// differs from the default (`none`).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdTabStop {
    /// Position of the tab stop, measured from the inline start of
    /// the containing block.
    pub position: NonNegativeLength,
    /// Alignment applied to text at this stop. Defaults to `left`
    /// when the author omits the keyword.
    pub alignment: BdTabStopAlignment,
    /// Leader glyph repeated up to this stop.
    pub leader: BdTabStopLeader,
}

impl ToCss for BdTabStop {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        self.position.to_css(dest)?;
        if !matches!(self.alignment, BdTabStopAlignment::Left) {
            dest.write_char(' ')?;
            self.alignment.to_css(dest)?;
        }
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
            },
        }
    }
}

impl Parse for BdTabStop {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let position = NonNegativeLength::parse(context, input)?;
        let alignment = input
            .try_parse(BdTabStopAlignment::parse)
            .unwrap_or_default();
        let leader = input
            .try_parse(|i| BdTabStopLeader::parse(context, i))
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
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let stops = input.parse_comma_separated(|i| BdTabStop::parse(context, i))?;
        if stops.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Stops(OwnedSlice::from(stops)))
    }
}
