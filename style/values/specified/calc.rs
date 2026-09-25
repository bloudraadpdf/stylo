/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! [Calc expressions][calc].
//!
//! [calc]: https://drafts.csswg.org/css-values/#calc-notation

use crate::color::parsing::ChannelKeyword;
use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::stylesheets::CssRuleType;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::generics::calc::{
    self as generic, CalcNodeLeaf, CalcUnits, MinMaxOp, ModRemOp, PositivePercentageBasis,
    ProgressClamping, RoundingStrategy, SortKey, TrigonometricFunction,
};
use crate::values::generics::length::GenericAnchorSizeFunction;
use crate::values::generics::position::{
    AnchorSideKeyword, GenericAnchorFunction, GenericAnchorSide, TreeScoped,
};
use crate::values::specified::angle::AngleDimension;
use crate::values::specified::length::{AbsoluteLength, FontRelativeLength, NoCalcLength};
use crate::values::specified::length::{
    ContainerRelativeLength, PageRelativeLength, ViewportPercentageLength,
};
use crate::values::specified::length::{FontBaseSize, LineHeightBase};
use crate::values::specified::resolution::ResolutionDimension;
use crate::values::specified::time::TimeDimension;
use crate::values::specified::{Angle, Resolution, Time};
use crate::values::{serialize_number, serialize_percentage, CSSFloat, DashedIdent};
use cssparser::{match_ignore_ascii_case, CowRcStr, Parser, SourceLocation, Token};
use debug_unreachable::debug_unreachable;
use smallvec::SmallVec;
use std::cmp;
use std::fmt::{self, Write};
use style_traits::values::specified::AllowedNumericType;
use style_traits::{
    CssWriter, ParseError, SpecifiedValueInfo, StyleParseErrorKind, ToCss, ToTyped, TypedValue,
};

/// The name of the mathematical function that we're parsing.
#[derive(Clone, Copy, Debug, Parse)]
pub enum MathFunction {
    /// `calc()`: https://drafts.csswg.org/css-values-4/#funcdef-calc
    Calc,
    /// `min()`: https://drafts.csswg.org/css-values-4/#funcdef-min
    Min,
    /// `max()`: https://drafts.csswg.org/css-values-4/#funcdef-max
    Max,
    /// `clamp()`: https://drafts.csswg.org/css-values-4/#funcdef-clamp
    Clamp,
    /// `round()`: https://drafts.csswg.org/css-values-4/#funcdef-round
    Round,
    /// `mod()`: https://drafts.csswg.org/css-values-4/#funcdef-mod
    Mod,
    /// `rem()`: https://drafts.csswg.org/css-values-4/#funcdef-rem
    Rem,
    /// `sin()`: https://drafts.csswg.org/css-values-4/#funcdef-sin
    Sin,
    /// `cos()`: https://drafts.csswg.org/css-values-4/#funcdef-cos
    Cos,
    /// `tan()`: https://drafts.csswg.org/css-values-4/#funcdef-tan
    Tan,
    /// `asin()`: https://drafts.csswg.org/css-values-4/#funcdef-asin
    Asin,
    /// `acos()`: https://drafts.csswg.org/css-values-4/#funcdef-acos
    Acos,
    /// `atan()`: https://drafts.csswg.org/css-values-4/#funcdef-atan
    Atan,
    /// `atan2()`: https://drafts.csswg.org/css-values-4/#funcdef-atan2
    Atan2,
    /// `pow()`: https://drafts.csswg.org/css-values-4/#funcdef-pow
    Pow,
    /// `sqrt()`: https://drafts.csswg.org/css-values-4/#funcdef-sqrt
    Sqrt,
    /// `hypot()`: https://drafts.csswg.org/css-values-4/#funcdef-hypot
    Hypot,
    /// `log()`: https://drafts.csswg.org/css-values-4/#funcdef-log
    Log,
    /// `exp()`: https://drafts.csswg.org/css-values-4/#funcdef-exp
    Exp,
    /// `abs()`: https://drafts.csswg.org/css-values-4/#funcdef-abs
    Abs,
    /// `sign()`: https://drafts.csswg.org/css-values-4/#funcdef-sign
    Sign,
    /// `progress()`: https://drafts.csswg.org/css-values-5/#funcdef-progress
    Progress,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TreeCountingFunction {
    SiblingIndex,
    SiblingCount,
}

impl TreeCountingFunction {
    fn from_name(name: &str) -> Option<Self> {
        if name.eq_ignore_ascii_case("sibling-index") {
            Some(Self::SiblingIndex)
        } else if name.eq_ignore_ascii_case("sibling-count") {
            Some(Self::SiblingCount)
        } else {
            None
        }
    }

    fn parse<'i, 't>(
        self,
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        location: SourceLocation,
    ) -> Result<CalcNode, ParseError<'i>> {
        if [
            CssRuleType::FontFace,
            CssRuleType::FontFeatureValues,
            CssRuleType::FontPaletteValues,
            CssRuleType::CounterStyle,
            CssRuleType::Page,
            CssRuleType::BdColour,
            CssRuleType::ColorProfile,
        ]
        .into_iter()
        .any(|rule_type| context.rule_types().contains(rule_type))
        {
            return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        input.parse_nested_block(|input| {
            input.expect_exhausted()?;
            Ok(())
        })?;
        Ok(CalcNode::Leaf(match self {
            Self::SiblingIndex => Leaf::SiblingIndex,
            Self::SiblingCount => Leaf::SiblingCount,
        }))
    }
}

/// A leaf node inside a `Calc` expression's AST.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
#[repr(u8)]
pub enum Leaf {
    /// The `calc-size()` basis placeholder.
    Size,
    /// `<length>`
    Length(NoCalcLength),
    /// `<angle>`
    Angle(AngleDimension),
    /// `<time>`
    Time(TimeDimension),
    /// `<resolution>`
    Resolution(ResolutionDimension),
    /// A component of a color.
    ColorComponent(ChannelKeyword),
    /// `<percentage>`
    Percentage(CSSFloat),
    /// `<number>`
    Number(CSSFloat),
    /// `sibling-index()`
    SiblingIndex,
    /// `sibling-count()`
    SiblingCount,
}

impl Leaf {
    fn as_length(&self) -> Option<&NoCalcLength> {
        match *self {
            Self::Length(ref l) => Some(l),
            _ => None,
        }
    }
}

impl ToCss for Leaf {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match *self {
            Self::Size => dest.write_str("size"),
            Self::Length(ref l) => l.to_css(dest),
            Self::Number(n) => serialize_number(n, /* was_calc = */ false, dest),
            Self::Resolution(ref r) => {
                crate::values::serialize_specified_dimension(r.dppx(), "dppx", false, dest)
            },
            Self::Percentage(p) => serialize_percentage(p, dest),
            Self::Angle(ref a) => crate::values::serialize_specified_dimension(
                a.degrees(),
                "deg",
                /* was_calc = */ false,
                dest,
            ),
            Self::Time(ref t) => {
                crate::values::serialize_specified_dimension(t.seconds(), "s", false, dest)
            },
            Self::ColorComponent(ref s) => s.to_css(dest),
            Self::SiblingIndex => dest.write_str("sibling-index()"),
            Self::SiblingCount => dest.write_str("sibling-count()"),
        }
    }
}

impl ToTyped for Leaf {
    fn to_typed(&self) -> Option<TypedValue> {
        // XXX Only supporting Length for now
        match *self {
            Self::Length(ref l) => l.to_typed(),
            _ => None,
        }
    }
}

/// A struct to hold a simplified `<length>` or `<percentage>` expression.
///
/// In some cases, e.g. DOMMatrix, we support calc(), but reject all the
/// relative lengths, and to_computed_pixel_length_without_context() handles
/// this case. Therefore, if you want to add a new field, please make sure this
/// function work properly.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToShmem, ToTyped)]
#[allow(missing_docs)]
#[typed_value(derive_fields)]
pub struct CalcLengthPercentage {
    #[css(skip)]
    pub clamping_mode: AllowedNumericType,
    pub node: CalcNode,
}

/// A validated mixed `<angle-percentage>` calculation.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem)]
pub struct CalcAnglePercentage {
    percentage: CSSFloat,
    angle: AngleDimension,
}

impl CalcAnglePercentage {
    fn from_node(mut node: CalcNode) -> Result<Self, ()> {
        node.simplify_and_sort();
        let CalcNode::Sum(items) = node else {
            return Err(());
        };
        let mut percentage = None;
        let mut angle = None;
        for item in items.into_vec() {
            match item {
                CalcNode::Leaf(Leaf::Percentage(value)) if percentage.is_none() => {
                    percentage = Some(value);
                },
                CalcNode::Leaf(Leaf::Angle(value)) if angle.is_none() => {
                    angle = Some(value);
                },
                _ => return Err(()),
            }
        }
        Ok(Self {
            percentage: percentage.ok_or(())?,
            angle: angle.ok_or(())?,
        })
    }
}

impl ToCss for CalcAnglePercentage {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        serialize_angle_percentage_calc(self.percentage, self.angle.degrees(), dest)
    }
}

impl ToComputedValue for CalcAnglePercentage {
    type ComputedValue = crate::values::computed::CalcAnglePercentage;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        Self::ComputedValue::new(
            crate::values::computed::Percentage(self.percentage),
            self.angle.to_computed_value(context),
        )
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self {
            percentage: computed.percentage().0,
            angle: AngleDimension::Deg(computed.angle().degrees()),
        }
    }
}

