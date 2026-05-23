/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Line Grid Module Level 1 — standard `line-grid` (§3),
//! `line-snap` (§6.1), and `box-snap` (§6.2) properties.
//!
//! <https://drafts.csswg.org/css-line-grid-1/#line-grid>
//! <https://drafts.csswg.org/css-line-grid-1/#line-snap-property>
//! <https://drafts.csswg.org/css-line-grid-1/#box-snap>
//!
//! The moegoe-native `-bd-line-grid` longhand (`bd_line_grid.rs`)
//! carries `none | match-parent | create` — moegoe's superset that adds
//! `none` for authoring convenience. The standard `line-grid` defined
//! here uses the spec-mandated two keywords `match-parent | create`
//! with `match-parent` as the initial value (per spec §3, contrast
//! with `-bd-line-grid`'s `None` initial).
//!
//! `line-snap` controls whether line boxes snap to the inherited line
//! grid (§6.1): `none | baseline | contain`. Inherited.
//!
//! `box-snap` controls how a box snaps to its line grid:
//! `none | block-start | block-end | center | baseline | last-baseline`.
//!
//! The moegoe-native `-bd-line-snap` longhand (`bd_line_grid.rs`) shares
//! the standard's three-keyword grammar; both surfaces project onto the
//! same downstream IR variant in `moegoe-css::computed_to_ir`.

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

/// Specified value of the standard `line-snap` property (§6.1).
///
/// `none` (initial) — line boxes are not snapped to the line grid.
/// `baseline` — the dominant baseline of each line box is snapped to
/// the nearest grid baseline.
/// `contain` — the entire line box is snapped to one or more grid
/// cells, expanding to the next cell when the natural line height
/// exceeds a single cell.
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
pub enum LineSnap {
    /// `none` — no grid snapping (initial).
    #[default]
    None,
    /// `baseline` — snap the dominant baseline to the grid.
    Baseline,
    /// `contain` — snap the entire line box into grid cells.
    Contain,
}

// Note: `LineSnap` deliberately omits `Copy` because it is stored on
// the inherited-text style struct, which Stylo's codegen treats as
// `NotCopy` for declaration storage (see the `Helper<…>::assert()`
// branches in the generated `properties.rs`).

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
