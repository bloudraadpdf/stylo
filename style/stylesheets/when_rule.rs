/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! [`@when`][when] and [`@else`][else_link] rules from
//! [CSS Conditional Rules Module Level 5][csswg].
//!
//! [when]: https://drafts.csswg.org/css-conditional-5/#when-rule
//! [else_link]: https://drafts.csswg.org/css-conditional-5/#else-rule
//! [csswg]: https://drafts.csswg.org/css-conditional-5/
//!
//! Per CR-WD §3.1, `@when` accepts a `<when-condition>` whose leaves
//! are either a `supports(<supports-condition>)` functional notation
//! or a `media(<media-condition>)` functional notation, recursively
//! combined with `not` / `and` / `or` / parentheses. The chained
//! `@else` rule (§3.2) follows immediately after an `@when` (or a
//! prior `@else`), with an optional condition; a trailing `@else`
//! with no condition is the default branch.
//!
//! Each `@when` / `@else` rule carries a shared `Arc<ChainConditions>`
//! plus the rule's own position within that chain. The
//! `chain_member_is_enabled` helper walks the chain from the head up
//! to (and including) the requested position, evaluating each
//! member's condition. The first matching entry is the active
//! branch; every later branch is suppressed even if its own
//! condition would have matched, per CSS Conditional 5 §3.2.
//!
//! `supports(...)` leaves are pre-evaluated at parse time — they
//! test syntactic UA support, which is device-independent and lives
//! entirely in `ParserContext`. `media(...)` leaves stay symbolic
//! so the evaluator can re-run them against the current `Device`
//! every time the cascade rebuilds, mirroring `@media` rule
//! semantics exactly.

use crate::context::QuirksMode;
use crate::derives::*;
use crate::media_queries::{Device, MediaList};
use crate::parser::ParserContext;
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::stylesheets::supports_rule::{parse_condition_or_declaration, SupportsCondition};
use crate::stylesheets::{CssRuleType, CssRules, CustomMediaEvaluator, CustomMediaMap};
use cssparser::{match_ignore_ascii_case, Parser, SourceLocation, Token};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// A boolean combinator for a `<when-condition>`.
///
/// Mirrors the structure used by `SupportsCondition` and
/// `QueryCondition`, but the leaf operands are themselves an entire
/// `@supports` predicate or an entire `@media` predicate.
///
/// `supports(...)` leaves are evaluated eagerly at parse time and
/// reduced to a stored boolean; this matches the contract that the
/// existing `SupportsRule.enabled` field carries (the syntactic
/// support test is device-independent and lives entirely in the
/// `ParserContext`). `media(...)` leaves stay symbolic so the
/// evaluator can re-run them against the current `Device` on every
/// cascade rebuild — exactly how the existing `@media` rule works.
#[derive(Clone, Debug, ToShmem)]
pub enum WhenCondition {
    /// `not <when-in-parens>` — logical negation.
    Not(Box<WhenCondition>),
    /// `( <when-condition> )` — explicit parentheses, retained so the
    /// CSSOM round-trip preserves authored grouping.
    InParens(Box<WhenCondition>),
    /// `<when-in-parens> [ and <when-in-parens> ]+` — conjunction.
    /// Always two or more operands.
    And(Box<[WhenCondition]>),
    /// `<when-in-parens> [ or <when-in-parens> ]+` — disjunction.
    /// Always two or more operands.
    Or(Box<[WhenCondition]>),
    /// `supports( <supports-condition> )` — eagerly evaluated at parse
    /// time. We retain the original `SupportsCondition` for CSSOM
    /// round-trip and keep the pre-computed verdict for the
    /// device-time evaluator to consult.
    Supports {
        /// The parsed predicate.
        condition: SupportsCondition,
        /// The pre-computed verdict from `condition.eval(&ParserContext)`.
        result: bool,
    },
    /// `media( <media-condition> )` — the body wraps a `<media-condition>`
    /// (a single boolean media expression with no media-type prefix
    /// and no comma list, per CSS Conditional 5 §3.1).
    Media(Arc<Locked<MediaList>>),
    /// `<general-enclosed>` fallback for forwards-compatible parsing
    /// of unknown leaves — always evaluates `false` per CSS Syntax 3
    /// §5.4.1. We retain the source text for CSSOM serialisation.
    GeneralEnclosed(String),
}

