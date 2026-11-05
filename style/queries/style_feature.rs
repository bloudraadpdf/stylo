/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Container style features:
//!
//! https://drafts.csswg.org/css-conditional-5/#typedef-style-feature

use super::feature_expression::Operator;
use crate::custom_properties::{self, ComputedCustomProperties, Name, SpecifiedValue};
use crate::derives::*;
use crate::dom::AttributeTracker;
use crate::parser::{Parse, ParserContext};
use crate::properties::CSSWideKeyword;
use crate::properties_and_values::value::ComputedValue as ComputedRegisteredValue;
use crate::stylesheets::{CssRuleType, Origin};
use crate::values::computed::{self, ToComputedValue};
use crate::values::specified::{Angle, Length, Number, Percentage, Resolution, Time};
use cssparser::{Parser, ParserInput, Token};
use selectors::kleene_value::KleeneValue;
use servo_arc::Arc;
use std::cmp::Ordering;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ParsingMode, StyleParseErrorKind, ToCss};

/// A `<style-range-value>`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub enum StyleRangeValue {
    /// A `<custom-property-name>`, substituted as if wrapped in `var()`.
    Property(Name),
    /// A `<style-feature-value>`.
    Value(#[ignore_malloc_size_of = "Arc"] Arc<SpecifiedValue>),
}

/// A `<style-feature>`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub enum StyleFeature {
    /// A `<style-feature-plain>`, or a `<style-feature-boolean>` without a value.
    Declaration {
        /// The custom property name.
        name: Name,
        /// The queried value.
        #[ignore_malloc_size_of = "Arc"]
        value: Option<Arc<SpecifiedValue>>,
    },
    /// A `<style-range>`.
    Range {
        /// The first value.
        first: StyleRangeValue,
        /// The first comparison and the second value.
        comparison: (Operator, StyleRangeValue),
        /// The second comparison and the third value.
        second: Option<(Operator, StyleRangeValue)>,
    },
}

fn parse_property_name<'i>(input: &mut Parser<'i, '_>) -> Result<Name, ParseError<'i>> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?;
    custom_properties::parse_name(ident)
        .map(Name::from)
        .map_err(|()| location.new_custom_error(StyleParseErrorKind::UnspecifiedError))
}

fn write_property_name<W: Write>(name: &Name, dest: &mut CssWriter<W>) -> fmt::Result {
    dest.write_str("--")?;
    crate::values::serialize_atom_identifier(name, dest)
}

impl ToCss for StyleRangeValue {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Property(name) => write_property_name(name, dest),
            Self::Value(value) => value.to_css(dest),
        }
    }
}

impl ToCss for StyleFeature {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Declaration { name, value } => {
                write_property_name(name, dest)?;
                if let Some(value) = value {
                    dest.write_str(": ")?;
                    value.to_css(dest)?;
                }
                Ok(())
            },
            Self::Range {
                first,
                comparison,
                second,
            } => {
                first.to_css(dest)?;
                for (operator, value) in std::iter::once(comparison).chain(second) {
                    dest.write_char(' ')?;
                    operator.to_css(dest)?;
                    dest.write_char(' ')?;
                    value.to_css(dest)?;
                }
                Ok(())
            },
        }
    }
}

impl StyleRangeValue {
    /// Parses the tokens up to the next `<mf-lt>`, `<mf-gt>` or `<mf-eq>`.
    fn parse<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, ParseError<'i>> {
        input.skip_whitespace();
        let start = input.position();
        loop {
            let state = input.state();
            if matches!(
                input.next_including_whitespace_and_comments(),
                Err(_) | Ok(&Token::Delim('<' | '>' | '='))
            ) {
                input.reset(&state);
                break;
            }
        }
        let css = input.slice_from(start);
        let mut value_input = ParserInput::new(css);
        Parser::new(&mut value_input)
            .parse_entirely(|input| {
                if let Ok(name) = input.try_parse(|input| {
                    let name = parse_property_name(input)?;
                    input.expect_exhausted()?;
                    Ok::<_, ParseError>(name)
                }) {
                    return Ok(Self::Property(name));
                }
                let value = SpecifiedValue::parse(input, &context.url_data, &context.namespaces)?;
                if value.css_text().is_empty() {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }
                Ok(Self::Value(Arc::new(value)))
            })
            .map_err(|_| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
    }
}

