/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-layer` Optional Content Group (OCG) opt-in (K3).
//!
//! Native fork-extension surface for PDF "layers" (ISO 32000-2 §8.11).
//! Authors assign an element subtree to a named OCG; the renderer
//! collects every distinct name on the document, registers one
//! `/OCG` per name, and brackets each bearing element's content
//! stream with `/OC /<name> BDC … EMC` so PDF viewers can toggle
//! the layer's visibility.
//!
//! `none` (initial) — the element belongs to no layer; its painted
//! content lives unconditionally in the page content stream.
//! `<custom-ident>` — the element and its descendants belong to the
//! named layer. Multiple elements bearing the same identifier
//! collapse to a single OCG dict in the document catalogue (the
//! identifier is the layer's stable identity).
//!
//! Per-element; not inherited (descendants pick up the bracketing
//! transitively through the painter's enter / exit pairing, not
//! through cascade inheritance — an element with no `-bd-pdf-layer`
//! declaration inside a `-bd-pdf-layer: map` ancestor still belongs
//! to `map` for visibility purposes because the OCG bracket wraps
//! its paint commands).

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::CustomIdent;
use cssparser::Parser;

/// Specified value of `-bd-pdf-layer`.
///
/// `none` (initial) — emit no OCG bracket. `<custom-ident>` — emit
/// `/OC /<name> BDC … EMC` around the bearing element's content.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfLayer {
    /// `none` (initial) — the element belongs to no Optional Content
    /// Group; its paint commands stream unconditionally.
    None,
    /// `<custom-ident>` — the element and its descendants belong to
    /// the named layer. Distinct identifiers register distinct OCGs;
    /// identical identifiers across multiple elements share a single
    /// OCG.
    Named(CustomIdent),
}

impl BdPdfLayer {
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

impl Parse for BdPdfLayer {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        Ok(Self::Named(CustomIdent::parse(input, &["none"])?))
    }
}
