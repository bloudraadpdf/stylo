/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values of smaller standard-CSS gap fillers (F21).
//!
//! Round-trip identically from the specified side.

use crate::values::computed::{BorderSideWidth, BorderStyle, Color};
use crate::values::generics::gap::GapRuleList as GenericGapRuleList;

pub use crate::values::specified::bd_gaps::{
    BorderClip, MaskBorderMode, Overlay, RuleBreak, RuleOverlap, RuleVisibilityItems,
};

/// A computed gap-decoration list with resolved values and repeater counts.
pub type GapRuleList<Value> = GenericGapRuleList<Value, crate::values::computed::Integer>;

/// The computed value of `column-rule-color` and `row-rule-color`.
pub type GapRuleColorList = GapRuleList<Color>;
/// The computed value of `column-rule-style` and `row-rule-style`.
pub type GapRuleStyleList = GapRuleList<BorderStyle>;
/// The computed value of `column-rule-width` and `row-rule-width`.
pub type GapRuleWidthList = GapRuleList<BorderSideWidth>;
