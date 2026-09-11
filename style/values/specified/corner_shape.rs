/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Borders and Box Decorations Level 4 corner shapes.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::{Context, ToComputedValue};
use crate::values::generics::rect::Rect;
use crate::values::specified::Number;
use crate::values::CSSFloat;
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// A specified corner shape, retaining numeric expressions until computation.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum CornerShape {
    /// `round`, equivalent to `superellipse(1)`.
    Round,
    /// `bevel` — straight diagonal between the two radius extents.
    Bevel,
    /// `scoop` — quarter-ellipse curving inward (concave).
    Scoop,
    /// `notch`, equivalent to `superellipse(-infinity)`.
    Notch,
    /// `square`, equivalent to `superellipse(infinity)`.
    Square,
    /// `squircle`, equivalent to `superellipse(2)`.
    Squircle,
    /// `superellipse(<number>)` with its complete curvature domain.
    Superellipse(SpecifiedSuperellipseCurvature),
}

impl ToComputedValue for CornerShape {
    type ComputedValue = crate::values::computed::CornerShape;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        let curvature = match self {
            Self::Round => SuperellipseCurvature::from_css_number(1.0),
            Self::Bevel => SuperellipseCurvature::from_css_number(0.0),
            Self::Scoop => SuperellipseCurvature::from_css_number(-1.0),
            Self::Squircle => SuperellipseCurvature::from_css_number(2.0),
            Self::Notch => SuperellipseCurvature::NegativeInfinity,
            Self::Square => SuperellipseCurvature::PositiveInfinity,
            Self::Superellipse(curvature) => curvature.to_computed_value(context),
        };
        Self::ComputedValue::from_curvature(curvature)
    }

    fn from_computed_value(value: &Self::ComputedValue) -> Self {
        Self::Superellipse(SpecifiedSuperellipseCurvature::from_computed_value(
            &value.curvature(),
        ))
    }
}

/// The specified argument of `superellipse()`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum SpecifiedSuperellipseCurvature {
    /// A number or numeric expression.
    Number(Number),
    /// The explicit `infinity` keyword.
    PositiveInfinity,
    /// The explicit `-infinity` keyword.
    NegativeInfinity,
}

impl ToComputedValue for SpecifiedSuperellipseCurvature {
    type ComputedValue = SuperellipseCurvature;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        match self {
            Self::Number(number) => SuperellipseCurvature::from_css_number(
                number
                    .resolve_unclamped()
                    .unwrap_or_else(|| number.to_computed_value(context)),
            ),
            Self::PositiveInfinity => SuperellipseCurvature::PositiveInfinity,
            Self::NegativeInfinity => SuperellipseCurvature::NegativeInfinity,
        }
    }

    fn from_computed_value(value: &Self::ComputedValue) -> Self {
        match value {
            SuperellipseCurvature::Finite(number) => Self::Number(Number::new(number.value())),
            SuperellipseCurvature::PositiveInfinity => Self::PositiveInfinity,
            SuperellipseCurvature::NegativeInfinity => Self::NegativeInfinity,
        }
    }
}

impl ToCss for SpecifiedSuperellipseCurvature {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Number(number) => number.to_css(dest),
            Self::PositiveInfinity => dest.write_str("infinity"),
            Self::NegativeInfinity => dest.write_str("-infinity"),
        }
    }
}

/// A valid CSS `superellipse()` curvature.
#[derive(
    Clone,
    Copy,
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
pub enum SuperellipseCurvature {
    /// A finite curvature, including zero and negative values.
    Finite(FiniteSuperellipseCurvature),
    /// Positive infinity, equivalent to `square`.
    PositiveInfinity,
    /// Negative infinity, equivalent to `notch`.
    NegativeInfinity,
}

/// A finite CSS `superellipse()` curvature.
#[derive(
    Clone,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(transparent)]
pub struct FiniteSuperellipseCurvature(CSSFloat);

impl SuperellipseCurvature {
    pub(crate) fn from_css_number(value: CSSFloat) -> Self {
        if value.is_nan() {
            // CSS Values 4 censors a top-level NaN numeric value to zero.
            return Self::Finite(FiniteSuperellipseCurvature(0.0));
        }
        if value == CSSFloat::INFINITY {
            return Self::PositiveInfinity;
        }
        if value == CSSFloat::NEG_INFINITY {
            return Self::NegativeInfinity;
        }
        debug_assert!(value.is_finite());
        Self::Finite(FiniteSuperellipseCurvature(value))
    }
}

impl FiniteSuperellipseCurvature {
    /// Returns the finite CSS curvature parameter.
    #[inline]
    pub fn value(self) -> CSSFloat {
        self.0
    }
}

impl Default for CornerShape {
    fn default() -> Self {
        Self::Round
    }
}

impl CornerShape {
    /// The CSS initial value (`round`).
    #[inline]
    pub fn round() -> Self {
        Self::Round
    }

    /// Whether this shape is the default `round` profile (either the
    /// bare keyword or `superellipse(1)`). The two spellings are
    /// observationally identical and the paint surface treats them
    /// interchangeably.
    #[inline]
    pub fn is_round(&self) -> bool {
        match self {
            Self::Round => true,
            Self::Superellipse(SpecifiedSuperellipseCurvature::Number(number)) => {
                number.resolve_unclamped().is_some_and(|value| value == 1.0)
            },
            _ => false,
        }
    }
}

