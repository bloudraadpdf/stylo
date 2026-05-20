/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Box Sizing Module Level 4 — `min-intrinsic-sizing` (§5.4).
//!
//! <https://drafts.csswg.org/css-sizing-4/#intrinsic-contribution-override>
//!
//! Grammar: `legacy | zero-if-scroll || zero-if-extrinsic`. The
//! `||` operator means "any combination, any order" of the two flag
//! keywords; `legacy` is the empty-flag sentinel (and is exclusive
//! with either flag being set).
//!
//! The value modulates how a box's intrinsic-size contribution to its
//! containing block is computed when the box has a scrollable overflow
//! region or its size is extrinsic (resolved against the containing
//! block rather than its own content).

use crate::derives::*;
use bitflags::bitflags;
use std::fmt::Write;
use style_traits::StyleParseErrorKind;

/// Specified value of the `min-intrinsic-sizing` property.
///
/// `legacy` is the empty-bits sentinel. Otherwise the value carries
/// one or both of `ZERO_IF_SCROLL` / `ZERO_IF_EXTRINSIC`. The parser
/// refuses to combine `legacy` with any flag.
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
#[css(bitflags(single = "legacy", mixed = "zero-if-scroll,zero-if-extrinsic"))]
#[repr(C)]
pub struct MinIntrinsicSizing(u8);
bitflags! {
    impl MinIntrinsicSizing: u8 {
        /// `legacy` — the empty-bit sentinel; preserves CSS 2.1
        /// intrinsic-contribution behaviour.
        const LEGACY = 0;
        /// `zero-if-scroll` — clamps the intrinsic contribution to
        /// zero when the box has scrollable overflow.
        const ZERO_IF_SCROLL = 1 << 0;
        /// `zero-if-extrinsic` — clamps the intrinsic contribution
        /// to zero when the size is extrinsic (resolved against the
        /// containing block).
        const ZERO_IF_EXTRINSIC = 1 << 1;
    }
}

impl MinIntrinsicSizing {
    /// Initial value (`legacy`).
    #[inline]
    pub fn legacy() -> Self {
        Self::empty()
    }

    /// Whether the value is the `legacy` sentinel (no flag bits).
    #[inline]
    pub fn is_legacy(&self) -> bool {
        self.is_empty()
    }
}
