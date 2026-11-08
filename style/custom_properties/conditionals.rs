/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Conditional value substitution (CSS Values 5 §8.3).

use super::*;
use crate::parser::ParserContext;
use crate::queries::FeatureType;
use crate::queries::condition::{Operator, QueryCondition};
use crate::queries::style_feature::{RangeOperand, StyleFeature, StyleRangeValue};
use crate::stylesheets::{CssRuleType, Origin};
use selectors::kleene_value::KleeneValue;
use style_traits::ParsingMode;

pub(super) fn parse_branches<'i>(
    input: &mut Parser<'i, '_>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
) -> Result<Box<[ConditionalBranch]>, ParseError<'i>> {
    let mut branches = Vec::new();
    while !input.is_exhausted() {
        input.skip_whitespace();
        let start = input.position();
        loop {
            match input.next()? {
                Token::Colon => break,
                Token::Semicolon => {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                },
                _ => {},
            }
        }
        let css = input.slice_from(start);
        let css = css[..css.len() - 1].trim_ascii_end();
        if css.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        let (first_token_type, last_token_type) = token_types(css);
        let start = start.byte_index() - input_start.byte_index();
        let condition = FunctionArgument {
            start,
            end: start + css.len(),
            first_token_type,
            last_token_type,
        };
        let value = input.parse_until_before(Delimiter::Semicolon, |input| {
            parse_substitution_argument(
                input,
                input_start,
                namespaces,
                references,
                missing_closing_characters,
            )
        })?;
        branches.push(ConditionalBranch { condition, value });
        if !input.is_exhausted() {
            input.expect_semicolon()?;
        }
    }
    if branches.is_empty() {
        return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    Ok(branches.into_boxed_slice())
}

fn context(url_data: &UrlExtraData) -> ParserContext<'_> {
    ParserContext::new(
        Origin::Author,
        url_data,
        Some(CssRuleType::Style),
        ParsingMode::DEFAULT,
        crate::context::QuirksMode::NoQuirks,
        Default::default(),
        None,
        None,
    )
}

fn collect_condition_names(condition: &QueryCondition, names: &mut Vec<Name>) {
    match condition {
        QueryCondition::StyleFeature(StyleFeature::Declaration { name, .. }) => {
            names.push(name.clone())
        },
        QueryCondition::StyleFeature(StyleFeature::Range {
            first,
            comparison,
            second,
        }) => {
            for value in std::iter::once(first)
                .chain(std::iter::once(&comparison.1))
                .chain(second.iter().map(|(_, value)| value))
            {
                if let StyleRangeValue::Property(name) = value {
                    names.push(name.clone());
                }
            }
        },
        QueryCondition::Style(condition)
        | QueryCondition::InParens(condition)
        | QueryCondition::Not(condition) => collect_condition_names(condition, names),
        QueryCondition::Operation(conditions, _) => {
            for condition in conditions {
                collect_condition_names(condition, names);
            }
        },
        _ => {},
    }
}

pub(super) fn condition_names(value: &VariableValue) -> Vec<Name> {
    let mut names = Vec::new();
    for reference in &value.references.refs {
        let SubstitutionFunction::If(branches) = &reference.function else {
            continue;
        };
        for ConditionalBranch { condition, .. } in &**branches {
            let mut input = ParserInput::new(&value.css[condition.start..condition.end]);
            if let Ok(condition) = QueryCondition::parse(
                &context(&value.url_data),
                &mut Parser::new(&mut input),
                FeatureType::Container,
            ) {
                collect_condition_names(&condition, &mut names);
            }
        }
    }
    for name in &mut names {
        if let Some((_, private)) = value
            .private_names
            .iter()
            .find(|(original, _)| original == name)
        {
            *name = private.clone();
        }
    }
    names
}

pub(super) fn condition_values(value: &VariableValue) -> Vec<VariableValue> {
    let mut values = Vec::new();
    for reference in &value.references.refs {
        let SubstitutionFunction::If(branches) = &reference.function else {
            continue;
        };
        for ConditionalBranch { condition, .. } in &**branches {
            let mut input = ParserInput::new(&value.css[condition.start..condition.end]);
            if let Ok(mut condition) = VariableValue::parse(
                &mut Parser::new(&mut input),
                &value.url_data,
                &Namespaces::default(),
            ) {
                condition.private_names = value.private_names.clone();
                for reference in &mut condition.references.refs {
                    if let Some(name) = reference.function.variable_name_mut() {
                        if let Some((_, private)) = value
                            .private_names
                            .iter()
                            .find(|(original, _)| original == name)
                        {
                            *name = private.clone();
                        }
                    }
                }
                values.extend(condition_values(&condition));
                values.push(condition);
            }
        }
    }
    values
}