pub(crate) fn serialize_angle_percentage_calc<W>(
    percentage: CSSFloat,
    angle_degrees: CSSFloat,
    dest: &mut CssWriter<W>,
) -> fmt::Result
where
    W: Write,
{
    dest.write_str("calc(")?;
    serialize_percentage(percentage, dest)?;
    if angle_degrees.is_sign_negative() {
        dest.write_str(" - ")?;
        crate::values::serialize_specified_dimension(angle_degrees.abs(), "deg", false, dest)?;
    } else {
        dest.write_str(" + ")?;
        crate::values::serialize_specified_dimension(angle_degrees, "deg", false, dest)?;
    }
    dest.write_char(')')
}

impl CalcLengthPercentage {
    fn same_unit_length_as(a: &Self, b: &Self) -> Option<(CSSFloat, CSSFloat)> {
        debug_assert_eq!(a.clamping_mode, b.clamping_mode);
        debug_assert_eq!(a.clamping_mode, AllowedNumericType::All);

        let a = a.node.as_leaf()?;
        let b = b.node.as_leaf()?;

        if a.sort_key() != b.sort_key() {
            return None;
        }

        let a = a.as_length()?.unitless_value();
        let b = b.as_length()?.unitless_value();
        return Some((a, b));
    }
}

impl SpecifiedValueInfo for CalcLengthPercentage {}

/// Should parsing anchor-positioning functions in `calc()` be allowed?
#[derive(Clone, Copy, PartialEq)]
pub enum AllowAnchorPositioningFunctions {
    /// Don't allow any anchor positioning function.
    No,
    /// Allow `anchor-size()` to be parsed.
    AllowAnchorSize,
    /// Allow `anchor()` and `anchor-size()` to be parsed.
    AllowAnchorAndAnchorSize,
}

bitflags! {
    /// Additional functions within math functions that are permitted to be parsed depending on
    /// the context of parsing (e.g. Parsing `inset` allows use of `anchor()` within `calc()`).
    #[derive(Clone, Copy, PartialEq, Eq)]
    struct AdditionalFunctions: u8 {
        /// `anchor()` function.
        const ANCHOR = 1 << 0;
        /// `anchor-size()` function.
        const ANCHOR_SIZE = 1 << 1;
    }
}

/// What is allowed to be parsed for math functions within in this context?
#[derive(Clone, Copy)]
pub struct AllowParse {
    /// Units allowed to be parsed.
    units: CalcUnits,
    /// Additional functions allowed to be parsed in this context.
    additional_functions: AdditionalFunctions,
    allow_size_keyword: bool,
}

impl AllowParse {
    /// Allow only specified units to be parsed, without any additional functions.
    pub fn new(units: CalcUnits) -> Self {
        Self {
            units,
            additional_functions: AdditionalFunctions::empty(),
            allow_size_keyword: false,
        }
    }

    /// Add new units to the allowed units to be parsed.
    fn new_including(mut self, units: CalcUnits) -> Self {
        self.units |= units;
        self
    }

    /// Should given unit be allowed to parse?
    fn includes(&self, unit: CalcUnits) -> bool {
        self.units.intersects(unit)
    }
}

impl generic::CalcNodeLeaf for Leaf {
    fn is_function(&self) -> bool {
        matches!(self, Self::SiblingIndex | Self::SiblingCount)
    }

    fn unit(&self) -> CalcUnits {
        match self {
            Leaf::Size => CalcUnits::LENGTH,
            Leaf::Length(_) => CalcUnits::LENGTH,
            Leaf::Angle(_) => CalcUnits::ANGLE,
            Leaf::Time(_) => CalcUnits::TIME,
            Leaf::Resolution(_) => CalcUnits::RESOLUTION,
            Leaf::ColorComponent(_) => CalcUnits::COLOR_COMPONENT,
            Leaf::Percentage(_) => CalcUnits::PERCENTAGE,
            Leaf::Number(_) => CalcUnits::empty(),
            Leaf::SiblingIndex | Leaf::SiblingCount => CalcUnits::empty(),
        }
    }

    fn unitless_value(&self) -> Option<f32> {
        Some(match *self {
            Self::Size => return None,
            Self::Length(ref l) => l.unitless_value(),
            Self::Percentage(n) | Self::Number(n) => n,
            Self::Resolution(ref r) => r.dppx(),
            Self::Angle(ref a) => a.degrees(),
            Self::Time(ref t) => t.seconds(),
            Self::ColorComponent(_) => return None,
            Self::SiblingIndex | Self::SiblingCount => return None,
        })
    }

    fn new_number(value: f32) -> Self {
        Self::Number(value)
    }

    fn new_angle_radians(value: f32) -> Result<Self, ()> {
        Ok(Self::Angle(AngleDimension::Rad(value)))
    }

    fn as_angle_radians(&self) -> Option<f32> {
        match self {
            Self::Angle(angle) => Some(angle.radians()),
            _ => None,
        }
    }

    fn compare(&self, other: &Self, basis: PositivePercentageBasis) -> Option<cmp::Ordering> {
        use self::Leaf::*;

        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return None;
        }

        if matches!(self, Percentage(..)) && matches!(basis, PositivePercentageBasis::Unknown) {
            return None;
        }

        let self_negative = self.is_negative().unwrap_or(false);
        if self_negative != other.is_negative().unwrap_or(false) {
            return Some(if self_negative {
                cmp::Ordering::Less
            } else {
                cmp::Ordering::Greater
            });
        }

