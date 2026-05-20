/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Display Module Level 4 longhands.
//!
//! Implements `reading-flow` (§3.1):
//!
//! - <https://drafts.csswg.org/css-display-4/#reading-flow>
//!
//! `reading-order` (§3.2) is also defined by Display 4 but uses
//! `<integer>` and is therefore wired directly in `longhands.toml` as a
//! plain `Integer` longhand sharing the same machinery as the flex
//! `order` property. Only `reading-flow` requires a bespoke keyword
//! enum, defined here.
//!
//! Both properties cascade through the `inherited_box` struct because
//! reading order is inherently a sequential-navigation property that
//! descendants observe transparently.

use crate::derives::*;

/// Specified value of the `reading-flow` property
/// (<https://drafts.csswg.org/css-display-4/#reading-flow>).
///
/// Selects which traversal order is exposed to sequential-navigation
/// agents (focus order, screen-reader linearisation) within a flex or
/// grid container.
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
pub enum ReadingFlow {
    /// Source order — the initial value.
    #[default]
    Normal,
    /// For flex containers: traversal follows the visual main-axis
    /// order.
    FlexVisual,
    /// For flex containers: traversal follows the resolved
    /// `flex-direction` (including reverse).
    FlexFlow,
    /// For grid containers: traversal follows row-major visual order.
    GridRows,
    /// For grid containers: traversal follows column-major visual
    /// order.
    GridColumns,
    /// For grid containers: traversal follows the `grid-auto-flow`
    /// placement order.
    GridOrder,
    /// Source order. Explicit synonym for `normal` retained for spec
    /// fidelity.
    SourceOrder,
}
