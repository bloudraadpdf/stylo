/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed corner shapes with resolved curvature.

use crate::derives::*;
use crate::values::animated::{Animate, Context, Procedure, ToAnimatedValue, ToAnimatedZero};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::corner_shape::SuperellipseCurvature;

/// A corner shape whose curvature has been resolved during the cascade.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(transparent)]
pub struct CornerShape(SuperellipseCurvature);

impl CornerShape {
    /// The initial quarter-ellipse shape.
    pub fn round() -> Self {
        Self(SuperellipseCurvature::from_css_number(1.0))
    }

    /// Constructs a shape from its resolved superellipse parameter.
    pub const fn from_curvature(curvature: SuperellipseCurvature) -> Self {
        Self(curvature)
    }

    /// Returns the resolved superellipse parameter.
    pub const fn curvature(self) -> SuperellipseCurvature {
        self.0
    }

    fn normalized_half_corner(self) -> f64 {
        match self.0 {
            SuperellipseCurvature::NegativeInfinity => 0.0,
            SuperellipseCurvature::PositiveInfinity => 1.0,
            SuperellipseCurvature::Finite(value) => {
                let curvature = f64::from(value.value());
                let convex = 0.5_f64.powf(2.0_f64.powf(-curvature.abs()));
                if curvature < 0.0 {
                    1.0 - convex
                } else {
                    convex
                }
            },
        }
    }
}

/// The normalised superellipse half corner used by CSS Borders interpolation.
#[derive(Clone, Copy, ComputeSquaredDistance, Debug, MallocSizeOf, PartialEq)]
pub struct AnimatedCornerShape(f64);

impl Animate for AnimatedCornerShape {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match procedure {
            Procedure::Interpolate { .. } => Ok(Self(self.0.animate(&other.0, procedure)?)),
            Procedure::Add | Procedure::Accumulate { .. } => Ok(*other),
        }
    }
}

impl ToAnimatedZero for AnimatedCornerShape {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

impl ToAnimatedValue for CornerShape {
    type AnimatedValue = AnimatedCornerShape;

    fn to_animated_value(self, _: &Context) -> Self::AnimatedValue {
        AnimatedCornerShape(self.normalized_half_corner())
    }

    fn from_animated_value(animated: Self::AnimatedValue) -> Self {
        let half_corner = animated.0.clamp(0.0, 1.0);
        let curvature = if half_corner == 0.0 {
            SuperellipseCurvature::NegativeInfinity
        } else if half_corner == 1.0 {
            SuperellipseCurvature::PositiveInfinity
        } else {
            let convex = half_corner.max(1.0 - half_corner);
            let magnitude = (0.5_f64.ln() / convex.ln()).log2();
            let value = if half_corner < 0.5 {
                -magnitude
            } else {
                magnitude
            };
            SuperellipseCurvature::from_css_number(value as f32)
        };
        Self(curvature)
    }
}

impl ToCss for CornerShape {
    fn to_css<W: fmt::Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        dest.write_str("superellipse(")?;
        self.0.to_css(dest)?;
        dest.write_char(')')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::animated::{Animate, Procedure, ToAnimatedZero};

    fn shape(value: f32) -> CornerShape {
        CornerShape::from_curvature(SuperellipseCurvature::from_css_number(value))
    }

    #[test]
    fn curvature_interpolates_in_normalized_half_corner_space() {
        for (from, to, progress, expected) in [
            (1.0, 0.0, -0.3, 1.4),
            (1.0, 0.0, 0.6, 0.36),
            (1.0, 0.0, 1.5, -0.46),
            (1.0, -1.0, 0.5, 0.0),
            (1.0, -1.0, 1.5, -2.95),
            (f32::NEG_INFINITY, f32::INFINITY, 0.5, 0.0),
            (f32::NEG_INFINITY, f32::INFINITY, 0.8, 1.64),
            (3.0, -2.0, 0.5, 0.16),
            (3.0, -2.0, 0.8, -0.9),
            (3.0, -2.0, 1.1, -2.99),
        ] {
            let from = AnimatedCornerShape(shape(from).normalized_half_corner());
            let to = AnimatedCornerShape(shape(to).normalized_half_corner());
            let actual = CornerShape::from_animated_value(
                from.animate(&to, Procedure::Interpolate { progress })
                    .unwrap(),
            );
            let SuperellipseCurvature::Finite(value) = actual.curvature() else {
                panic!("expected finite curvature {expected}, got {actual:?}");
            };
            assert!(
                (value.value() - expected).abs() < 0.005,
                "{actual:?} != {expected}"
            );
        }
    }

    #[test]
    fn composition_replaces_when_corner_shape_has_no_addition_procedure() {
        let underlying = AnimatedCornerShape(shape(1.0).normalized_half_corner());
        let effect = AnimatedCornerShape(shape(-1.0).normalized_half_corner());
        for procedure in [Procedure::Add, Procedure::Accumulate { count: 2 }] {
            assert_eq!(underlying.animate(&effect, procedure).unwrap(), effect);
        }
        assert!(effect.to_animated_zero().is_err());
    }

    #[test]
    fn half_corner_bounds_include_infinite_curvature_and_clamp_overshoot() {
        for (half_corner, expected) in [
            (-0.3, f32::NEG_INFINITY),
            (0.0, f32::NEG_INFINITY),
            (0.5, 0.0),
            (1.0, f32::INFINITY),
            (1.5, f32::INFINITY),
        ] {
            assert_eq!(
                CornerShape::from_animated_value(AnimatedCornerShape(half_corner)),
                shape(expected),
            );
        }
        for value in [f32::NEG_INFINITY, -3.0, -1.0, 0.0, 1.0, 3.0, f32::INFINITY] {
            let value = shape(value);
            assert_eq!(
                CornerShape::from_animated_value(AnimatedCornerShape(
                    value.normalized_half_corner()
                )),
                value,
            );
        }
    }
}
