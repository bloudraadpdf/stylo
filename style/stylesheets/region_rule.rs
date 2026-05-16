/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A top-level `@region <selector> { <declarations> }` rule.
//!
//! moegoe Family 17 — CSS Regions Level 1 §6.4 region styling
//! (PDFreactor / Prince compatibility). The rule carries a standard
//! CSS selector list that targets elements *when they appear inside
//! a region-chain descendant*; the moegoe-css cascade reader is
//! responsible for scoping the declarations to named-flow content.
//!
//! Mirrors `style_rule.rs` (selector + declaration block) but is
//! kept distinct because:
//!
//! 1. The rule has different cascade semantics (descendant-of-region
//!    predicate that the standard selector engine cannot express).
//! 2. It must be enumerable independently from the regular style-rule
//!    stream so moegoe-css can walk and apply it.
//!
//! Nested rules and `&` nesting are not supported — the spec text
//! only defines a flat declaration body.

use crate::derives::*;
use crate::properties::PropertyDeclarationBlock;
use crate::selector_parser::SelectorImpl;
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use cssparser::SourceLocation;
#[cfg(feature = "gecko")]
use malloc_size_of::{
    MallocSizeOf, MallocSizeOfOps, MallocUnconditionalShallowSizeOf, MallocUnconditionalSizeOf,
};
use selectors::SelectorList;
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::CssStringWriter;

/// A top-level `@region` rule.
///
/// The selector list constrains *which elements* receive the
/// declarations; the at-rule itself further constrains *when* —
/// only when the matched element is a descendant of an element
/// participating in a CSS Regions named-flow chain.
#[derive(Debug, ToShmem)]
pub struct RegionRule {
    /// Selector list (parsed via the standard CSS selector parser).
    pub selectors: SelectorList<SelectorImpl>,
    /// Property declaration block (the body of the rule).
    pub block: Arc<Locked<PropertyDeclarationBlock>>,
    /// Source position the rule was found at.
    pub source_location: SourceLocation,
}

impl RegionRule {
    /// Measure heap usage. Mirrors `StyleRule::size_of`.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        let mut n = 0;
        n += self.selectors.unconditional_size_of(ops);
        n += self.block.unconditional_shallow_size_of(ops)
            + self.block.read_with(guard).size_of(ops);
        n
    }
}

impl ToCssWithGuard for RegionRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        use cssparser::ToCss;
        dest.write_str("@region ")?;
        self.selectors.to_css(dest)?;
        dest.write_str(" { ")?;
        let declaration_block = self.block.read_with(guard);
        declaration_block.to_css(dest)?;
        if !declaration_block.declarations().is_empty() {
            dest.write_char(' ')?;
        }
        dest.write_char('}')
    }
}

impl DeepCloneWithLock for RegionRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        RegionRule {
            selectors: self.selectors.clone(),
            block: Arc::new(lock.wrap(self.block.read_with(guard).clone())),
            source_location: self.source_location,
        }
    }
}
