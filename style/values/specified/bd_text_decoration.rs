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
