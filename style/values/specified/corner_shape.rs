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
//! | `superellipse(k)` | Lamé curve with exponent `k` (k = 2 ⇒ `round`).   |
//!
//! `superellipse(<number>)` carries the exponent k of the Lamé curve
//! `|x/a|^k + |y/b|^k = 1`. CSS Backgrounds 4 §5.5 specifies
//! `k = 2` ⇒ `round` (a quarter-ellipse), `k = 1` ⇒ `bevel`
//! (a straight diagonal), `k → ∞` ⇒ `square` (no rounding), and
//! `0 < k < 1` produces a concave `scoop`-family curve. Negative,
//! zero, and non-finite exponents are clamped at parse time so the
//! computed value always carries a finite positive number.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::Number;
use crate::values::CSSFloat;
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// CSS Backgrounds 4 §5.5 `corner-shape` per-corner value.
///
/// Stored in computed form (the exponent inside `Superellipse` is
/// already a finite, positive `CSSFloat`). The bare keywords lower
/// to canonical Lamé exponents at paint time:
/// `round` ⇒ k = 2, `bevel` ⇒ k = 1, `scoop` ⇒ k = 0.5,
/// `square` ⇒ k = +∞, `notch` ⇒ the special two-right-angle profile
/// that has no superellipse equivalent and is rendered as a literal
/// V-shape inside the radius extents.
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
    /// `superellipse(<number>)` — Lamé curve with finite positive
    /// exponent k. `k = 2` is the canonical `round`; `k = 1` is
    /// `bevel`; large k approaches `square`; `0 < k < 1` curves
    /// concavely toward `scoop`.
    Superellipse(CSSFloat),
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
            Self::Superellipse(k) => (*k - 2.0).abs() <= CSSFloat::EPSILON,
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

/// Parse-time clamp for `superellipse(<number>)` arguments.
///
/// CSS Backgrounds 4 §5.5 specifies the curve `|x/a|^k + |y/b|^k = 1`,
/// which is only well defined for finite, strictly positive `k`.
/// Authors writing `superellipse(0)`, `superellipse(-2)`, or
/// `superellipse(NaN)` get a fail-safe clamp to `MIN_SUPERELLIPSE_K`
/// rather than a parse error; the rendered geometry approaches `scoop`
/// but stays well-formed so downstream paint never has to handle the
/// singular case.
const MIN_SUPERELLIPSE_K: CSSFloat = 0.000_1;

fn clamp_superellipse(k: CSSFloat) -> CSSFloat {
    if !k.is_finite() || k <= 0.0 {
        MIN_SUPERELLIPSE_K
    } else {
        k
    }
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
                let n = Number::parse(context, i)?;
                Ok(Self::Superellipse(clamp_superellipse(n.get())))
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
