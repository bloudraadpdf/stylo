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
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped, Parse,
)]
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
/// Controls how the footnote marker is positioned relative to the
/// footnote body's first line. `outside` (initial, per Prince's
/// reference manual `prince.md:4208`) renders the marker as a
/// hanging marker on the inline-start edge of the first body line;
/// `inside` renders the marker as the first inline item in the
/// body's flow. Inherits.
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
    Outside,
    Inside,
}

/// Specified value of `-bd-footnote-fragmentation`.
///
/// Governs the paginator's response when a footnote body overflows
/// the bottom of the page on which its reference falls. Vocabulary
/// is moegoe's four-arm refinement of PDFreactor matrix line 15234
/// (`-ro-footnote-fragmentation: auto | none`):
///
/// - `continue` (initial) — split the footnote body across pages;
///   the call's page hosts the head, subsequent pages host the
///   continuation. Matches PDFreactor `auto`.
/// - `repeat`           — split as `continue`, and re-emit the
///   footnote marker at the start of every continuation page.
/// - `break`            — force a page break *before* the
///   footnote's call so the entire body fits on one page (the
///   call moves with the body).
/// - `avoid`            — keep the call on its current page but
///   defer the body to the next page's footnote area. Matches
///   PDFreactor `none`.
///
/// Inherits per CSS GCPM 3 §2.5 and PDFreactor.
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
    Continue,
    Repeat,
    Break,
    Avoid,
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
