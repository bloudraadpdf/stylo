/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-role-map` document-level RoleMap descriptor.
//!
//! Pre-seeds the PDF structure-tree `/RoleMap` dictionary (ISO 32000-2
//! §14.7.3) with a list of `(custom-role-name -> standard-role)`
//! rewrites. The renderer's tag-tree builder already records role-map
//! entries on demand whenever a `-bd-pdf-tag: <custom-ident>` cascade
//! resolves to a custom role; this longhand lets authors register
//! the mappings up-front, independent of where (and whether) the
//! custom names appear in the body.
//!
//! Grammar (Prince `prince-pdf-role-map`, `prince.md`):
//!
//! ```text
//! none | <custom-ident> : <std-role> [ , <custom-ident> : <std-role> ]*
//! ```
//!
//! The `<std-role>` token is one of the [`BdPdfStandardRole`] keywords
//! (the same vocabulary `-bd-pdf-tag-namespace` uses for its standard
//! roles). Document-level; the cascade reader only honours
//! declarations on `:root`.

use super::bd_pdf_tag::BdPdfStandardRole;
use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::CustomIdent;
use crate::OwnedSlice;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// One `(custom-ident, standard-role)` entry of `-bd-pdf-role-map`.
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
pub struct BdPdfRoleMapEntry {
    /// Author-supplied custom role name.
    pub custom: CustomIdent,
    /// PDF/UA standard role the custom name aliases.
    pub standard: BdPdfStandardRole,
}

impl ToCss for BdPdfRoleMapEntry {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        self.custom.to_css(dest)?;
        dest.write_str(": ")?;
        self.standard.to_css(dest)
    }
}

/// Specified value of `-bd-pdf-role-map`.
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
pub enum BdPdfRoleMap {
    /// `none` — no pre-seeded RoleMap entries.
    None,
    /// Comma-separated list of `(custom-ident, standard-role)` pairs.
    Entries(OwnedSlice<BdPdfRoleMapEntry>),
}

impl ToCss for BdPdfRoleMap {
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
            }
        }
    }
}

impl BdPdfRoleMap {
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

impl Parse for BdPdfRoleMap {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let mut entries: Vec<BdPdfRoleMapEntry> = Vec::new();
        loop {
            let custom = CustomIdent::parse(input, &["none"])?;
            input.expect_colon()?;
            let standard = BdPdfStandardRole::parse(input)?;
            entries.push(BdPdfRoleMapEntry { custom, standard });
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
