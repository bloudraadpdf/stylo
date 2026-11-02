/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed types for CSS values related to effects.

use crate::values::computed::color::Color;
use crate::values::computed::length::{Length, NonNegativeLength};
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Angle, Number};
use crate::values::generics::effects::BoxShadow as GenericBoxShadow;
use crate::values::generics::effects::Filter as GenericFilter;
use crate::values::generics::effects::SimpleShadow as GenericSimpleShadow;

/// A computed value for a single shadow of the `box-shadow` property.
pub type BoxShadow = GenericBoxShadow<Color, Length, NonNegativeLength, Length>;

/// A computed entry of `box-shadow-offset`.
pub type BoxShadowOffset = crate::values::generics::effects::BoxShadowOffset<Length>;

#[cfg(feature = "servo")]
impl crate::properties::style_structs::Effects {
    /// Compose the visible shadow layers, repeating component lists to match the offset list.
    pub fn box_shadows(&self) -> impl Iterator<Item = BoxShadow> + '_ {
        use crate::properties::longhands::box_shadow_position::computed_value::single_value::T as Position;
        self.box_shadow_offset
            .0
            .iter()
            .enumerate()
            .filter_map(|(index, offset)| {
                let BoxShadowOffset::Offset(offset) = offset else {
                    return None;
                };
                Some(BoxShadow {
                    base: GenericSimpleShadow {
                        color: self.box_shadow_color.0[index % self.box_shadow_color.0.len()]
                            .clone(),
                        horizontal: offset.width,
                        vertical: offset.height,
                        blur: self.box_shadow_blur.0[index % self.box_shadow_blur.0.len()],
                    },
                    spread: self.box_shadow_spread.0[index % self.box_shadow_spread.0.len()],
                    inset: self.box_shadow_position.0[index % self.box_shadow_position.0.len()]
                        == Position::Inset,
                })
            })
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::values::animated::{Animate, Procedure};
    use crate::values::generics::size::Size2D;

    #[test]
    fn a_missing_shadow_offset_interpolates_from_zero() {
        let offset = BoxShadowOffset::Offset(Size2D::new(Length::new(10.0), Length::new(20.0)));
        let halfway = BoxShadowOffset::None
            .animate(&offset, Procedure::Interpolate { progress: 0.5 })
            .unwrap();
        assert_eq!(
            halfway,
            BoxShadowOffset::Offset(Size2D::new(Length::new(5.0), Length::new(10.0)))
        );
        use crate::properties::longhands::box_shadow_offset::computed_value::List;
        let one = List(vec![offset.clone()].into());
        let two = List(vec![offset.clone(), offset].into());
        assert!(one
            .animate(&two, Procedure::Interpolate { progress: 0.5 })
            .is_err());
        use crate::properties::animated_properties::AnimationValue;
        let from = AnimationValue::BoxShadowOffset(List(vec![BoxShadowOffset::None].into()));
        let to = AnimationValue::BoxShadowOffset(one);
        assert_eq!(
            from.animate(&to, Procedure::Interpolate { progress: 0.5 })
                .unwrap(),
            AnimationValue::BoxShadowOffset(List(vec![halfway].into()))
        );
        assert!(to
            .animate(
                &AnimationValue::BoxShadowOffset(two),
                Procedure::Interpolate { progress: 0.5 }
            )
            .is_err());
        assert_eq!(
            BoxShadowOffset::None
                .animate(
                    &BoxShadowOffset::None,
                    Procedure::Interpolate { progress: 0.5 }
                )
                .unwrap(),
            BoxShadowOffset::None
        );
    }
}

/// A computed value for a single `filter`.
pub type Filter = GenericFilter<Angle, Number, Length, SimpleShadow, ComputedUrl>;

/// A computed value for the `drop-shadow()` filter.
pub type SimpleShadow = GenericSimpleShadow<Color, Length, NonNegativeLength>;
