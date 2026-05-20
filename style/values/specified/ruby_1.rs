/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Ruby Annotation Layout Module Level 1 longhands.
//!
//! Implements `ruby-merge` (§4.3) and `ruby-overhang` (§4.4):
//!
//! - <https://drafts.csswg.org/css-ruby-1/#rubymerge>
//! - <https://drafts.csswg.org/css-ruby-1/#rubyoverhang>
//!
//! Both are inherited and apply to ruby annotation containers. The
//! moegoe ingestion layer reads these alongside `ruby-position` and
//! `ruby-align` to drive annotation grouping and inter-character
//! overhang policy.

use crate::derives::*;

/// Specified value of the `ruby-merge` property
/// (<https://drafts.csswg.org/css-ruby-1/#rubymerge>).
///
/// Controls how consecutive ruby annotation containers in a single
/// ruby segment combine their annotation glyphs.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum RubyMerge {
    /// Each annotation stays paired with its corresponding base.
    #[default]
    Separate,
    /// All annotations in a segment are merged into a single run.
    Collapse,
    /// The UA picks `separate` or `collapse` based on heuristics.
    Auto,
}

/// Specified value of the `ruby-overhang` property
/// (<https://drafts.csswg.org/css-ruby-1/#rubyoverhang>).
///
/// Controls whether ruby annotations are allowed to overhang adjacent
/// content outside the ruby base.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum RubyOverhang {
    /// Annotations may overhang adjacent atomic inlines.
    #[default]
    Auto,
    /// Annotations are clipped to the ruby base extents.
    None,
}
