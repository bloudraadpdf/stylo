/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-change-bar-*` revision-mark properties (Family 16).
//!
//! Native moegoe fork-extension surface mirroring PDFreactor's
//! `-ro-change-bar*` family for pre-press change-mark rules
//! (`docs/reference-manuals/pdfreactor.md:13805–13937`). The
//! paginator allocates a margin-side track, the paint stage draws
//! a vertical bar covering the fragment's vertical extent.
//!
//! All longhands apply to all elements; they are not inherited.

use crate::derives::*;
use crate::values::specified::color::Color;
use crate::OwnedStr;

/// Specified value of `-bd-change-bar-align`.
///
/// Controls which side of the column / page the change bar sits
/// on. `start` / `end` map to writing-mode-aware sides; explicit
/// physical keywords are accepted for compatibility.
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
pub enum BdChangeBarAlign {
    #[default]
    Start,
    End,
    Inside,
    Outside,
    Left,
    Right,
}

/// Specified value of `-bd-change-bar-exclusion`.
///
/// Controls which kinds of content the change bar suppresses
/// itself over (typically `none` or `headings`).
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
pub enum BdChangeBarExclusion {
    #[default]
    None,
    Headings,
}

/// Specified value of `-bd-change-bar-colour`.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdChangeBarColour {
    /// `auto` — fall back to `currentcolor`.
    Auto,
    /// Explicit colour.
    Colour(Color),
}

impl BdChangeBarColour {
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

impl crate::parser::Parse for BdChangeBarColour {
    fn parse<'i, 't>(
        context: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
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

/// Specified value of `-bd-change-bar-name`.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdChangeBarName {
    /// `none` — the element does not contribute to any change-bar group.
    None,
    /// `<custom-ident>` — group identifier.
    Ident(OwnedStr),
}

impl Default for BdChangeBarName {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdChangeBarName {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl crate::parser::Parse for BdChangeBarName {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let ident = input.expect_ident()?;
        Ok(Self::Ident(ident.as_ref().to_owned().into()))
    }
}

// Note: `-bd-change-bar-offset` and `-bd-change-bar-width` reuse
// the existing `Length` / `NonNegativeLength` predefined types in
// `longhands.toml` directly — no `BdChangeBarOffset` /
// `BdChangeBarWidth` alias is required.