        match (self, other) {
            (&Size, &Size) => Some(cmp::Ordering::Equal),
            (&Percentage(ref one), &Percentage(ref other)) => one.partial_cmp(other),
            (&Length(ref one), &Length(ref other)) => one.partial_cmp(other),
            (&Angle(ref one), &Angle(ref other)) => one.degrees().partial_cmp(&other.degrees()),
            (&Time(ref one), &Time(ref other)) => one.seconds().partial_cmp(&other.seconds()),
            (&Resolution(ref one), &Resolution(ref other)) => one.dppx().partial_cmp(&other.dppx()),
            (&Number(ref one), &Number(ref other)) => one.partial_cmp(other),
            (&ColorComponent(ref one), &ColorComponent(ref other)) => one.partial_cmp(other),
            (&SiblingIndex, &SiblingIndex) | (&SiblingCount, &SiblingCount) => None,
            _ => {
                match *self {
                    Length(..) | Percentage(..) | Angle(..) | Time(..) | Number(..)
                    | Resolution(..) | ColorComponent(..) | Size => {},
                    SiblingIndex | SiblingCount => {},
                }
                unsafe {
                    debug_unreachable!("Forgot a branch?");
                }
            },
        }
    }

    fn as_number(&self) -> Option<f32> {
        match *self {
            Leaf::Size
            | Leaf::Length(_)
            | Leaf::Angle(_)
            | Leaf::Time(_)
            | Leaf::Resolution(_)
            | Leaf::Percentage(_)
            | Leaf::ColorComponent(_) => None,
            Leaf::SiblingIndex | Leaf::SiblingCount => None,
            Leaf::Number(value) => Some(value),
        }
    }

    fn sort_key(&self) -> SortKey {
        match *self {
            Self::Size => SortKey::Px,
            Self::Number(..) => SortKey::Number,
            Self::Percentage(..) => SortKey::Percentage,
            Self::Time(..) => SortKey::S,
            Self::Resolution(..) => SortKey::Dppx,
            Self::Angle(..) => SortKey::Deg,
            Self::Length(ref l) => match *l {
                NoCalcLength::Absolute(..) => SortKey::Px,
                NoCalcLength::FontRelative(ref relative) => match *relative {
                    FontRelativeLength::Em(..) => SortKey::Em,
                    FontRelativeLength::Ex(..) => SortKey::Ex,
                    FontRelativeLength::Rex(..) => SortKey::Rex,
                    FontRelativeLength::Ch(..) => SortKey::Ch,
                    FontRelativeLength::Rch(..) => SortKey::Rch,
                    FontRelativeLength::Cap(..) => SortKey::Cap,
                    FontRelativeLength::Rcap(..) => SortKey::Rcap,
                    FontRelativeLength::Ic(..) => SortKey::Ic,
                    FontRelativeLength::Ric(..) => SortKey::Ric,
                    FontRelativeLength::Rem(..) => SortKey::Rem,
                    FontRelativeLength::Lh(..) => SortKey::Lh,
                    FontRelativeLength::Rlh(..) => SortKey::Rlh,
                },
                NoCalcLength::ViewportPercentage(ref vp) => match *vp {
                    ViewportPercentageLength::Vh(..) => SortKey::Vh,
                    ViewportPercentageLength::Svh(..) => SortKey::Svh,
                    ViewportPercentageLength::Lvh(..) => SortKey::Lvh,
                    ViewportPercentageLength::Dvh(..) => SortKey::Dvh,
                    ViewportPercentageLength::Vw(..) => SortKey::Vw,
                    ViewportPercentageLength::Svw(..) => SortKey::Svw,
                    ViewportPercentageLength::Lvw(..) => SortKey::Lvw,
                    ViewportPercentageLength::Dvw(..) => SortKey::Dvw,
                    ViewportPercentageLength::Vmax(..) => SortKey::Vmax,
                    ViewportPercentageLength::Svmax(..) => SortKey::Svmax,
                    ViewportPercentageLength::Lvmax(..) => SortKey::Lvmax,
                    ViewportPercentageLength::Dvmax(..) => SortKey::Dvmax,
                    ViewportPercentageLength::Vmin(..) => SortKey::Vmin,
                    ViewportPercentageLength::Svmin(..) => SortKey::Svmin,
                    ViewportPercentageLength::Lvmin(..) => SortKey::Lvmin,
                    ViewportPercentageLength::Dvmin(..) => SortKey::Dvmin,
                    ViewportPercentageLength::Vb(..) => SortKey::Vb,
                    ViewportPercentageLength::Svb(..) => SortKey::Svb,
                    ViewportPercentageLength::Lvb(..) => SortKey::Lvb,
                    ViewportPercentageLength::Dvb(..) => SortKey::Dvb,
                    ViewportPercentageLength::Vi(..) => SortKey::Vi,
                    ViewportPercentageLength::Svi(..) => SortKey::Svi,
                    ViewportPercentageLength::Lvi(..) => SortKey::Lvi,
                    ViewportPercentageLength::Dvi(..) => SortKey::Dvi,
                },
                NoCalcLength::ContainerRelative(ref cq) => match *cq {
                    ContainerRelativeLength::Cqw(..) => SortKey::Cqw,
                    ContainerRelativeLength::Cqh(..) => SortKey::Cqh,
                    ContainerRelativeLength::Cqi(..) => SortKey::Cqi,
                    ContainerRelativeLength::Cqb(..) => SortKey::Cqb,
                    ContainerRelativeLength::Cqmin(..) => SortKey::Cqmin,
                    ContainerRelativeLength::Cqmax(..) => SortKey::Cqmax,
                },
                NoCalcLength::PageRelative(ref pr) => match *pr {
                    PageRelativeLength::Pw(..) => SortKey::BdPw,
                    PageRelativeLength::Pi(..) => SortKey::BdPi,
                    PageRelativeLength::Ph(..) => SortKey::BdPh,
                    PageRelativeLength::Pb(..) => SortKey::BdPb,
                    PageRelativeLength::Pmin(..) => SortKey::BdPmin,
                    PageRelativeLength::Pmax(..) => SortKey::BdPmax,
                    PageRelativeLength::Bw(..) => SortKey::BdBw,
                    PageRelativeLength::Bi(..) => SortKey::BdBi,
                    PageRelativeLength::Bh(..) => SortKey::BdBh,
                    PageRelativeLength::Bb(..) => SortKey::BdBb,
                    PageRelativeLength::Bmin(..) => SortKey::BdBmin,
                    PageRelativeLength::Bmax(..) => SortKey::BdBmax,
                },
                NoCalcLength::ServoCharacterWidth(..) => unreachable!(),
            },
            Self::ColorComponent(..) => SortKey::ColorComponent,
            Self::SiblingIndex | Self::SiblingCount => SortKey::Other,
        }
    }

    fn simplify(&mut self) {
        if let Self::Length(NoCalcLength::Absolute(ref mut abs)) = *self {
            *abs = AbsoluteLength::Px(abs.to_px());
        }
    }

    /// Tries to merge one sum to another, that is, perform `x` + `y`.
    ///
    /// Only handles leaf nodes, it's the caller's responsibility to simplify
    /// them before calling this if needed.
    fn try_sum_in_place(&mut self, other: &Self) -> Result<(), ()> {
        use self::Leaf::*;

        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return Err(());
        }

        match (self, other) {
            (&mut Number(ref mut one), &Number(ref other))
            | (&mut Percentage(ref mut one), &Percentage(ref other)) => {
                *one += *other;
            },
            (&mut Angle(ref mut one), &Angle(ref other)) => {
                *one = AngleDimension::Deg(one.degrees() + other.degrees());
            },
            (&mut Time(ref mut one), &Time(ref other)) => {
                *one = TimeDimension::from_seconds(one.seconds() + other.seconds());
            },
            (&mut Resolution(ref mut one), &Resolution(ref other)) => {
                *one = ResolutionDimension::from_dppx(one.dppx() + other.dppx());
            },
            (&mut Length(ref mut one), &Length(ref other)) => {
                *one = one.try_op(other, std::ops::Add::add)?;
            },
            (&mut ColorComponent(_), &ColorComponent(_)) => {
                // Can not get the sum of color components, because they haven't been resolved yet.
                return Err(());
            },
            (&mut SiblingIndex, &SiblingIndex) | (&mut SiblingCount, &SiblingCount) => {
                return Err(());
            },
            _ => {
                match *other {
                    Number(..) | Percentage(..) | Angle(..) | Time(..) | Resolution(..)
                    | Length(..) | ColorComponent(..) | Size => {},
                    SiblingIndex | SiblingCount => {},
                }
                unsafe {
                    debug_unreachable!();
                }
            },
        }

        Ok(())
    }

    fn try_product_in_place(&mut self, other: &mut Self) -> bool {
        if let Self::Number(ref mut left) = *self {
            if let Self::Number(ref right) = *other {
                // Both sides are numbers, so we can just modify the left side.
                *left *= *right;
                true
            } else {
                // The right side is not a number, so the result should be in the units of the right
                // side.
                if other.map(|v| v * *left).is_ok() {
                    std::mem::swap(self, other);
                    true
                } else {
                    false
                }
            }
        } else if let Self::Number(ref right) = *other {
            // The left side is not a number, but the right side is, so the result is the left
            // side unit.
            self.map(|v| v * *right).is_ok()
        } else {
            // Neither side is a number, so a product is not possible.
            false
        }
    }

    fn try_op<O>(&self, other: &Self, op: O) -> Result<Self, ()>
    where
        O: Fn(f32, f32) -> f32,
    {
        use self::Leaf::*;

        if std::mem::discriminant(self) != std::mem::discriminant(other) {
            return Err(());
        }

        match (self, other) {
            (&Number(one), &Number(other)) => {
                return Ok(Leaf::Number(op(one, other)));
            },
            (&Percentage(one), &Percentage(other)) => {
                return Ok(Leaf::Percentage(op(one, other)));
            },
            (&Angle(ref one), &Angle(ref other)) => {
                return Ok(Leaf::Angle(AngleDimension::Deg(op(
                    one.degrees(),
                    other.degrees(),
                ))));
            },
            (&Resolution(ref one), &Resolution(ref other)) => {
                return Ok(Leaf::Resolution(ResolutionDimension::from_dppx(op(
                    one.dppx(),
                    other.dppx(),
                ))));
            },
            (&Time(ref one), &Time(ref other)) => {
                return Ok(Leaf::Time(TimeDimension::from_seconds(op(
                    one.seconds(),
                    other.seconds(),
                ))));
            },
            (&Length(ref one), &Length(ref other)) => {
                return Ok(Leaf::Length(one.try_op(other, op)?));
            },
            _ => Err(()),
        }
    }

    fn can_scale(&self) -> bool {
        !matches!(
            self,
            Self::Size | Self::ColorComponent(..) | Self::SiblingIndex | Self::SiblingCount
        )
    }

    fn map(&mut self, mut op: impl FnMut(f32) -> f32) -> Result<(), ()> {
        Ok(match self {
            Leaf::Size => return Err(()),
            Leaf::Length(one) => *one = one.map(op),
            Leaf::Angle(one) => *one = AngleDimension::Deg(op(one.degrees())),
            Leaf::Time(one) => *one = TimeDimension::from_seconds(op(one.seconds())),
            Leaf::Resolution(one) => *one = ResolutionDimension::from_dppx(op(one.dppx())),
            Leaf::Percentage(one) => *one = op(*one),
            Leaf::Number(one) => *one = op(*one),
            Leaf::ColorComponent(..) => return Err(()),
            Leaf::SiblingIndex | Leaf::SiblingCount => return Err(()),
        })
    }
}

impl GenericAnchorSide<Box<CalcNode>> {
    fn parse_in_calc<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(k) = input.try_parse(|i| AnchorSideKeyword::parse(i)) {
            return Ok(Self::Keyword(k));
        }
        Ok(Self::Percentage(Box::new(CalcNode::parse_argument(
            context,
            input,
            AllowParse::new(CalcUnits::PERCENTAGE),
        )?)))
    }
}

impl GenericAnchorFunction<Box<CalcNode>, Box<CalcNode>> {
    fn parse_in_calc<'i, 't>(
        context: &ParserContext,
        additional_functions: AdditionalFunctions,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if !static_prefs::pref!("layout.css.anchor-positioning.enabled") {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        input.parse_nested_block(|i| {
            let target_element = i.try_parse(|i| DashedIdent::parse(context, i)).ok();
            let side = GenericAnchorSide::parse_in_calc(context, i)?;
            let target_element = if target_element.is_none() {
                i.try_parse(|i| DashedIdent::parse(context, i)).ok()
            } else {
                target_element
            };
            let fallback = i
                .try_parse(|i| {
                    i.expect_comma()?;
                    Ok::<Box<CalcNode>, ParseError<'i>>(Box::new(
                        CalcNode::parse_argument(
                            context,
                            i,
                            AllowParse {
                                units: CalcUnits::LENGTH_PERCENTAGE,
                                additional_functions,
                                allow_size_keyword: false,
                            },
                        )?
                        .into_length_or_percentage(AllowedNumericType::All)
                        .map_err(|_| i.new_custom_error(StyleParseErrorKind::UnspecifiedError))?
                        .node,
                    ))
                })
                .ok();
            Ok(Self {
                target_element: TreeScoped::with_default_level(
                    target_element.unwrap_or_else(DashedIdent::empty),
                ),
                side,
                fallback: fallback.into(),
            })
        })
    }
}

