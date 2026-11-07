/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Anchored container features:
//!
//! https://drafts.csswg.org/css-anchor-position-2/#anchored-container

use super::feature::{AllowsRanges, Evaluator, FeatureFlags, QueryFeatureDescription};
use crate::derives::*;
use crate::logical_geometry::WritingMode;
use crate::parser::{Parse, ParserContext};
use crate::properties::ComputedValues;
use crate::values::computed::Context;
use crate::values::specified::position::{
    DashedIdentAndOrTryTactic, PositionAreaQuery, PositionTryFallbacksItem,
};
use cssparser::Parser;
use selectors::kleene_value::KleeneValue;
use style_traits::ParseError;

/// The position option that layout applied to an element: its index into `position-try-fallbacks`
/// and the writing mode of its containing block.
#[derive(Clone, Copy, Debug)]
pub struct AppliedPositionOption {
    /// The index into `position-try-fallbacks`.
    pub index: usize,
    /// The writing mode of the containing block.
    pub containing_writing_mode: WritingMode,
}

/// The `position-try-fallbacks` entry that layout applied to an anchored query container.
#[derive(Clone, Debug)]
pub struct AnchoredFallback {
    item: PositionTryFallbacksItem,
    containing_writing_mode: WritingMode,
}

impl AnchoredFallback {
    /// The entry that `option` applies from the `position-try-fallbacks` of `style`.
    pub(crate) fn new(style: &ComputedValues, option: AppliedPositionOption) -> Option<Self> {
        let item = style
            .clone_position_try_fallbacks()
            .0
            .get(option.index)?
            .clone();
        Some(Self {
            item,
            containing_writing_mode: option.containing_writing_mode,
        })
    }
}

/// A value of the `fallback` container feature.
///
/// https://drafts.csswg.org/css-anchor-position-2/#descdef-container-fallback
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToShmem)]
pub enum FallbackQuery {
    /// `none`
    None,
    /// `<dashed-ident> || <try-tactic>`
    IdentAndOrTactic(DashedIdentAndOrTryTactic),
    /// `<position-area-query>`
    PositionArea(PositionAreaQuery),
}

impl Parse for FallbackQuery {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        if let Ok(option) = input.try_parse(|i| DashedIdentAndOrTryTactic::parse(context, i)) {
            return Ok(Self::IdentAndOrTactic(option));
        }
        PositionAreaQuery::parse(context, input).map(Self::PositionArea)
    }
}

impl FallbackQuery {
    /// Whether `fallback`, applied to a container in writing mode `wm`, matches this value.
    fn matches(&self, fallback: Option<&AnchoredFallback>, wm: WritingMode) -> bool {
        let Some(fallback) = fallback else {
            return matches!(self, Self::None);
        };
        match (self, &fallback.item) {
            (Self::IdentAndOrTactic(query), PositionTryFallbacksItem::IdentAndOrTactic(item)) => {
                query == item
            },
            (Self::PositionArea(query), PositionTryFallbacksItem::PositionArea(area)) => {
                query.matches(*area, fallback.containing_writing_mode, wm)
            },
            _ => false,
        }
    }
}

fn eval_fallback(context: &Context, query: Option<&FallbackQuery>) -> KleeneValue {
    let Some(info) = context.container_info.as_ref() else {
        return KleeneValue::Unknown;
    };
    let fallback = info.anchored_fallback();
    KleeneValue::from(match query {
        None => fallback.is_some(),
        Some(query) => query.matches(fallback, info.writing_mode()),
    })
}

