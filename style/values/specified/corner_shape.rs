/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Backgrounds and Borders Module Level 4 §5.5 `corner-shape`
//! property values.
//!
//! `corner-shape` controls the geometric profile of each corner whose
//! size is set by `border-radius`. The default `round` reproduces the
//! existing CSS Backgrounds 3 quarter-ellipse curve; the other
//! keywords cut, scoop, notch, or square off the corner using the
//! same radius extents.
//!
//! The keyword family is:
//!
//! | Keyword           | Corner geometry                                   |
//! | ----------------- | ------------------------------------------------- |
//! | `round`           | Quarter-ellipse curve (default).                  |
//! | `bevel`           | Straight diagonal between the two radius extents. |
//! | `scoop`           | Quarter-ellipse curving *into* the box.           |
//! | `notch`           | Two right-angle segments meeting at the corner.   |
//! | `square`          | No corner shaping — square corner.                |
//! | `superellipse(k)` | CSS superellipse with curvature `k` (`k = 1` ⇒ `round`). |
//!
//! `superellipse(<number>)` carries the CSS superellipse curvature K.
//! CSS Borders 4 defines `K = 1` as `round`, `K = 0` as `bevel`,
//! `K = -1` as `scoop`, and the two signed infinities as `square` and
//! `notch`, respectively. Every finite value, including zero and negative
//! values, remains distinct.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::Number;
use crate::values::CSSFloat;
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// CSS Backgrounds 4 §5.5 `corner-shape` per-corner value.
///
/// Stored in computed form. The closed [`SuperellipseCurvature`] type keeps
/// NaN out of geometry while preserving every finite value and both signed
/// infinities.
///
/// The `Default` impl returns `Round`, matching the CSS Backgrounds 3
/// behaviour of the existing `border-radius` longhands.
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
pub enum CornerShape {
    /// `round` — quarter-ellipse (default; identical to CSS
    /// Backgrounds 3 corner geometry).
    Round,
    /// `bevel` — straight diagonal between the two radius extents.
    Bevel,
    /// `scoop` — quarter-ellipse curving inward (concave).
    Scoop,
    /// `notch` — two right-angle segments meeting at the radius
    /// crossing point, forming an inward V.
    Notch,
    /// `square` — no corner shaping; the corner is sharp even when
    /// `border-radius` is non-zero (the radius extents are still
    /// reserved by the paint surface so that adjacent corners and
    /// border ring geometry stay aligned).
    Square,
    /// `superellipse(<number>)` with its complete curvature domain.
    Superellipse(SuperellipseCurvature),
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
    fn from_css_number(value: CSSFloat) -> Self {
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
    /// The CSS Backgrounds 4 §5.5 initial value is `round`.
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
    /// bare keyword or `superellipse(2)`). The two spellings are
    /// observationally identical and the paint surface treats them
    /// interchangeably.
    #[inline]
    pub fn is_round(&self) -> bool {
        match self {
            Self::Round => true,
            Self::Superellipse(SuperellipseCurvature::Finite(k)) => {
                (k.value() - 1.0).abs() <= CSSFloat::EPSILON
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
) -> Result<SuperellipseCurvature, ParseError<'i>> {
    if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
        return match_ignore_ascii_case! { &ident,
            "infinity" => Ok(SuperellipseCurvature::PositiveInfinity),
            "-infinity" => Ok(SuperellipseCurvature::NegativeInfinity),
            "nan" => Ok(SuperellipseCurvature::from_css_number(CSSFloat::NAN)),
            _ => Err(input.new_custom_error::<_, StyleParseErrorKind>(
                StyleParseErrorKind::UnspecifiedError,
            )),
        };
    }

    let number = Number::parse(context, input)?;
    Ok(SuperellipseCurvature::from_css_number(number.raw_value()))
}

impl Parse for CornerShape {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        // `superellipse(<number>)` — functional notation with one
        // numeric argument carrying the Lamé exponent.
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

        // Bare keywords: round | bevel | scoop | notch | square.
        let ident = input.expect_ident()?;
        match_ignore_ascii_case! { ident,
            "round" => Ok(Self::Round),
            "bevel" => Ok(Self::Bevel),
            "scoop" => Ok(Self::Scoop),
            "notch" => Ok(Self::Notch),
            "square" => Ok(Self::Square),
            _ => Err(input.new_custom_error::<_, StyleParseErrorKind>(
                StyleParseErrorKind::UnspecifiedError,
            )),
        }
    }
}

/// The four per-corner `corner-shape` longhand values, packed for
/// shorthand serialisation.
///
/// Stored as physical corners in declaration order (TL, TR, BR, BL),
/// matching the CSS Backgrounds 3 §5.1 convention used by
/// `border-radius` so the corresponding longhand pairs always
/// project onto the same corner.
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// CSS Backgrounds 4 §5.5 — shorthand grammar mirrors
    /// `border-radius`'s four-value tile (1 ⇒ all four; 2 ⇒
    /// TL+BR / TR+BL; 3 ⇒ TL / TR+BL / BR; 4 ⇒ TL TR BR BL).
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let a = CornerShape::parse(context, input)?;
        let b = match input.try_parse(|i| CornerShape::parse(context, i)) {
            Ok(b) => b,
            Err(_) => {
                return Ok(Self {
                    top_left: a,
                    top_right: a,
                    bottom_right: a,
                    bottom_left: a,
                });
            },
        };
        let c = match input.try_parse(|i| CornerShape::parse(context, i)) {
            Ok(c) => c,
            Err(_) => {
                return Ok(Self {
                    top_left: a,
                    top_right: b,
                    bottom_right: a,
                    bottom_left: b,
                });
            },
        };
        let d = match input.try_parse(|i| CornerShape::parse(context, i)) {
            Ok(d) => d,
            Err(_) => {
                return Ok(Self {
                    top_left: a,
                    top_right: b,
                    bottom_right: c,
                    bottom_left: b,
                });
            },
        };
        Ok(Self {
            top_left: a,
            top_right: b,
            bottom_right: c,
            bottom_left: d,
        })
    }
}

