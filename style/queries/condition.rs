/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A query condition:
//!
//! https://drafts.csswg.org/mediaqueries-4/#typedef-media-condition
//! https://drafts.csswg.org/css-contain-3/#typedef-container-condition

use super::style_feature::StyleFeature;
use super::{FeatureFlags, FeatureType, QueryFeatureExpression};
use crate::derives::*;
use crate::stylesheets::CustomMediaEvaluator;
use crate::values::{computed, AtomString, DashedIdent};
use crate::{error_reporting::ContextualParseError, parser::Parse, parser::ParserContext};
use cssparser::{match_ignore_ascii_case, Parser, SourcePosition, Token};
use selectors::kleene_value::KleeneValue;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// A binary `and` or `or` operator.
#[derive(Clone, Copy, Debug, Eq, MallocSizeOf, Parse, PartialEq, ToCss, ToShmem)]
#[allow(missing_docs)]
pub enum Operator {
    And,
    Or,
}

/// Whether to allow an `or` condition or not during parsing.
#[derive(Clone, Copy, Debug, Eq, MallocSizeOf, PartialEq, ToCss)]
enum AllowOr {
    Yes,
    No,
}

/// The `<scroll-state-query>` of a `scroll-state()` query.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub enum ScrollStateQuery {
    /// A bare `<scroll-state-feature>`.
    Feature(QueryFeatureExpression),
    /// Any other `<scroll-state-query>`.
    Condition(Box<QueryCondition>),
}

impl ToCss for ScrollStateQuery {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: fmt::Write,
    {
        dest.write_str("scroll-state")?;
        match *self {
            // NOTE: QueryFeatureExpression already includes the parenthesis.
            Self::Feature(ref f) => f.to_css(dest),
            Self::Condition(ref c) => {
                dest.write_char('(')?;
                c.to_css(dest)?;
                dest.write_char(')')
            },
        }
    }
}

/// A boolean value for a pref query.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    Eq,
    Parse,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToShmem,
)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum BoolValue {
    False,
    True,
}

/// Simple values we support for -moz-pref(). We don't want to deal with calc() and other
/// shenanigans for now.
#[derive(
    Clone,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToShmem,
)]
#[repr(u8)]
pub enum MozPrefFeatureValue<I> {
    /// No pref value, implicitly bool, but also used to represent missing prefs.
    #[css(skip)]
    None,
    /// A bool value.
    Boolean(BoolValue),
    /// An integer value, useful for int prefs.
    Integer(I),
    /// A string pref value.
    String(crate::values::AtomString),
}

type SpecifiedMozPrefFeatureValue = MozPrefFeatureValue<crate::values::specified::Integer>;
/// The computed -moz-pref() value.
pub type ComputedMozPrefFeatureValue = MozPrefFeatureValue<crate::values::computed::Integer>;

/// A custom -moz-pref(<name>, <value>) query feature.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct MozPrefFeature {
    name: crate::values::AtomString,
    value: SpecifiedMozPrefFeatureValue,
}

impl MozPrefFeature {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        feature_type: FeatureType,
    ) -> Result<Self, ParseError<'i>> {
        use crate::parser::Parse;
        if !context.chrome_rules_enabled() || feature_type != FeatureType::Media {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        let name = AtomString::parse(context, input)?;
        let value = if input.try_parse(|i| i.expect_comma()).is_ok() {
            SpecifiedMozPrefFeatureValue::parse(context, input)?
        } else {
            SpecifiedMozPrefFeatureValue::None
        };
        Ok(Self { name, value })
    }

    #[cfg(feature = "gecko")]
    fn matches(&self, ctx: &computed::Context) -> KleeneValue {
        use crate::values::computed::ToComputedValue;
        let value = self.value.to_computed_value(ctx);
        KleeneValue::from(unsafe {
            crate::gecko_bindings::bindings::Gecko_EvalMozPrefFeature(self.name.as_ptr(), &value)
        })
    }

    #[cfg(feature = "servo")]
    fn matches(&self, _: &computed::Context) -> KleeneValue {
        KleeneValue::Unknown
    }
}

impl ToCss for MozPrefFeature {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: fmt::Write,
    {
        self.name.to_css(dest)?;
        if !matches!(self.value, MozPrefFeatureValue::None) {
            dest.write_str(", ")?;
            self.value.to_css(dest)?;
        }
        Ok(())
    }
}

