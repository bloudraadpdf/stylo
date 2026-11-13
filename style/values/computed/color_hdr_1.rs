/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed dynamic-range limits retain the three canonical contributions.

use crate::derives::*;
use crate::values::animated::{Animate, Procedure, ToAnimatedZero};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

/// Normalised contributions of the three dynamic-range limits.
#[derive(Clone, ComputeSquaredDistance, Copy, Debug, MallocSizeOf, PartialEq, ToAnimatedValue, ToComputedValue, ToResolvedValue, ToShmem, ToTyped)]
pub struct DynamicRangeLimit {
    #[compute(no_field_bound)]
    standard: f64,
    #[compute(no_field_bound)]
    constrained: f64,
    #[compute(no_field_bound)]
    no_limit: f64,
}

impl DynamicRangeLimit {
    /// A pure standard dynamic-range limit.
    pub const STANDARD: Self = Self { standard: 1.0, constrained: 0.0, no_limit: 0.0 };
    /// A pure constrained dynamic-range limit.
    pub const CONSTRAINED: Self = Self { standard: 0.0, constrained: 1.0, no_limit: 0.0 };
    /// The unrestricted dynamic range of the output medium.
    pub const NO_LIMIT: Self = Self { standard: 0.0, constrained: 0.0, no_limit: 1.0 };

    /// Return the standard, constrained and unrestricted contributions.
    pub const fn contributions(self) -> [f64; 3] {
        [self.standard, self.constrained, self.no_limit]
    }

    /// Flatten weighted limits into normalised contributions.
    pub fn mix(items: impl IntoIterator<Item = (Self, f64)>) -> Self {
        let mut sums = [0.0; 3];
        let mut total = 0.0;
        for (value, percentage) in items {
            for (sum, contribution) in sums.iter_mut().zip(value.contributions()) {
                *sum += contribution * percentage;
            }
            total += percentage;
        }
        if total <= 0.0 || !total.is_finite() {
            return Self::NO_LIMIT;
        }
        Self { standard: sums[0] / total, constrained: sums[1] / total, no_limit: sums[2] / total }
    }
}

impl Default for DynamicRangeLimit {
    fn default() -> Self {
        Self::NO_LIMIT
    }
}

impl ToCss for DynamicRangeLimit {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        let values = ["standard", "constrained", "no-limit"].into_iter().zip(self.contributions());
        for (name, percentage) in values.clone() {
            if percentage == 1.0 {
                return dest.write_str(name);
            }
        }
        dest.write_str("dynamic-range-limit-mix(")?;
        for (index, (name, percentage)) in values.filter(|(_, percentage)| *percentage > 0.0).enumerate() {
            if index != 0 {
                dest.write_str(", ")?;
            }
            dest.write_str(name)?;
            dest.write_char(' ')?;
            ((percentage * 100.0) as f32).to_css(dest)?;
            dest.write_char('%')?;
        }
        dest.write_char(')')
    }
}

impl Animate for DynamicRangeLimit {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        let (left, right) = procedure.weights();
        Ok(Self::mix([(*self, left.max(0.0)), (*other, right.max(0.0))]))
    }
}

impl ToAnimatedZero for DynamicRangeLimit {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contributions_normalise_nested_mixtures_and_interpolate_without_snapping() {
        let inner = DynamicRangeLimit::mix([(DynamicRangeLimit::STANDARD, 0.1), (DynamicRangeLimit::NO_LIMIT, 0.3)]);
        let outer = DynamicRangeLimit::mix([(inner, 0.2), (DynamicRangeLimit::STANDARD, 0.8)]);
        assert_eq!(outer.to_css_string(), "dynamic-range-limit-mix(standard 85%, no-limit 15%)");
        let animated = DynamicRangeLimit::NO_LIMIT.animate(&DynamicRangeLimit::STANDARD, Procedure::Interpolate { progress: 0.25 }).unwrap();
        assert_eq!(animated.to_css_string(), "dynamic-range-limit-mix(standard 25%, no-limit 75%)");
        assert_eq!(DynamicRangeLimit::default().to_css_string(), "no-limit");
    }
}
