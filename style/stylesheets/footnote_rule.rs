/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A nested [`@footnote`][footnote] rule inside `@page`.
//!
//! [footnote]: https://www.w3.org/TR/css-gcpm-3/#footnote-display

use crate::derives::*;
use crate::properties::PropertyDeclarationBlock;
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use cssparser::SourceLocation;
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOf, MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::CssStringWriter;

/// A nested `@footnote` rule.
#[derive(Clone, Debug, ToShmem)]
pub struct FootnoteRule {
    /// The declaration block this footnote rule contains.
    pub block: Arc<Locked<PropertyDeclarationBlock>>,
    /// The source position this rule was found at.
    pub source_location: SourceLocation,
}

impl FootnoteRule {
    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        self.block.unconditional_shallow_size_of(ops) + self.block.read_with(guard).size_of(ops)
    }

    /// Gets the CSS rule name for this nested rule.
    #[inline]
    pub fn name(&self) -> &'static str {
        "footnote"
    }
}

impl ToCssWithGuard for FootnoteRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        dest.write_str("@footnote { ")?;
        let declaration_block = self.block.read_with(guard);
        declaration_block.to_css(dest)?;
        if !declaration_block.declarations().is_empty() {
            dest.write_char(' ')?;
        }
        dest.write_char('}')
    }
}

impl DeepCloneWithLock for FootnoteRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        FootnoteRule {
            block: Arc::new(lock.wrap(self.block.read_with(guard).clone())),
            source_location: self.source_location,
        }
    }
}
