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
use crate::values::specified::length::LengthPercentage;
use cssparser::Parser;

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
