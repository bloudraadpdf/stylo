/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-index` — BFO Publisher's `-bfo-index` (Family 23).
//!
//! Native moegoe fork-extension surface for BFO's declarative
//! book-index generation
//! (`docs/reference-manuals/bfo.md:10514`). The other BFO
//! properties listed in audit Family 23 are aliases of pre-existing
//! moegoe surfaces (handled by the compat translator) or
//! dropped-with-warning entries; only `-bd-index` requires a new
//! longhand.
//!
//! `@bfo env { ... }` at-rule rewriting is not implemented as a
//! Stylo descriptor — per the audit, the compat walker rewrites
//! `@bfo env { name: value; }` to `:root { --bfo-name: value; }`
//! and translates known names (e.g. `bfo-pdf-profile`) to
//! `-bd-pdf-conformance` before the stylesheet reaches Stylo.

use crate::derives::*;
use crate::OwnedStr;

/// Specified value of `-bd-index`.
///
/// `none` (initial) — the element does not contribute to any
/// book index. `<index-name>` — the element is an index entry in
/// the named index. `<index-name> as <key>` — explicit sort key
/// override.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdIndex {
    /// `none`.
    None,
    /// `<index-name> [as <key>]?`.
    Entry {
        /// Named index identifier.
        name: OwnedStr,
        /// Optional explicit sort key.
        key: Option<OwnedStr>,
    },
}

impl Default for BdIndex {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdIndex {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl crate::parser::Parse for BdIndex {
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
        let name = input.expect_ident()?.as_ref().to_owned();
        let key = input
            .try_parse(|i| -> Result<OwnedStr, style_traits::ParseError<'i>> {
                i.expect_ident_matching("as")?;
                let k = i.expect_string()?;
                Ok(k.as_ref().to_owned().into())
            })
            .ok();
        Ok(Self::Entry {
            name: name.into(),
            key,
        })
    }
}
