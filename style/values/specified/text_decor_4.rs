/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Text Decoration Module Level 4 longhands not yet wired in
//! Stylo's standard surface.
//!
//! Implements the standard `text-decoration-trim`, `text-emphasis-skip`,
//! and the four `text-decoration-skip-{self,box,inset,spaces}` longhands
//! that the spec splits out from the (informational) `text-decoration-skip`
//! shorthand. `text-decoration-skip-ink` already exists in `text.rs`.
//!
//! Specs:
//!
//! - `text-decoration-trim` —
//!   <https://drafts.csswg.org/css-text-decor-4/#text-decoration-trim>
//! - `text-emphasis-skip` —
//!   <https://drafts.csswg.org/css-text-decor-4/#text-emphasis-skip>
//! - `text-decoration-skip-{self,box,inset,spaces}` —
//!   <https://drafts.csswg.org/css-text-decor-4/#text-decoration-skip-property>
//!
//! `text-decoration-trim` cascades through the `text` style struct
//! because it controls the geometry of the painted line for the element
//! itself (matching `text-decoration-{color,style,thickness}`). The
//! emphasis and skip longhands inherit and cascade through
//! `inherited_text` so descendant runs pick up the authored policy.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::Length;
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::values::SequenceWriter;
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// Specified value of the `text-decoration-trim` property
/// (<https://drafts.csswg.org/css-text-decor-4/#text-decoration-trim>).
///
/// `auto` (initial) — UA chooses the trim distance per side.
/// `<length>{1,2}` — explicit trim distance at the start (and optionally
/// the end) of the decoration line. A single length applies to both
/// edges.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum TextDecorationTrim {
    /// `auto` — UA-chosen trim distances (initial).
    Auto,
    /// `<length>{1,2}` — explicit trim distances. `end` defaults to
    /// `start` when only one length is authored.
    Length {
        /// Trim distance at the start edge of the line.
        start: Length,
        /// Trim distance at the end edge of the line.
        end: Length,
    },
}

impl TextDecorationTrim {
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

impl ToCss for TextDecorationTrim {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Length { start, end } => {
                start.to_css(dest)?;
                if start != end {
                    dest.write_char(' ')?;
                    end.to_css(dest)?;
                }
                Ok(())
            },
        }
    }
}

impl Parse for TextDecorationTrim {
    fn parse<'i, 't>(
        ctx: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let start = Length::parse(ctx, input)?;
        let end = input
            .try_parse(|i| Length::parse(ctx, i))
            .unwrap_or_else(|_| start.clone());
        Ok(Self::Length { start, end })
    }
}

/// Specified value of the `text-emphasis-skip` property
/// (<https://drafts.csswg.org/css-text-decor-4/#text-emphasis-skip>).
///
/// Grammar per spec: `<spaces> || <punctuation> || <symbols> ||
/// <narrow>` where each `<…>` token is a keyword name. Each flag is
/// independently togglable; the initial value is `spaces punctuation`.
///
/// `ToCss` is hand-rolled because the bitflags initial (`spaces
/// punctuation`) needs to serialise authored-order keywords; the
/// `#[css(bitflags)]` derive emits keywords in declaration order and
/// would not handle the "no keywords" form (which spec says is
/// disallowed for this property — at least one of the four flags must
/// be set per the grammar). Parse is derived via the `Parse` derive
/// with the `bitflags` attribute.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[css(bitflags(mixed = "spaces,punctuation,symbols,narrow"))]
#[repr(C)]
pub struct TextEmphasisSkip(u8);
bitflags! {
    impl TextEmphasisSkip: u8 {
        /// `spaces` — skip emphasis on white-space runs.
        const SPACES = 1 << 0;
        /// `punctuation` — skip emphasis on punctuation characters.
        const PUNCTUATION = 1 << 1;
        /// `symbols` — skip emphasis on symbol characters.
        const SYMBOLS = 1 << 2;
        /// `narrow` — skip emphasis on narrow-width characters.
        const NARROW = 1 << 3;
    }
}

impl TextEmphasisSkip {
    /// Initial value per spec (`spaces punctuation`).
    #[inline]
    pub fn initial() -> Self {
        Self::SPACES | Self::PUNCTUATION
    }
}