impl StyleFeature {
    /// Parses a `<style-feature>`.
    pub fn parse<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, ParseError<'i>> {
        input
            .try_parse(|input| Self::parse_declaration(context, input))
            .or_else(|_| Self::parse_range(context, input))
    }

    fn parse_declaration<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, ParseError<'i>> {
        let name = parse_property_name(input)?;
        let value = if input.try_parse(|input| input.expect_colon()).is_ok() {
            let value = SpecifiedValue::parse(input, &context.url_data, &context.namespaces)?;
            let _ = input.try_parse(cssparser::parse_important);
            Some(Arc::new(value))
        } else {
            None
        };
        input.expect_exhausted()?;
        Ok(Self::Declaration { name, value })
    }

    fn parse_range<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, ParseError<'i>> {
        let first = StyleRangeValue::parse(context, input)?;
        let first_operator = Operator::parse(input)?;
        let middle = StyleRangeValue::parse(context, input)?;
        let second = match input.try_parse(Operator::parse) {
            Ok(operator) if first_operator.is_compatible_with(operator) => {
                Some((operator, StyleRangeValue::parse(context, input)?))
            },
            Ok(_) => return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
            Err(_) => None,
        };
        input.expect_exhausted()?;
        Ok(Self::Range {
            first,
            comparison: (first_operator, middle),
            second,
        })
    }

    /// Evaluates the feature against the query container of `context`.
    pub fn matches(&self, context: &computed::Context) -> KleeneValue {
        let Some(container) = StyleQueryContainer::new(context) else {
            return KleeneValue::Unknown;
        };
        KleeneValue::from(match self {
            Self::Declaration { name, value } => {
                container.declaration_matches(name, value.as_ref())
            },
            Self::Range {
                first,
                comparison,
                second,
            } => container.range_matches(first, comparison, second.as_ref()),
        })
    }
}

/// A computed `<style-range-value>`.
#[derive(Clone, Copy)]
enum RangeOperand {
    Number(f32),
    Percentage(f32),
    Length(f32),
    Angle(f32),
    Time(f32),
    Resolution(f32),
}

impl RangeOperand {
    fn compare(self, other: Self) -> Option<Ordering> {
        use RangeOperand::*;
        match (self, other) {
            (Number(a), Number(b))
            | (Percentage(a), Percentage(b))
            | (Length(a), Length(b))
            | (Angle(a), Angle(b))
            | (Time(a), Time(b))
            | (Resolution(a), Resolution(b)) => a.partial_cmp(&b),
            (Number(zero), Length(b)) if zero == 0. => zero.partial_cmp(&b),
            (Length(a), Number(zero)) if zero == 0. => a.partial_cmp(&zero),
            _ => None,
        }
    }

    fn parse<'i>(
        context: &ParserContext,
        computed: &computed::Context,
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(number) = input.try_parse(|input| Number::parse(context, input)) {
            return Ok(Self::Number(number.to_computed_value(computed)));
        }
        if let Ok(percentage) = input.try_parse(|input| Percentage::parse(context, input)) {
            return Ok(Self::Percentage(percentage.to_computed_value(computed).0));
        }
        if let Ok(length) = input.try_parse(|input| Length::parse(context, input)) {
            return Ok(Self::Length(length.to_computed_value(computed).px()));
        }
        if let Ok(angle) = input.try_parse(|input| Angle::parse(context, input)) {
            return Ok(Self::Angle(angle.to_computed_value(computed).degrees()));
        }
        if let Ok(time) = input.try_parse(|input| Time::parse(context, input)) {
            return Ok(Self::Time(time.to_computed_value(computed).seconds()));
        }
        let resolution = Resolution::parse(context, input)?;
        Ok(Self::Resolution(
            resolution.to_computed_value(computed).dppx(),
        ))
    }
}

/// The query container, as seen from a style query evaluation context.
struct StyleQueryContainer<'a, 'b> {
    context: &'a computed::Context<'b>,
    stylist: &'a crate::stylist::Stylist,
    container: &'a computed::StyleContainer<'b>,
}

