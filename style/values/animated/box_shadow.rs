/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Coordinated animation of the box-shadow shorthand. Individual longhands
//! retain their own animation types; the shorthand keeps shadow-list pairing.

use crate::derives::*;
use crate::properties::animated_properties::{AnimationValue, AnimationValueMap};
use crate::properties::{longhands, ComputedValues, LonghandId, PropertyDeclarationId};
use crate::values::animated::color::Color;
use crate::values::animated::{lists, Animate, Procedure};
use crate::values::computed::{BoxShadowOffset, BoxShadowPosition, Length, NonNegativeLength};
use crate::values::generics::effects::{GenericBoxShadow, GenericSimpleShadow};
use crate::Zero;

/// The five components of a coordinated shadow list, in declaration order.
pub const COMPONENTS: [LonghandId; 5] = [
    LonghandId::BoxShadowColor,
    LonghandId::BoxShadowOffset,
    LonghandId::BoxShadowBlur,
    LonghandId::BoxShadowSpread,
    LonghandId::BoxShadowPosition,
];

type Shadow = GenericBoxShadow<Color, Length, NonNegativeLength, Length>;

/// A typed shadow list that preserves per-layer inset and transparent padding.
#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub struct AnimatedBoxShadowList(Vec<Shadow>);

impl AnimatedBoxShadowList {
    /// Coordinate the computed longhands, optionally replacing components with
    /// already sampled values from lower effects in the animation stack.
    pub fn from_computed_values(
        style: &ComputedValues,
        overrides: Option<&AnimationValueMap>,
    ) -> Self {
        let value = |id| {
            overrides
                .and_then(|map| map.get(&PropertyDeclarationId::Longhand(id).to_owned()))
                .cloned()
                .or_else(|| {
                    AnimationValue::from_computed_values(PropertyDeclarationId::Longhand(id), style)
                })
                .unwrap()
        };
        let AnimationValue::BoxShadowColor(colors) = value(LonghandId::BoxShadowColor) else {
            unreachable!()
        };
        let AnimationValue::BoxShadowOffset(offsets) = value(LonghandId::BoxShadowOffset) else {
            unreachable!()
        };
        let AnimationValue::BoxShadowBlur(blurs) = value(LonghandId::BoxShadowBlur) else {
            unreachable!()
        };
        let AnimationValue::BoxShadowSpread(spreads) = value(LonghandId::BoxShadowSpread) else {
            unreachable!()
        };
        let AnimationValue::BoxShadowPosition(positions) = value(LonghandId::BoxShadowPosition)
        else {
            unreachable!()
        };
        Self(
            offsets
                .0
                .iter()
                .enumerate()
                .filter_map(|(index, offset)| {
                    let BoxShadowOffset::Offset(horizontal, vertical) = offset else {
                        return None;
                    };
                    Some(Shadow {
                        base: GenericSimpleShadow {
                            color: colors.0[index % colors.0.len()].clone(),
                            horizontal: *horizontal,
                            vertical: *vertical,
                            blur: blurs.0[index % blurs.0.len()],
                        },
                        spread: spreads.0[index % spreads.0.len()],
                        inset: positions.0[index % positions.0.len()] == BoxShadowPosition::Inset,
                    })
                })
                .collect(),
        )
    }