impl WhenCondition {
    /// Parse a `<when-condition>` from the current parser position.
    ///
    /// <https://drafts.csswg.org/css-conditional-5/#typedef-when-condition>
    ///
    /// `context` is taken by `&mut` so that the eager evaluation of
    /// `supports(...)` leaves can run inside
    /// `ParserContext::nest_for_rule(CssRuleType::Style, …)` — the
    /// same nesting `SupportsRule::eval` already requires (see
    /// `style/stylesheets/supports_rule.rs::Declaration::eval`).
    ///
    /// `shared_lock` is the stylesheet's `SharedRwLock`, required to
    /// wrap the inner `MediaList` produced by `media(...)` leaves.
    pub fn parse<'i, 't>(
        context: &mut ParserContext,
        shared_lock: &SharedRwLock,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("not")).is_ok() {
            let inner = Self::parse_in_parens(context, shared_lock, input)?;
            return Ok(WhenCondition::Not(Box::new(inner)));
        }

        let first = Self::parse_in_parens(context, shared_lock, input)?;

        let location = input.current_source_location();
        // Closures used as `match`-arm values lose their identity to
        // each other, so we have to coerce each one to a function
        // pointer with an explicit signature.
        fn build_and(v: Vec<WhenCondition>) -> WhenCondition {
            WhenCondition::And(v.into_boxed_slice())
        }
        fn build_or(v: Vec<WhenCondition>) -> WhenCondition {
            WhenCondition::Or(v.into_boxed_slice())
        }
        let (keyword, wrapper): (&str, fn(Vec<WhenCondition>) -> WhenCondition) = match input.next()
        {
            Err(..) => return Ok(first),
            Ok(&Token::Ident(ref ident)) => match_ignore_ascii_case! { &ident,
                "and" => ("and", build_and),
                "or" => ("or", build_or),
                _ => return Err(location.new_custom_error(
                    StyleParseErrorKind::UnspecifiedError,
                )),
            },
            Ok(t) => return Err(location.new_unexpected_token_error(t.clone())),
        };

        let mut conditions: Vec<WhenCondition> = Vec::with_capacity(2);
        conditions.push(first);
        loop {
            conditions.push(Self::parse_in_parens(context, shared_lock, input)?);
            if input
                .try_parse(|input| input.expect_ident_matching(keyword))
                .is_err()
            {
                return Ok(wrapper(conditions));
            }
        }
    }

    /// Parse a `<when-in-parens>` — either a parenthesised
    /// `<when-condition>`, or one of the functional leaves
    /// `supports(...)` / `media(...)`. Anything else is captured as a
    /// `<general-enclosed>` block so the surrounding rule remains
    /// valid per CSS Syntax 3 forwards-compatible parsing.
    fn parse_in_parens<'i, 't>(
        context: &mut ParserContext,
        shared_lock: &SharedRwLock,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        input.skip_whitespace();
        let start = input.position();
        let location = input.current_source_location();
        match *input.next()? {
            Token::ParenthesisBlock => {
                let nested = input.try_parse(|input| {
                    input.parse_nested_block(|input| Self::parse(context, shared_lock, input))
                });
                if let Ok(nested) = nested {
                    return Ok(WhenCondition::InParens(Box::new(nested)));
                }
            },
            Token::Function(ref name) => {
                let function_name = name.clone();
                let leaf = match_ignore_ascii_case! { &function_name,
                    "supports" => {
                        // The CSS Conditional 5 §3.1 `supports(...)`
                        // leaf accepts either a fully-parenthesised
                        // `<supports-condition>` (`supports((color:
                        // red))`) or a bare `<declaration>`
                        // (`supports(color: red)`). The existing
                        // `parse_condition_or_declaration` helper
                        // covers both shapes, matching how
                        // `@import supports(...)` is already parsed
                        // by Stylo.
                        let parsed = input.try_parse(|input| {
                            input.parse_nested_block(parse_condition_or_declaration)
                        });
                        match parsed {
                            Ok(condition) => {
                                // Eager evaluation matches the
                                // `SupportsRule.enabled` contract:
                                // syntactic support is
                                // device-independent and the
                                // `ParserContext` we hold is the
                                // freshest UA snapshot available.
                                // `Declaration::eval` requires
                                // `CssRuleType::Style` in the
                                // nesting context (see
                                // `supports_rule.rs::Declaration::eval`'s
                                // debug-assert), so we nest just as
                                // `SupportsRule` does.
                                let result = context.nest_for_rule(
                                    CssRuleType::Style,
                                    |ctx| condition.eval(ctx),
                                );
                                Some(WhenCondition::Supports { condition, result })
                            },
                            Err(_) => None,
                        }
                    },
                    "media" => {
                        let parsed = input.try_parse(|input| -> Result<MediaList, ParseError<'i>> {
                            input.parse_nested_block(|input| Ok(MediaList::parse(context, input)))
                        });
                        match parsed {
                            Ok(list) => Some(WhenCondition::Media(Arc::new(
                                shared_lock.wrap(list),
                            ))),
                            Err(_) => None,
                        }
                    },
                    _ => None,
                };
                if let Some(leaf) = leaf {
                    return Ok(leaf);
                }
            },
            ref t => return Err(location.new_unexpected_token_error(t.clone())),
        }
        input.parse_nested_block(consume_any_value)?;
        Ok(WhenCondition::GeneralEnclosed(
            input.slice_from(start).to_owned(),
        ))
    }

    /// Evaluate this condition against the current device. The
    /// `supports(...)` leaves return their pre-computed verdict (set
    /// at parse time); the `media(...)` leaves evaluate freshly.
    pub fn eval(
        &self,
        device: &Device,
        quirks_mode: QuirksMode,
        custom_media_map: &CustomMediaMap,
        guard: &SharedRwLockReadGuard,
    ) -> bool {
        match *self {
            WhenCondition::Not(ref inner) => {
                !inner.eval(device, quirks_mode, custom_media_map, guard)
            },
            WhenCondition::InParens(ref inner) => {
                inner.eval(device, quirks_mode, custom_media_map, guard)
            },
            WhenCondition::And(ref operands) => operands
                .iter()
                .all(|c| c.eval(device, quirks_mode, custom_media_map, guard)),
            WhenCondition::Or(ref operands) => operands
                .iter()
                .any(|c| c.eval(device, quirks_mode, custom_media_map, guard)),
            WhenCondition::Supports { result, .. } => result,
            WhenCondition::Media(ref list) => list.read_with(guard).evaluate(
                device,
                quirks_mode,
                &mut CustomMediaEvaluator::new(custom_media_map, guard),
            ),
            WhenCondition::GeneralEnclosed(_) => false,
        }
    }
}

