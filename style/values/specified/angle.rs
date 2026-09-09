/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified angles.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::angle::Angle as ComputedAngle;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::calc::{CalcNode, Leaf};
use crate::values::specified::length::{FontBaseSize, LineHeightBase};
use crate::values::CSSFloat;
use crate::Zero;
use cssparser::{match_ignore_ascii_case, Parser, Token};
use std::f32::consts::PI;
use std::fmt::{self, Write};
use std::ops::Neg;
use style_traits::{CssWriter, ParseError, SpecifiedValueInfo, ToCss};

/// A specified angle dimension.
#[cfg_attr(feature = "servo", derive(Deserialize, Serialize))]
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, PartialOrd, ToCss, ToShmem)]
pub enum AngleDimension {
    /// An angle with degree unit.
    #[css(dimension)]
    Deg(CSSFloat),
    /// An angle with gradian unit.
    #[css(dimension)]
    Grad(CSSFloat),
    /// An angle with radian unit.
    #[css(dimension)]
    Rad(CSSFloat),
    /// An angle with turn unit.
    #[css(dimension)]
    Turn(CSSFloat),
}

impl SpecifiedValueInfo for AngleDimension {}

impl Zero for AngleDimension {
    fn zero() -> Self {
        AngleDimension::Deg(0.)
    }

    fn is_zero(&self) -> bool {
        self.unitless_value() == 0.0
    }
}

impl AngleDimension {
    fn computed(&self) -> ComputedAngle {
        let degrees = self.degrees();
        ComputedAngle::from_degrees(if degrees.is_finite() && degrees != 0.0 {
            degrees
        } else {
            0.0
        })
    }

    /// Returns the amount of degrees this angle represents.
    #[inline]
    pub fn degrees(&self) -> CSSFloat {
        const DEG_PER_RAD: f32 = 180.0 / PI;
        const DEG_PER_TURN: f32 = 360.0;
        const DEG_PER_GRAD: f32 = 180.0 / 200.0;

        match *self {
            AngleDimension::Deg(d) => d,
            AngleDimension::Rad(rad) => rad * DEG_PER_RAD,
            AngleDimension::Turn(turns) => turns * DEG_PER_TURN,
            AngleDimension::Grad(gradians) => gradians * DEG_PER_GRAD,
        }
    }

    /// Returns the angle in radians.
    pub fn radians(&self) -> CSSFloat {
        self.degrees() * (PI / 180.0)
    }

    /// Parses a literal angle dimension.
    pub fn parse(value: CSSFloat, unit: &str) -> Result<Self, ()> {
        match_ignore_ascii_case! { unit,
            "deg" => Ok(Self::Deg(value)),
            "grad" => Ok(Self::Grad(value)),
            "turn" => Ok(Self::Turn(value)),
            "rad" => Ok(Self::Rad(value)),
            _ => Err(())
        }
    }

    fn unitless_value(&self) -> CSSFloat {
        match *self {
            AngleDimension::Deg(v)
            | AngleDimension::Rad(v)
            | AngleDimension::Turn(v)
            | AngleDimension::Grad(v) => v,
        }
    }

    fn unit(&self) -> &'static str {
        match *self {
            AngleDimension::Deg(_) => "deg",
            AngleDimension::Rad(_) => "rad",
            AngleDimension::Turn(_) => "turn",
            AngleDimension::Grad(_) => "grad",
        }
    }
}

impl ToComputedValue for AngleDimension {
    type ComputedValue = ComputedAngle;

    fn to_computed_value(&self, _: &Context) -> Self::ComputedValue {
        self.computed()
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self::Deg(computed.degrees())
    }
}

/// A specified angle, retained until its calculation context is available.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct Angle(AngleValue);

#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
enum AngleValue {
    Dimension(AngleDimension),
    Calc(Box<CalcNode>),
}

impl Zero for Angle {
    fn zero() -> Self {
        Self::from_degrees(0.0, false)
    }

    fn is_zero(&self) -> bool {
        self.degrees_without_context()
            .is_ok_and(|value| value == 0.0)
    }
}

impl ToCss for Angle {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match &self.0 {
            AngleValue::Dimension(value) => crate::values::serialize_specified_dimension(
                value.unitless_value(),
                value.unit(),
                false,
                dest,
            ),
            AngleValue::Calc(node) => node.to_css(dest),
        }
    }
}

impl ToComputedValue for Angle {
    type ComputedValue = ComputedAngle;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        self.to_computed_value_with_base_size(
            context,
            FontBaseSize::CurrentStyle,
            LineHeightBase::CurrentStyle,
        )
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self::from_degrees(computed.degrees(), false)
    }
}