impl ToCss for TextEmphasisSkip {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if self.is_empty() {
            // The spec grammar requires at least one keyword; the empty
            // form should never round-trip through here. Emit `none` as
            // a defensive fallback so the serialisation always produces
            // a parseable token.
            return dest.write_str("none");
        }
        let mut writer = SequenceWriter::new(dest, " ");
        macro_rules! maybe_write {
            ($ident:ident => $str:expr) => {
                if self.contains(TextEmphasisSkip::$ident) {
                    writer.raw_item($str)?;
                }
            };
        }
        maybe_write!(SPACES => "spaces");
        maybe_write!(PUNCTUATION => "punctuation");
        maybe_write!(SYMBOLS => "symbols");
        maybe_write!(NARROW => "narrow");
        Ok(())
    }
}

/// Per-aspect value common to all four `text-decoration-skip-*` longhands.
///
/// CSS Text Decor 4 §5 defines per-aspect skip behaviour identically as
/// `none | auto | objects | spaces | leading-spaces trailing-spaces |
/// edges | box-decoration`. Each aspect (self, box, inset, spaces) reuses
/// the same value enum but the spec gives each its own default per aspect:
///
/// - `text-decoration-skip-self`: `objects`
/// - `text-decoration-skip-box`: `none`
/// - `text-decoration-skip-inset`: `none`
/// - `text-decoration-skip-spaces`: `start end`
///
/// The "leading-spaces trailing-spaces" form is unique to
/// `text-decoration-skip-spaces` — modelled here as a sub-enum so the
/// shared serialiser preserves the authored shape.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum TextDecorationSkipKind {
    /// `none` — no skip behaviour.
    None,
    /// `auto` — UA selects per context.
    Auto,
    /// `objects` — skip atomic inlines.
    Objects,
    /// `spaces` — skip leading and trailing spaces (single keyword).
    Spaces,
    /// `start` `end` — per-side spaces skipping.
    /// `text-decoration-skip-spaces` initial maps here as `(true, true)`.
    SpacesAtSides {
        /// Skip spaces at the start edge of the run.
        start: bool,
        /// Skip spaces at the end edge of the run.
        end: bool,
    },
    /// `edges` — skip the leading and trailing edge of the run.
    Edges,
    /// `box-decoration` — skip the area covered by the decoration of
    /// in-flow descendant boxes.
    BoxDecoration,
}

impl TextDecorationSkipKind {
    /// `none` value.
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// `objects` value (initial for `text-decoration-skip-self`).
    #[inline]
    pub fn objects() -> Self {
        Self::Objects
    }

    /// `start end` value (initial for `text-decoration-skip-spaces`).
    #[inline]
    pub fn start_end() -> Self {
        Self::SpacesAtSides {
            start: true,
            end: true,
        }
    }

    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for TextDecorationSkipKind {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Auto => dest.write_str("auto"),
            Self::Objects => dest.write_str("objects"),
            Self::Spaces => dest.write_str("spaces"),
            Self::SpacesAtSides { start, end } => {
                let mut writer = SequenceWriter::new(dest, " ");
                if *start {
                    writer.raw_item("start")?;
                }
                if *end {
                    writer.raw_item("end")?;
                }
                // Spec requires at least one keyword when this variant is used.
                if !start && !end {
                    writer.raw_item("none")?;
                }
                Ok(())
            },
            Self::Edges => dest.write_str("edges"),
            Self::BoxDecoration => dest.write_str("box-decoration"),
        }
    }
}

impl Parse for TextDecorationSkipKind {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let start_location = input.current_source_location();
        // First try the single-keyword forms.
        if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            match_ignore_ascii_case! { &ident,
                "none" => return Ok(Self::None),
                "auto" => return Ok(Self::Auto),
                "objects" => return Ok(Self::Objects),
                "spaces" => return Ok(Self::Spaces),
                "edges" => return Ok(Self::Edges),
                "box-decoration" => return Ok(Self::BoxDecoration),
                "start" => {
                    let end = input
                        .try_parse(|i| i.expect_ident_matching("end"))
                        .is_ok();
                    return Ok(Self::SpacesAtSides { start: true, end });
                },
                "end" => {
                    let start = input
                        .try_parse(|i| i.expect_ident_matching("start"))
                        .is_ok();
                    return Ok(Self::SpacesAtSides { start, end: true });
                },
                _ => return Err(start_location.new_unexpected_token_error(
                    cssparser::Token::Ident(ident.clone()),
                )),
            }
        }
        Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }
}
