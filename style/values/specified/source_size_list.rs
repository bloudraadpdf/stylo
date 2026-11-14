/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! https://html.spec.whatwg.org/multipage/#source-size-list

use crate::media_queries::Device;
use crate::parser::{Parse, ParserContext};
use crate::queries::{FeatureType, QueryCondition};
use crate::stylesheets::CustomMediaEvaluator;
use crate::values::computed::{self, ToComputedValue};
use crate::values::specified::{Length, NoCalcLength, ViewportPercentageLength};
use app_units::Au;
use cssparser::{Delimiter, Parser, Token};
use selectors::context::QuirksMode;
use style_traits::ParseError;

#[derive(Debug)]
enum SourceSizeValue {
    Auto,
    Length(Length),
}

impl Parse for SourceSizeValue {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }
        Length::parse_non_negative(context, input).map(Self::Length)
    }
}

/// A source size and its optional media condition.
#[derive(Debug)]
pub struct SourceSize {
    condition: Option<QueryCondition>,
    value: SourceSizeValue,
}

impl Parse for SourceSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(value) = input.try_parse(|input| SourceSizeValue::parse(context, input)) {
            return Ok(Self {
                condition: None,
                value,
            });
        }
        let condition = QueryCondition::parse(context, input, FeatureType::Media)?;
        let value = SourceSizeValue::parse(context, input)?;
        Ok(Self {
            condition: Some(condition),
            value,
        })
    }
}

/// An ordered HTML source-size list, including unresolved `auto` fallbacks.
#[derive(Debug)]
pub struct SourceSizeList {
    source_sizes: Vec<SourceSize>,
}

impl SourceSizeList {
    /// Create an empty list with the default `100vw` source size.
    pub fn empty() -> Self {
        Self {
            source_sizes: vec![],
        }
    }

    /// Evaluate using an eligible image's observed concrete width, if available.
    pub fn evaluate(&self, device: &Device, quirks_mode: QuirksMode, auto_size: Option<Au>) -> Au {
        computed::Context::for_media_query_evaluation(device, quirks_mode, |context| {
            for source_size in &self.source_sizes {
                if source_size.condition.as_ref().is_some_and(|condition| {
                    !condition
                        .matches(context, &mut CustomMediaEvaluator::none())
                        .to_bool(false)
                }) {
                    continue;
                }
                match &source_size.value {
                    SourceSizeValue::Auto => {
                        if let Some(width) = auto_size {
                            return width;
                        }
                    },
                    SourceSizeValue::Length(value) => {
                        return value.to_computed_value(context).into()
                    },
                }
            }
            Length::NoCalc(NoCalcLength::ViewportPercentage(
                ViewportPercentageLength::Vw(100.),
            ))
            .to_computed_value(context)
            .into()
        })
    }

    /// Parse each comma-separated source size with the CSS engine's grammars.
    pub fn parse<'i, 't>(context: &ParserContext, input: &mut Parser<'i, 't>) -> Self {
        let mut source_sizes = vec![];
        loop {
            if let Ok(size) = input
                .parse_until_before(Delimiter::Comma, |input| SourceSize::parse(context, input))
            {
                source_sizes.push(size);
            }
            match input.next() {
                Ok(&Token::Comma) => {},
                Err(..) => break,
                _ => unreachable!(),
            }
        }
        Self { source_sizes }
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::stylesheets::{Origin, UrlExtraData};
    use cssparser::ParserInput;
    use style_traits::ParsingMode;

    fn sizes(text: &str) -> SourceSizeList {
        let url: UrlExtraData = url::Url::parse("about:blank").unwrap().into();
        let context = ParserContext::new(
            Origin::Author,
            &url,
            None,
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(text);
        SourceSizeList::parse(&context, &mut Parser::new(&mut input))
    }

    fn evaluate(text: &str, auto: Option<f32>) -> f32 {
        sizes(text)
            .evaluate(
                &crate::test_support::test_device(),
                QuirksMode::NoQuirks,
                auto.map(Au::from_f32_px),
            )
            .to_f32_px()
    }

    #[test]
    fn unresolved_auto_keeps_later_length_and_media_fallbacks() {
        assert_eq!(evaluate("auto, 100px", None), 100.0);
        assert_eq!(
            evaluate("auto, (min-width: 600px) 200px, 300px", None),
            200.0
        );
        assert_eq!(
            evaluate("auto, (min-width: 900px) 200px, 300px", None),
            300.0
        );
        assert_eq!(evaluate("auto", None), 800.0);
        assert_eq!(evaluate("AUTO, auto, 20vw", None), 160.0);
    }

    #[test]
    fn observed_auto_width_obeys_order_and_conditions_without_guessing() {
        assert_eq!(evaluate("auto, 100px", Some(43.0)), 43.0);
        assert_eq!(evaluate("auto, 100px", Some(0.0)), 0.0);
        assert_eq!(
            evaluate("(min-width: 900px) auto, 100px", Some(43.0)),
            100.0
        );
        assert_eq!(evaluate("(min-width: 600px) auto, 100px", Some(43.0)), 43.0);
        assert_eq!(evaluate("30px, auto", Some(43.0)), 30.0);
    }

    #[test]
    fn invalid_entries_and_math_preserve_source_size_recovery() {
        for text in [
            "-1px, 23px",
            "20%, 23px",
            "junk, 23px",
            "auto junk, 23px",
            "auto 10px, 23px",
        ] {
            assert_eq!(evaluate(text, None), 23.0, "{text}");
        }
        assert_eq!(evaluate("calc(50vw - 10px)", None), 390.0);
        assert_eq!(
            evaluate("(max-width: 900px) min(100px, 40vw), 20px", None),
            100.0
        );
        assert_eq!(evaluate("", None), 800.0);
        assert_eq!(evaluate("auto,,", None), 800.0);
        assert_eq!(evaluate("0px", None), 0.0);
    }
}