impl Angle {
    pub(crate) fn to_computed_value_with_base_size(
        &self,
        context: &Context,
        base_size: FontBaseSize,
        line_height_base: LineHeightBase,
    ) -> ComputedAngle {
        let degrees = match &self.0 {
            AngleValue::Dimension(value) => value.degrees(),
            AngleValue::Calc(node) => node
                .resolve_angle(context, base_size, line_height_base)
                .expect("a validated angle calculation must resolve in its element context"),
        };
        AngleDimension::Deg(degrees).to_computed_value(context)
    }

    /// Creates an angle with the given value in degrees.
    pub fn from_degrees(value: CSSFloat, was_calc: bool) -> Self {
        if was_calc {
            Self::from_calc_node(CalcNode::Leaf(Leaf::Angle(AngleDimension::Deg(value))))
        } else {
            Self(AngleValue::Dimension(AngleDimension::Deg(value)))
        }
    }

    /// Creates an angle with the given value in radians.
    pub fn from_radians(value: CSSFloat) -> Self {
        Self(AngleValue::Dimension(AngleDimension::Rad(value)))
    }

    /// Returns a zero angle.
    pub fn zero() -> Self {
        <Self as Zero>::zero()
    }

    /// Resolves an angle which does not require element context.
    pub fn degrees_without_context(&self) -> Result<CSSFloat, ()> {
        match &self.0 {
            AngleValue::Dimension(value) => Ok(value.degrees()),
            AngleValue::Calc(node) => node.resolve_angle_without_context(),
        }
    }

    /// Computes an angle which does not require element context.
    pub fn to_computed_value_without_context(&self) -> Result<ComputedAngle, ()> {
        self.degrees_without_context()
            .map(|degrees| AngleDimension::Deg(degrees).computed())
    }

    /// Whether this specified angle came from a calculation.
    pub fn was_calc(&self) -> bool {
        matches!(self.0, AngleValue::Calc(_))
    }

    pub(crate) fn from_calc_node(node: CalcNode) -> Self {
        Self(AngleValue::Calc(Box::new(node)))
    }

    /// Creates an angle from a resolved calculation.
    pub fn from_calc(degrees: CSSFloat) -> Self {
        Self::from_degrees(degrees, true)
    }

    /// Parses a literal angle dimension.
    pub fn parse_dimension(value: CSSFloat, unit: &str, was_calc: bool) -> Result<Self, ()> {
        let value = AngleDimension::parse(value, unit)?;
        Ok(if was_calc {
            Self::from_calc_node(CalcNode::Leaf(Leaf::Angle(value)))
        } else {
            Self(AngleValue::Dimension(value))
        })
    }

    /// Parses an angle, including unitless zero where permitted by its property.
    pub fn parse_with_unitless<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_internal(context, input, AllowUnitlessZeroAngle::Yes)
    }

    pub(super) fn parse_internal<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        allow_unitless_zero: AllowUnitlessZeroAngle,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let token = input.next()?;
        match *token {
            Token::Dimension {
                value, ref unit, ..
            } => Self::parse_dimension(value, unit, false)
                .map_err(|()| location.new_unexpected_token_error(token.clone())),
            Token::Function(ref name) => {
                let function = CalcNode::math_function(context, name, location)?;
                CalcNode::parse_angle(context, input, function)
            },
            Token::Number { value: 0.0, .. }
                if matches!(allow_unitless_zero, AllowUnitlessZeroAngle::Yes) =>
            {
                Ok(Self::zero())
            },
            ref token => Err(location.new_unexpected_token_error(token.clone())),
        }
    }
}

/// Whether a property's grammar permits unitless zero angles.
#[allow(missing_docs)]
pub enum AllowUnitlessZeroAngle {
    Yes,
    No,
}

impl Parse for Angle {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_internal(context, input, AllowUnitlessZeroAngle::No)
    }
}

impl SpecifiedValueInfo for Angle {}

impl Neg for Angle {
    type Output = Self;

    fn neg(self) -> Self {
        match self.0 {
            AngleValue::Dimension(value) => {
                let value = match value {
                    AngleDimension::Deg(value) => AngleDimension::Deg(-value),
                    AngleDimension::Grad(value) => AngleDimension::Grad(-value),
                    AngleDimension::Rad(value) => AngleDimension::Rad(-value),
                    AngleDimension::Turn(value) => AngleDimension::Turn(-value),
                };
                Self(AngleValue::Dimension(value))
            },
            AngleValue::Calc(mut node) => {
                node.negate();
                Self(AngleValue::Calc(node))
            },
        }
    }
}
