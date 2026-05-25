/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-filter-resolution` per-element raster resolution for
//! CSS Filter Effects primitives.
//!
//! Aliases the Prince `prince-filter-resolution` descriptor (Prince
//! manual, `prince.md`). Controls the pixel density at which a
//! CSS Filter Effects L1 filter chain (`filter: url(#id)`,
//! `filter: blur()`, etc.) rasterises onto a backing pixmap.
//!
//! Per-element; not inherited. Initial `auto` — defer to the
//! backend default (a derivative of the page DPI; in moegoe the
//! `FILTER_RASTER_SCALE` constant in `moegoe-pdf`). An explicit
//! `<resolution>` clamps the per-element raster to the supplied
//! density (in dppx, after converting `dpi` / `dpcm` / `x` via the
//! standard CSS Values 4 §6.7 conversion).

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::Resolution;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// Specified value of the `-bd-filter-resolution` property.
///
/// `auto` (initial) defers to the backend's raster-density default.
/// `<resolution>` pins the per-element filter rasterisation density.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdFilterResolution {
    /// `auto` — defer to the backend default raster density.
    Auto,
    /// `<resolution>` — explicit per-element density.
    Resolution(Resolution),
}

impl BdFilterResolution {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto` (initial — no cascade override).
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToCss for BdFilterResolution {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Resolution(r) => r.to_css(dest),
        }
    }
}

impl Parse for BdFilterResolution {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        Ok(Self::Resolution(Resolution::parse(context, input)?))
    }
}
