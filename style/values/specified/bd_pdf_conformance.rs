/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-conformance` and `-bd-pdf-version` properties (G4).
//!
//! Native moegoe fork-extension surface for PDF/A and PDF/UA
//! conformance validation. `-bd-pdf-conformance` selects the krilla
//! `Validator`; `-bd-pdf-version` overrides the PDF baseline version
//! when the chosen conformance permits more than one. Both apply to
//! all elements but the moegoe renderer only honours `:root`
//! declarations — they are document-level capabilities.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use cssparser::{match_ignore_ascii_case, Parser, Token};

/// Specified value of the `-bd-pdf-conformance` property.
///
/// One variant per krilla `Validator` enum entry plus `none`. Spec
/// vocabulary (`a1a` … `ua1`) matches the short-form identifiers
/// authors copy from PDF/A and PDF/UA compliance documents.
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
pub enum BdPdfConformanceValue {
    #[default]
    None,
    A1a,
    A1b,
    A2a,
    A2b,
    A2u,
    A3a,
    A3b,
    A3u,
    A4,
    A4e,
    A4f,
    Ua1,
    // F1 — PDF/X pre-press conformance levels. PDF/X-1a (greyscale +
    // CMYK + spot only, ISO 15930-1/4), PDF/X-3 (adds RGB/Lab,
    // ISO 15930-3/6), PDF/X-4 (adds transparency + layers,
    // ISO 15930-7/8). Year suffix matches the inventory text and
    // round-trips through the keyword serialisation. PDFreactor
    // `pdfreactor.md:3174` and Prince `prince.md:9168` use these
    // verbatim.
    #[css(keyword = "pdf-x-1a-2001")]
    PdfX1A2001,
    #[css(keyword = "pdf-x-1a-2003")]
    PdfX1A2003,
    #[css(keyword = "pdf-x-3-2002")]
    PdfX32002,
    #[css(keyword = "pdf-x-3-2003")]
    PdfX32003,
    #[css(keyword = "pdf-x-4")]
    PdfX4,
    #[css(keyword = "pdf-x-4p")]
    PdfX4P,
}

impl BdPdfConformanceValue {
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

/// Specified value of the `-bd-pdf-version` property.
///
/// `auto` defers to the krilla validator's recommended version (or
/// the moegoe baseline when no conformance is set). The numeric
/// variants name explicit PDF baseline versions.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfVersionValue {
    #[default]
    Auto,
    #[css(keyword = "1.4")]
    V14,
    #[css(keyword = "1.5")]
    V15,
    #[css(keyword = "1.6")]
    V16,
    #[css(keyword = "1.7")]
    V17,
    #[css(keyword = "2.0")]
    V20,
}

impl BdPdfVersionValue {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl Parse for BdPdfVersionValue {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        let location = input.current_source_location();
        // `auto` is parsed as an identifier; PDF versions are
        // parsed as numbers (`1.4`, `1.5`, `1.6`, `1.7`, `2.0`).
        if let Ok(version) = input.try_parse(|i| {
            let token = i.next()?.clone();
            match token {
                Token::Number { value, .. } => Ok(value),
                _ => Err(i.new_custom_error::<_, style_traits::StyleParseErrorKind>(
                    style_traits::StyleParseErrorKind::UnspecifiedError,
                )),
            }
        }) {
            return match version_key(version) {
                Some(v) => Ok(v),
                None => Err(location
                    .new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)),
            };
        }
        let ident = input.expect_ident()?;
        match_ignore_ascii_case! { ident,
            "auto" => Ok(Self::Auto),
            _ => Err(location.new_custom_error(
                style_traits::StyleParseErrorKind::UnspecifiedError
            )),
        }
    }
}

fn version_key(value: f32) -> Option<BdPdfVersionValue> {
    // Versions are stored verbatim in CSS source as decimal
    // numbers; comparison against canonical values uses a small
    // epsilon to tolerate the lossless f32 round-trip.
    const EPS: f32 = 1e-4;
    let candidates = [
        (1.4_f32, BdPdfVersionValue::V14),
        (1.5_f32, BdPdfVersionValue::V15),
        (1.6_f32, BdPdfVersionValue::V16),
        (1.7_f32, BdPdfVersionValue::V17),
        (2.0_f32, BdPdfVersionValue::V20),
    ];
    candidates
        .into_iter()
        .find(|(target, _)| (value - target).abs() < EPS)
        .map(|(_, variant)| variant)
}