/// Helper to consume any well-formed token sequence inside parens.
/// Mirrors `supports_rule::consume_any_value`.
fn consume_any_value<'i, 't>(input: &mut Parser<'i, 't>) -> Result<(), ParseError<'i>> {
    input.expect_no_error_token().map_err(|err| err.into())
}

/// Serialise a `WhenCondition` with the read guard available. Required
/// for the `WhenCondition::Media` arm, which holds a `Locked<MediaList>`.
fn serialise_condition_with_guard(
    condition: &WhenCondition,
    guard: &SharedRwLockReadGuard,
    dest: &mut CssStringWriter,
) -> fmt::Result {
    match *condition {
        WhenCondition::Not(ref inner) => {
            dest.write_str("not ")?;
            serialise_condition_with_guard(inner, guard, dest)
        },
        WhenCondition::InParens(ref inner) => {
            dest.write_char('(')?;
            serialise_condition_with_guard(inner, guard, dest)?;
            dest.write_char(')')
        },
        WhenCondition::And(ref ops) => {
            let mut first = true;
            for op in ops.iter() {
                if !first {
                    dest.write_str(" and ")?;
                }
                first = false;
                serialise_condition_with_guard(op, guard, dest)?;
            }
            Ok(())
        },
        WhenCondition::Or(ref ops) => {
            let mut first = true;
            for op in ops.iter() {
                if !first {
                    dest.write_str(" or ")?;
                }
                first = false;
                serialise_condition_with_guard(op, guard, dest)?;
            }
            Ok(())
        },
        WhenCondition::Supports { ref condition, .. } => {
            dest.write_str("supports(")?;
            condition.to_css(&mut CssWriter::new(dest))?;
            dest.write_char(')')
        },
        WhenCondition::Media(ref list) => {
            dest.write_str("media(")?;
            list.read_with(guard).to_css(&mut CssWriter::new(dest))?;
            dest.write_char(')')
        },
        WhenCondition::GeneralEnclosed(ref s) => dest.write_str(s),
    }
}

/// An ordered sequence of every condition in a `@when` / `@else`
/// chain. Entry 0 is the leading `@when`'s condition (always
/// `Some`); each subsequent entry corresponds to a chained `@else`
/// — `Some(cond)` for `@else <cond>`, `None` for the trailing
/// unconditional `@else`.
///
/// All chain members hold the same `Arc<ChainConditions>` so the
/// per-branch enabled decision can be made by scanning entries
/// `[0..self.chain_position]` and evaluating `self.chain_position`
/// itself, with the guarantee that the answer is consistent across
/// every member when the device hasn't changed.
pub type ChainConditions = [Option<WhenCondition>];

