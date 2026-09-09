/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified time values.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::time::Time as ComputedTime;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::calc::{CalcNode, Leaf};
use crate::values::CSSFloat;
use crate::Zero;
use cssparser::{match_ignore_ascii_case, Parser, Token};
use std::fmt::{self, Write};
use style_traits::values::specified::AllowedNumericType;
use style_traits::{CssWriter, ParseError, SpecifiedValueInfo, StyleParseErrorKind, ToCss};

/// A literal time dimension.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct TimeDimension {
    seconds: CSSFloat,
    unit: TimeUnit,
}

/// A time unit.
#[derive(Clone, Copy, Debug, Eq, MallocSizeOf, PartialEq, ToShmem)]
pub enum TimeUnit {
    /// Seconds.
    Second,
    /// Milliseconds.
    Millisecond,
}

impl TimeDimension {
    /// Creates a time dimension in seconds.
    pub fn from_seconds(seconds: CSSFloat) -> Self {
        Self {
            seconds,
            unit: TimeUnit::Second,
        }
    }

    /// Returns the time in seconds.
    pub fn seconds(self) -> CSSFloat {
        self.seconds
    }

    /// Parses a literal time dimension.
    pub fn parse_dimension(value: CSSFloat, unit: &str) -> Result<Self, ()> {
        let (seconds, unit) = match_ignore_ascii_case! { unit,
            "s" => (value, TimeUnit::Second),
            "ms" => (value / 1000.0, TimeUnit::Millisecond),
            _ => return Err(())
        };
        Ok(Self { seconds, unit })
    }
}

impl ToCss for TimeDimension {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let (value, unit) = match self.unit {
            TimeUnit::Second => (self.seconds, "s"),
            TimeUnit::Millisecond => (self.seconds * 1000.0, "ms"),
        };
        crate::values::serialize_specified_dimension(value, unit, false, dest)
    }
}

/// A specified time, retained until its calculation context is available.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct Time(TimeValue);

#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
enum TimeValue {
    Dimension(TimeDimension),
    Calc(Box<CalcNode>, AllowedNumericType),
}

impl Time {
    /// Creates a literal time in seconds.
    pub fn from_seconds(seconds: CSSFloat) -> Self {
        Self(TimeValue::Dimension(TimeDimension::from_seconds(seconds)))
    }

    /// Creates a time with its calculation range.
    pub fn from_seconds_with_calc_clamping_mode(
        seconds: CSSFloat,
        clamping_mode: Option<AllowedNumericType>,
    ) -> Self {
        match clamping_mode {
            None => Self::from_seconds(seconds),
            Some(mode) => Self::from_calc_node(
                CalcNode::Leaf(Leaf::Time(TimeDimension::from_seconds(seconds))),
                mode,
            ),
        }
    }

    pub(crate) fn from_calc_node(node: CalcNode, mode: AllowedNumericType) -> Self {
        Self(TimeValue::Calc(Box::new(node), mode))
    }

    fn parse_with_clamping_mode<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        clamping_mode: AllowedNumericType,
    ) -> Result<Self, ParseError<'i>> {
        use style_traits::ParsingMode;

        let location = input.current_source_location();
        match *input.next()? {
            Token::Dimension {
                value, ref unit, ..
            } if clamping_mode.is_ok(ParsingMode::DEFAULT, value) => {
                TimeDimension::parse_dimension(value, unit)
                    .map(|value| Self(TimeValue::Dimension(value)))
                    .map_err(|()| location.new_custom_error(StyleParseErrorKind::UnspecifiedError))
            },
            Token::Function(ref name) => {
                let function = CalcNode::math_function(context, name, location)?;
                CalcNode::parse_time(context, input, clamping_mode, function)
            },
            ref token => Err(location.new_unexpected_token_error(token.clone())),
        }
    }

    /// Parses a non-negative time value.
    pub fn parse_non_negative<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_clamping_mode(context, input, AllowedNumericType::NonNegative)
    }
}

impl Zero for Time {
    fn zero() -> Self {
        Self::from_seconds(0.0)
    }

    fn is_zero(&self) -> bool {
        matches!(self.0, TimeValue::Dimension(value) if value.seconds() == 0.0)
    }
}

impl ToComputedValue for Time {
    type ComputedValue = ComputedTime;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        let seconds = match &self.0 {
            TimeValue::Dimension(value) => value.seconds(),
            TimeValue::Calc(node, mode) => mode.clamp(
                node.resolve_time(context)
                    .expect("a validated time calculation must resolve in its element context"),
            ),
        };
        let seconds = if seconds == 0.0 {
            0.0
        } else {
            crate::values::normalize(seconds).clamp(f32::MIN, f32::MAX)
        };
        ComputedTime::from_seconds(seconds)
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self::from_seconds(computed.seconds())
    }
}

impl Parse for Time {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_clamping_mode(context, input, AllowedNumericType::All)
    }
}

impl ToCss for Time {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match &self.0 {
            TimeValue::Dimension(value) => value.to_css(dest),
            TimeValue::Calc(node, _) => node.to_css(dest),
        }
    }
}

impl SpecifiedValueInfo for Time {}
