/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Conditional value substitution (CSS Values 5 §8.3).

use super::*;
use crate::parser::ParserContext;
use crate::queries::FeatureType;
use crate::queries::condition::{Operator, QueryCondition};
use crate::queries::style_feature::StyleFeature;
use crate::stylesheets::{CssRuleType, Origin};
use selectors::kleene_value::KleeneValue;
use style_traits::ParsingMode;

pub(super) fn parse_branches<'i>(
    input: &mut Parser<'i, '_>,
    input_start: SourcePosition,
    namespaces: &Namespaces,
    references: &mut References,
    missing_closing_characters: &mut String,
) -> Result<Box<[[FunctionArgument; 2]]>, ParseError<'i>> {
    let mut branches = Vec::new();
    while !input.is_exhausted() {
        let condition = input.parse_until_before(Delimiter::Colon, |input| {
            parse_substitution_argument(
                input,
                input_start,
                namespaces,
                references,
                missing_closing_characters,
            )
        })?;
        input.expect_colon()?;
        let value = input.parse_until_before(Delimiter::Semicolon, |input| {
            parse_substitution_argument(
                input,
                input_start,
                namespaces,
                references,
                missing_closing_characters,
            )
        })?;
        branches.push([condition, value]);
        if !input.is_exhausted() {
            input.expect_semicolon()?;
        }
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

pub(super) fn condition_names(value: &VariableValue) -> Vec<Name> {
    fn collect(condition: &QueryCondition, names: &mut Vec<Name>) {
        match condition {
            QueryCondition::StyleFeature(StyleFeature::Declaration { name, .. }) => {
                names.push(name.clone())
            },
            QueryCondition::Style(condition)
            | QueryCondition::InParens(condition)
            | QueryCondition::Not(condition) => collect(condition, names),
            QueryCondition::Operation(conditions, _) => {
                for condition in conditions {
                    collect(condition, names);
                }
            },
            _ => {},
        }
    }
    let mut names = Vec::new();
    for reference in &value.references.refs {
        let SubstitutionFunction::If(branches) = &reference.function else {
            continue;
        };
        for [condition, _] in &**branches {
            let mut input = ParserInput::new(&value.css[condition.start..condition.end]);
            if let Ok(condition) = QueryCondition::parse(
                &context(&value.url_data),
                &mut Parser::new(&mut input),
                FeatureType::Container,
            ) {
                collect(&condition, &mut names);
            }
        }
    }
    names
}

impl<'a, 'b, 't> Substitutor<'a, 'b, 't> {
    pub(super) fn conditional<'v>(
        &mut self,
        css: &'v str,
        url_data: &UrlExtraData,
        branches: &[[FunctionArgument; 2]],
        frame: Option<usize>,
        references: &mut ReferenceIter<'v>,
    ) -> Result<Substitution<'v>, ()>
    where
        'a: 'v,
    {
        for [condition, value] in branches {
            let condition = self.chunk(
                css,
                condition.start,
                condition.end,
                condition.first_token_type,
                condition.last_token_type,
                url_data,
                frame,
                references,
            );
            if condition
                .is_ok_and(|condition| self.condition_matches(&condition.css, url_data, frame))
            {
                return self.chunk(
                    css,
                    value.start,
                    value.end,
                    value.first_token_type,
                    value.last_token_type,
                    url_data,
                    frame,
                    references,
                );
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

    fn enclosed_query(&self, css: &str, url_data: &UrlExtraData) -> KleeneValue {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        let result = parser.parse_entirely(|input| {
            let name = input.expect_function()?.clone();
            input.parse_nested_block(|input| {
                if name.eq_ignore_ascii_case("supports") {
                    let query =
                        crate::stylesheets::supports_rule::parse_condition_or_declaration(input)?;
                    return Ok(query.eval(&context(url_data)));
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
