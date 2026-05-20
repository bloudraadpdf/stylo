/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for CSS Ruby Annotation Layout Module Level 1
//! longhands.
//!
//! Specified-to-computed is the identity for both `ruby-merge` and
//! `ruby-overhang`; the specified types derive `ToComputedValue`.

pub use crate::values::specified::ruby_1::{RubyMerge, RubyOverhang};
