/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Navigation 1 `@navigation` conditions, with legacy two-phase preview support.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::stylesheets::{CssRules, UrlExtraData};
use crate::values::{specified::url::SpecifiedUrl, DashedIdent};
use cssparser::{Parser, SourceLocation, Token};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOf, MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, CssWriter, ParseError, ToCss};

/// The navigation phase of a document, which `@navigation` conditions query.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NavigationPhase {
    /// No navigation preview runs.
    #[default]
    Idle,
    /// The document shows the preview of an outbound navigation.
    Preview,
}

/// The typed navigation state supplied by the document's Navigation API.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct CurrentNavigationState {
    pub from_url: String,
    pub to_url: String,
    pub phase: CurrentNavigationPhase,
    pub navigation_type: NavigationType,
    pub old_index: i64,
    pub new_index: i64,
    pub source_element: Option<usize>,
}

/// The current navigation's precommit or committed phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq, MallocSizeOf, Parse, ToCss, ToShmem)]
#[allow(missing_docs)]
pub enum CurrentNavigationPhase {
    Loading,
    Ready,
    Committed,
}

/// The session-history operation of a current navigation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum NavigationType {
    Push,
    Replace,
    Reload,
    Traverse,
}

/// A navigation type test, including the direction of a traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq, MallocSizeOf, Parse, ToCss, ToShmem)]
#[allow(missing_docs)]
pub enum NavigationHistoryQuery {
    Reload,
    Traverse,
    Back,
    Forward,
}

/// The URL of a navigation queried by a location test.
#[derive(Clone, Copy, Debug, MallocSizeOf, Parse, ToCss, ToShmem)]
#[allow(missing_docs)]
pub enum NavigationRelation {
    At,
    From,
    To,
}

/// A supported location operand. Unimplemented location syntaxes use general-enclosed.
#[derive(Clone, Debug, MallocSizeOf, ToCss, ToShmem)]
pub enum NavigationLocation {
    /// A reference to a named location.
    Name(DashedIdent),
    /// A URL resolved against the stylesheet URL.
    Url(SpecifiedUrl),
    /// A URLPattern resolved against the containing stylesheet.
    Pattern(NavigationUrlPattern),
}

/// The quoted URLPattern argument and its stylesheet URL context.
#[derive(Clone, Debug, MallocSizeOf, ToShmem)]
pub struct NavigationUrlPattern {
    argument: String,
    url_data: UrlExtraData,
}

impl NavigationUrlPattern {
    #[cfg(feature = "servo")]
    fn matcher(&self) -> Option<urlpattern::UrlPattern<regex::Regex>> {
        let init = urlpattern::UrlPatternInit::parse_constructor_string::<regex::Regex>(
            &self.argument,
            Some(self.url_data.0.as_ref().clone()),
        )
        .ok()?;
        urlpattern::UrlPattern::parse(init, Default::default()).ok()
    }

    #[cfg(feature = "servo")]
    fn matches(&self, address: &str) -> bool {
        let Some(pattern) = self.matcher() else {
            return false;
        };
        let Ok(url) = address.parse() else {
            return false;
        };
        pattern
            .test(urlpattern::UrlPatternMatchInput::Url(url))
            .unwrap_or(false)
    }
}

impl ToCss for NavigationUrlPattern {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        dest.write_str("url-pattern(")?;
        cssparser::serialize_string(&self.argument, dest)?;
        dest.write_char(')')
    }
}

impl NavigationLocation {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(name) = input.try_parse(|input| DashedIdent::parse(context, input)) {
            return Ok(Self::Name(name));
        }
        if input
            .try_parse(|input| input.expect_function_matching("url-pattern"))
            .is_ok()
        {
            let argument =
                input.parse_nested_block(|input| Ok(input.expect_string_cloned()?.to_string()))?;
            let pattern = NavigationUrlPattern {
                argument,
                url_data: context.url_data.clone(),
            };
            #[cfg(feature = "servo")]
            if pattern.matcher().is_none() {
                return Err(
                    input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError)
                );
            }
            return Ok(Self::Pattern(pattern));
        }
        SpecifiedUrl::parse(context, input).map(Self::Url)
    }
}

/// An `@navigation` condition, including the general-enclosed extension grammar.
#[derive(Clone, Debug, MallocSizeOf, ToShmem)]
pub enum NavigationCondition {
    /// The legacy two-phase navigation preview condition.
    Preview,
    /// A typed navigation URL test.
    Location(NavigationRelation, NavigationLocation),
    /// A navigation joining two locations, in either direction.
    Between(NavigationLocation, NavigationLocation),
    /// A current navigation phase test.
    Phase(CurrentNavigationPhase),
    /// A session-history operation test.
    History(NavigationHistoryQuery),
    /// A nested boolean condition.
    Parenthesized(Box<Self>),
    /// Negation.
    Not(Box<Self>),
    /// Conjunction.
    And(Vec<Self>),
    /// Disjunction.
    Or(Vec<Self>),
    /// An unsupported but syntactically valid extension, which evaluates false.
    GeneralEnclosed(String),
}