impl<'a, 'b, 't> Substitutor<'a, 'b, 't> {
    pub(super) fn conditional<'v>(
        &mut self,
        css: &'v str,
        url_data: &UrlExtraData,
        branches: &[ConditionalBranch],
        frame: Option<usize>,
        references: &mut ReferenceIter<'v>,
    ) -> Result<Substitution<'v>, ()>
    where
        'a: 'v,
    {
        let mut tainted = false;
        for ConditionalBranch { condition, value } in branches {
            let mut input = ParserInput::new(&css[condition.start..condition.end]);
            let condition = VariableValue::parse(
                &mut Parser::new(&mut input),
                url_data,
                &Namespaces::default(),
            )
            .ok();
            let condition = condition.map(|mut condition| {
                condition.private_names = self.private_names.clone();
                condition
            });
            let condition = condition
                .as_ref()
                .and_then(|condition| self.substitute_value(condition, frame).ok());
            if condition.is_some_and(|condition| {
                tainted |= !condition.attr_tainted.is_empty();
                self.condition_matches(&condition.css, url_data, frame, &mut tainted)
            }) {
                return self
                    .chunk(
                        css,
                        value.start,
                        value.end,
                        value.first_token_type,
                        value.last_token_type,
                        url_data,
                        frame,
                        references,
                    )
                    .map(|value| if tainted { value.attr_tainted() } else { value });
            }
            while references
                .next_if(|reference| reference.end <= value.end)
                .is_some()
            {}
        }
        Ok(Substitution::default())
    }

    fn condition_matches(
        &mut self,
        css: &str,
        url_data: &UrlExtraData,
        frame: Option<usize>,
        tainted: &mut bool,
    ) -> bool {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        if parser
            .try_parse(|input| input.expect_ident_matching("else"))
            .is_ok()
        {
            return parser.is_exhausted();
        }
        parser
            .parse_entirely(|input| {
                QueryCondition::parse(&context(url_data), input, FeatureType::Container)
            })
            .is_ok_and(|condition| {
                let mut names = Vec::new();
                collect_condition_names(&condition, &mut names);
                *tainted |= names.iter().any(|name| {
                    self.query_variable(frame, name)
                        .is_some_and(|value| !value.attr_tainted.is_empty())
                });
                self.query_matches(&condition, url_data, frame)
                    .to_bool(false)
            })
    }

    fn query_matches(
        &mut self,
        condition: &QueryCondition,
        url_data: &UrlExtraData,
        frame: Option<usize>,
    ) -> KleeneValue {
        match condition {
            QueryCondition::StyleFeature(StyleFeature::Declaration { name, value }) => {
                KleeneValue::from(self.style_query(frame, name, value.as_deref()))
            },
            QueryCondition::StyleFeature(StyleFeature::Range {
                first,
                comparison,
                second,
            }) => {
                let result = (|| {
                    let mut left = self.range_operand(first, url_data, frame)?;
                    for (operator, value) in std::iter::once(comparison).chain(second) {
                        let right = self.range_operand(value, url_data, frame)?;
                        if !left
                            .compare(right)
                            .is_some_and(|order| operator.evaluate(order))
                        {
                            return Some(false);
                        }
                        left = right;
                    }
                    Some(true)
                })();
                KleeneValue::from(result.unwrap_or(false))
            },
            QueryCondition::Style(condition) | QueryCondition::InParens(condition) => {
                self.query_matches(condition, url_data, frame)
            },
            QueryCondition::Not(condition) => !self.query_matches(condition, url_data, frame),
            QueryCondition::Operation(conditions, Operator::And) => {
                KleeneValue::any_false(conditions.iter(), |condition| {
                    self.query_matches(condition, url_data, frame)
                })
            },
            QueryCondition::Operation(conditions, Operator::Or) => {
                KleeneValue::any(conditions.iter(), |condition| {
                    self.query_matches(condition, url_data, frame)
                })
            },
            QueryCondition::GeneralEnclosed(css) => self.enclosed_query(css, url_data),
            _ => KleeneValue::Unknown,
        }
    }

    fn range_operand(
        &mut self,
        value: &StyleRangeValue,
        url_data: &UrlExtraData,
        frame: Option<usize>,
    ) -> Option<RangeOperand> {
        let css = match value {
            StyleRangeValue::Property(name) => self.query_variable(frame, name)?.css.into_owned(),
            StyleRangeValue::Value(value) => {
                self.substitute_value(value, frame).ok()?.css.into_owned()
            },
        };
        let mut input = ParserInput::new(&css);
        Parser::new(&mut input)
            .parse_entirely(|input| {
                RangeOperand::parse(&context(url_data), self.computed_context, input)
            })
            .ok()
    }

    fn enclosed_query(&self, css: &str, url_data: &UrlExtraData) -> KleeneValue {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        let result = parser.parse_entirely(|input| {
            let name = input.expect_function()?.clone();
            input.parse_nested_block(|input| {
                if name.eq_ignore_ascii_case("supports") {
                    let query =
                        crate::stylesheets::supports_rule::parse_condition_or_declaration(input)?;
                    let mut parser_context = context(url_data);
                    parser_context.quirks_mode = self.computed_context.quirks_mode;
                    return Ok(query.eval(&parser_context));
                }
                if name.eq_ignore_ascii_case("media") {
                    let condition = input
                        .try_parse(|input| {
                            QueryCondition::parse(&context(url_data), input, FeatureType::Media)
                        })
                        .or_else(|_| {
                            crate::queries::QueryFeatureExpression::parse_in_parenthesis_block(
                                &context(url_data),
                                input,
                                FeatureType::Media,
                            )
                            .map(QueryCondition::Feature)
                        })?;
                    return Ok(computed::Context::for_media_query_evaluation(
                        self.stylist.device(),
                        self.computed_context.quirks_mode,
                        |context| {
                            condition
                                .matches(
                                    context,
                                    &mut crate::stylesheets::CustomMediaEvaluator::none(),
                                )
                                .to_bool(false)
                        },
                    ));
                }
                Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
            })
        });
        result.map_or(KleeneValue::Unknown, KleeneValue::from)
    }
}
