/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Animation implementation for various font-related types.

use super::{Animate, Context, Procedure, ToAnimatedValue, ToAnimatedZero};
use crate::values::computed::font::{FontPalette, FontVariationSettings};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};

/// <https://drafts.csswg.org/css-fonts-4/#font-variation-settings-def>
///
/// Note that the ComputedValue implementation will already have sorted and de-dup'd
/// the lists of settings, so we can just iterate over the two lists together and
/// animate their individual values.
impl Animate for FontVariationSettings {
    #[inline]
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        let result: Box<[_]> =
            super::lists::by_computed_value::animate(&self.0, &other.0, procedure)?;
        Ok(Self(result))
    }
}

impl ComputeSquaredDistance for FontVariationSettings {
    #[inline]
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        super::lists::by_computed_value::squared_distance(&self.0, &other.0)
    }
}

impl ToAnimatedZero for FontVariationSettings {
    #[inline]
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

impl Animate for FontPalette {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        if self == other {
            return Ok(self.clone());
        }
        let (first, second) = procedure.weights();
        if second <= 0.0 {
            return Ok(self.clone());
        }
        if first <= 0.0 {
            return Ok(other.clone());
        }
        Ok(self.mixed_with(other, second / (first + second)))
    }
}

impl ToAnimatedValue for FontPalette {
    type AnimatedValue = Self;

    fn to_animated_value(self, _context: &Context) -> Self {
        self
    }

    fn from_animated_value(animated: Self) -> Self {
        animated
    }
}

impl ComputeSquaredDistance for FontPalette {
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        Ok(SquaredDistance::from_sqrt(if self == other {
            0.0
        } else {
            1.0
        }))
    }
}

impl ToAnimatedZero for FontPalette {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

#[cfg(test)]
mod palette_tests {
    use super::*;
    use style_traits::ToCss;

    #[test]
    fn font_palette_interpolates_to_a_palette_mix() {
        let mixed = FontPalette::light()
            .animate(
                &FontPalette::dark(),
                Procedure::Interpolate { progress: 0.3 },
            )
            .expect("font palettes interpolate");
        assert_eq!(
            mixed.to_css_string(),
            "palette-mix(in oklab, light 70%, dark 30%)"
        );
    }

    #[test]
    fn font_palette_preserves_endpoints_and_additive_mix() {
        let light = FontPalette::light();
        let dark = FontPalette::dark();
        let before = light
            .animate(&dark, Procedure::Interpolate { progress: -0.25 })
            .unwrap();
        let after = light
            .animate(&dark, Procedure::Interpolate { progress: 1.25 })
            .unwrap();
        let additive = light.animate(&dark, Procedure::Add).unwrap();
        assert_eq!(before, light);
        assert_eq!(after, dark);
        assert_eq!(
            additive.to_css_string(),
            "palette-mix(in oklab, light, dark)"
        );
    }
}
