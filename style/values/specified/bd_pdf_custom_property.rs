/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-custom-property` document-level custom Info-dict
//! property descriptor.
//!
//! Threads one or more `(name, value)` pairs into the PDF Info
//! dictionary as custom entries (ISO 32000-2 §14.3.3 — "any number of
//! additional entries containing application-specific data" — and
//! mirrored into the XMP packet so PDF/A consumers (which require
//! Info-dict entries to be reflected in XMP per ISO 19005-1 §6.7.3)
//! see the same metadata. This is moegoe's native equivalent of
//! PDFreactor's `customDocumentProperties` configuration property
//! (`docs/reference-manuals/pdfreactor.md` line 8911); neither Prince
//! nor PDFreactor exposes a CSS surface for this, so the longhand is
//! fork-extension territory.
//!
//! Grammar:
//!
//! ```text
//! none | <custom-ident> <string> [ , <custom-ident> <string> ]*
//! ```
//!
//! The `<custom-ident>` is the PDF Info-dict key (rendered as a PDF
//! name token with the leading `/`; spec-reserved names like `/Title`,
//! `/Author`, `/Subject`, `/Keywords`, `/Creator`, `/Producer`,
//! `/CreationDate`, `/ModDate`, `/Trapped`, `/GTS_PDFXVersion`,
//! `/PTEX.Fullbanner` are rejected at the cascade-reader boundary so
//! authors cannot collide with the standard slots). The `<string>` is
//! the PDF Info-dict value, emitted as a PDF `text string` per ISO
//! 32000-2 §7.9.2.2 (UTF-16BE BOM-prefixed when the string contains
//! non-ASCII, or PDFDocEncoded for ASCII).
//!
//! Document-level; the cascade reader only honours declarations on
//! `:root`. Multiple declarations on the same element merge by
//! concatenation at the cascade reader; pairs with identical names
//! follow last-wins semantics there.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::CustomIdent;
use crate::{OwnedSlice, OwnedStr};
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// One `(custom-ident, string)` entry of `-bd-pdf-custom-property`.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C)]
pub struct BdPdfCustomPropertyEntry {
    /// PDF Info-dictionary key (custom name). Encoded as a PDF name
    /// token with the standard `#xx` escaping for non-printable or
    /// reserved characters at the renderer boundary.
    pub name: CustomIdent,
    /// PDF Info-dictionary value (text string).
    pub value: OwnedStr,
}

impl ToCss for BdPdfCustomPropertyEntry {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        self.name.to_css(dest)?;
        dest.write_char(' ')?;
        // CSS string escaping is provided by the OwnedStr ToCss impl.
        self.value.to_css(dest)
    }
}

/// Specified value of `-bd-pdf-custom-property`.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfCustomProperty {
    /// `none` — no custom Info-dict entries.
    None,
    /// Comma-separated list of `(custom-ident, string)` pairs.
    Entries(OwnedSlice<BdPdfCustomPropertyEntry>),
}

impl ToCss for BdPdfCustomProperty {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Entries(entries) => {
                let mut first = true;
                for entry in entries.iter() {
                    if !first {
                        dest.write_str(", ")?;
                    }
                    entry.to_css(dest)?;
                    first = false;
                }
                Ok(())
            },
        }
    }
}

impl BdPdfCustomProperty {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Whether the value is `none` (initial).
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfCustomProperty {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let mut entries: Vec<BdPdfCustomPropertyEntry> = Vec::new();
        loop {
            let name = CustomIdent::parse(input, &["none"])?;
            let value = input.expect_string()?.as_ref().to_owned().into();
            entries.push(BdPdfCustomPropertyEntry { name, value });
            if input.try_parse(|i| i.expect_comma()).is_err() {
                break;
            }
        }
        if entries.is_empty() {
            return Err(input.new_error_for_next_token());
        }
        Ok(Self::Entries(OwnedSlice::from(entries)))
    }
}