/// Decide whether the chain member at `position` is active given
/// the device. The first member whose condition evaluates true (or
/// whose condition is `None`, marking the trailing default branch)
/// is the active branch; every member after it is inactive even if
/// its own condition would have matched.
pub(crate) fn chain_member_is_enabled(
    chain: &ChainConditions,
    position: usize,
    device: &Device,
    quirks_mode: QuirksMode,
    custom_media_map: &CustomMediaMap,
    guard: &SharedRwLockReadGuard,
) -> bool {
    debug_assert!(position < chain.len(), "chain position out of bounds");
    for earlier in &chain[..position] {
        let matches = match earlier {
            Some(cond) => cond.eval(device, quirks_mode, custom_media_map, guard),
            None => true,
        };
        if matches {
            return false;
        }
    }
    match &chain[position] {
        Some(cond) => cond.eval(device, quirks_mode, custom_media_map, guard),
        None => true,
    }
}

/// An [`@when`][when] rule.
///
/// [when]: https://drafts.csswg.org/css-conditional-5/#when-rule
#[derive(Debug, ToShmem)]
pub struct WhenRule {
    /// The parsed condition.
    pub condition: WhenCondition,
    /// Child rules.
    pub rules: Arc<Locked<CssRules>>,
    /// Snapshot of every condition in this chain, shared with every
    /// `@else` rule that follows. Entry 0 is this `@when`'s own
    /// condition.
    pub chain: Arc<Box<ChainConditions>>,
    /// Source-location stamp used for cascade ordering & CSSOM.
    pub source_location: SourceLocation,
}

impl WhenRule {
    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        self.rules.unconditional_shallow_size_of(ops)
            + self.rules.read_with(guard).size_of(guard, ops)
    }

    /// Is this `@when` the active branch for its chain?
    pub fn enabled(
        &self,
        device: &Device,
        quirks_mode: QuirksMode,
        custom_media_map: &CustomMediaMap,
        guard: &SharedRwLockReadGuard,
    ) -> bool {
        chain_member_is_enabled(&self.chain, 0, device, quirks_mode, custom_media_map, guard)
    }
}

impl ToCssWithGuard for WhenRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        dest.write_str("@when ")?;
        serialise_condition_with_guard(&self.condition, guard, dest)?;
        self.rules.read_with(guard).to_css_block(guard, dest)
    }
}

impl DeepCloneWithLock for WhenRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        let rules = self.rules.read_with(guard);
        WhenRule {
            condition: self.condition.clone(),
            rules: Arc::new(lock.wrap(rules.deep_clone_with_lock(lock, guard))),
            chain: self.chain.clone(),
            source_location: self.source_location.clone(),
        }
    }
}

/// An [`@else`][else_link] rule.
///
/// [else_link]: https://drafts.csswg.org/css-conditional-5/#else-rule
///
/// The trailing `@else { … }` form (no condition) carries
/// `condition: None` and represents the unconditional fallback at the
/// tail of a `@when` / `@else` chain. A guarded `@else (<condition>)
/// { … }` carries `Some(WhenCondition)`.
#[derive(Debug, ToShmem)]
pub struct ElseRule {
    /// The parsed condition, if any. `None` for the trailing default
    /// branch.
    pub condition: Option<WhenCondition>,
    /// Child rules.
    pub rules: Arc<Locked<CssRules>>,
    /// Snapshot of every condition in this chain. Position 0 is the
    /// leading `@when`'s condition; this rule sits at
    /// `chain_position`.
    pub chain: Arc<Box<ChainConditions>>,
    /// Index of this rule inside the shared `chain`. Always
    /// `>= 1` because position 0 is the leading `@when`.
    pub chain_position: u32,
    /// Source-location stamp.
    pub source_location: SourceLocation,
}

impl ElseRule {
    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        self.rules.unconditional_shallow_size_of(ops)
            + self.rules.read_with(guard).size_of(guard, ops)
    }

    /// Is this `@else` the active branch for its chain?
    pub fn enabled(
        &self,
        device: &Device,
        quirks_mode: QuirksMode,
        custom_media_map: &CustomMediaMap,
        guard: &SharedRwLockReadGuard,
    ) -> bool {
        chain_member_is_enabled(
            &self.chain,
            self.chain_position as usize,
            device,
            quirks_mode,
            custom_media_map,
            guard,
        )
    }
}

impl ToCssWithGuard for ElseRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        dest.write_str("@else")?;
        if let Some(ref condition) = self.condition {
            dest.write_char(' ')?;
            serialise_condition_with_guard(condition, guard, dest)?;
        }
        self.rules.read_with(guard).to_css_block(guard, dest)
    }
}

impl DeepCloneWithLock for ElseRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        let rules = self.rules.read_with(guard);
        ElseRule {
            condition: self.condition.clone(),
            rules: Arc::new(lock.wrap(rules.deep_clone_with_lock(lock, guard))),
            chain: self.chain.clone(),
            chain_position: self.chain_position,
            source_location: self.source_location.clone(),
        }
    }
}