impl<'a, 'b> StyleQueryContainer<'a, 'b> {
    fn new(context: &'a computed::Context<'b>) -> Option<Self> {
        Some(Self {
            context,
            stylist: context.builder.stylist?,
            container: context.style_container.as_ref()?,
        })
    }

    fn custom_properties(&self) -> &'a ComputedCustomProperties {
        self.context.inherited_custom_properties()
    }

    fn get(
        &self,
        custom_properties: &'a ComputedCustomProperties,
        name: &Name,
    ) -> Option<&'a ComputedRegisteredValue> {
        custom_properties.get(self.stylist.get_custom_property_registration(name), name)
    }

    fn initial(&self, name: &Name) -> Option<&'a ComputedRegisteredValue> {
        self.get(self.stylist.get_custom_property_initial_values(), name)
    }

    fn inherited(&self, name: &Name) -> Option<&'a ComputedRegisteredValue> {
        match self.container.parent_style {
            Some(ref parent) => self.get(parent.custom_properties(), name),
            None => self.initial(name),
        }
    }

    /// Replaces `var()`, `attr()` and `env()` against the container.
    fn substitute(&self, value: &SpecifiedValue) -> Option<SpecifiedValue> {
        if !value.has_references() {
            return Some(value.clone());
        }
        let mut attributes = AttributeTracker::new(self.container.attributes);
        custom_properties::substitute(
            value,
            self.custom_properties(),
            self.stylist,
            self.context,
            self.context.scope,
            &mut attributes,
        )
        .ok()
        .map(|substituted| substituted.into_value(&value.url_data))
    }

    fn declaration_matches(&self, name: &Name, value: Option<&Arc<SpecifiedValue>>) -> bool {
        let current = self.get(self.custom_properties(), name);
        let Some(value) = value else {
            return current != self.initial(name);
        };
        let mut keyword_input = ParserInput::new(value.css_text());
        let queried = match CSSWideKeyword::parse(&mut Parser::new(&mut keyword_input)) {
            Ok(keyword) => match keyword {
                CSSWideKeyword::Initial => self.initial(name).cloned(),
                CSSWideKeyword::Inherit => self.inherited(name).cloned(),
                CSSWideKeyword::Unset => {
                    if self
                        .stylist
                        .get_custom_property_registration(name)
                        .inherits()
                    {
                        self.inherited(name).cloned()
                    } else {
                        self.initial(name).cloned()
                    }
                },
                CSSWideKeyword::Revert
                | CSSWideKeyword::RevertLayer
                | CSSWideKeyword::RevertRule => return false,
            },
            Err(()) => match self.substitute(value) {
                Some(value) => {
                    let registration = self.stylist.get_custom_property_registration(name);
                    match custom_properties::compute_variable_value(
                        &Arc::new(value),
                        registration,
                        self.context,
                    ) {
                        Some(value) => Some(value),
                        None => return false,
                    }
                },
                None => None,
            },
        };
        queried.as_ref() == current
    }

    fn operand(&self, value: &StyleRangeValue) -> Option<RangeOperand> {
        let value = match value {
            StyleRangeValue::Property(name) => self
                .get(self.custom_properties(), name)?
                .to_variable_value(),
            StyleRangeValue::Value(value) => self.substitute(value)?,
        };
        let context = ParserContext::new(
            Origin::Author,
            &value.url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            self.context.quirks_mode,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(value.css_text());
        Parser::new(&mut input)
            .parse_entirely(|input| RangeOperand::parse(&context, self.context, input))
            .ok()
    }

    fn range_matches(
        &self,
        first: &StyleRangeValue,
        comparison: &(Operator, StyleRangeValue),
        second: Option<&(Operator, StyleRangeValue)>,
    ) -> bool {
        let Some(mut left) = self.operand(first) else {
            return false;
        };
        for (operator, value) in std::iter::once(comparison).chain(second) {
            let Some(right) = self.operand(value) else {
                return false;
            };
            if !left
                .compare(right)
                .is_some_and(|ordering| operator.evaluate(ordering))
            {
                return false;
            }
            left = right;
        }
        true
    }
}
