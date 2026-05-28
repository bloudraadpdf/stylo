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

/// Specified value of `-bd-pdf-layer-visible` (K18).
///
/// Per ISO 32000-2 §8.11.3.3, an Optional Content Group's
/// configuration dictionary `/OCGs` entry pairs a default-visibility
/// list (`/ON` / `/OFF`) with each registered OCG. This longhand
/// flips the initial visibility of the layer assigned via
/// `-bd-pdf-layer: <ident>`; PDF viewers honour it as the layer's
/// starting state, and authors can toggle it through the OCG panel
/// at runtime.
///
/// `auto` (initial) — the renderer chooses the default. Today the
/// moegoe-side wire-through hard-codes "visible" for backwards
/// compatibility with the K3 baseline; `auto` preserves that
/// behaviour and reserves room for a future per-conformance default.
/// `on` — the layer is visible on document open. `off` — the layer
/// is hidden on document open (the author opted the subtree out of
/// the default view but the content is still browseable through the
/// OCG panel).
///
/// Per-element; not inherited. Pairs with `-bd-pdf-layer: <ident>`;
/// declaring `-bd-pdf-layer-visible` without a `-bd-pdf-layer`
/// declaration has no observable effect because no OCG is registered
/// for the subtree. Mirrors the inheritance rules of
/// `-bd-pdf-layer-intent`.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
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
pub enum BdPdfLayerVisible {
    /// `auto` (initial) — the renderer chooses the default
    /// visibility. Today this resolves to "visible" so that an
    /// element opting into a layer without further configuration
    /// keeps the K3 baseline behaviour.
    #[default]
    Auto,
    /// `on` — the layer is visible on document open.
    On,
    /// `off` — the layer is hidden on document open. The content
    /// remains discoverable through the PDF viewer's OCG panel.
    Off,
}

impl BdPdfLayerVisible {
    /// Whether the value is at its initial `auto`.
    #[inline]
    pub fn is_auto(self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// Specified value of `-bd-pdf-layer-intent` (G66).
///
/// Per ISO 32000-2 §8.11.2.1, an Optional Content Group's `/Intent`
/// entry tells a conforming reader whether the layer represents a
/// user-visible variant of the document (`view` — the default and
/// historical CSS-author expectation) or design-time metadata that
/// readers ignore during ordinary rendering (`design` — typically
/// proofing, commentary, or markup layers that should not print).
///
/// Used in concert with `-bd-pdf-layer: <ident>`; an element bearing
/// `-bd-pdf-layer-intent` without a `-bd-pdf-layer` declaration has
/// no observable effect because no OCG is registered for the
/// subtree.
///
/// Per-element; not inherited. Mirrors the inheritance rules of
/// `-bd-pdf-layer` — the OCG bracket is the unit of authority,
/// descendants inherit visibility through bracket nesting at paint
/// time, not through cascade inheritance.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BdPdfLayerIntent {
    /// `view` (initial) — the layer is honoured during ordinary
    /// rendering. PDF viewers show / hide it through the OCG panel
    /// like any normal layer.
    #[default]
    View,
    /// `design` — the layer carries design-time metadata.
    /// Conforming readers omit it from ordinary rendering (and from
    /// print output by default).
    Design,
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::parser::ParserContext;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::{ParsingMode, ToCss};

    fn parse_layer_visible(css: &str) -> BdPdfLayerVisible {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let _context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfLayerVisible::parse(input))
            .expect("layer visibility should parse")
    }

    #[test]
    fn layer_visible_initial_is_auto() {
        assert_eq!(BdPdfLayerVisible::default(), BdPdfLayerVisible::Auto);
        assert!(BdPdfLayerVisible::default().is_auto());
    }

    #[test]
    fn layer_visible_all_variants_round_trip() {
        for css in ["auto", "on", "off"] {
            let value = parse_layer_visible(css);
            assert_eq!(value.to_css_string(), css);
        }
    }

    #[test]
    fn layer_visible_off_parses() {
        assert_eq!(parse_layer_visible("off"), BdPdfLayerVisible::Off);
    }
}