/// Represents a condition.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub enum QueryCondition {
    /// A simple feature expression, implicitly parenthesized.
    Feature(QueryFeatureExpression),
    /// A custom media query reference in a boolean context, implicitly parenthesized.
    Custom(DashedIdent),
    /// A negation of a condition.
    Not(Box<QueryCondition>),
    /// A set of joint operations.
    Operation(Box<[QueryCondition]>, Operator),
    /// A condition wrapped in parenthesis.
    InParens(Box<QueryCondition>),
    /// A `style()` query.
    Style(Box<QueryCondition>),
    /// A `<style-feature>` inside a `style()` query.
    StyleFeature(StyleFeature),
    /// A `scroll-state()` query.
    ScrollState(ScrollStateQuery),
    /// A -moz-pref() query.
    MozPref(MozPrefFeature),
    /// [ <function-token> <any-value>? ) ] | [ ( <any-value>? ) ]
    GeneralEnclosed(String),
}

impl ToCss for QueryCondition {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: fmt::Write,
    {
        match *self {
            // NOTE(emilio): QueryFeatureExpression already includes the
            // parenthesis.
            QueryCondition::Feature(ref f) => f.to_css(dest),
            QueryCondition::Custom(ref name) => {
                dest.write_char('(')?;
                name.to_css(dest)?;
                dest.write_char(')')
            },
            QueryCondition::Not(ref c) => {
                dest.write_str("not ")?;
                c.to_css(dest)
            },
            QueryCondition::InParens(ref c) => {
                dest.write_char('(')?;
                c.to_css(dest)?;
                dest.write_char(')')
            },
            QueryCondition::Style(ref c) => {
                dest.write_str("style(")?;
                c.to_css(dest)?;
                dest.write_char(')')
            },
            QueryCondition::StyleFeature(ref f) => f.to_css(dest),
            QueryCondition::ScrollState(ref q) => q.to_css(dest),
            QueryCondition::MozPref(ref c) => {
                dest.write_str("-moz-pref(")?;
                c.to_css(dest)?;
                dest.write_char(')')
            },
            QueryCondition::Operation(ref list, op) => {
                let mut iter = list.iter();
                iter.next().unwrap().to_css(dest)?;
                for item in iter {
                    dest.write_char(' ')?;
                    op.to_css(dest)?;
                    dest.write_char(' ')?;
                    item.to_css(dest)?;
                }
                Ok(())
            },
            QueryCondition::GeneralEnclosed(ref s) => dest.write_str(&s),
        }
    }
}

/// <https://drafts.csswg.org/css-syntax-3/#typedef-any-value>
fn serialize_any_value<'i, 't>(
    input: &mut Parser<'i, 't>,
    output: &mut String,
) -> Result<(), ParseError<'i>> {
    while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
        if token.is_parse_error() {
            return Err(input.new_unexpected_token_error(token));
        }
        let closing = match token {
            Token::Function(_) | Token::ParenthesisBlock => Some(')'),
            Token::SquareBracketBlock => Some(']'),
            Token::CurlyBracketBlock => Some('}'),
            _ => None,
        };
        cssparser::ToCss::to_css(&token, output).unwrap();
        if let Some(closing) = closing {
            input.parse_nested_block(|nested| serialize_any_value(nested, output))?;
            output.push(closing);
        }
    }
    Ok(())
}