impl ToCss for CornerShape {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Round => dest.write_str("round"),
            Self::Bevel => dest.write_str("bevel"),
            Self::Scoop => dest.write_str("scoop"),
            Self::Notch => dest.write_str("notch"),
            Self::Square => dest.write_str("square"),
            Self::Squircle => dest.write_str("squircle"),
            Self::Superellipse(k) => {
                dest.write_str("superellipse(")?;
                k.to_css(dest)?;
                dest.write_char(')')
            },
        }
    }
}

impl ToCss for SuperellipseCurvature {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Finite(value) => value.value().to_css(dest),
            Self::PositiveInfinity => dest.write_str("infinity"),
            Self::NegativeInfinity => dest.write_str("-infinity"),
        }
    }
}

fn parse_superellipse_curvature<'i, 't>(
    context: &ParserContext,
    input: &mut Parser<'i, 't>,
) -> Result<SpecifiedSuperellipseCurvature, ParseError<'i>> {
    if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
        return match_ignore_ascii_case! { &ident,
            "infinity" => Ok(SpecifiedSuperellipseCurvature::PositiveInfinity),
            "-infinity" => Ok(SpecifiedSuperellipseCurvature::NegativeInfinity),
            _ => Err(input.new_custom_error::<_, StyleParseErrorKind>(
                StyleParseErrorKind::UnspecifiedError,
            )),
        };
    }

    Number::parse(context, input).map(SpecifiedSuperellipseCurvature::Number)
}

impl Parse for CornerShape {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(value) = input.try_parse(|i| {
            let location = i.current_source_location();
            let function = i.expect_function()?.clone();
            if !function.eq_ignore_ascii_case("superellipse") {
                return Err(location.new_custom_error::<_, StyleParseErrorKind>(
                    StyleParseErrorKind::UnexpectedFunction(function.clone()),
                ));
            }
            i.parse_nested_block(|i| {
                parse_superellipse_curvature(context, i).map(Self::Superellipse)
            })
        }) {
            return Ok(value);
        }

        let ident = input.expect_ident()?;
        match_ignore_ascii_case! { ident,
            "round" => Ok(Self::Round),
            "bevel" => Ok(Self::Bevel),
            "scoop" => Ok(Self::Scoop),
            "notch" => Ok(Self::Notch),
            "square" => Ok(Self::Square),
            "squircle" => Ok(Self::Squircle),
            _ => Err(input.new_custom_error::<_, StyleParseErrorKind>(
                StyleParseErrorKind::UnspecifiedError,
            )),
        }
    }
}

/// The four physical corner shapes in declaration order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CornerShapeRect {
    /// `corner-top-left-shape`.
    pub top_left: CornerShape,
    /// `corner-top-right-shape`.
    pub top_right: CornerShape,
    /// `corner-bottom-right-shape`.
    pub bottom_right: CornerShape,
    /// `corner-bottom-left-shape`.
    pub bottom_left: CornerShape,
}

impl CornerShapeRect {
    /// Initial value — all corners `round`.
    pub fn round() -> Self {
        Self {
            top_left: CornerShape::Round,
            top_right: CornerShape::Round,
            bottom_right: CornerShape::Round,
            bottom_left: CornerShape::Round,
        }
    }
}

impl Parse for CornerShapeRect {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let Rect(top_left, top_right, bottom_right, bottom_left) =
            Rect::parse_with(context, input, CornerShape::parse)?;
        Ok(Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        })
    }
}

impl ToCss for CornerShapeRect {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        Rect(
            &self.top_left,
            &self.top_right,
            &self.bottom_right,
            &self.bottom_left,
        )
        .to_css(dest)
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::ParserInput;
    use style_traits::{ParsingMode, ToCss};

    fn parse_corner_shape(css: &str) -> CornerShape {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = ParserContext::new(
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
            .parse_entirely(|input| CornerShape::parse(&context, input))
            .expect("corner shape should parse")
    }

    #[test]
    fn superellipse_retains_finite_negative_curvature() {
        assert_eq!(
            parse_corner_shape("superellipse(-100)"),
            CornerShape::Superellipse(SpecifiedSuperellipseCurvature::Number(Number::new(-100.0)))
        );
    }

    #[test]
    fn superellipse_retains_both_signed_infinities() {
        assert_eq!(
            parse_corner_shape("superellipse(infinity)"),
            CornerShape::Superellipse(SpecifiedSuperellipseCurvature::PositiveInfinity)
        );
        assert_eq!(
            parse_corner_shape("superellipse(-infinity)"),
            CornerShape::Superellipse(SpecifiedSuperellipseCurvature::NegativeInfinity)
        );
        assert_eq!(
            parse_corner_shape("superellipse(calc(-infinity))").to_css_string(),
            "superellipse(calc(-infinity))"
        );
    }

    #[test]
    fn superellipse_censors_nan_to_finite_zero() {
        let shape = parse_corner_shape("superellipse(calc(NaN))");
        assert_eq!(shape.to_css_string(), "superellipse(calc(NaN))");
        assert_eq!(
            SuperellipseCurvature::from_css_number(CSSFloat::NAN).to_css_string(),
            "0"
        );
    }

    #[test]
    fn css_curvature_one_is_round() {
        assert!(parse_corner_shape("superellipse(1)").is_round());
        assert!(!parse_corner_shape("superellipse(2)").is_round());
    }
}
