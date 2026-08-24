/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified percentages.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::percentage::Percentage as ComputedPercentage;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::generics::NonNegative;
use crate::values::specified::calc::CalcNode;
use crate::values::specified::Number;
use crate::values::{normalize, serialize_percentage, CSSFloat};
use cssparser::{Parser, Token};
use std::fmt::{self, Write};
use style_traits::values::specified::AllowedNumericType;
use style_traits::{CssWriter, ParseError, SpecifiedValueInfo, ToCss, ToTyped, TypedValue};

/// A percentage value.
#[derive(Clone, Debug, Default, MallocSizeOf, ToShmem)]
pub struct Percentage {
    /// The percentage value as a float.
    ///
    /// [0 .. 100%] maps to [0.0 .. 1.0]
    value: CSSFloat,
    /// If this percentage came from a calc() expression, this tells how
    /// clamping should be done on the value.
    calc_clamping_mode: Option<AllowedNumericType>,
    /// An expression which requires computed-value context.
    calc: Option<Box<CalcNode>>,
}

impl PartialEq for Percentage {
    fn eq(&self, other: &Self) -> bool {
        self.calc_clamping_mode == other.calc_clamping_mode
            && self.calc == other.calc
            && (self.value == other.value || (self.value.is_nan() && other.value.is_nan()))
    }
}

impl ToCss for Percentage {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self.calc {
            Some(ref calc) => calc.to_css(dest),
            None => {
                if self.calc_clamping_mode.is_some() {
                    dest.write_str("calc(")?;
                }
                serialize_percentage(self.value, dest)?;
                if self.calc_clamping_mode.is_some() {
                    dest.write_char(')')?;
                }
                Ok(())
            },
        }
    }
}

impl Percentage {
    /// Creates a percentage from a numeric value.
    pub(super) fn new_with_clamping_mode(
        value: CSSFloat,
        calc_clamping_mode: Option<AllowedNumericType>,
    ) -> Self {
        Self {
            value,
            calc_clamping_mode,
            calc: None,
        }
    }

    pub(super) fn from_calc_node(
        node: CalcNode,
        clamping_mode: Option<AllowedNumericType>,
    ) -> Self {
        Self {
            value: CSSFloat::NAN,
            calc_clamping_mode: clamping_mode,
            calc: Some(Box::new(node)),
        }
    }

    /// Creates a percentage from a numeric value.
    pub fn new(value: CSSFloat) -> Self {
        Self::new_with_clamping_mode(value, None)
    }

    /// `0%`
    #[inline]
    pub fn zero() -> Self {
        Percentage {
            value: 0.,
            calc_clamping_mode: None,
            calc: None,
        }
    }

    /// `100%`
    #[inline]
    pub fn hundred() -> Self {
        Percentage {
            value: 1.,
            calc_clamping_mode: None,
            calc: None,
        }
    }

    /// Gets the underlying value for this float.
    pub fn get(&self) -> CSSFloat {
        self.clamp(self.resolve_without_context())
    }

    /// Returns the value when it can be resolved without element context.
    pub fn resolve(&self) -> Option<CSSFloat> {
        let value = match self.calc {
            Some(ref calc) => calc.resolve_percentage_without_context().ok()?,
            None => self.value,
        };
        Some(self.clamp(value))
    }

    fn resolve_without_context(&self) -> CSSFloat {
        self.calc
            .as_ref()
            .and_then(|calc| calc.resolve_percentage_without_context().ok())
            .unwrap_or(self.value)
    }

    fn clamp(&self, value: CSSFloat) -> CSSFloat {
        self.calc_clamping_mode
            .map_or(value, |mode| mode.clamp(value))
    }

    /// Returns this percentage as a number.
    pub fn to_number(&self) -> Number {
        match self.calc {
            Some(ref calc) => {
                Number::from_calc_node(calc.percentage_as_number(), self.calc_clamping_mode)
            },
            None => Number::new_with_clamping_mode(self.value, self.calc_clamping_mode),
        }
    }