impl ToCss for CornerShapeRect {
    /// Serialise using the same compaction rules as `border-radius`:
    /// emit the shortest equivalent tile (1, 2, 3, or 4 values).
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let tl = self.top_left;
        let tr = self.top_right;
        let br = self.bottom_right;
        let bl = self.bottom_left;
        tl.to_css(dest)?;
        let four_distinct = tr != bl || tl != br || tr != tl;
        let three_distinct = tr != bl || tl != br;
        let two_distinct = tl != tr;
        if !two_distinct && !three_distinct && !four_distinct {
            return Ok(());
        }
        dest.write_char(' ')?;
        tr.to_css(dest)?;
        if !three_distinct && !four_distinct {
            return Ok(());
        }
        dest.write_char(' ')?;
        br.to_css(dest)?;
        if !four_distinct {
            return Ok(());
        }
        dest.write_char(' ')?;
        bl.to_css(dest)
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
        let CornerShape::Superellipse(SuperellipseCurvature::Finite(curvature)) =
            parse_corner_shape("superellipse(-100)")
        else {
            panic!("finite negative curvature must retain its closed state");
        };
        assert_eq!(curvature.value(), -100.0);
    }

    #[test]
    fn superellipse_retains_both_signed_infinities() {
        assert_eq!(
            parse_corner_shape("superellipse(infinity)"),
            CornerShape::Superellipse(SuperellipseCurvature::PositiveInfinity)
        );
        assert_eq!(
            parse_corner_shape("superellipse(-infinity)"),
            CornerShape::Superellipse(SuperellipseCurvature::NegativeInfinity)
        );
        assert_eq!(
            parse_corner_shape("superellipse(calc(-infinity))"),
            CornerShape::Superellipse(SuperellipseCurvature::NegativeInfinity)
        );
    }

    #[test]
    fn superellipse_censors_nan_to_finite_zero() {
        let shape = parse_corner_shape("superellipse(calc(NaN))");
        assert_eq!(shape.to_css_string(), "superellipse(0)");
    }

    #[test]
    fn css_curvature_one_is_round() {
        assert!(parse_corner_shape("superellipse(1)").is_round());
        assert!(!parse_corner_shape("superellipse(2)").is_round());
    }
}
