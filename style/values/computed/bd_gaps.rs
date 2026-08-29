/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values of smaller standard-CSS gap fillers (F21).
//!
//! Round-trip identically from the specified side.

use crate::derives::*;
use crate::values::animated::{
    Animate, Context as AnimatedContext, Procedure, ToAnimatedValue, ToAnimatedZero,
};
use crate::values::computed::length::CSSPixelLength;
use crate::values::computed::{BorderStyle, Color};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use crate::values::generics::gap::GapRuleList as GenericGapRuleList;
use crate::values::resolved::{Context as ResolvedContext, ToResolvedValue};
use app_units::Au;
use std::fmt;
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::bd_gaps::{
    BorderClip, MaskBorderMode, Overlay, RuleBreak, RuleOverlap, RuleVisibilityItems,
};

/// A computed gap-decoration list with resolved values and repeater counts.
pub type GapRuleList<Value> = GenericGapRuleList<Value, crate::values::computed::Integer>;

/// The computed value of `column-rule-color` and `row-rule-color`.
pub type GapRuleColorList = GapRuleList<Color>;
/// The computed value of `column-rule-style` and `row-rule-style`.
pub type GapRuleStyleList = GapRuleList<BorderStyle>;

/// A computed gap-rule width and its device-pixel snapping quantum.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToTyped)]
#[repr(C)]
#[typed_value(derive_fields)]
pub struct GapRuleWidth {
    length: Au,
    #[typed_value(skip)]
    device_pixel: Au,
}

impl GapRuleWidth {
    pub(crate) fn new(length: Au, device_pixel: Au) -> Self {
        debug_assert!(device_pixel > Au(0));
        Self {
            length: super::border::snap_as_border_width(length.max(Au(0)), device_pixel.0),
            device_pixel,
        }
    }

    /// Return the snapped computed length.
    pub fn length(&self) -> Au {
        self.length
    }

    /// The initial `medium` width for the default one-CSS-pixel device quantum.
    pub fn medium() -> Self {
        Self::new(Au::from_px(3), Au::from_px(1))
    }
}

impl ToResolvedValue for GapRuleWidth {
    type ResolvedValue = Self;

    fn to_resolved_value(self, _: &ResolvedContext) -> Self::ResolvedValue {
        self
    }

    fn from_resolved_value(resolved: Self::ResolvedValue) -> Self {
        resolved
    }
}

impl ToCss for GapRuleWidth {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: fmt::Write,
    {
        self.length.to_css(dest)
    }
}

/// The intermediate animation representation of a gap-rule width.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq)]
pub struct AnimatedGapRuleWidth {
    length: CSSPixelLength,
    device_pixel: Au,
}

impl Animate for AnimatedGapRuleWidth {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        if self.device_pixel != other.device_pixel {
            return Err(());
        }
        Ok(Self {
            length: self.length.animate(&other.length, procedure)?,
            device_pixel: self.device_pixel,
        })
    }
}

impl ComputeSquaredDistance for AnimatedGapRuleWidth {
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        if self.device_pixel != other.device_pixel {
            return Err(());
        }
        self.length.compute_squared_distance(&other.length)
    }
}

impl ToAnimatedZero for AnimatedGapRuleWidth {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Ok(Self {
            length: CSSPixelLength::new(0.0),
            device_pixel: self.device_pixel,
        })
    }
}

impl ToAnimatedValue for GapRuleWidth {
    type AnimatedValue = AnimatedGapRuleWidth;

    fn to_animated_value(self, context: &AnimatedContext) -> Self::AnimatedValue {
        AnimatedGapRuleWidth {
            length: self.length.to_animated_value(context),
            device_pixel: self.device_pixel,
        }
    }

    fn from_animated_value(animated: Self::AnimatedValue) -> Self {
        Self::new(
            Au::from_animated_value(animated.length),
            animated.device_pixel,
        )
    }
}

/// The computed value of `column-rule-width` and `row-rule-width`.
pub type GapRuleWidthList = GapRuleList<GapRuleWidth>;

#[cfg(test)]
mod tests {
    use super::{AnimatedGapRuleWidth, GapRuleWidth};
    use crate::values::animated::{Animate, Procedure, ToAnimatedValue};
    use crate::values::computed::length::CSSPixelLength;
    use app_units::Au;

    #[test]
    fn animated_gap_rule_width_snaps_to_its_device_pixel_quantum() {
        let from = AnimatedGapRuleWidth {
            length: CSSPixelLength::new(3.0),
            device_pixel: Au::from_px(1),
        };
        let to = AnimatedGapRuleWidth {
            length: CSSPixelLength::new(40.0),
            device_pixel: Au::from_px(1),
        };
        let animated = from
            .animate(&to, Procedure::Interpolate { progress: 0.3 })
            .expect("matching device-pixel quanta must interpolate");

        assert_eq!(
            GapRuleWidth::from_animated_value(animated).length(),
            Au::from_px(14)
        );
    }
}