    /// Returns the calc() clamping mode for this percentage.
    pub fn calc_clamping_mode(&self) -> Option<AllowedNumericType> {
        self.calc_clamping_mode
    }

    /// Returns the retained calculation tree, if this value is contextual.
    pub(super) fn calc_node(&self) -> Option<&CalcNode> {
        self.calc.as_deref()
    }

    /// Reverses this percentage, preserving calc-ness.
    ///
    /// For example: If it was 20%, convert it into 80%.
    pub fn reverse(&mut self) {
        match self.calc {
            Some(ref mut calc) => **calc = calc.reversed_percentage(),
            None => self.value = 1. - self.value,
        }
    }

    /// Parses a specific kind of percentage.
    pub fn parse_with_clamping_mode<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        num_context: AllowedNumericType,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        match *input.next()? {
            Token::Percentage { unit_value, .. }
                if num_context.is_ok(context.parsing_mode, unit_value) =>
            {
                Ok(Percentage::new(unit_value))
            },
            Token::Function(ref name) => {
                let function = CalcNode::math_function(context, name, location)?;
                let node = CalcNode::parse_percentage_node(context, input, function)?;
                match node.resolve_percentage_without_context() {
                    Ok(value) => Ok(Percentage::new_with_clamping_mode(value, Some(num_context))),
                    Err(()) => Ok(Percentage::from_calc_node(node, Some(num_context))),
                }
            },
            ref t => Err(location.new_unexpected_token_error(t.clone())),
        }
    }

    /// Parses a percentage token, but rejects it if it's negative.
    pub fn parse_non_negative<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_clamping_mode(context, input, AllowedNumericType::NonNegative)
    }

    /// Parses a percentage token, but rejects it if it's negative or more than
    /// 100%.
    pub fn parse_zero_to_a_hundred<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_clamping_mode(context, input, AllowedNumericType::ZeroToOne)
    }

    /// Clamp to 100% if the value is over 100%.
    #[inline]
    pub fn clamp_to_hundred(self) -> Self {
        Percentage {
            value: self.value.min(1.),
            calc_clamping_mode: self.calc_clamping_mode,
            calc: self.calc,
        }
    }
}

impl Parse for Percentage {
    #[inline]
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_clamping_mode(context, input, AllowedNumericType::All)
    }
}

impl ToComputedValue for Percentage {
    type ComputedValue = ComputedPercentage;

    #[inline]
    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        let value = self.calc.as_ref().map_or(self.value, |calc| {
            calc.resolve_percentage(context).unwrap_or(CSSFloat::NAN)
        });
        ComputedPercentage(normalize(self.clamp(value)))
    }

    #[inline]
    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Percentage::new(computed.0)
    }
}

impl SpecifiedValueInfo for Percentage {}

impl ToTyped for Percentage {
    fn to_typed(&self) -> Option<TypedValue> {
        match self.calc {
            Some(ref calc) => calc.to_typed(),
            None => None,
        }
    }
}

/// Turns the percentage into a plain float.
pub trait ToPercentage {
    /// Returns whether this percentage used to be a calc().
    fn is_calc(&self) -> bool {
        false
    }
    /// Turns the percentage into a plain float.
    fn to_percentage(&self) -> CSSFloat;
}

impl ToPercentage for Percentage {
    fn is_calc(&self) -> bool {
        self.calc_clamping_mode.is_some()
    }

    fn to_percentage(&self) -> CSSFloat {
        self.get()
    }
}

/// A wrapper of Percentage, whose value must be >= 0.
pub type NonNegativePercentage = NonNegative<Percentage>;

impl Parse for NonNegativePercentage {
    #[inline]
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(NonNegative(Percentage::parse_non_negative(context, input)?))
    }
}

impl NonNegativePercentage {
    /// Convert to ComputedPercentage, for FontFaceRule size-adjust getter.
    #[inline]
    pub fn compute(&self) -> ComputedPercentage {
        ComputedPercentage(
            self.0
                .resolve()
                .expect("font-face percentages cannot depend on an element"),
        )
    }
}
