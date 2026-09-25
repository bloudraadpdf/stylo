/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Animated types for CSS colors.

use style_traits::owned_slice::OwnedSlice;

use crate::color::mix::ColorInterpolationMethod;
use crate::color::AbsoluteColor;
use crate::values::animated::{Animate, Procedure, ToAnimatedZero};
use crate::values::computed::Percentage;
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use crate::values::generics::color::{
    ColorMixFlags, GenericColor, GenericColorMix, GenericColorMixItem, GenericColorMixPercentage,
};

impl Animate for AbsoluteColor {
    #[inline]
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        use crate::color::mix;

        let (left_weight, right_weight) = procedure.weights();

        Ok(mix::mix_many(
            ColorInterpolationMethod::best_interpolation_between(self, other),
            [
                mix::ColorMixItem::new(*self, left_weight as crate::color::ColorFloat),
                mix::ColorMixItem::new(*other, right_weight as crate::color::ColorFloat),
            ],
            ColorMixFlags::empty(),
        ))
    }
}

impl ComputeSquaredDistance for AbsoluteColor {
    #[inline]
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        let start = [
            self.alpha,
            self.components.0 * self.alpha,
            self.components.1 * self.alpha,
            self.components.2 * self.alpha,
        ];
        let end = [
            other.alpha,
            other.components.0 * other.alpha,
            other.components.1 * other.alpha,
            other.components.2 * other.alpha,
        ];
        start
            .iter()
            .zip(&end)
            .map(|(this, other)| this.compute_squared_distance(other))
            .sum()
    }
}

/// An animated value for `<color>`.
pub type Color = GenericColor<Percentage>;

/// An animated value for `<color-mix>`.
pub type ColorMix = GenericColorMix<Color, Percentage>;

impl Animate for Color {
    #[inline]
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        // A mix carries its weights as percentages, whose precision loses the
        // sum of one that interpolation relies on. Absolute endpoints need no
        // mix, so they keep the precision of the procedure.
        if let (Some(left), Some(right)) = (self.as_absolute(), other.as_absolute()) {
            return Ok(Self::Absolute(left.animate(right, procedure)?));
        }

        let (left_weight, right_weight) = procedure.weights();
        let interpolation = ColorInterpolationMethod::srgb();

        Ok(Self::from_color_mix(
            ColorMix::new(
                interpolation,
                OwnedSlice::from_slice(&[
                    GenericColorMixItem {
                        color: self.clone(),
                        percentage: GenericColorMixPercentage::Explicit(Percentage(
                            left_weight as f32,
                        )),
                    },
                    GenericColorMixItem {
                        color: other.clone(),
                        percentage: GenericColorMixPercentage::Explicit(Percentage(
                            right_weight as f32,
                        )),
                    },
                ]),
                // See https://github.com/w3c/csswg-drafts/issues/7324
                ColorMixFlags::empty(),
            )
            .expect("an animated mix has two endpoints"),
        ))
    }
}

impl ComputeSquaredDistance for Color {
    #[inline]
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        let current_color = AbsoluteColor::TRANSPARENT_BLACK;
        self.resolve_to_absolute(&current_color)
            .compute_squared_distance(&other.resolve_to_absolute(&current_color))
    }
}

impl ToAnimatedZero for Color {
    #[inline]
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Ok(Color::Absolute(AbsoluteColor::TRANSPARENT_BLACK))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorSpace;

    /// CSS Easing 1 allows an output progress outside [0, 1], so a huge
    /// progress must still extrapolate towards the second colour.
    #[test]
    fn a_huge_progress_extrapolates_past_the_second_colour() {
        let blue = Color::Absolute(AbsoluteColor::srgb_legacy(0, 0, 255, 1.0));
        let green = Color::Absolute(AbsoluteColor::srgb_legacy(0, 128, 0, 1.0));

        let result = blue
            .animate(
                &green,
                Procedure::Interpolate {
                    progress: 2_147_483_712.125,
                },
            )
            .expect("absolute colors interpolate");
        let components = result
            .as_absolute()
            .expect("absolute endpoints produce an absolute result")
            .raw_components();

        assert!(components[1] > 1.0, "{components:?}");
        assert!(components[2] < 0.0, "{components:?}");
    }

    #[test]
    fn computed_modern_colors_interpolate_in_default_oklab_space() {
        let black = Color::Absolute(AbsoluteColor::BLACK);
        let modern_white =
            Color::Absolute(AbsoluteColor::new(ColorSpace::Srgb, 1.0, 1.0, 1.0, 1.0));

        let result = black
            .animate(&modern_white, Procedure::Interpolate { progress: 0.3 })
            .expect("absolute colors interpolate");
        let absolute = result
            .as_absolute()
            .expect("absolute endpoints produce an absolute result");

        assert_eq!(absolute.color_space, ColorSpace::Oklab);
    }
}
