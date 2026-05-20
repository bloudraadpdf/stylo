/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-running-copy` property.
//!
//! Replaces the prior `--moegoe-running-copy` custom-property hatch
//! (see `crates/moegoe-css/src/computed_to_ir.rs` documentation note
//! on the running-element conversion) with a first-class Stylo
//! longhand.
//!
//! GCPM §1.2.1 spec note describes a "copy" variant of the running-
//! element pattern where an element flagged with `position:
//! running(<name>)` is removed from the normal flow and surfaced only
//! through `element(<name>)` references in margin boxes. Authors who
//! also want the source element to stay in its in-flow position
//! while *also* feeding margin-box references opt in via
//! `-bd-running-copy: keep`.
//!
//! Cascade through the `box` style struct because the property is a
//! direct sibling of `position` semantically.

use crate::derives::*;

/// Specified value of the `-bd-running-copy` property.
///
/// `none` (initial) — standard GCPM behaviour: a `position: running(...)`
/// element is removed from in-flow content and surfaced only via
/// margin-box `element()` references.
///
/// `keep` — moegoe extension: the source element remains in normal
/// flow and is *also* copied into margin-box references.
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
pub enum BdRunningCopy {
    /// Standard GCPM running-element behaviour (the initial value).
    #[default]
    None,
    /// Source element stays in flow and is also copied into margin
    /// boxes.
    Keep,
}

impl BdRunningCopy {
    /// Whether the value is the default (`none`).
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}