impl GenericAnchorSizeFunction<Box<CalcNode>> {
    fn parse_in_calc<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if !static_prefs::pref!("layout.css.anchor-positioning.enabled") {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        GenericAnchorSizeFunction::parse_inner(context, input, |i| {
            Ok(Box::new(
                CalcNode::parse_argument(
                    context,
                    i,
                    AllowParse::new(CalcUnits::LENGTH_PERCENTAGE),
                )?
                .into_length_or_percentage(AllowedNumericType::All)
                .map_err(|_| i.new_custom_error(StyleParseErrorKind::UnspecifiedError))?
                .node,
            ))
        })
    }
}

/// Specified `anchor()` function in math functions.
pub type CalcAnchorFunction = generic::GenericCalcAnchorFunction<Leaf>;
/// Specified `anchor-size()` function in math functions.
pub type CalcAnchorSizeFunction = generic::GenericCalcAnchorSizeFunction<Leaf>;

/// A calc node representation for specified values.
pub type CalcNode = generic::GenericCalcNode<Leaf>;
impl CalcNode {
    /// Tries to parse a single element in the expression, that is, a
    /// `<length>`, `<angle>`, `<time>`, `<percentage>`, `<resolution>`, etc.
    ///
    /// May return a "complex" `CalcNode`, in the presence of a parenthesized
    /// expression, for example.
    pub(crate) fn parse_one<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        allowed: AllowParse,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        match input.next()? {
            &Token::Number { value, .. } => Ok(CalcNode::Leaf(Leaf::Number(value))),
            &Token::Dimension {
                value, ref unit, ..
            } => {
                if allowed.includes(CalcUnits::LENGTH) {
                    if let Ok(l) = NoCalcLength::parse_dimension(context, value, unit) {
                        return Ok(CalcNode::Leaf(Leaf::Length(l)));
                    }
                }
                if allowed.includes(CalcUnits::ANGLE) {
                    if let Ok(a) = AngleDimension::parse(value, unit) {
                        return Ok(CalcNode::Leaf(Leaf::Angle(a)));
                    }
                }
                if allowed.includes(CalcUnits::TIME) {
                    if let Ok(t) = TimeDimension::parse_dimension(value, unit) {
                        return Ok(CalcNode::Leaf(Leaf::Time(t)));
                    }
                }
                if allowed.includes(CalcUnits::RESOLUTION) {
                    if let Ok(t) = ResolutionDimension::parse_dimension(value, unit) {
                        return Ok(CalcNode::Leaf(Leaf::Resolution(t)));
                    }
                }
                return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            },
            &Token::Percentage { unit_value, .. } if allowed.includes(CalcUnits::PERCENTAGE) => {
                Ok(CalcNode::Leaf(Leaf::Percentage(unit_value)))
            },
            &Token::ParenthesisBlock => {
                input.parse_nested_block(|input| CalcNode::parse_argument(context, input, allowed))
            },
            &Token::Function(ref name)
                if allowed
                    .additional_functions
                    .intersects(AdditionalFunctions::ANCHOR)
                    && name.eq_ignore_ascii_case("anchor") =>
            {
                let anchor_function = GenericAnchorFunction::parse_in_calc(
                    context,
                    allowed.additional_functions,
                    input,
                )?;
                Ok(CalcNode::Anchor(Box::new(anchor_function)))
            },
            &Token::Function(ref name)
                if allowed
                    .additional_functions
                    .intersects(AdditionalFunctions::ANCHOR_SIZE)
                    && name.eq_ignore_ascii_case("anchor-size") =>
            {
                let anchor_size_function =
                    GenericAnchorSizeFunction::parse_in_calc(context, input)?;
                Ok(CalcNode::AnchorSize(Box::new(anchor_size_function)))
            },
            &Token::Function(ref name) => {
                if let Some(function) = TreeCountingFunction::from_name(name) {
                    return function.parse(context, input, location);
                }
                let function = CalcNode::math_function(context, name, location)?;
                CalcNode::parse(context, input, function, allowed)
            },
            &Token::Ident(ref ident) => {
                let leaf = match_ignore_ascii_case! { &**ident,
                    "size" if allowed.allow_size_keyword => Leaf::Size,
                    "e" => Leaf::Number(std::f32::consts::E),
                    "pi" => Leaf::Number(std::f32::consts::PI),
                    "infinity" => Leaf::Number(f32::INFINITY),
                    "-infinity" => Leaf::Number(f32::NEG_INFINITY),
                    "nan" => Leaf::Number(f32::NAN),
                    _ => {
                        if crate::color::parsing::rcs_enabled() &&
                            allowed.includes(CalcUnits::COLOR_COMPONENT)
                        {
                            if let Ok(channel_keyword) = ChannelKeyword::from_ident(&ident) {
                                Leaf::ColorComponent(channel_keyword)
                            } else {
                                return Err(location
                                    .new_unexpected_token_error(Token::Ident(ident.clone())));
                            }
                        } else {
                            return Err(
                                location.new_unexpected_token_error(Token::Ident(ident.clone()))
                            );
                        }
                    },
                };
                Ok(CalcNode::Leaf(leaf))
            },
            t => Err(location.new_unexpected_token_error(t.clone())),
        }
    }

    /// Parse a top-level `calc` expression, with all nested sub-expressions.
    ///
    /// This is in charge of parsing, for example, `2 + 3 * 100%`.
    pub fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
        allowed: AllowParse,
    ) -> Result<Self, ParseError<'i>> {
        input.parse_nested_block(|input| {
            match function {
                MathFunction::Calc => Self::parse_argument(context, input, allowed),
                MathFunction::Clamp => {
                    let min = Self::parse_argument(context, input, allowed)?;
                    input.expect_comma()?;
                    let center = Self::parse_argument(context, input, allowed)?;
                    input.expect_comma()?;
                    let max = Self::parse_argument(context, input, allowed)?;
                    Ok(Self::Clamp {
                        min: Box::new(min),
                        center: Box::new(center),
                        max: Box::new(max),
                    })
                },
                MathFunction::Round => {
                    let strategy = input.try_parse(parse_rounding_strategy);

                    // <rounding-strategy> = nearest | up | down | to-zero
                    // https://drafts.csswg.org/css-values-4/#calc-syntax
                    fn parse_rounding_strategy<'i, 't>(
                        input: &mut Parser<'i, 't>,
                    ) -> Result<RoundingStrategy, ParseError<'i>> {
                        Ok(try_match_ident_ignore_ascii_case! { input,
                            "nearest" => RoundingStrategy::Nearest,
                            "up" => RoundingStrategy::Up,
                            "down" => RoundingStrategy::Down,
                            "to-zero" => RoundingStrategy::ToZero,
                        })
                    }

                    if strategy.is_ok() {
                        input.expect_comma()?;
                    }

                    let value = Self::parse_argument(context, input, allowed)?;

                    // <step> defaults to the number 1 if not provided
                    // https://drafts.csswg.org/css-values-4/#funcdef-round
                    let step = input.try_parse(|input| {
                        input.expect_comma()?;
                        Self::parse_argument(context, input, allowed)
                    });

                    let step = step.unwrap_or(Self::Leaf(Leaf::Number(1.0)));

                    Ok(Self::Round {
                        strategy: strategy.unwrap_or(RoundingStrategy::Nearest),
                        value: Box::new(value),
                        step: Box::new(step),
                    })
                },
                MathFunction::Mod | MathFunction::Rem => {
                    let dividend = Self::parse_argument(context, input, allowed)?;
                    input.expect_comma()?;
                    let divisor = Self::parse_argument(context, input, allowed)?;

                    let op = match function {
                        MathFunction::Mod => ModRemOp::Mod,
                        MathFunction::Rem => ModRemOp::Rem,
                        _ => unreachable!(),
                    };
                    Ok(Self::ModRem {
                        dividend: Box::new(dividend),
                        divisor: Box::new(divisor),
                        op,
                    })
                },
                MathFunction::Min | MathFunction::Max => {
                    // TODO(emilio): The common case for parse_comma_separated
                    // is just one element, but for min / max is two, really...
                    //
                    // Consider adding an API to cssparser to specify the
                    // initial vector capacity?
                    let arguments = input.parse_comma_separated(|input| {
                        let result = Self::parse_argument(context, input, allowed)?;
                        Ok(result)
                    })?;

                    let op = match function {
                        MathFunction::Min => MinMaxOp::Min,
                        MathFunction::Max => MinMaxOp::Max,
                        _ => unreachable!(),
                    };

                    Ok(Self::MinMax(arguments.into(), op))
                },
                MathFunction::Sin | MathFunction::Cos | MathFunction::Tan => {
                    let units = if allowed.includes(CalcUnits::COLOR_COMPONENT) {
                        CalcUnits::ANGLE | CalcUnits::COLOR_COMPONENT
                    } else {
                        CalcUnits::ANGLE
                    };
                    let argument = Self::parse_argument(context, input, AllowParse::new(units))?;
                    let operation = match function {
                        MathFunction::Sin => TrigonometricFunction::Sin,
                        MathFunction::Cos => TrigonometricFunction::Cos,
                        MathFunction::Tan => TrigonometricFunction::Tan,
                        _ => unreachable!(),
                    };
                    let node = Self::Trigonometric(Box::new(argument), operation);
                    node.unit().map_err(|()| {
                        input.new_custom_error(StyleParseErrorKind::UnspecifiedError)
                    })?;
                    Ok(node.resolve().map(Self::Leaf).unwrap_or(node))
                },
                MathFunction::Asin | MathFunction::Acos | MathFunction::Atan => {
                    let units = if allowed.includes(CalcUnits::COLOR_COMPONENT) {
                        CalcUnits::COLOR_COMPONENT
                    } else {
                        CalcUnits::empty()
                    };
                    let argument = Self::parse_argument(context, input, AllowParse::new(units))?;
                    let operation = match function {
                        MathFunction::Asin => TrigonometricFunction::Asin,
                        MathFunction::Acos => TrigonometricFunction::Acos,
                        MathFunction::Atan => TrigonometricFunction::Atan,
                        _ => unreachable!(),
                    };
                    let node = Self::Trigonometric(Box::new(argument), operation);
                    node.unit().map_err(|()| {
                        input.new_custom_error(StyleParseErrorKind::UnspecifiedError)
                    })?;
                    Ok(node.resolve().map(Self::Leaf).unwrap_or(node))
                },
                MathFunction::Atan2 => {
                    let allow_all = allowed.new_including(CalcUnits::ALL);
                    let a = Self::parse_argument(context, input, allow_all)?;
                    input.expect_comma()?;
                    let b = Self::parse_argument(context, input, allow_all)?;

                    let radians = Self::try_resolve(input, || {
                        if let Ok(a) = a.to_number() {
                            let b = b.to_number()?;
                            return Ok(a.atan2(b));
                        }

                        if let Ok(a) = a.to_percentage() {
                            let b = b.to_percentage()?;
                            return Ok(a.atan2(b));
                        }

                        if let Ok(a) = a.to_time() {
                            let b = b.to_time()?;
                            return Ok(a.seconds().atan2(b.seconds()));
                        }

                        if let Ok(a) = a.to_angle() {
                            let b = b.to_angle()?;
                            return Ok(a.radians().atan2(b.radians()));
                        }

                        if let Ok(a) = a.to_resolution() {
                            let b = b.to_resolution()?;
                            return Ok(a.dppx().atan2(b.dppx()));
                        }

                        let a = a.into_length_or_percentage(AllowedNumericType::All)?;
                        let b = b.into_length_or_percentage(AllowedNumericType::All)?;
                        let (a, b) = CalcLengthPercentage::same_unit_length_as(&a, &b).ok_or(())?;

                        Ok(a.atan2(b))
                    })?;

                    Ok(Self::Leaf(Leaf::Angle(AngleDimension::Rad(radians))))
                },
                MathFunction::Pow => {
                    let units = if allowed.includes(CalcUnits::COLOR_COMPONENT) {
                        CalcUnits::COLOR_COMPONENT
                    } else {
                        CalcUnits::empty()
                    };
                    let a = Self::parse_argument(context, input, AllowParse::new(units))?;
                    input.expect_comma()?;
                    let b = Self::parse_argument(context, input, AllowParse::new(units))?;
                    let node = Self::Pow(Box::new(a), Box::new(b));
                    node.unit().map_err(|()| {
                        input.new_custom_error(StyleParseErrorKind::UnspecifiedError)
                    })?;
                    Ok(node.resolve().map(Self::Leaf).unwrap_or(node))
                },
                MathFunction::Sqrt => {
                    let a = Self::parse_number_argument(context, input)?;

                    let number = a.sqrt();

                    Ok(Self::Leaf(Leaf::Number(number)))
                },
                MathFunction::Hypot => {
                    let arguments = input.parse_comma_separated(|input| {
                        let result = Self::parse_argument(context, input, allowed)?;
                        Ok(result)
                    })?;

                    Ok(Self::Hypot(arguments.into()))
                },
                MathFunction::Log => {
                    let a = Self::parse_number_argument(context, input)?;
                    let b = input
                        .try_parse(|input| {
                            input.expect_comma()?;
                            Self::parse_number_argument(context, input)
                        })
                        .ok();

                    let number = match b {
                        Some(b) => a.log(b),
                        None => a.ln(),
                    };

                    Ok(Self::Leaf(Leaf::Number(number)))
                },
                MathFunction::Exp => {
                    let a = Self::parse_number_argument(context, input)?;
                    let number = a.exp();
                    Ok(Self::Leaf(Leaf::Number(number)))
                },
                MathFunction::Abs => {
                    let node = Self::parse_argument(context, input, allowed)?;
                    Ok(Self::Abs(Box::new(node)))
                },
                MathFunction::Sign => {
                    // The sign of a percentage is dependent on the percentage basis, so if
                    // percentages aren't allowed (so there's no basis) we shouldn't allow them in
                    // sign(). The rest of the units are safe tho.
                    let node = Self::parse_argument(
                        context,
                        input,
                        allowed.new_including(CalcUnits::ALL - CalcUnits::PERCENTAGE),
                    )?;
                    Ok(Self::Sign(Box::new(node)))
                },
                MathFunction::Progress => {
                    let progress_allowed = allowed.new_including(CalcUnits::ALL);
                    let clamping = if input
                        .try_parse(|input| input.expect_ident_matching("no-clamp"))
                        .is_ok()
                    {
                        ProgressClamping::Unclamped
                    } else {
                        ProgressClamping::Clamped
                    };
                    let value = Box::new(Self::parse_argument(context, input, progress_allowed)?);
                    input.expect_comma()?;
                    let start = Box::new(Self::parse_argument(context, input, progress_allowed)?);
                    input.expect_comma()?;
                    let end = Box::new(Self::parse_argument(context, input, progress_allowed)?);
                    Ok(Self::Progress {
                        value,
                        start,
                        end,
                        clamping,
                    })
                },
            }
        })
    }

    fn parse_number_argument<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<CSSFloat, ParseError<'i>> {
        Self::parse_argument(context, input, AllowParse::new(CalcUnits::empty()))?
            .to_number()
            .map_err(|()| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }

    fn parse_argument<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        allowed: AllowParse,
    ) -> Result<Self, ParseError<'i>> {
        let mut sum = SmallVec::<[CalcNode; 1]>::new();
        let first = Self::parse_product(context, input, allowed)?;
        sum.push(first);
        loop {
            let start = input.state();
            match input.next_including_whitespace() {
                Ok(&Token::WhiteSpace(_)) => {
                    if input.is_exhausted() {
                        break; // allow trailing whitespace
                    }
                    match *input.next()? {
                        Token::Delim('+') => {
                            let rhs = Self::parse_product(context, input, allowed)?;
                            if sum.last_mut().unwrap().try_sum_in_place(&rhs).is_err() {
                                sum.push(rhs);
                            }
                        },
                        Token::Delim('-') => {
                            let mut rhs = Self::parse_product(context, input, allowed)?;
                            rhs.negate();
                            if sum.last_mut().unwrap().try_sum_in_place(&rhs).is_err() {
                                sum.push(rhs);
                            }
                        },
                        _ => {
                            input.reset(&start);
                            break;
                        },
                    }
                },
                _ => {
                    input.reset(&start);
                    break;
                },
            }
        }

        Ok(if sum.len() == 1 {
            sum.drain(..).next().unwrap()
        } else {
            Self::Sum(sum.into_boxed_slice().into())
        })
    }

    /// Parse a top-level `calc` expression, and all the products that may
    /// follow, and stop as soon as a non-product expression is found.
    ///
    /// This should parse correctly:
    ///
    /// * `2`
    /// * `2 * 2`
    /// * `2 * 2 + 2` (but will leave the `+ 2` unparsed).
    ///
    fn parse_product<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        allowed: AllowParse,
    ) -> Result<Self, ParseError<'i>> {
        let mut product = SmallVec::<[CalcNode; 1]>::new();
        let first = Self::parse_one(context, input, allowed)?;
        product.push(first);

        loop {
            let start = input.state();
            match input.next() {
                Ok(&Token::Delim('*')) => {
                    let mut rhs = Self::parse_one(context, input, allowed)?;

                    // We can unwrap here, becuase we start the function by adding a node to
                    // the list.
                    if !product.last_mut().unwrap().try_product_in_place(&mut rhs) {
                        product.push(rhs);
                    }
                },
                Ok(&Token::Delim('/')) => {
                    let rhs = Self::parse_one(context, input, allowed)?;

                    enum InPlaceDivisionResult {
                        /// The right was merged into the left.
                        Merged,
                        /// The right is not a number or could not be resolved, so the left is
                        /// unchanged.
                        Unchanged,
                        /// The right was resolved, but was not a number, so the calculation is
                        /// invalid.
                        Invalid,
                    }

                    fn try_division_in_place(
                        left: &mut CalcNode,
                        right: &CalcNode,
                    ) -> InPlaceDivisionResult {
                        if let Ok(resolved) = right.resolve() {
                            if let Some(number) = resolved.as_number() {
                                if number != 1.0 && left.is_product_distributive() {
                                    if left.map(|l| l / number).is_err() {
                                        return InPlaceDivisionResult::Invalid;
                                    }
                                    return InPlaceDivisionResult::Merged;
                                }
                            } else if let (Ok(left_angle), Ok(right_angle)) =
                                (left.to_angle(), right.to_angle())
                            {
                                *left = CalcNode::Leaf(Leaf::Number(
                                    left_angle.radians() / right_angle.radians(),
                                ));
                                return InPlaceDivisionResult::Merged;
                            } else {
                                // Color components are valid denominators, but they can't resolve
                                // at parse time.
                                return if resolved.unit().contains(CalcUnits::COLOR_COMPONENT) {
                                    InPlaceDivisionResult::Unchanged
                                } else {
                                    InPlaceDivisionResult::Invalid
                                };
                            }
                        }
                        InPlaceDivisionResult::Unchanged
                    }

                    // The right hand side of a division *must* be a number, so if we can
                    // already resolve it, then merge it with the last node on the product list.
                    // We can unwrap here, becuase we start the function by adding a node to
                    // the list.
                    match try_division_in_place(&mut product.last_mut().unwrap(), &rhs) {
                        InPlaceDivisionResult::Merged => {},
                        InPlaceDivisionResult::Unchanged => {
                            product.push(Self::Invert(Box::new(rhs)))
                        },
                        InPlaceDivisionResult::Invalid => {
                            return Err(
                                input.new_custom_error(StyleParseErrorKind::UnspecifiedError)
                            )
                        },
                    }
                },
                _ => {
                    input.reset(&start);
                    break;
                },
            }
        }

        Ok(if product.len() == 1 {
            product.drain(..).next().unwrap()
        } else {
            Self::Product(product.into_boxed_slice().into())
        })
    }

    fn try_resolve<'i, 't, F>(
        input: &Parser<'i, 't>,
        closure: F,
    ) -> Result<CSSFloat, ParseError<'i>>
    where
        F: FnOnce() -> Result<CSSFloat, ()>,
    {
        closure().map_err(|()| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }

    /// Tries to simplify this expression into a `<length>` or `<percentage>`
    /// value.
    pub fn into_length_or_percentage(
        mut self,
        clamping_mode: AllowedNumericType,
    ) -> Result<CalcLengthPercentage, ()> {
        self.simplify_and_sort();

        // Although we allow numbers inside CalcLengthPercentage, calculations that resolve to a
        // number result is still not allowed.
        let unit = self.unit()?;
        if !CalcUnits::LENGTH_PERCENTAGE.intersects(unit) {
            Err(())
        } else {
            Ok(CalcLengthPercentage {
                clamping_mode,
                node: self,
            })
        }
    }

    /// Tries to simplify this expression into a time dimension.
    fn to_time(&self) -> Result<TimeDimension, ()> {
        match self.resolve()? {
            Leaf::Time(time) => Ok(time),
            _ => Err(()),
        }
    }

    /// Tries to simplify the expression into a `<resolution>` value.
    fn to_resolution(&self) -> Result<ResolutionDimension, ()> {
        match self.resolve()? {
            Leaf::Resolution(resolution) => Ok(resolution),
            _ => Err(()),
        }
    }

    /// Tries to simplify this expression into an angle dimension.
    fn to_angle(&self) -> Result<AngleDimension, ()> {
        match self.resolve()? {
            Leaf::Angle(angle) => Ok(angle),
            _ => Err(()),
        }
    }

    /// Tries to simplify this expression into a `<number>` value.
    fn to_number(&self) -> Result<CSSFloat, ()> {
        let number = if let Leaf::Number(number) = self.resolve()? {
            number
        } else {
            return Err(());
        };

        let result = number;

        Ok(result)
    }

    /// Tries to simplify this expression into a `<percentage>` value.
    fn to_percentage(&self) -> Result<CSSFloat, ()> {
        if let Leaf::Percentage(percentage) = self.resolve()? {
            Ok(percentage)
        } else {
            Err(())
        }
    }

    /// Given a function name, and the location from where the token came from,
    /// return a mathematical function corresponding to that name or an error.
    #[inline]
    pub fn math_function<'i>(
        _: &ParserContext,
        name: &CowRcStr<'i>,
        location: cssparser::SourceLocation,
    ) -> Result<MathFunction, ParseError<'i>> {
        let function = match MathFunction::from_ident(&*name) {
            Ok(f) => f,
            Err(()) => {
                return Err(location.new_unexpected_token_error(Token::Function(name.clone())))
            },
        };

        Ok(function)
    }

    /// Convenience parsing function for `<length> | <percentage>`, and, optionally, `anchor()`.
    pub fn parse_length_or_percentage<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        clamping_mode: AllowedNumericType,
        function: MathFunction,
        allow_anchor: AllowAnchorPositioningFunctions,
    ) -> Result<CalcLengthPercentage, ParseError<'i>> {
        let allowed = if allow_anchor == AllowAnchorPositioningFunctions::No {
            AllowParse::new(CalcUnits::LENGTH_PERCENTAGE)
        } else {
            AllowParse {
                units: CalcUnits::LENGTH_PERCENTAGE,
                additional_functions: match allow_anchor {
                    AllowAnchorPositioningFunctions::No => unreachable!(),
                    AllowAnchorPositioningFunctions::AllowAnchorSize => {
                        AdditionalFunctions::ANCHOR_SIZE
                    },
                    AllowAnchorPositioningFunctions::AllowAnchorAndAnchorSize => {
                        AdditionalFunctions::ANCHOR | AdditionalFunctions::ANCHOR_SIZE
                    },
                },
                allow_size_keyword: false,
            }
        };
        Self::parse(context, input, function, allowed)?
            .into_length_or_percentage(clamping_mode)
            .map_err(|()| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }

    /// Convenience parsing function for percentages.
    pub fn parse_percentage<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
    ) -> Result<CSSFloat, ParseError<'i>> {
        Self::parse_percentage_node(context, input, function)?
            .to_percentage()
            .map(crate::values::normalize)
            .map_err(|()| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }

    /// Parses a percentage calculation while retaining its expression tree.
    pub fn parse_percentage_node<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_typed_node(context, input, function, CalcUnits::PERCENTAGE)
    }

    /// Convenience parsing function for `<length>`.
    pub fn parse_length<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        clamping_mode: AllowedNumericType,
        function: MathFunction,
    ) -> Result<CalcLengthPercentage, ParseError<'i>> {
        Self::parse(context, input, function, AllowParse::new(CalcUnits::LENGTH))?
            .into_length_or_percentage(clamping_mode)
            .map_err(|()| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }

    /// Parse the calculation argument of `calc-size()`, where `size` is a
    /// length-valued placeholder for the separately parsed basis.
    pub fn parse_calc_size<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let mut allowed = AllowParse::new(CalcUnits::LENGTH_PERCENTAGE);
        allowed.allow_size_keyword = true;
        Self::parse_argument(context, input, allowed)?
            .require_unit(input, CalcUnits::LENGTH_PERCENTAGE)
    }

    fn require_unit<'i, 't>(
        self,
        input: &Parser<'i, 't>,
        required: CalcUnits,
    ) -> Result<Self, ParseError<'i>> {
        let unit = self
            .unit()
            .map_err(|()| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))?;
        if !required.intersects(unit) {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(self)
    }

    /// Convenience parsing function for `<number>`.
    pub fn parse_number_node<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_typed_node(context, input, function, CalcUnits::empty())
    }

    /// Parse a function which represents a `<number>`.
    pub fn parse_number_function<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        name: CowRcStr<'i>,
        location: SourceLocation,
    ) -> Result<Self, ParseError<'i>> {
        if let Some(function) = TreeCountingFunction::from_name(&name) {
            return function.parse(context, input, location);
        }
        let function = Self::math_function(context, &name, location)?;
        Self::parse_number_node(context, input, function)
    }

    fn resolve_contextual_leaves(&self, context: &Context) -> Self {
        self.resolve_contextual_leaves_with_base_size(
            context,
            FontBaseSize::CurrentStyle,
            LineHeightBase::CurrentStyle,
        )
    }

    fn resolve_contextual_leaves_with_base_size(
        &self,
        context: &Context,
        base_size: FontBaseSize,
        line_height_base: LineHeightBase,
    ) -> Self {
        self.map_leaves(|leaf| match *leaf {
            Leaf::Length(length) => Leaf::Length(NoCalcLength::from_px(
                length
                    .to_computed_value_in_calc(context, base_size, line_height_base)
                    .px(),
            )),
            Leaf::SiblingIndex => Leaf::Number(context.sibling_index()),
            Leaf::SiblingCount => Leaf::Number(context.sibling_count()),
            _ => leaf.clone(),
        })
    }

    /// Resolves an angle calculation in its element context.
    pub(crate) fn resolve_angle(
        &self,
        context: &Context,
        base_size: FontBaseSize,
        line_height_base: LineHeightBase,
    ) -> Result<CSSFloat, ()> {
        self.resolve_contextual_leaves_with_base_size(context, base_size, line_height_base)
            .resolve_angle_without_context()
    }

    /// Resolves an angle calculation without element context.
    pub fn resolve_angle_without_context(&self) -> Result<CSSFloat, ()> {
        self.to_angle().map(|angle| angle.degrees())
    }

    /// Resolves a time calculation in its element context.
    pub fn resolve_time(&self, context: &Context) -> Result<CSSFloat, ()> {
        self.resolve_contextual_leaves(context)
            .to_time()
            .map(|time| time.seconds())
    }

    /// Resolves a resolution calculation in its element context.
    pub fn resolve_resolution(&self, context: &Context) -> Result<CSSFloat, ()> {
        self.resolve_contextual_leaves(context)
            .to_resolution()
            .map(|resolution| resolution.dppx())
    }

    /// Resolve a `<number>` calculation after converting context-dependent
    /// lengths to their canonical pixel value.
    pub fn resolve_number(&self, context: &Context) -> Result<CSSFloat, ()> {
        self.resolve_contextual_leaves(context).to_number()
    }

    /// Resolve a `<number>` calculation which has no contextual units.
    pub fn resolve_number_without_context(&self) -> Result<CSSFloat, ()> {
        self.to_number()
    }

    /// Resolves a percentage calculation with element-dependent leaves.
    pub fn resolve_percentage(&self, context: &Context) -> Result<CSSFloat, ()> {
        self.resolve_contextual_leaves(context).to_percentage()
    }

    /// Resolves a percentage calculation which has no contextual leaves.
    pub fn resolve_percentage_without_context(&self) -> Result<CSSFloat, ()> {
        self.to_percentage()
    }

    /// Converts a percentage-valued expression into its equivalent number.
    pub fn percentage_as_number(&self) -> Self {
        self.map_leaves(|leaf| match *leaf {
            Leaf::Percentage(value) => Leaf::Number(value),
            _ => leaf.clone(),
        })
    }

    /// Converts a number-valued expression into its equivalent percentage.
    pub fn number_as_percentage(&self) -> Self {
        let children = vec![self.clone(), Self::Leaf(Leaf::Percentage(1.0))];
        let mut node = Self::Product(children.into_boxed_slice().into());
        node.simplify_and_sort();
        node
    }

    /// Returns `100% - self` while retaining contextual leaves.
    pub fn reversed_percentage(&self) -> Self {
        let mut value = self.clone();
        value.negate();
        let children = vec![Self::Leaf(Leaf::Percentage(1.0)), value];
        let mut node = Self::Sum(children.into_boxed_slice().into());
        node.simplify_and_sort();
        node
    }

    /// Convenience parsing function for `<number>`.
    pub fn parse_number<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
    ) -> Result<CSSFloat, ParseError<'i>> {
        Self::parse_number_node(context, input, function)?
            .to_number()
            .map_err(|()| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }

    fn parse_typed_node<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
        unit: CalcUnits,
    ) -> Result<Self, ParseError<'i>> {
        let mut node = Self::parse(context, input, function, AllowParse::new(unit))?;
        if node.unit() != Ok(unit) {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        node.simplify_and_sort();
        Ok(node)
    }

    /// Convenience parsing function for `<angle>`.
    pub fn parse_angle<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
    ) -> Result<Angle, ParseError<'i>> {
        Self::parse_typed_node(context, input, function, CalcUnits::ANGLE)
            .map(Angle::from_calc_node)
    }

    /// Convenience parsing function for a mixed `<angle-percentage>`.
    pub fn parse_angle_percentage<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
    ) -> Result<CalcAnglePercentage, ParseError<'i>> {
        let node = Self::parse(
            context,
            input,
            function,
            AllowParse::new(CalcUnits::ANGLE_PERCENTAGE),
        )?;
        CalcAnglePercentage::from_node(node)
            .map_err(|()| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }

    /// Convenience parsing function for `<time>`.
    pub fn parse_time<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        clamping_mode: AllowedNumericType,
        function: MathFunction,
    ) -> Result<Time, ParseError<'i>> {
        Self::parse_typed_node(context, input, function, CalcUnits::TIME)
            .map(|node| Time::from_calc_node(node, clamping_mode))
    }

    /// Convenience parsing function for `<resolution>`.
    pub fn parse_resolution<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        function: MathFunction,
    ) -> Result<Resolution, ParseError<'i>> {
        Self::parse_typed_node(context, input, function, CalcUnits::RESOLUTION)
            .map(Resolution::from_calc_node)
    }
}