    /// Project a sampled list back into the existing native longhand values.
    pub fn components(&self) -> [AnimationValue; 5] {
        let shadows = &self.0;
        if shadows.is_empty() {
            return [
                AnimationValue::BoxShadowColor(
                    longhands::box_shadow_color::computed_value::OwnedList(
                        std::iter::once(Color::Absolute(
                            crate::color::AbsoluteColor::TRANSPARENT_BLACK,
                        ))
                        .collect(),
                    ),
                ),
                AnimationValue::BoxShadowOffset(
                    longhands::box_shadow_offset::computed_value::OwnedList(
                        std::iter::once(BoxShadowOffset::None).collect(),
                    ),
                ),
                AnimationValue::BoxShadowBlur(
                    longhands::box_shadow_blur::computed_value::OwnedList(
                        std::iter::once(NonNegativeLength::zero()).collect(),
                    ),
                ),
                AnimationValue::BoxShadowSpread(
                    longhands::box_shadow_spread::computed_value::OwnedList(
                        std::iter::once(Length::zero()).collect(),
                    ),
                ),
                AnimationValue::BoxShadowPosition(
                    longhands::box_shadow_position::computed_value::OwnedList(
                        std::iter::once(BoxShadowPosition::Outset).collect(),
                    ),
                ),
            ];
        }
        [
            AnimationValue::BoxShadowColor(longhands::box_shadow_color::computed_value::OwnedList(
                shadows
                    .iter()
                    .map(|shadow| shadow.base.color.clone())
                    .collect(),
            )),
            AnimationValue::BoxShadowOffset(
                longhands::box_shadow_offset::computed_value::OwnedList(
                    shadows
                        .iter()
                        .map(|shadow| {
                            BoxShadowOffset::Offset(shadow.base.horizontal, shadow.base.vertical)
                        })
                        .collect(),
                ),
            ),
            AnimationValue::BoxShadowBlur(longhands::box_shadow_blur::computed_value::OwnedList(
                shadows.iter().map(|shadow| shadow.base.blur).collect(),
            )),
            AnimationValue::BoxShadowSpread(
                longhands::box_shadow_spread::computed_value::OwnedList(
                    shadows.iter().map(|shadow| shadow.spread).collect(),
                ),
            ),
            AnimationValue::BoxShadowPosition(
                longhands::box_shadow_position::computed_value::OwnedList(
                    shadows
                        .iter()
                        .map(|shadow| {
                            if shadow.inset {
                                BoxShadowPosition::Inset
                            } else {
                                BoxShadowPosition::Outset
                            }
                        })
                        .collect(),
                ),
            ),
        ]
    }
}

impl Animate for AnimatedBoxShadowList {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        lists::with_zero::animate(&self.0, &other.0, procedure).map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::values::animated::ToAnimatedZero;

    fn shadow(inset: bool, horizontal: f32) -> Shadow {
        Shadow {
            base: GenericSimpleShadow {
                color: Color::Absolute(crate::color::AbsoluteColor::BLACK),
                horizontal: Length::new(horizontal),
                vertical: Length::new(2.0),
                blur: NonNegativeLength::zero(),
            },
            spread: Length::zero(),
            inset,
        }
    }

    #[test]
    fn shadow_add_concatenates_and_interpolation_pads_transparent_layers() {
        let from = AnimatedBoxShadowList(vec![shadow(false, 10.0)]);
        let to = AnimatedBoxShadowList(vec![shadow(false, 20.0), shadow(true, 30.0)]);
        assert_eq!(from.animate(&to, Procedure::Add).unwrap().0.len(), 3);
        let sample = from
            .animate(&to, Procedure::Interpolate { progress: 0.5 })
            .unwrap();
        assert_eq!(sample.0.len(), 2);
        assert_eq!(sample.0[0].base.horizontal, Length::new(15.0));
        assert!(sample.0[1].inset);
        assert_eq!(
            sample.0[1],
            to.0[1]
                .to_animated_zero()
                .unwrap()
                .animate(&to.0[1], Procedure::Interpolate { progress: 0.5 })
                .unwrap()
        );
    }

    #[test]
    fn shadow_inset_mismatch_rejects_the_whole_list_and_none_has_no_shadow() {
        let from = AnimatedBoxShadowList(vec![shadow(false, 10.0)]);
        let to = AnimatedBoxShadowList(vec![shadow(true, 20.0)]);
        assert!(from
            .animate(&to, Procedure::Interpolate { progress: 0.5 })
            .is_err());
        let empty = AnimatedBoxShadowList(Vec::new());
        let [_, offset, _, _, _] = empty.components();
        assert!(
            matches!(offset, AnimationValue::BoxShadowOffset(value) if value.0.as_slice() == [BoxShadowOffset::None])
        );
        let sample = empty
            .animate(&to, Procedure::Interpolate { progress: 0.5 })
            .unwrap();
        assert_eq!(
            sample.0[0],
            to.0[0]
                .to_animated_zero()
                .unwrap()
                .animate(&to.0[0], Procedure::Interpolate { progress: 0.5 })
                .unwrap()
        );
    }
}