impl NavigationCondition {
    /// Parse an `@navigation` prelude.
    pub fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("not"))
            .is_ok()
        {
            let condition = Self::Not(Box::new(Self::parse_in_parens(context, input)?));
            input.expect_exhausted()?;
            return Ok(condition);
        }
        let first = Self::parse_in_parens(context, input)?;
        if input.is_exhausted() {
            return Ok(first);
        }
        let and = input
            .try_parse(|input| input.expect_ident_matching("and"))
            .is_ok();
        if !and {
            input.expect_ident_matching("or")?;
        }
        let mut conditions = vec![first];
        loop {
            conditions.push(Self::parse_in_parens(context, input)?);
            if input.is_exhausted() {
                break;
            }
            input.expect_ident_matching(if and { "and" } else { "or" })?;
        }
        Ok(if and {
            Self::And(conditions)
        } else {
            Self::Or(conditions)
        })
    }

    fn parse_in_parens<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let token = input.next()?.clone();
        if matches!(token, Token::ParenthesisBlock) {
            let start = input.state();
            if let Ok(condition) = input.try_parse(|input| {
                input.parse_nested_block(|input| {
                    if let Ok(condition) = input.try_parse(|input| Self::parse(context, input)) {
                        return Ok(Self::Parenthesized(Box::new(condition)));
                    }
                    if input
                        .try_parse(|input| input.expect_ident_matching("preview"))
                        .is_ok()
                    {
                        return Ok(Self::Preview);
                    }
                    if input
                        .try_parse(|input| input.expect_ident_matching("between"))
                        .is_ok()
                    {
                        input.expect_colon()?;
                        let first = NavigationLocation::parse(context, input)?;
                        input.expect_ident_matching("and")?;
                        return Ok(Self::Between(
                            first,
                            NavigationLocation::parse(context, input)?,
                        ));
                    }
                    if input
                        .try_parse(|input| input.expect_ident_matching("phase"))
                        .is_ok()
                    {
                        input.expect_colon()?;
                        if input
                            .try_parse(|input| input.expect_ident_matching("preview"))
                            .is_ok()
                        {
                            return Ok(Self::Preview);
                        }
                        return Ok(Self::Phase(CurrentNavigationPhase::parse(input)?));
                    }
                    if input
                        .try_parse(|input| input.expect_ident_matching("history"))
                        .is_ok()
                    {
                        input.expect_colon()?;
                        return Ok(Self::History(NavigationHistoryQuery::parse(input)?));
                    }
                    let relation = NavigationRelation::parse(input)?;
                    input.expect_colon()?;
                    Ok(Self::Location(
                        relation,
                        NavigationLocation::parse(context, input)?,
                    ))
                })
            }) {
                return Ok(condition);
            }
            input.reset(&start);
        } else if !matches!(token, Token::Function(_)) {
            return Err(input.new_unexpected_token_error(token));
        }
        let mut css = String::new();
        cssparser::ToCss::to_css(&token, &mut css).unwrap();
        input.parse_nested_block(|input| {
            crate::queries::condition::serialize_any_value(input, &mut css)
        })?;
        css.push(')');
        Ok(Self::GeneralEnclosed(css))
    }

    /// Evaluate with the consumer's legacy preview state. No Navigation API state
    /// is exposed by this context, so CSS Navigation §4 gives null for every URL relation.
    pub fn matches(&self, phase: NavigationPhase) -> bool {
        match self {
            Self::Preview => phase == NavigationPhase::Preview,
            Self::Location(..)
            | Self::Between(..)
            | Self::Phase(_)
            | Self::History(_)
            | Self::GeneralEnclosed(_) => false,
            Self::Parenthesized(condition) => condition.matches(phase),
            Self::Not(condition) => !condition.matches(phase),
            Self::And(conditions) => conditions.iter().all(|condition| condition.matches(phase)),
            Self::Or(conditions) => conditions.iter().any(|condition| condition.matches(phase)),
        }
    }

    /// Evaluate against the current document state and its named locations.
    #[cfg(feature = "servo")]
    pub fn matches_navigation(
        &self,
        phase: NavigationPhase,
        state: Option<&CurrentNavigationState>,
        locations: &[super::LocationRule],
        document_base: Option<&str>,
    ) -> bool {
        let location = |location: &NavigationLocation, url: &str| match location {
            NavigationLocation::Name(name) => locations
                .iter()
                .rev()
                .find(|rule| rule.name == *name)
                .is_some_and(|rule| rule.matches_with_document_base(url, document_base)),
            NavigationLocation::Url(value) => value
                .url()
                .as_ref()
                .is_some_and(|value| value.as_str() == url),
            NavigationLocation::Pattern(pattern) => pattern.matches(url),
        };
        match self {
            Self::Preview => phase == NavigationPhase::Preview,
            Self::Location(relation, target) => state.is_some_and(|state| {
                location(
                    target,
                    match relation {
                        NavigationRelation::From => &state.from_url,
                        NavigationRelation::To => &state.to_url,
                        NavigationRelation::At => {
                            if state.phase == CurrentNavigationPhase::Committed {
                                &state.to_url
                            } else {
                                &state.from_url
                            }
                        },
                    },
                )
            }),
            Self::Between(first, second) => state.is_some_and(|state| {
                (location(first, &state.from_url) && location(second, &state.to_url))
                    || (location(second, &state.from_url) && location(first, &state.to_url))
            }),
            Self::Phase(phase) => state.is_some_and(|state| state.phase == *phase),
            Self::History(kind) => state.is_some_and(|state| match kind {
                NavigationHistoryQuery::Reload => state.navigation_type == NavigationType::Reload,
                NavigationHistoryQuery::Traverse => {
                    state.navigation_type == NavigationType::Traverse
                },
                NavigationHistoryQuery::Back => {
                    state.navigation_type == NavigationType::Traverse
                        && state.new_index < state.old_index
                },
                NavigationHistoryQuery::Forward => {
                    state.navigation_type == NavigationType::Traverse
                        && state.new_index > state.old_index
                },
            }),
            Self::Parenthesized(condition) => {
                condition.matches_navigation(phase, state, locations, document_base)
            },
            Self::Not(condition) => {
                !condition.matches_navigation(phase, state, locations, document_base)
            },
            Self::And(conditions) => conditions.iter().all(|condition| {
                condition.matches_navigation(phase, state, locations, document_base)
            }),
            Self::Or(conditions) => conditions.iter().any(|condition| {
                condition.matches_navigation(phase, state, locations, document_base)
            }),
            Self::GeneralEnclosed(_) => false,
        }
    }
}

