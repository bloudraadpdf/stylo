/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A nested `@-bd-sidenote` rule inside `@page`.
//!
//! moegoe Family 7 — native sidenote area. Each authored rule may carry an optional
//! name identifier (e.g. `@-bd-sidenote left { ... }`) which scopes
//! the descriptors to a named sidenote flow. The block is a property
//! declaration list; the recognised descriptor longhands are the
//! `-bd-sidenote-*` family registered in `longhands.toml` plus the
//! geometry shorthand longhands inherited from generic page-area
//! handling (`flow`, `width`, `offset`, `align`, `gap`).
//!
//! The at-rule mirrors `@footnote` (`footnote_rule.rs`) so moegoe's
//! page-extract walker can iterate sidenote rules alongside margin
//! boxes and footnote areas.

use crate::derives::*;
use crate::properties::PropertyDeclarationBlock;
use crate::shared_lock::{DeepCloneWithLock, Locked};
use crate::shared_lock::{SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::values::AtomIdent;
use cssparser::SourceLocation;
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOf, MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::CssStringWriter;

/// A nested `@-bd-sidenote` rule.
///
/// The optional `name` selects a specific sidenote flow (e.g.
/// `@-bd-sidenote left`); when absent the rule applies to the
/// default flow.
#[derive(Clone, Debug, ToShmem)]
pub struct SidenoteRule {
    /// Optional sidenote-flow name. `None` for an unnamed rule.
    pub name: Option<AtomIdent>,
    /// The declaration block this sidenote rule contains.
    pub block: Arc<Locked<PropertyDeclarationBlock>>,
    /// The source position this rule was found at.
    pub source_location: SourceLocation,
}

impl SidenoteRule {
    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        self.block.unconditional_shallow_size_of(ops) + self.block.read_with(guard).size_of(ops)
    }

    /// Gets the CSS rule name for this nested rule.
    #[inline]
    pub fn name_token(&self) -> &'static str {
        "-bd-sidenote"
    }
}

impl ToCssWithGuard for SidenoteRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        dest.write_str("@-bd-sidenote")?;
        if let Some(name) = &self.name {
            dest.write_char(' ')?;
            // AtomIdent serialises with proper escaping via its Display impl.
            write!(dest, "{}", name.0)?;
        }
        dest.write_str(" { ")?;
        let declaration_block = self.block.read_with(guard);
        declaration_block.to_css(dest)?;
        if !declaration_block.declarations().is_empty() {
            dest.write_char(' ')?;
        }
        dest.write_char('}')
    }
}

impl DeepCloneWithLock for SidenoteRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        SidenoteRule {
            name: self.name.clone(),
            block: Arc::new(lock.wrap(self.block.read_with(guard).clone())),
            source_location: self.source_location,
        }
    }
}
