/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-*` document metadata properties.
//!
//! Native moegoe fork-extension surface for PDF document metadata.
//! Each property writes a slot of the resulting PDF info dictionary /
//! XMP metadata packet (`-bd-pdf-title`, `-bd-pdf-author`,
//! `-bd-pdf-subject`, `-bd-pdf-keywords`, `-bd-pdf-xmp`). The properties
//! apply to all elements, are not inherited, and accept
//! `none | <string>+`. Concatenation across elements that set the
//! same property is the renderer's responsibility (first-non-empty-wins
//! for `title` / `subject` / `xmp`, accumulate for `author` /
//! `keywords`), mirroring PDFreactor's `-ro-{title,author,subject,
//! keywords}` semantics (see `docs/reference-manuals/pdfreactor.md`
//! §Metadata). XMP is moegoe-specific — neither PDFreactor nor Prince
//! exposes a raw-XMP escape hatch as a CSS property.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::{OwnedSlice, OwnedStr};
use cssparser::Parser;
use style_traits::{ParseError, StyleParseErrorKind};

/// Specified value of a `-bd-pdf-*` document-metadata property.
///
/// `none` clears the slot. `<string>+` contributes one or more
/// quoted strings; the renderer is responsible for joining and
/// merging declarations across elements.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfMetaValue {
    /// `none` — no contribution to the PDF metadata slot.
    None,
    /// `<string>+` — one or more author strings.
    Strings(#[css(iterable)] OwnedSlice<OwnedStr>),
}

impl BdPdfMetaValue {
    /// `none` value (initial).
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

impl Parse for BdPdfMetaValue {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let mut strings: Vec<OwnedStr> = Vec::new();
        loop {
            match input.try_parse(|i| -> Result<OwnedStr, ParseError<'i>> {
                let s = i.expect_string()?;
                Ok(s.as_ref().to_owned().into())
            }) {
                Ok(s) => strings.push(s),
                Err(_) => break,
            }
        }
        if strings.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Strings(OwnedSlice::from(strings)))
    }
}