impl QueryCondition {
    /// Parse a single condition.
    pub fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        feature_type: FeatureType,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_internal(context, input, feature_type, AllowOr::Yes)
    }

    fn visit<F>(&self, visitor: &mut F)
    where
        F: FnMut(&Self),
    {
        visitor(self);
        match *self {
            Self::Custom(..)
            | Self::Feature(..)
            | Self::GeneralEnclosed(..)
            | Self::StyleFeature(..)
            | Self::ScrollState(ScrollStateQuery::Feature(..))
            | Self::MozPref(..) => {},
            Self::ScrollState(ScrollStateQuery::Condition(ref cond)) => cond.visit(visitor),
            Self::Not(ref cond) | Self::Style(ref cond) => cond.visit(visitor),
            Self::Operation(ref conds, _op) => {
                for cond in conds.iter() {
                    cond.visit(visitor);
                }
            },
            Self::InParens(ref cond) => cond.visit(visitor),
        }
    }

    /// Returns the union of all flags in the expression. This is useful for
    /// container queries.
    pub fn cumulative_flags(&self) -> FeatureFlags {
        let mut result = FeatureFlags::empty();
        self.visit(&mut |condition| match condition {
            Self::Style(..) => result.insert(FeatureFlags::STYLE),
            Self::GeneralEnclosed(..) => result.insert(FeatureFlags::GENERAL_ENCLOSED),
            Self::Feature(ref f) | Self::ScrollState(ScrollStateQuery::Feature(ref f)) => {
                result.insert(f.feature_flags())
            },
            _ => {},
        });
        result
    }

    /// Parse a single condition, disallowing `or` expressions.
    ///
    /// To be used from the legacy query syntax.
    pub fn parse_disallow_or<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        feature_type: FeatureType,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_internal(context, input, feature_type, AllowOr::No)
    }

    /// https://drafts.csswg.org/mediaqueries-5/#typedef-media-condition or
    /// https://drafts.csswg.org/mediaqueries-5/#typedef-media-condition-without-or
    /// (depending on `allow_or`).
    fn parse_internal<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        feature_type: FeatureType,
        allow_or: AllowOr,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        if input.try_parse(|i| i.expect_ident_matching("not")).is_ok() {
            let inner_condition = Self::parse_in_parens(context, input, feature_type)?;
            return Ok(QueryCondition::Not(Box::new(inner_condition)));
        }

        let first_condition = Self::parse_in_parens(context, input, feature_type)?;
        let operator = match input.try_parse(Operator::parse) {
            Ok(op) => op,
            Err(..) => return Ok(first_condition),
        };

        if allow_or == AllowOr::No && operator == Operator::Or {
            return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }

        let mut conditions = vec![];
        conditions.push(first_condition);
        conditions.push(Self::parse_in_parens(context, input, feature_type)?);

        let delim = match operator {
            Operator::And => "and",
            Operator::Or => "or",
        };

        loop {
            if input.try_parse(|i| i.expect_ident_matching(delim)).is_err() {
                return Ok(QueryCondition::Operation(
                    conditions.into_boxed_slice(),
                    operator,
                ));
            }

            conditions.push(Self::parse_in_parens(context, input, feature_type)?);
        }
    }

    fn parse_in_parenthesis_block<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
        feature_type: FeatureType,
    ) -> Result<Self, ParseError<'i>> {
        // Base case. Make sure to preserve this error as it's more generally
        // relevant.
        let feature_error = match input.try_parse(|input| {
            Self::parse_feature_in_parenthesis_block(context, input, feature_type)
        }) {
            Ok(feature) => return Ok(feature),
            Err(e) => e,
        };
        if matches!(feature_type, FeatureType::Media | FeatureType::Container)
            && static_prefs::pref!("layout.css.custom-media.enabled")
        {
            if let Ok(custom) = input.try_parse(|input| DashedIdent::parse(context, input)) {
                return Ok(Self::Custom(custom));
            }
        }
        if let Ok(inner) = Self::parse(context, input, feature_type) {
            return Ok(Self::InParens(Box::new(inner)));
        }
        Err(feature_error)
    }

    fn parse_feature_in_parenthesis_block<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
        feature_type: FeatureType,
    ) -> Result<Self, ParseError<'i>> {
        if feature_type == FeatureType::Style {
            let feature = StyleFeature::parse(context, input)?;
            return Ok(Self::InParens(Box::new(Self::StyleFeature(feature))));
        }
        QueryFeatureExpression::parse_in_parenthesis_block(context, input, feature_type)
            .map(Self::Feature)
    }

    /// https://drafts.csswg.org/css-conditional-5/#typedef-style-query
    fn parse_style_query<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
        feature_type: FeatureType,
    ) -> Result<Self, ParseError<'i>> {
        if !static_prefs::pref!("layout.css.style-queries.enabled")
            || feature_type != FeatureType::Container
        {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        if let Ok(condition) = input.try_parse(|input| {
            let condition = Self::parse(context, input, FeatureType::Style)?;
            input.expect_exhausted()?;
            Ok::<_, ParseError>(condition)
        }) {
            return Ok(condition);
        }
        StyleFeature::parse(context, input).map(Self::StyleFeature)
    }

    /// https://drafts.csswg.org/css-conditional-5/#typedef-scroll-state-query
    fn parse_scroll_state_query<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
        feature_type: FeatureType,
    ) -> Result<ScrollStateQuery, ParseError<'i>> {
        if !static_prefs::pref!("layout.css.scroll-state.enabled")
            || feature_type != FeatureType::Container
        {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        if let Ok(feature) = input.try_parse(|input| {
            let feature = QueryFeatureExpression::parse_in_parenthesis_block(
                context,
                input,
                FeatureType::ScrollState,
            )?;
            input.expect_exhausted()?;
            Ok::<_, ParseError>(feature)
        }) {
            return Ok(ScrollStateQuery::Feature(feature));
        }
        Self::parse(context, input, FeatureType::ScrollState)
            .map(|condition| ScrollStateQuery::Condition(Box::new(condition)))
    }

    fn try_parse_block<'i, T, F>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
        start: SourcePosition,
        parse: F,
    ) -> Option<T>
    where
        F: for<'tt> FnOnce(&mut Parser<'i, 'tt>) -> Result<T, ParseError<'i>>,
    {
        let nested = input.try_parse(|input| input.parse_nested_block(parse));
        match nested {
            Ok(nested) => Some(nested),
            Err(e) => {
                // We're about to swallow the error in a `<general-enclosed>`
                // condition, so report it while we can.
                let loc = e.location;
                let error = ContextualParseError::InvalidMediaRule(input.slice_from(start), e);
                context.log_css_error(loc, error);
                None
            },
        }
    }

    /// Parse a condition in parentheses, or `<general-enclosed>`.
    ///
    /// https://drafts.csswg.org/mediaqueries/#typedef-media-in-parens
    pub fn parse_in_parens<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        feature_type: FeatureType,
    ) -> Result<Self, ParseError<'i>> {
        input.skip_whitespace();
        let start = input.position();
        let start_location = input.current_source_location();
        let token = input.next()?.clone();
        match token {
            Token::ParenthesisBlock => {
                let nested = Self::try_parse_block(context, input, start, |input| {
                    Self::parse_in_parenthesis_block(context, input, feature_type)
                });
                if let Some(nested) = nested {
                    return Ok(nested);
                }
            },
            Token::Function(ref name) => {
                match_ignore_ascii_case! { name,
                    "style" => {
                        let query = Self::try_parse_block(context, input, start, |input| {
                            Self::parse_style_query(context, input, feature_type)
                        });
                        if let Some(query) = query {
                            return Ok(Self::Style(Box::new(query)));
                        }
                    },
                    "scroll-state" => {
                        let query = Self::try_parse_block(context, input, start, |input| {
                            Self::parse_scroll_state_query(context, input, feature_type)
                        });
                        if let Some(query) = query {
                            return Ok(Self::ScrollState(query));
                        }
                    },
                    "-moz-pref" => {
                        let feature = Self::try_parse_block(context, input, start, |input| {
                            MozPrefFeature::parse(context, input, feature_type)
                        });
                        if let Some(feature) = feature {
                            return Ok(Self::MozPref(feature));
                        }
                    },
                    _ => {},
                }
            },
            ref t => return Err(start_location.new_unexpected_token_error(t.clone())),
        }
        let mut serialization = String::new();
        cssparser::ToCss::to_css(&token, &mut serialization).unwrap();
        input.parse_nested_block(|nested| serialize_any_value(nested, &mut serialization))?;
        serialization.push(')');
        Ok(Self::GeneralEnclosed(serialization))
    }

    /// Whether this condition matches the device and quirks mode.
    /// https://drafts.csswg.org/mediaqueries/#evaluating
    /// https://drafts.csswg.org/mediaqueries/#typedef-general-enclosed
    /// Kleene 3-valued logic is adopted here due to the introduction of
    /// <general-enclosed>.
    pub fn matches(
        &self,
        context: &computed::Context,
        custom: &mut CustomMediaEvaluator,
    ) -> KleeneValue {
        match *self {
            QueryCondition::Custom(ref f) => custom.matches(f, context),
            QueryCondition::Feature(ref f) => f.matches(context),
            QueryCondition::GeneralEnclosed(_) => KleeneValue::Unknown,
            QueryCondition::InParens(ref c) => c.matches(context, custom),
            QueryCondition::Not(ref c) => !c.matches(context, custom),
            QueryCondition::Style(ref c) => c.matches(context, custom),
            QueryCondition::StyleFeature(ref f) => f.matches(context),
            QueryCondition::ScrollState(ScrollStateQuery::Feature(ref f)) => f.matches(context),
            QueryCondition::ScrollState(ScrollStateQuery::Condition(ref c)) => {
                c.matches(context, custom)
            },
            QueryCondition::MozPref(ref c) => c.matches(context),
            QueryCondition::Operation(ref conditions, op) => {
                debug_assert!(!conditions.is_empty(), "We never create an empty op");
                match op {
                    Operator::And => {
                        KleeneValue::any_false(conditions.iter(), |c| c.matches(context, custom))
                    },
                    Operator::Or => {
                        KleeneValue::any(conditions.iter(), |c| c.matches(context, custom))
                    },
                }
            },
        }
    }
}
