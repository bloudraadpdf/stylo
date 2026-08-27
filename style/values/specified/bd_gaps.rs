/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe smaller standard-CSS gap fillers (F21).
//!
//! Properties in this module are either Stylo upstream additions
//! that the fork ungated for non-Gecko engines (e.g. `overlay`) or
//! Prince proprietary surface admitted as a native `-bd-*` keyword
//! (`border-clip`). The CSS-standard names are kept where possible;
//! Prince aliases live in the moegoe-css compat translator.

use crate::derives::*;

/// Controls how gap decorations break at visible intersections.
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
pub enum RuleBreak {
    None,
    #[default]
    Normal,
    Intersection,
}

/// Specified value of `overlay` (F21.24).
///
/// CSS Position 4 — controls whether an element participates in
/// the top-layer overlay (popover / modal-dialog stack). `auto`
/// defers to the element's open/closed state; `none` (initial)
/// keeps it in normal flow. Animation-only-keyword spec preserved.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum Overlay {
    #[default]
    None,
    Auto,
}

/// Specified value of `-bd-border-clip` (Tier 5 §A.5.6).
///
/// Native counterpart to Prince's `border-clip`. Controls the geometry
/// used to join two adjacent border sides at a rounded corner when
/// `border-radius` is non-zero. CSS Backgrounds 3 §7.7 does not
/// prescribe a single shape for the join — Prince admits three:
///
/// - `square` (initial) — the corner is closed by a straight diagonal
///   miter from the outer-radius arc endpoint to the inner-radius arc
///   endpoint. Matches the default CSS Backgrounds 3 behaviour.
/// - `round` — the corner is closed by an arc that follows the inner
///   border radius, smoothing the colour/style transition.
/// - `bevel` — the corner is closed by a flat cut perpendicular to the
///   line bisecting the two adjacent sides.
///
/// The property only affects paint geometry when adjacent sides differ
/// in colour or style; uniform-side borders draw a single rounded ring
/// regardless of value.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BorderClip {
    /// Straight diagonal miter (CSS Backgrounds 3 default).
    #[default]
    Square,
    /// Arc following the inner border radius.
    Round,
    /// Flat cut perpendicular to the corner bisector.
    Bevel,
}

/// Specified value of `mask-border-mode` (F21.8).
///
/// Determines whether the mask-border source image is interpreted
/// via its luminance or alpha channel. Mirrors mask-mode but applies
/// to the mask-border family.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum MaskBorderMode {
    #[default]
    Alpha,
    Luminance,
}
