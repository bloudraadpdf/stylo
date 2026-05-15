/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe bookmark-target + PDF link-type CSS surface (F10).
//!
//! `bookmark-target` is the standard-named Prince/PFB extension
//! per `prince.md:1088`; it complements the existing
//! `bookmark-{label,level,state}` longhands by pointing the
//! bookmark at a destination (URL or counter index).
//!
//! `-bd-pdf-link-type` governs whether a bookmark / link annotation
//! opens in the viewer or embeds.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::url::SpecifiedUrl;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// Specified value of `bookmark-target`.
///
/// `none | <url> | <integer>`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem)]
#[repr(C, u8)]
pub enum BookmarkTarget {
    /// `none` — no link.
    None,
    /// `<url>` — link to an external or internal URL.
    Url(SpecifiedUrl),
    /// `<integer>` — counter index into a generated list.
    Counter(i32),
}

impl style_traits::ToTyped for BookmarkTarget {}

impl BookmarkTarget {
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

impl Parse for BookmarkTarget {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        if let Ok(url) = input.try_parse(|i| SpecifiedUrl::parse(context, i)) {
            return Ok(Self::Url(url));
        }
        let v = input.expect_integer()?;
        Ok(Self::Counter(v))
    }
}

impl ToCss for BookmarkTarget {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::None => dest.write_str("none"),
            Self::Url(u) => u.to_css(dest),
            Self::Counter(i) => i.to_css(dest),
        }
    }
}

/// Specified value of `-bd-pdf-link-type`.
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
pub enum BdPdfLinkType {
    #[default]
    Auto,
    None,
    Link,
    Embed,
}
