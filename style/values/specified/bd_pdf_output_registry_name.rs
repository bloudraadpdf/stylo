/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-output-registry-name` property (K10).
//!
//! Document-level descriptor projecting onto the PDF output-intent
//! `RegistryName` entry (ISO 32000-2 §14.11.5, Table 401). The slot
//! is typically a URL identifying the registry that hosts the output
//! profile (for example `http://www.color.org`), but the spec also
//! permits a free-form string identifier.
//!
//! Grammar: `none | <url> | <string>`. `none` (initial) leaves the
//! slot unset. The cascade reader only honours declarations on
//! `:root`.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::url::SpecifiedUrl;
use crate::OwnedStr;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value of the `-bd-pdf-output-registry-name` property.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfOutputRegistryName {
    /// `none` — no `RegistryName` entry is emitted.
    None,
    /// `<url>` — registry identifier URL.
    Url(SpecifiedUrl),
    /// `<string>` — literal registry-name string.
    String(OwnedStr),
}

impl BdPdfOutputRegistryName {
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

impl Parse for BdPdfOutputRegistryName {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        if let Ok(url) = input.try_parse(|i| SpecifiedUrl::parse(context, i)) {
            return Ok(Self::Url(url));
        }
        let s = input.expect_string()?;
        Ok(Self::String(s.as_ref().to_owned().into()))
    }
}