/// https://drafts.csswg.org/css-anchor-position-2/#anchored-container
pub static ANCHORED_FEATURES: [QueryFeatureDescription; 1] = [feature!(
    atom!("fallback"),
    AllowsRanges::No,
    Evaluator::Fallback(eval_fallback),
    FeatureFlags::CONTAINER_REQUIRES_ANCHORED,
)];

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::test_support::{author_parser_context, test_url_data};
    use crate::values::specified::position::PositionArea;
    use cssparser::ParserInput;

    fn parse<T: Parse>(source: &str) -> T {
        let url_data = test_url_data();
        let context = author_parser_context(&url_data);
        let mut input = ParserInput::new(source);
        Parser::new(&mut input)
            .parse_entirely(|input| T::parse(&context, input))
            .unwrap()
    }

    fn applied(
        item: PositionTryFallbacksItem,
        containing_writing_mode: WritingMode,
    ) -> AnchoredFallback {
        AnchoredFallback {
            item,
            containing_writing_mode,
        }
    }

    fn area(source: &str, containing_writing_mode: WritingMode) -> AnchoredFallback {
        applied(
            PositionTryFallbacksItem::PositionArea(parse::<PositionArea>(source)),
            containing_writing_mode,
        )
    }

    fn option(source: &str) -> AnchoredFallback {
        applied(
            PositionTryFallbacksItem::IdentAndOrTactic(parse(source)),
            WritingMode::horizontal_tb(),
        )
    }

    fn matches(query: &str, fallback: Option<&AnchoredFallback>, wm: WritingMode) -> bool {
        parse::<FallbackQuery>(query).matches(fallback, wm)
    }

    #[test]
    fn none_matches_only_a_container_without_an_applied_fallback() {
        let horizontal = WritingMode::horizontal_tb();
        assert!(matches("none", None, horizontal));
        assert!(!matches("--foo", None, horizontal));
        assert!(!matches("any", None, horizontal));
        assert!(!matches("none", Some(&option("--foo")), horizontal));
    }

    #[test]
    fn names_and_try_tactics_match_the_applied_option_in_order() {
        let horizontal = WritingMode::horizontal_tb();
        let fallback = option("--bar flip-block");
        assert!(matches("--bar flip-block", Some(&fallback), horizontal));
        assert!(matches("flip-block --bar", Some(&fallback), horizontal));
        assert!(!matches("--bar", Some(&fallback), horizontal));
        assert!(!matches("flip-block", Some(&fallback), horizontal));
        let fallback = option("flip-inline flip-block");
        assert!(matches("flip-inline flip-block", Some(&fallback), horizontal));
        assert!(!matches("flip-block flip-inline", Some(&fallback), horizontal));
        assert!(!matches("top", Some(&fallback), horizontal));
    }

    #[test]
    fn position_area_queries_match_the_physical_area_with_any_axis() {
        let horizontal = WritingMode::horizontal_tb();
        let fallback = area("bottom", horizontal);
        for query in [
            "bottom",
            "span-all bottom",
            "block-end",
            "block-end span-all",
            "self-block-end",
            "span-all self-block-end",
            "self-y-end",
        ] {
            assert!(matches(query, Some(&fallback), horizontal), "{query}");
        }
        let fallback = area("span-bottom", horizontal);
        for query in ["any", "any span-bottom", "span-end any", "any span-all"] {
            assert!(matches(query, Some(&fallback), horizontal), "{query}");
        }
        let fallback = area("right span-bottom", horizontal);
        for (query, expected) in [
            ("span-bottom", true),
            ("right", true),
            ("left", false),
            ("bottom", false),
        ] {
            assert_eq!(matches(query, Some(&fallback), horizontal), expected, "{query}");
        }
        assert!(!matches("--foo", Some(&fallback), horizontal));
    }

    #[test]
    fn logical_position_area_queries_resolve_against_the_container_and_its_containing_block() {
        let containing = WritingMode::WRITING_MODE_VERTICAL_LR;
        let container = WritingMode::RTL | WritingMode::INLINE_REVERSED;
        for applied in ["self-block-end self-inline-start", "block-end inline-end"] {
            let fallback = area(applied, containing);
            for query in [
                "bottom right",
                "self-inline-start self-block-end",
                "inline-end block-end",
            ] {
                assert!(matches(query, Some(&fallback), container), "{applied}: {query}");
            }
            assert!(!matches("top right", Some(&fallback), container), "{applied}");
        }
    }
}
