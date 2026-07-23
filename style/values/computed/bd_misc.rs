/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values of moegoe `-bd-*` miscellaneous declarative-tuning
//! properties (F32).
//!
//! Most types round-trip identically from the specified side. The
//! `Integer` and `Number`-backed variants split because Stylo's
//! `Integer` / `Number` computed forms are plain `i32` / `f32`, so
//! the auto-derived enum `ToComputedValue` cannot retain a wrapper
//! type. Those expose a `Computed*` variant under the original name.

pub use crate::values::specified::bd_misc::{
    BdCaptionPage, BdColumnClip, BdFlow, BdLang, BdLineBreakOpportunity, BdLineBreakRule, BdObjectSlice,
    BdPositionOrigin, BdReplacedElement, BdShrinkToFit, BdTabSnap, BdTargetCandidate,
    BdTruncateMarginAfterBreak,
};

pub use crate::values::specified::bd_misc::ComputedBdIntegerAuto as BdIntegerAuto;
pub use crate::values::specified::bd_misc::ComputedBdScaleContent as BdScaleContent;
