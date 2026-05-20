/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Color HDR Module Level 1 — `dynamic-range-limit` (§4).
//!
//! <https://drafts.csswg.org/css-color-hdr-1/#dynamic-range-limit>
//!
//! Grammar: `standard | high | constrained-high`. Selects the
//! dynamic-range tone-mapping cap applied to descendant content.
//! `standard` (initial) clamps to SDR; `high` allows the renderer's
//! full HDR range; `constrained-high` allows HDR up to an
//! implementation-defined ceiling.

use crate::derives::*;

/// Specified value of the `dynamic-range-limit` property.
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
pub enum DynamicRangeLimit {
    /// `standard` — clamp descendants to the SDR luminance ceiling
    /// (initial).
    #[default]
    Standard,
    /// `high` — allow the renderer's full HDR range.
    High,
    /// `constrained-high` — allow HDR up to an implementation-defined
    /// ceiling.
    ConstrainedHigh,
}
