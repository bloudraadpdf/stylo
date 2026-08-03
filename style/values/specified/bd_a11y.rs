/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe text-replace / tooltip CSS surface (F12).
//!
//! `-bd-text-replace`: ordered text transformations applied before
//! line-breaking. Each item contains two strings plus optional
//! replacement-point and matching-method keywords.
//!
//! `-bd-tooltip`: `none | <string>` — PDF tooltip annotation.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::{OwnedSlice, OwnedStr};
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// Pipeline position at which a text replacement runs.
#[derive(
    Clone,
    Debug,
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
#[repr(u8)]
pub enum BdTextReplacementPoint {
    /// Before white-space processing (the authored source text).
    Source,
    /// After white-space processing. This is the initial value.
    WhiteSpace,
    /// After white-space processing and `text-transform`.
    TextTransform,
    /// After glyph shaping.
    Shaped,
    /// During inline layout, retaining the source for logical operations.
    HybridLayout,
}

/// Matching or transformation method used by a text replacement.
#[derive(
    Clone,
    Debug,
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
#[repr(u8)]
pub enum BdTextReplacementMethod {
    /// Case-sensitive literal replacement. This is the initial value.
    Strict,
    /// Case-insensitive replacement.
    IgnoreCase,
    /// Replacement after folding case, presentation forms, and accents.
    IgnoreVariants,
    /// Regular-expression replacement.
    Regex,
    /// Unicode transliteration from the first string to the second.
    Transliterate,
}

/// One ordered `-bd-text-replace` operation.
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
#[repr(C)]
pub struct BdTextReplacement {
    /// Literal pattern, or transliteration source identifier.
    pub from: OwnedStr,
    /// Literal replacement, or transliteration destination identifier.
    pub to: OwnedStr,
    /// Pipeline point at which the operation executes.
    pub point: BdTextReplacementPoint,
    /// Matching/transformation method.
    pub method: BdTextReplacementMethod,
}

impl ToCss for BdTextReplacement {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        self.from.to_css(dest)?;
        dest.write_char(' ')?;
        self.to.to_css(dest)?;
        if self.point != BdTextReplacementPoint::WhiteSpace {
            dest.write_char(' ')?;
            self.point.to_css(dest)?;
        }
        if self.method != BdTextReplacementMethod::Strict {
            dest.write_char(' ')?;
            self.method.to_css(dest)?;
        }
        Ok(())
    }
}

/// Specified value of `-bd-text-replace`.
///
/// `none | [ <string> <string> [ <point> || <method> ]? ]#`.
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
pub enum BdTextReplace {
    /// `none` — no replacement.
    None,
    /// Ordered replacement operations.
    Pairs(OwnedSlice<BdTextReplacement>),
}

impl ToCss for BdTextReplace {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Pairs(replacements) => {
                let mut first = true;
                for replacement in replacements.iter() {
                    if !first {
                        dest.write_str(", ")?;
                    }
                    replacement.to_css(dest)?;
                    first = false;
                }
                Ok(())
            },
        }
    }
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
        let mut replacements = Vec::new();
        loop {
            let from: OwnedStr = input.expect_string()?.as_ref().to_owned().into();
            if from.is_empty() {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            let to: OwnedStr = input.expect_string()?.as_ref().to_owned().into();

            let mut point = None;
            let mut method = None;
            loop {
                if point.is_none() {
                    if let Ok(value) = input.try_parse(BdTextReplacementPoint::parse) {
                        point = Some(value);
                        continue;
                    }
                }
                if method.is_none() {
                    if let Ok(value) = input.try_parse(BdTextReplacementMethod::parse) {
                        method = Some(value);
                        continue;
                    }
                }
                break;
            }

            replacements.push(BdTextReplacement {
                from,
                to,
                point: point.unwrap_or(BdTextReplacementPoint::WhiteSpace),
                method: method.unwrap_or(BdTextReplacementMethod::Strict),
            });

            if input.is_exhausted() {
                break;
            }
            // Commas are canonical, but retain the original native spelling's
            // whitespace-separated list form for literal replacement pairs.
            let _ = input.try_parse(|i| i.expect_comma());
        }
        if replacements.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Pairs(OwnedSlice::from(replacements)))
    }
}

/// Specified value of `-bd-tooltip`.
///
/// `none | <string>`.
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
