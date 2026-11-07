/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `@navigation` conditional group rule (CSS View Transitions 2, two-phase cross-document navigation).

use crate::derives::*;
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::stylesheets::CssRules;
use cssparser::{Parser, SourceLocation};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, ParseError};

/// The navigation phase of a document, which `@navigation` conditions query.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum NavigationPhase {
    /// No navigation preview runs.
    #[default]
    Idle,
    /// The document shows the preview of an outbound navigation.
    Preview,
}

/// An `@navigation` condition.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ToShmem)]
pub enum NavigationCondition {
    /// `(preview)`.
    Preview,
}

impl NavigationCondition {
    /// Parse an `@navigation` prelude.
    pub fn parse<'i, 't>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i>> {
        input.expect_parenthesis_block()?;
        let condition = input.parse_nested_block(|input| {
            Ok(try_match_ident_ignore_ascii_case! { input,
                "preview" => Self::Preview,
            })
        })?;
        input.expect_exhausted()?;
        Ok(condition)
    }

    /// Whether the condition matches the navigation phase of the document.
    pub fn matches(self, phase: NavigationPhase) -> bool {
        match self {
            Self::Preview => phase == NavigationPhase::Preview,
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
        self.rules.unconditional_shallow_size_of(ops)
            + self.rules.read_with(guard).size_of(guard, ops)
    }
}

impl ToCssWithGuard for NavigationRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        dest.write_str(match self.condition {
            NavigationCondition::Preview => "@navigation (preview)",
        })?;
        self.rules.read_with(guard).to_css_block(guard, dest)
    }
}

impl DeepCloneWithLock for NavigationRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        let rules = self.rules.read_with(guard);
        NavigationRule {
            condition: self.condition,
            rules: Arc::new(lock.wrap(rules.deep_clone_with_lock(lock, guard))),
            source_location: self.source_location.clone(),
        }
    }
}
