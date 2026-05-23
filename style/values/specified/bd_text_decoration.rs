/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe per-position text-decoration properties (Prince G — `text-overline`
//! / `text-underline` / `text-line-through` longhand surface).
//!
//! CSS Text Decoration 4 provides only `text-decoration-{color,style,
//! thickness,line}` — a single colour / style / thickness applies to every
//! drawn line. Prince exposes `text-overline`, `text-underline` and
//! `text-line-through` shorthands that style each line position
//! independently. Moegoe carries this as native `-bd-text-{overline,
//! underline,linethrough}-{color,style,thickness}` longhands, with `auto`
//! initials so the cascade reader can fall back to the standard
//! `text-decoration-*` value when a per-position override is absent.
//!
//! The Stylo-side shape is intentionally minimal: each longhand is an
//! `auto | <value>` enum that carries the inheritance / cascading
//! behaviour, and the IR conversion folds the resolved per-position
//! triple into a single `TextDecorationLineStyle` per line position.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::text::GenericTextDecorationLength;
use crate::values::specified::color::Color;
use crate::values::specified::length::{Length, LengthPercentage};
use crate::OwnedSlice;
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// Specified value of `-bd-text-{position}-color`.
///
/// `auto` (initial) — fall back to the standard `text-decoration-color`
/// cascade. `<color>` — override the colour for this line position only.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTextDecorationLineColour {
    /// `auto` — defer to `text-decoration-color`.
    Auto,
    /// Explicit colour override.
    Colour(Color),
}

impl BdTextDecorationLineColour {
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

impl Parse for BdTextDecorationLineColour {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
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

/// Specified value of `-bd-text-{position}-style`.
///
/// Mirrors Stylo's standard `text-decoration-style` keyword set with an
/// additional `auto` value meaning "fall back to `text-decoration-style`".
///
/// Intentionally non-`Copy` so the property-generation framework's
/// `AssertNotCopy` shadow trait does not collide with `AssertCopy` for
/// this specified-value type (Stylo's generated property tables expect
/// non-`Copy` specified values for non-trivial enums).
#[derive(
    Clone, Debug, Eq, MallocSizeOf, Parse, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(u8)]
pub enum BdTextDecorationLineStyle {
    /// `auto` — defer to `text-decoration-style`.
    Auto,
    /// `solid`.
    Solid,
    /// `double`.
    Double,
    /// `dotted`.
    Dotted,
    /// `dashed`.
    Dashed,
    /// `wavy`.
    Wavy,
}

impl BdTextDecorationLineStyle {
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

/// Specified value of `-bd-text-{position}-thickness`.
///
/// `auto` (initial) — fall back to the standard `text-decoration-thickness`
/// cascade. `from-font` / `<length-percentage>` — override the thickness
/// for this line position only.
///
/// Wrapped in a newtype rather than a type alias so the generated
/// property-table trait impls (`AssertNotCopy`) do not collide with the
/// standard `text-decoration-thickness` longhand (which uses the same
/// generic instantiation `GenericTextDecorationLength<LengthPercentage>`).
/// `ToComputedValue` is implemented manually so the specified-side wrapper
/// resolves to the computed-side wrapper rather than to the bare generic.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdTextDecorationLineThickness(pub GenericTextDecorationLength<LengthPercentage>);

impl BdTextDecorationLineThickness {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(GenericTextDecorationLength::Auto)
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self.0, GenericTextDecorationLength::Auto)
    }
}

impl Parse for BdTextDecorationLineThickness {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        Ok(Self(GenericTextDecorationLength::parse(context, input)?))
    }
}

/// Specified value of `-bd-text-underline-offset`.
///
/// `auto` (initial) — defer to the standard `text-underline-offset`
/// cascade. `<length>` — explicit offset between the underline and the
/// alphabetic baseline for the underline line position only.
///
/// Newtype wrapper around the same generic `text-underline-offset`
/// payload (`LengthPercentageOrAuto`) used by the standard property,
/// so the cascade reader can distinguish per-position overrides from
/// the global value and apply Prince's per-position semantics.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdTextUnderlineOffset(pub crate::values::specified::LengthPercentageOrAuto);

impl BdTextUnderlineOffset {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(crate::values::specified::LengthPercentageOrAuto::Auto)
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(
            self.0,
            crate::values::specified::LengthPercentageOrAuto::Auto
        )
    }
}

impl Parse for BdTextUnderlineOffset {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        Ok(Self(crate::values::specified::LengthPercentageOrAuto::parse(
            context, input,
        )?))
    }
}

/// Specified value of `-bd-text-underline-position`.
///
/// `auto` (initial) — defer to the standard `text-underline-position`
/// cascade. The remaining keywords mirror the standard property's
/// `from-font | under | [ left | right ]` set, applied to the
/// `-bd-text-underline-*` per-position triple. The standard property
/// uses bitflags to model `from-font || left`, etc.; the per-position
/// surface keeps the simpler enum because per-position overrides are
/// authored explicitly (one value at a time).
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
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
pub enum BdTextUnderlinePosition {
    /// `auto` — defer to `text-underline-position` (initial).
    Auto,
    /// `from-font` — use the underline metrics declared by the font.
    FromFont,
    /// `under` — place the underline below the glyph box.
    Under,
    /// `left` — in vertical text, place to the left of the run.
    Left,
    /// `right` — in vertical text, place to the right of the run.
    Right,
}

