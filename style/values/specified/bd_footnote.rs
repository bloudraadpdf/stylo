/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe footnote-area styling extensions (F6).
//!
//! Native fork-extension surface for footnote-area styling beyond
//! `@footnote` / `footnote-display` / `footnote-policy`. These
//! longhands cover the typographic short-rule above the footnote
//! area, inside/outside marker positioning, and the PDFreactor
//! footnote-fragmentation knob. The audit at
//! `docs/audits/CSS-COVERAGE-AUDIT-2026-05-14/stylo-push-plan.md`
//! family 6 enumerates the source vendors and citations.

use crate::derives::*;
use crate::values::specified::length::LengthPercentage;

/// Specified value of `-bd-footnote-rule-length`.
///
/// Length of the short typographic rule drawn above the footnote
/// area. Initial `100%` (full inline-size). Maps onto
/// PDFreactor `-ro-border-length` per the audit.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped, Parse)]
#[repr(C)]
pub struct BdFootnoteRuleLength(pub LengthPercentage);

impl BdFootnoteRuleLength {
    /// Initial value (`100%`).
    #[inline]
    pub fn full() -> Self {
        Self(LengthPercentage::hundred_percent())
    }
}

/// Specified value of `footnote-style-position`.
///
/// Marker placement on the outer (binding side away) or inner
/// (binding side near) edge. Inherits.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum FootnoteStylePosition {
    #[default]
    Inside,
    Outside,
}

/// Specified value of `-bd-footnote-fragmentation`.
///
/// Governs whether footnotes may split across pages.
/// `auto` (initial) leaves the heuristic to the paginator;
/// `normal` permits splitting; `keep` forbids it.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdFootnoteFragmentation {
    #[default]
    Auto,
    Normal,
    Keep,
}

/// Specified value of `float-placement`.
///
/// CSS Page Floats 3 §3.2.4 keyword, extended per Prince to admit
/// `inline-footnote`. Initial `block` (CSS default for floats).
/// Not inherited.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum FloatPlacement {
    #[default]
    Block,
    Column,
    Region,
    Page,
    InlineFootnote,
}
