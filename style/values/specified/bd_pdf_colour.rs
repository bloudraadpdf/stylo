/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe per-page colour-space + PDF overprint properties (F2).
//!
//! This module carries the F2 keyword surface — the per-page
//! `colorspace` selector and the two `overprint` toggles. The
//! richer F2 work (extending the `Color` enum with spot / separation
//! variants, the `@-bd-colour` at-rule, and the `-bd-spot()` /
//! `-bd-separation()` colour functions) is deferred per the audit
//! plan: it requires touching every colour-consuming property in
//! the cascade, the IR `Color` enum, and the bladsy colour-space
//! API. The F2 keyword surface here parses and computes
//! independently and so can land ahead of the wider work.
//!
//! | Property | Source |
//! |----------|--------|
//! | `-bd-pdf-page-colourspace` | Prince `-prince-pdf-page-colorspace` 8948 |
//! | `-bd-pdf-overprint` | PDFreactor `-ro-pdf-overprint` 17222 |
//! | `-bd-pdf-overprint-content` | PDFreactor `-ro-pdf-overprint-content` 17222 |

use crate::derives::*;

/// `-bd-pdf-page-colourspace` (`@page`-only).
///
/// Selects the working colour space for the page content stream.
/// `auto` (initial) lets the renderer pick based on the output
/// intent / conformance; the named keywords force a specific
/// device colour space.
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
pub enum BdPdfPageColourSpace {
    #[default]
    Auto,
    Rgb,
    Cmyk,
    Grey,
}

/// `-bd-pdf-overprint`.
///
/// PDFreactor `-ro-pdf-overprint`. Controls the PDF overprint
/// `/OP` and `/op` operator emission per ISO 32000-2 §8.6.7.
/// `auto` (initial) lets the renderer decide based on the
/// conformance; `preserve` emits explicit overprint operators
/// for stroke and fill; `none` disables overprint entirely.
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
pub enum BdPdfOverprint {
    #[default]
    Auto,
    Preserve,
    None,
}

/// `-bd-pdf-overprint-content`.
///
/// Separate overprint policy for text content (PDFreactor
/// `-ro-pdf-overprint-content`). Distinct enum (rather than a
/// type alias of `BdPdfOverprint`) so the property cascade can
/// route them independently.
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
pub enum BdPdfOverprintContent {
    #[default]
    Auto,
    Preserve,
    None,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::{Parser, ParserInput};
    use style_traits::ToCss;

    fn parse_colourspace(css: &str) -> BdPdfPageColourSpace {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfPageColourSpace::parse(input))
            .expect("colourspace should parse")
    }

    #[test]
    fn page_colourspace_round_trips() {
        for css in ["auto", "rgb", "cmyk", "grey"] {
            let value = parse_colourspace(css);
            assert_eq!(value.to_css_string(), css);
        }
    }
}
