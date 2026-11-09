/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed angles.

use crate::derives::*;
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use crate::values::CSSFloat;
use crate::Zero;
use std::f64::consts::PI;
use std::fmt::{self, Write};
use std::ops::Neg;
use std::{f32, f64};
use style_traits::{CssWriter, ToCss};

#[cfg(feature = "gecko")]
type AngleFloat = CSSFloat;
#[cfg(not(feature = "gecko"))]
type AngleFloat = f64;

/// A computed angle in degrees.
#[derive(
    Add,
    Animate,
    Clone,
    Copy,
    Debug,
    Deserialize,
    MallocSizeOf,
    PartialEq,
    PartialOrd,
    Serialize,
    ToAnimatedZero,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C)]
pub struct Angle(AngleFloat);

impl ToCss for Angle {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        self.degrees().to_css(dest)?;
        dest.write_str("deg")
    }
}

const RAD_PER_DEG: f64 = PI / 180.0;

impl Angle {
    /// Creates a computed `Angle` value from a radian amount.
    pub fn from_radians(radians: CSSFloat) -> Self {
        #[cfg(feature = "gecko")]
        let degrees = radians / RAD_PER_DEG as f32;
        #[cfg(not(feature = "gecko"))]
        let degrees = f64::from(radians) / RAD_PER_DEG;
        Angle(degrees)
    }

    /// Creates a computed `Angle` value from a degrees amount.
    #[inline]
    pub fn from_degrees(degrees: CSSFloat) -> Self {
        Angle(degrees.into())
    }

    /// Returns the amount of radians this angle represents.
    #[inline]
    pub fn radians(&self) -> CSSFloat {
        self.radians64().min(f32::MAX as f64).max(f32::MIN as f64) as f32
    }

    /// Returns the amount of radians this angle represents as a `f64`.
    ///
    /// Gecko stores angles as singles, but does this computation using doubles.
    ///
    /// This is significant enough to mess up rounding to the nearest
    /// quarter-turn for 225 degrees, for example.
    #[inline]
    pub fn radians64(&self) -> f64 {
        f64::from(self.0) * RAD_PER_DEG
    }

    /// Return the value in degrees.
    #[inline]
    pub fn degrees(&self) -> CSSFloat {
        #[cfg(feature = "gecko")]
        {
            self.0
        }
        #[cfg(not(feature = "gecko"))]
        {
            self.0 as CSSFloat
        }
    }
}

impl Zero for Angle {
    #[inline]
    fn zero() -> Self {
        Angle(0.0)
    }

    #[inline]
    fn is_zero(&self) -> bool {
        self.0 == 0.
    }
}

impl ComputeSquaredDistance for Angle {
    #[inline]
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        // Use the formula for calculating the distance between angles defined in SVG:
        // https://www.w3.org/TR/SVG/animate.html#complexDistances
        self.radians64()
            .compute_squared_distance(&other.radians64())
    }
}

impl Neg for Angle {
    type Output = Angle;

    #[inline]
    fn neg(self) -> Angle {
        Angle(-self.0)
    }
}

#[cfg(all(test, not(feature = "gecko")))]
mod tests {
    use super::*;
    use crate::values::animated::{Animate, Procedure};

    #[test]
    fn extrapolated_radians_preserve_the_same_matrix_as_an_authored_angle() {
        let actual = Angle::from_radians(10.0)
            .animate(
                &Angle::from_radians(20.0),
                Procedure::Interpolate { progress: 2.0 },
            )
            .unwrap();
        let expected = Angle::from_radians(30.0);
        assert!((actual.radians64() - expected.radians64()).abs() < 1e-12);
        assert!(
            (actual.radians64().tan() * 110.0 - expected.radians64().tan() * 110.0).abs() < 1e-10
        );
        assert_eq!(actual.degrees(), expected.degrees());
        let degrees = Angle::from_degrees(10.0)
            .animate(
                &Angle::from_degrees(20.0),
                Procedure::Interpolate { progress: 2.0 },
            )
            .unwrap();
        assert_eq!(degrees.degrees(), 30.0);
    }
}
