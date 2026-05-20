/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Line Grid Module Level 1 — standard `line-grid` (§3) and
//! `box-snap` (§6) properties.
//!
//! <https://drafts.csswg.org/css-line-grid-1/#line-grid>
//! <https://drafts.csswg.org/css-line-grid-1/#box-snap>
//!
//! The moegoe-native `-bd-line-grid` longhand (`bd_line_grid.rs`)
//! carries `none | match-parent | create` — moegoe's superset that adds
//! `none` for authoring convenience. The standard `line-grid` defined
//! here uses the spec-mandated two keywords `match-parent | create`
//! with `match-parent` as the initial value (per spec §3, contrast
//! with `-bd-line-grid`'s `None` initial).
//!
//! `box-snap` controls how a box snaps to its line grid:
//! `none | block-start | block-end | center | baseline | last-baseline`.

use crate::derives::*;

/// Specified value of the standard `line-grid` property (§3).
///
/// `match-parent` (initial) — the element shares its parent's grid.
/// `create` — the element establishes a new line grid.
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
pub enum LineGrid {
    /// `match-parent` — element shares parent grid (initial).
    #[default]
    MatchParent,
    /// `create` — element establishes a new line grid.
    Create,
}

/// Specified value of the `box-snap` property (§6).
///
/// `none` (initial) — the box does not snap to the line grid.
/// `block-start | block-end | center | baseline | last-baseline` —
/// snap the corresponding box edge / baseline to the grid.
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
pub enum BoxSnap {
    /// `none` — no grid snapping (initial).
    #[default]
    None,
    /// `block-start` — snap the block-start edge.
    BlockStart,
    /// `block-end` — snap the block-end edge.
    BlockEnd,
    /// `center` — snap the block-axis centre.
    Center,
    /// `baseline` — snap the first baseline.
    Baseline,
    /// `last-baseline` — snap the last baseline.
    LastBaseline,
}
