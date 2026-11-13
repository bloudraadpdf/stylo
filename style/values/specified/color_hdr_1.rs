/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified dynamic-range limits and retained mixing percentages.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::{color_hdr_1 as computed, Context, ToComputedValue};
use crate::values::specified::Percentage;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// The authored HDR limit, including nested weighted mixtures.
#[derive(Clone, Debug, Default, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
pub enum DynamicRangeLimit {
    /// Restrict output to the standard dynamic range.
    Standard,
    /// Restrict output to the constrained dynamic range.
    Constrained,
    /// Use the full dynamic range of the output medium.
    #[default]
    NoLimit,
    /// Combine the limits with their authored percentages.
    #[css(function = "dynamic-range-limit-mix")]
    Mix(Vec<DynamicRangeLimitMixItem>),
}

/// One limit and its percentage in an authored mixture.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
pub struct DynamicRangeLimitMixItem {
    value: DynamicRangeLimit,
    percentage: Percentage,
}

impl Parse for DynamicRangeLimit {
    fn parse<'i, 't>(context: &ParserContext, input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i>> {
        for (keyword, value) in [
            ("standard", Self::Standard),
            ("constrained", Self::Constrained),
            ("no-limit", Self::NoLimit),
        ] {
            if input.try_parse(|input| input.expect_ident_matching(keyword)).is_ok() {
                return Ok(value);
            }
        }
        input.expect_function_matching("dynamic-range-limit-mix")?;
        input.parse_nested_block(|input| {
            let items = input.parse_comma_separated(|input| {
                let value = input.try_parse(|input| Self::parse(context, input)).ok();
                let percentage = Percentage::parse_non_negative(context, input)?;
                if percentage.calc_clamping_mode().is_none() && percentage.resolve().is_some_and(|value| value > 1.0) {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }
                let value = match value {
                    Some(value) => value,
                    None => Self::parse(context, input)?,
                };
                Ok(DynamicRangeLimitMixItem { value, percentage })
            })?;
            if items.len() < 2 || items.iter().all(|item| item.percentage.resolve() == Some(0.0)) {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            Ok(Self::Mix(items))
        })
    }
}

impl ToCss for DynamicRangeLimit {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Standard => dest.write_str("standard"),
            Self::Constrained => dest.write_str("constrained"),
            Self::NoLimit => dest.write_str("no-limit"),
            Self::Mix(items) => {
                dest.write_str("dynamic-range-limit-mix(")?;
                for (index, item) in items.iter().enumerate() {
                    if index != 0 {
                        dest.write_str(", ")?;
                    }
                    item.to_css(dest)?;
                }
                dest.write_char(')')
            }
        }
    }
}

impl ToCss for DynamicRangeLimitMixItem {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        self.value.to_css(dest)?;
        dest.write_char(' ')?;
        self.percentage.to_css(dest)
    }
}

impl ToComputedValue for DynamicRangeLimit {
    type ComputedValue = computed::DynamicRangeLimit;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        match self {
            Self::Standard => computed::DynamicRangeLimit::STANDARD,
            Self::Constrained => computed::DynamicRangeLimit::CONSTRAINED,
            Self::NoLimit => computed::DynamicRangeLimit::NO_LIMIT,
            Self::Mix(items) => computed::DynamicRangeLimit::mix(items.iter().map(|item| {
                let percentage = item.percentage.to_computed_value(context).0.clamp(0.0, 1.0);
                (
                    item.value.to_computed_value(context),
                    crate::values::generics::calc::authored_number(percentage),
                )
            })),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        let contributions = computed.contributions();
        for (index, value) in [Self::Standard, Self::Constrained, Self::NoLimit].into_iter().enumerate() {
            if contributions[index] == 1.0 {
                return value;
            }
        }
        Self::Mix(
            [Self::Standard, Self::Constrained, Self::NoLimit]
                .into_iter()
                .zip(contributions)
                .filter(|(_, percentage)| *percentage > 0.0)
                .map(|(value, percentage)| DynamicRangeLimitMixItem {
                    value,
                    percentage: Percentage::new(percentage as f32),
                })
                .collect(),
        )
    }
}
