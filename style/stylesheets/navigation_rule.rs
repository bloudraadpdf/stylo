/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Navigation 1 `@navigation` conditions, with legacy two-phase preview support.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::stylesheets::CssRules;
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
}

impl NavigationLocation {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(name) = input.try_parse(|input| DashedIdent::parse(context, input)) {
            return Ok(Self::Name(name));
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
                        .try_parse(|input| {
                            input.expect_ident_matching("phase")?;
                            input.expect_colon()?;
                            input.expect_ident_matching("preview")
                        })
                        .is_ok()
                    {
                        return Ok(Self::Preview);
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
            Self::Location(..) | Self::GeneralEnclosed(_) => false,
            Self::Parenthesized(condition) => condition.matches(phase),
            Self::Not(condition) => !condition.matches(phase),
            Self::And(conditions) => conditions.iter().all(|condition| condition.matches(phase)),
            Self::Or(conditions) => conditions.iter().any(|condition| condition.matches(phase)),
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
