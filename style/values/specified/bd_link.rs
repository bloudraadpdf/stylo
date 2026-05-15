/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe link-styling CSS surface (F11).
//!
//! `-bd-link` declares an element as carrying a PDF link annotation
//! without requiring an HTML `<a>`. `-bd-link-area` selects which
//! box shape the link rectangle covers.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::url::SpecifiedUrl;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value of `-bd-link`.
///
/// `none | auto | url(...)`. `auto` lets the renderer infer the
/// target from an enclosing `<a>` ancestor or `target-counter()`
/// usage (per PDFreactor `-ro-link`).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdLink {
    /// `none` — no link annotation.
    None,
    /// `auto` (initial) — defer to renderer.
    Auto,
    /// `url(...)` — explicit link target.
    Url(SpecifiedUrl),
}

impl BdLink {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdLink {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        Ok(Self::Url(SpecifiedUrl::parse(context, input)?))
    }
}

/// Specified value of `-bd-link-area`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdLinkArea {
    #[default]
    BorderBox,
    PaddingBox,
    ContentBox,
    Text,
}