impl BdTextUnderlinePosition {
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

/// Specified value of `-bd-text-decoration-trim` (moegoe fork).
///
/// Author-facing single-length companion to the standard
/// `text-decoration-trim` from CSS Text Decoration 4 §1.3. The standard
/// grammar accepts `auto | <length>{1,2}` (one length per edge); the
/// `-bd-` variant collapses the per-edge form into `none | auto |
/// <length>` so authors that only want a symmetric trim can express it
/// in a single declaration. Spelt as a separate longhand rather than a
/// shorthand because it cascades independently from the underlying
/// `text-decoration-trim`: `none` here means "no trim, override
/// `text-decoration-trim`", and `auto` (initial) means "defer to
/// `text-decoration-trim`".
///
/// Cascade-side: any non-`auto` value overrides the resolved
/// `text-decoration-trim` for this element only (the standard
/// longhand still inherits / cascades normally for descendants).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTextDecorationTrim {
    /// `auto` — defer to `text-decoration-trim` (initial).
    Auto,
    /// `none` — explicitly disable trimming on this element, regardless
    /// of the standard `text-decoration-trim` cascade.
    None,
    /// `<length>` — apply this trim symmetrically to both edges of the
    /// decoration line, overriding the standard cascade.
    Length(Length),
}

impl BdTextDecorationTrim {
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

impl ToCss for BdTextDecorationTrim {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::None => dest.write_str("none"),
            Self::Length(len) => len.to_css(dest),
        }
    }
}

impl Parse for BdTextDecorationTrim {
    fn parse<'i, 't>(
        ctx: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            match_ignore_ascii_case! { &ident,
                "auto" => return Ok(Self::Auto),
                "none" => return Ok(Self::None),
                _ => return Err(location.new_unexpected_token_error(
                    cssparser::Token::Ident(ident.clone()),
                )),
            }
        }
        Ok(Self::Length(Length::parse(ctx, input)?))
    }
}

/// Skip category for `-bd-text-decoration-skip` (moegoe fork).
///
/// One of the per-category skip behaviours the under/over-line can
/// honour. Independent from the standard
/// `text-decoration-skip-{self,box,inset,spaces}` longhands — when an
/// author writes `-bd-text-decoration-skip: spaces`, the cascade reader
/// fans the keyword out onto the relevant aspect (here:
/// `text-decoration-skip-spaces`).
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
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum BdTextDecorationSkipCategory {
    /// `objects` — skip atomic inline replaced elements.
    Objects,
    /// `spaces` — skip both leading and trailing white-space runs.
    Spaces,
    /// `leading-spaces` — skip white-space at the start of the run only.
    LeadingSpaces,
    /// `trailing-spaces` — skip white-space at the end of the run only.
    TrailingSpaces,
    /// `edges` — skip the leading and trailing edge of the run.
    Edges,
    /// `box-decoration` — skip the area covered by descendant box
    /// decorations.
    BoxDecoration,
}

/// Specified value of `-bd-text-decoration-skip` (moegoe fork).
///
/// Author-facing collapsed form of the standard CSS Text Decoration 4
/// `text-decoration-skip` shorthand. Grammar:
/// `none | <category>#` where `<category>` is one of `objects |
/// spaces | leading-spaces | trailing-spaces | edges | box-decoration`.
/// Multiple categories are accepted as a comma-separated list; each
/// category enables the corresponding skip behaviour. The cascade
/// reader fans the resulting set onto the IR
/// `TextDecoration::bd_skip` field consumed by paint.
///
/// `none` (initial) defers to the standard `text-decoration-skip-*`
/// cascade — paint observes the existing per-aspect IR fields. A
/// non-empty category list overrides that cascade for this element
/// only.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTextDecorationSkip {
    /// `none` — defer to the standard `text-decoration-skip-*`
    /// cascade. This is the initial.
    None,
    /// One or more comma-separated `<category>` keywords. Each
    /// category bit enables that skip behaviour on top of the standard
    /// cascade.
    Categories(OwnedSlice<BdTextDecorationSkipCategory>),
}

impl BdTextDecorationSkip {
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

impl ToCss for BdTextDecorationSkip {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Categories(cats) => {
                let mut first = true;
                for cat in cats.iter() {
                    if !first {
                        dest.write_str(", ")?;
                    }
                    cat.to_css(dest)?;
                    first = false;
                }
                Ok(())
            }
        }
    }
}

impl Parse for BdTextDecorationSkip {
    fn parse<'i, 't>(
        _ctx: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let categories =
            input.parse_comma_separated(|i| BdTextDecorationSkipCategory::parse(i))?;
        if categories.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Categories(OwnedSlice::from(categories)))
    }
}

/// Specified value of `-bd-text-emphasis-skip` (moegoe fork).
///
/// Author-facing single-keyword variant of the standard
/// `text-emphasis-skip` from CSS Text Decoration 4 §1.7.3. The
/// standard property is a bitflags initial of `spaces punctuation`; the
/// `-bd-` variant collapses to a single keyword so authors that want a
/// single category can express it without learning the bitflags
/// serialisation. The cascade reader projects the keyword onto the IR
/// `TextEmphasisSkip` struct.
///
/// Initial is `auto`, which defers to the standard
/// `text-emphasis-skip` cascade. A concrete keyword overrides that
/// cascade for this element only.
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
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum BdTextEmphasisSkip {
    /// `auto` — defer to the standard `text-emphasis-skip` (initial).
    Auto,
    /// `none` — explicitly disable all category-based skipping.
    None,
    /// `spaces` — skip emphasis on whitespace runs only.
    Spaces,
    /// `punctuation` — skip emphasis on punctuation characters only.
    Punctuation,
    /// `symbols` — skip emphasis on symbol characters only.
    Symbols,
    /// `narrow` — skip emphasis on narrow-width characters only.
    Narrow,
}

impl BdTextEmphasisSkip {
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
