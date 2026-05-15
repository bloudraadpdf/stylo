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

/// Specified value of `border-clip` (F21.13).
///
/// Prince proprietary — controls whether a border is clipped at the
/// padding edge of the inner box. `normal` (initial) — standard
/// behaviour; the border is drawn unclipped. `clip` — the border
/// stroke is clipped to the padding-box outline.
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
    #[default]
    Normal,
    Clip,
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