#[cfg(test)]
mod tree_counting_tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::font_metrics::FontMetrics;
    use crate::media_queries::MediaType;
    use crate::properties::{style_structs::Font, ComputedValues};
    use crate::queries::values::PrefersColorScheme;
    use crate::servo::media_queries::{Device, FontMetricsProvider};
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use crate::values::computed::font::GenericFontFamily;
    use crate::values::computed::{CSSPixelLength, Length, ToComputedValue};
    use crate::values::specified;
    use cssparser::{Parser, ParserInput};
    use euclid::{Scale, Size2D};
    use style_traits::ParsingMode;
    use style_traits::{CSSPixel, DevicePixel};

    #[derive(Debug)]
    struct TestFontMetricsProvider;

    impl FontMetricsProvider for TestFontMetricsProvider {
        fn query_font_metrics(
            &self,
            _vertical: bool,
            _font: &Font,
            _base_size: CSSPixelLength,
            _flags: crate::values::specified::font::QueryFontMetricsFlags,
        ) -> FontMetrics {
            FontMetrics::default()
        }

        fn base_size_for_generic(&self, _generic: GenericFontFamily) -> Length {
            Length::new(16.0)
        }
    }

    fn context() -> ParserContext<'static> {
        context_for(CssRuleType::Style)
    }

    fn context_for(rule_type: CssRuleType) -> ParserContext<'static> {
        let url_data = Box::leak(Box::new(UrlExtraData::from(
            url::Url::parse("https://example.invalid/").unwrap(),
        )));
        ParserContext::new(
            Origin::Author,
            url_data,
            Some(rule_type),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        )
    }

    fn parse_length_percentage(css: &str) -> Result<specified::LengthPercentage, ParseError<'_>> {
        let mut input = ParserInput::new(css);
        Parser::new(&mut input)
            .parse_entirely(|input| specified::LengthPercentage::parse(&context(), input))
    }

    fn compute_percentage(css: &str) -> f32 {
        let mut input = ParserInput::new(css);
        let specified = Parser::new(&mut input)
            .parse_entirely(|input| specified::Percentage::parse(&context(), input))
            .expect("the percentage calculation must parse");
        with_computed_context(|context| specified.to_computed_value(context).0)
    }

    fn with_computed_context<R>(evaluate: impl FnOnce(&Context) -> R) -> R {
        let initial_values =
            ComputedValues::initial_values_with_font_override(Font::initial_values());
        let device = Device::new(
            MediaType::print(),
            QuirksMode::NoQuirks,
            Size2D::<f32, CSSPixel>::new(800.0, 600.0),
            Scale::<f32, CSSPixel, DevicePixel>::new(1.0),
            Box::new(TestFontMetricsProvider),
            initial_values,
            PrefersColorScheme::Light,
        );
        crate::values::computed::Context::for_media_query_evaluation(
            &device,
            QuirksMode::NoQuirks,
            evaluate,
        )
    }

    #[test]
    fn percentage_sign_resolves_contextual_lengths_before_multiplication() {
        assert_eq!(compute_percentage("calc(sign(20rem - 20px) * 180%)"), 1.8,);
    }

    #[test]
    fn resolution_calculations_retain_context_until_computation() {
        for (css, expected, dppx) in [
            (
                "calc(1dppx * sibling-index())",
                "calc(1dppx * sibling-index())",
                None,
            ),
            (
                "calc(1dppx * sign(1em - 10px))",
                "calc(1dppx * sign(1em - 10px))",
                Some(1.0),
            ),
            (
                "calc(1dppx * sign(1em - 10000px))",
                "calc(1dppx * sign(1em - 10000px))",
                Some(0.0),
            ),
            ("96dpi", "96dpi", Some(1.0)),
            ("calc(96dpi)", "calc(1dppx)", Some(1.0)),
            (
                "calc(infinity * 1dppx)",
                "calc(infinity * 1dppx)",
                Some(f32::MAX),
            ),
        ] {
            let mut input = ParserInput::new(css);
            let resolution = Parser::new(&mut input)
                .parse_entirely(|input| Resolution::parse(&context(), input))
                .unwrap_or_else(|_| panic!("{css} must parse"));
            assert_eq!(resolution.to_css_string(), expected);
            if let Some(dppx) = dppx {
                assert_eq!(
                    with_computed_context(|context| resolution.to_computed_value(context).dppx()),
                    dppx,
                    "{css}"
                );
            }
        }
    }

    #[test]
    fn contextual_time_and_angle_values_retain_their_calculations() {
        for (css, expected, time) in [
            (
                "calc(1s / sign(1em - 20px))",
                "calc(1s / sign(1em - 20px))",
                true,
            ),
            (
                "calc(1deg / sign(1em - 20px))",
                "calc(1deg / sign(1em - 20px))",
                false,
            ),
            ("calc(1000ms)", "calc(1s)", true),
            ("calc(1turn)", "calc(360deg)", false),
            ("1000ms", "1000ms", true),
            ("1turn", "1turn", false),
            (
                "calc(sibling-index() * 2rad * pi)",
                "calc(360deg * sibling-index())",
                false,
            ),
            (
                "calc(sibling-count() * 1s)",
                "calc(1s * sibling-count())",
                true,
            ),
        ] {
            let mut input = ParserInput::new(css);
            let mut input = Parser::new(&mut input);
            let serialised = if time {
                Time::parse(&context(), &mut input).unwrap().to_css_string()
            } else {
                Angle::parse(&context(), &mut input)
                    .unwrap()
                    .to_css_string()
            };
            assert_eq!(serialised, expected);
            input.expect_exhausted().unwrap();
        }
    }

    #[test]
    fn contextual_dimensions_resolve_before_top_level_range_clamping() {
        with_computed_context(|computed_context| {
            for (expression, seconds, degrees) in [
                ("sign(10000px - 1em)", 1.0, 1.0_f32),
                ("sign(1em - 10000px)", -1.0, -1.0),
                ("1 / sign(1em - 1em)", f32::MAX, 0.0),
                ("-1 / sign(1em - 1em)", f32::MIN, 0.0),
                ("infinity - infinity", 0.0, 0.0),
                ("-0", 0.0, 0.0),
            ] {
                let css = format!("calc(1s * ({expression}))");
                let mut input = ParserInput::new(&css);
                let time = Time::parse(&context(), &mut Parser::new(&mut input)).unwrap();
                assert_eq!(
                    time.to_computed_value(computed_context).seconds().to_bits(),
                    seconds.to_bits(),
                    "{css}"
                );

                let css = format!("calc(1deg * ({expression}))");
                let mut input = ParserInput::new(&css);
                let angle = Angle::parse(&context(), &mut Parser::new(&mut input)).unwrap();
                assert_eq!(
                    angle
                        .to_computed_value(computed_context)
                        .degrees()
                        .to_bits(),
                    degrees.to_bits(),
                    "{css}"
                );
            }

            let mut input = ParserInput::new("calc(1s * sign(1em - 10000px))");
            let duration =
                Time::parse_non_negative(&context(), &mut Parser::new(&mut input)).unwrap();
            assert_eq!(duration.to_computed_value(computed_context).seconds(), 0.0);

            let mut input = ParserInput::new("oblique calc(10deg / sign(1em - 10000px))");
            let descriptor = crate::font_face::FontStyle::parse(
                &context_for(CssRuleType::FontFace),
                &mut Parser::new(&mut input),
            )
            .unwrap();
            assert!(matches!(
                descriptor.compute(computed_context),
                crate::font_face::ComputedFontStyleDescriptor::Oblique(-10.0, -10.0)
            ));
        });
    }

    #[test]
    fn deferred_dimensions_reject_incompatible_result_types() {
        for css in [
            "calc(1s + 1deg)",
            "calc(1em)",
            "calc(1px * 1px)",
            "calc(sign(1em))",
        ] {
            let mut input = ParserInput::new(css);
            assert!(
                Time::parse(&context(), &mut Parser::new(&mut input)).is_err(),
                "{css}"
            );
            let mut input = ParserInput::new(css);
            assert!(
                Angle::parse(&context(), &mut Parser::new(&mut input)).is_err(),
                "{css}"
            );
        }
    }

    #[test]
    fn nan_comparisons_retain_unresolved_units_in_specified_values() {
        for (css, expected) in [
            (
                "calc(1 * min(NaN * 2px, NaN * 4em))",
                "calc(1 * min(NaN * 1px, NaN * 1em))",
            ),
            (
                "calc(1 * clamp(NaN * 2em, NaN * 4px, NaN * 8pt))",
                "calc(1 * clamp(NaN * 1em, NaN * 1px, NaN * 1px))",
            ),
        ] {
            assert_eq!(
                parse_length_percentage(css).unwrap().to_css_string(),
                expected
            );
        }
    }

    #[test]
    fn percentage_progress_resolves_contextual_lengths_before_multiplication() {
        assert_eq!(
            compute_percentage("calc(progress(10rem, 20px, 100px) * 180%)"),
            1.8,
        );
        assert_eq!(
            compute_percentage("calc(progress(no-clamp 0px, 10px, 20px) * 100%)"),
            -1.0,
        );
        assert_eq!(
            compute_percentage("calc(progress(20px, 10px, 10px) * 100%)"),
            0.0,
        );

        let mut input = ParserInput::new("calc(progress(10px, 1s, 20px) * 100%)");
        assert!(
            Parser::new(&mut input)
                .parse_entirely(|input| specified::Percentage::parse(&context(), input))
                .is_err(),
            "progress() arguments with inconsistent types must be rejected",
        );
    }

    #[test]
    fn length_calculations_retain_tree_counting_functions() {
        for css in ["calc(10px * sibling-index())", "calc(5% * sibling-count())"] {
            let value = parse_length_percentage(css)
                .expect("tree-counting functions must parse in calculations");
            assert_eq!(value.to_css_string(), css);
        }

        assert!(parse_length_percentage("calc(10px * sibling-index(1))").is_err());
    }

    #[test]
    fn color_retains_tree_counting_calculation() {
        let css = "hsl(calc(50deg * sibling-index()) 50% 50%)";
        let mut input = ParserInput::new(css);
        let value = Parser::new(&mut input)
            .parse_entirely(|input| specified::Color::parse(&context(), input))
            .expect("tree-counting colour components must survive until computed-value time");

        assert_eq!(
            value.to_css_string(),
            "hsl(calc(50deg * sibling-index()) 50 50)"
        );
    }

    #[test]
    fn numeric_values_retain_tree_counting_calculations() {
        for (css, serialised) in [
            ("sibling-index()", "sibling-index()"),
            ("sibling-count()", "sibling-count()"),
            ("calc(sibling-index())", "sibling-index()"),
            ("calc(sibling-count())", "sibling-count()"),
            ("calc(0.5 * sibling-index())", "calc(0.5 * sibling-index())"),
            ("calc(2 * sibling-count())", "calc(2 * sibling-count())"),
            ("hypot(3, sibling-index())", "hypot(3, sibling-index())"),
        ] {
            let mut number_input = ParserInput::new(css);
            let number = Parser::new(&mut number_input)
                .parse_entirely(|input| specified::Number::parse(&context(), input))
                .expect("number calculations must survive until computed-value time");
            assert_eq!(number.to_css_string(), serialised);
            assert!(number.resolve().is_none());

            let mut integer_input = ParserInput::new(css);
            let integer = Parser::new(&mut integer_input)
                .parse_entirely(|input| specified::Integer::parse(&context(), input))
                .expect("integer calculations must survive until computed-value time");
            assert_eq!(integer.to_css_string(), serialised);
            assert!(integer.resolve().is_none());
        }
    }

    #[test]
    fn grid_calculations_clamp_only_the_computed_line_number() {
        for (css, expected) in [
            ("span calc(-2)", "span 1"),
            ("span calc(0)", "span 1"),
            ("span calc(-2) i", "span i"),
            ("span calc(2.5)", "span 3"),
            ("span calc(20000)", "span 10000"),
            ("calc(-20000)", "-10000"),
        ] {
            let mut input = ParserInput::new(css);
            let value = Parser::new(&mut input)
                .parse_entirely(|input| specified::GridLine::parse(&context(), input))
                .unwrap_or_else(|_| panic!("{css} must parse"));
            assert_eq!(value.to_css_string(), css);
            let computed = with_computed_context(|context| value.to_computed_value(context));
            assert_eq!(computed.to_css_string(), expected, "{css}");
        }
    }

    #[test]
    fn grid_lanes_reversal_order_is_canonical_only_after_computation() {
        let css = "row track-reverse fill-reverse";
        let mut input = ParserInput::new(css);
        let value = Parser::new(&mut input)
            .parse_entirely(|input| {
                specified::position::GridLanesDirection::parse(&context(), input)
            })
            .unwrap();
        assert_eq!(value.to_css_string(), css);
        let computed = with_computed_context(|context| value.to_computed_value(context));
        assert_eq!(computed.to_css_string(), "row fill-reverse track-reverse");
    }

    #[test]
    fn text_combine_digit_calculations_round_and_clamp_when_computed() {
        for (css, count) in [
            ("digits calc(3 * 1.0)", 3),
            ("digits calc(2e0 * 2e+0)", 4),
            ("digits calc(2.5)", 3),
            ("digits calc(3 * sign(1em - 1px))", 3),
        ] {
            let mut input = ParserInput::new(css);
            let value = Parser::new(&mut input)
                .parse_entirely(|input| specified::TextCombineUpright::parse(&context(), input))
                .unwrap_or_else(|_| panic!("{css} must parse"));
            let computed = with_computed_context(|context| value.to_computed_value(context));
            assert_eq!(
                computed,
                crate::values::computed::TextCombineUpright::Digits(count),
                "{css}"
            );
        }
    }

    #[test]
    fn percentage_values_retain_tree_counting_calculations() {
        for css in ["calc(10% * sibling-index())", "calc(25% * sibling-count())"] {
            let mut input = ParserInput::new(css);
            let percentage = Parser::new(&mut input)
                .parse_entirely(|input| specified::Percentage::parse(&context(), input))
                .expect("percentage calculations must survive until computed-value time");

            assert_eq!(percentage.to_css_string(), css);
            assert!(percentage.resolve().is_none());
            assert_eq!(
                specified::LengthPercentage::from(percentage.clone()).to_css_string(),
                css
            );
            assert!(percentage.to_number().resolve().is_none());
        }
    }

    #[test]
    fn descriptor_values_reject_tree_counting_calculations() {
        for rule_type in [
            CssRuleType::FontFace,
            CssRuleType::FontFeatureValues,
            CssRuleType::FontPaletteValues,
            CssRuleType::CounterStyle,
            CssRuleType::Page,
        ] {
            let mut number_input = ParserInput::new("sibling-index()");
            assert!(Parser::new(&mut number_input)
                .parse_entirely(|input| specified::Number::parse(&context_for(rule_type), input))
                .is_err());

            let mut percentage_input = ParserInput::new("calc(10% * sibling-index())");
            assert!(Parser::new(&mut percentage_input)
                .parse_entirely(|input| specified::Percentage::parse(
                    &context_for(rule_type),
                    input
                ))
                .is_err());
        }
    }
}
