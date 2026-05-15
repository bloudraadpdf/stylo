/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-page-colorbar-*`, `-bd-page-marks`,
//! `-bd-page-print-mark-set` page-margin print-shop tooling
//! (Family 20).
//!
//! Native moegoe fork-extension surface for PDFreactor's
//! `-ro-colorbar-*` and `-ro-marks` properties (see
//! `docs/reference-manuals/pdfreactor.md:14026–14043, 16279`).
//! Eight positional colour-bar slots plus an offset, a marks
//! shorthand, and a print-mark-set synthesised property.
//!
//! All longhands are `@page` descriptors (`rule_types_allowed =
//! ["page"]`).

use crate::derives::*;
use crate::values::specified::url::UrlOrNone;

/// Specified value of a `-bd-page-colorbar-*` slot.
///
/// `none` (initial) — slot empty. `auto` — engine default colour
/// bar (per ISO 12647). `<url>` — explicit colour-bar artwork.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdColorBarPosition {
    /// `none`.
    None,
    /// `auto`.
    Auto,
    /// `<url>`.
    Url(UrlOrNone),
}

impl Default for BdColorBarPosition {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdColorBarPosition {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl crate::parser::Parse for BdColorBarPosition {
    fn parse<'i, 't>(
        context: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        if input
            .try_parse(|i| i.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        Ok(Self::Url(UrlOrNone::parse(context, input)?))
    }
}

// `-bd-page-colorbar-offset` uses the predefined `Length` type
// directly.

impl BdColorBarPosition {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

/// Specified value of `-bd-page-print-mark-set`.
///
/// Selects one of the well-known print-mark presets.
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
pub enum BdPrintMarkSet {
    #[default]
    Auto,
    None,
    Default,
    Iso12647,
    Pdfx,
    Custom,
}