impl ToCss for NavigationCondition {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Preview => dest.write_str("(preview)"),
            Self::Location(relation, location) => {
                dest.write_char('(')?;
                relation.to_css(dest)?;
                dest.write_str(": ")?;
                location.to_css(dest)?;
                dest.write_char(')')
            },
            Self::Between(first, second) => {
                dest.write_str("(between: ")?;
                first.to_css(dest)?;
                dest.write_str(" and ")?;
                second.to_css(dest)?;
                dest.write_char(')')
            },
            Self::Phase(value) => {
                dest.write_str("(phase: ")?;
                value.to_css(dest)?;
                dest.write_char(')')
            },
            Self::History(value) => {
                dest.write_str("(history: ")?;
                value.to_css(dest)?;
                dest.write_char(')')
            },
            Self::Parenthesized(condition) => {
                dest.write_char('(')?;
                condition.to_css(dest)?;
                dest.write_char(')')
            },
            Self::Not(condition) => {
                dest.write_str("not ")?;
                condition.to_css(dest)
            },
            Self::And(conditions) | Self::Or(conditions) => {
                for (index, condition) in conditions.iter().enumerate() {
                    if index != 0 {
                        dest.write_str(if matches!(self, Self::And(_)) {
                            " and "
                        } else {
                            " or "
                        })?;
                    }
                    condition.to_css(dest)?;
                }
                Ok(())
            },
            Self::GeneralEnclosed(css) => dest.write_str(css),
        }
    }
}

/// An `@navigation <condition> { … }` rule.
#[derive(Debug, ToShmem)]
pub struct NavigationRule {
    /// The condition.
    pub condition: NavigationCondition,
    /// The child rules.
    pub rules: Arc<Locked<CssRules>>,
    /// The source position of the rule.
    pub source_location: SourceLocation,
}

impl NavigationRule {
    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        self.condition.size_of(ops)
            + self.rules.unconditional_shallow_size_of(ops)
            + self.rules.read_with(guard).size_of(guard, ops)
    }
}

impl ToCssWithGuard for NavigationRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        dest.write_str("@navigation ")?;
        self.condition.to_css(&mut CssWriter::new(dest))?;
        self.rules.read_with(guard).to_css_block(guard, dest)
    }
}

impl DeepCloneWithLock for NavigationRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        let rules = self.rules.read_with(guard);
        NavigationRule {
            condition: self.condition.clone(),
            rules: Arc::new(lock.wrap(rules.deep_clone_with_lock(lock, guard))),
            source_location: self.source_location.clone(),
        }
    }
}
