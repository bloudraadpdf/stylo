/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Exclusions Module Level 1 — `wrap-flow` (§5.1).
//!
//! <https://drafts.csswg.org/css-exclusions-1/#propdef-wrap-flow>
//!
//! Grammar: `auto | both | start | end | minimum | maximum | clear`.
//! Selects how inline content wraps around an exclusion element
//! (a block-level box with `position: absolute` or a float).

use crate::derives::*;

/// Specified value of the `wrap-flow` property.
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
pub enum WrapFlow {
    /// `auto` — exclusion has no effect; content flows over it.
    #[default]
    Auto,
    /// `both` — content wraps along both inline sides.
    Both,
    /// `start` — content wraps along the inline-start side only.
    Start,
    /// `end` — content wraps along the inline-end side only.
    End,
    /// `minimum` — content wraps along the side with less available
    /// space.
    Minimum,
    /// `maximum` — content wraps along the side with more available
    /// space.
    Maximum,
    /// `clear` — content does not flow alongside; forced to the next
    /// available band below the exclusion.
    Clear,
}
