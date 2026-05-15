/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-page-rotation` and `-bd-rotate-body` properties (F19).
//!
//! Both are `@page`-only descriptors that flag per-page rotation. The
//! two have distinct semantics:
//!
//! * `-bd-pdf-page-rotation` (PDFreactor `-ro-pdf-page-rotation`,
//!   `pdfreactor.md:17250`) sets the PDF `/Rotate` entry on the page
//!   dictionary. Viewers rotate the rendered page by this multiple of
//!   90 degrees when displaying / printing. The content stream is
//!   unaffected — it remains in the same coordinate system as the
//!   page box. Values are restricted to `0 | 90 | 180 | 270`.
//!
//! * `-bd-rotate-body` (Prince `-prince-rotate-body`,
//!   `prince.md:9520`) rotates the body content of the page rather
//!   than the page itself. The page box is unchanged and the content
//!   is laid out into a rotated coordinate frame. Values are
//!   `none | <angle>`.
//!
//! Neither property is inherited. They only apply inside `@page`
//! contexts; the moegoe renderer reads them from the resolved
//! page-rule cascade.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::Angle;
use cssparser::{Parser, Token};
use style_traits::{ParseError, StyleParseErrorKind};

/// Specified value of `-bd-pdf-page-rotation`.
///
/// One variant per legal PDF `/Rotate` value (ISO 32000-2 §14.4.4).
/// PDFreactor and Prince both restrict this to multiples of 90; the
/// strict enum mirrors the spec and avoids smuggling free-form
/// numeric input through to the renderer.
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
pub enum BdPdfPageRotation {
    #[default]
    #[css(keyword = "0")]
    Zero,
    #[css(keyword = "90")]
    Ninety,
    #[css(keyword = "180")]
    OneEighty,
    #[css(keyword = "270")]
    TwoSeventy,
}

impl BdPdfPageRotation {
    /// Initial value (`0`).
    #[inline]
    pub fn zero() -> Self {
        Self::Zero
    }

    /// Degrees of rotation as a `u16`.
    #[inline]
    pub fn degrees(&self) -> u16 {
        match self {
            Self::Zero => 0,
            Self::Ninety => 90,
            Self::OneEighty => 180,
            Self::TwoSeventy => 270,
        }
    }
}

impl Parse for BdPdfPageRotation {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let token = input.next()?.clone();
        let value = match token {
            Token::Number { value, .. } => value,
            _ => {
                return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            },
        };
        // Round-tripping through f32 — accept the canonical four
        // values with a small tolerance.
        const EPS: f32 = 1e-4;
        let candidates = [
            (0.0_f32, Self::Zero),
            (90.0_f32, Self::Ninety),
            (180.0_f32, Self::OneEighty),
            (270.0_f32, Self::TwoSeventy),
        ];
        candidates
            .into_iter()
            .find(|(target, _)| (value - target).abs() < EPS)
            .map(|(_, variant)| variant)
            .ok_or_else(|| location.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }
}

/// Specified value of `-bd-rotate-body`.
///
/// `none` (initial) leaves the body content unrotated. An `<angle>`
/// rotates the content frame; Prince accepts arbitrary angles though
/// in practice only multiples of 90 are useful.
///
/// `ToComputedValue` is implemented manually in
/// [`crate::values::computed::bd_page_rotation`] so the inner `Angle`
/// resolves into its computed counterpart.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdRotateBody {
    /// `none` — body is not rotated.
    None,
    /// `<angle>` — rotate the body content by the given angle.
    Angle(Angle),
}

impl BdRotateBody {
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

impl Parse for BdRotateBody {
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
        Ok(Self::Angle(Angle::parse(context, input)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::{ParsingMode, ToCss};

    fn make_url() -> UrlExtraData {
        UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap())
    }

    fn parse_rotation(css: &str) -> BdPdfPageRotation {
        let url_data = make_url();
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Page),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfPageRotation::parse(&context, input))
            .expect("page rotation should parse")
    }

    #[test]
    fn pdf_page_rotation_round_trips() {
        for css in ["0", "90", "180", "270"] {
            let value = parse_rotation(css);
            assert_eq!(value.to_css_string(), css);
        }
    }

    #[test]
    fn pdf_page_rotation_rejects_arbitrary_angles() {
        let url_data = make_url();
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Page),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        for css in ["45", "360", "1.5"] {
            let mut input = ParserInput::new(css);
            let mut parser = Parser::new(&mut input);
            assert!(
                parser
                    .parse_entirely(|input| BdPdfPageRotation::parse(&context, input))
                    .is_err(),
                "expected `{css}` to fail to parse",
            );
        }
    }
}
