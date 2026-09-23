/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Color mixing/interpolation.

use super::{AbsoluteColor, ColorFlags, ColorSpace};
use crate::color::ColorMixItemList;
use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::color::ColorMixFlags;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// A hue-interpolation-method as defined in [1].
///
/// [1]: https://drafts.csswg.org/css-color-4/#typedef-hue-interpolation-method
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
#[repr(u8)]
pub enum HueInterpolationMethod {
    /// https://drafts.csswg.org/css-color-4/#shorter
    Shorter,
    /// https://drafts.csswg.org/css-color-4/#longer
    Longer,
    /// https://drafts.csswg.org/css-color-4/#increasing
    Increasing,
    /// https://drafts.csswg.org/css-color-4/#decreasing
    Decreasing,
    /// https://drafts.csswg.org/css-color-4/#specified
    Specified,
}

/// https://drafts.csswg.org/css-color-4/#color-interpolation-method
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    ToShmem,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
)]
#[repr(C)]
pub struct ColorInterpolationMethod {
    /// The color-space the interpolation should be done in.
    pub space: ColorSpace,
    /// The hue interpolation method.
    pub hue: HueInterpolationMethod,
}

impl ColorInterpolationMethod {
    /// Returns the srgb interpolation method.
    pub const fn srgb() -> Self {
        Self {
            space: ColorSpace::Srgb,
            hue: HueInterpolationMethod::Shorter,
        }
    }

    /// Return the oklab interpolation method used for default color
    /// interpolcation.
    pub const fn oklab() -> Self {
        Self {
            space: ColorSpace::Oklab,
            hue: HueInterpolationMethod::Shorter,
        }
    }

    /// Return true if the this is the default method.
    pub fn is_default(&self) -> bool {
        self.space == ColorSpace::Oklab
    }

    /// Decides the best method for interpolating between the given colors.
    /// https://drafts.csswg.org/css-color-4/#interpolation-space
    pub fn best_interpolation_between(left: &AbsoluteColor, right: &AbsoluteColor) -> Self {
        // The default color space to use for interpolation is Oklab. However,
        // if either of the colors are in legacy rgb(), hsl() or hwb(), then
        // interpolation is done in sRGB.
        if !left.is_legacy_syntax() || !right.is_legacy_syntax() {
            Self::default()
        } else {
            Self::srgb()
        }
    }
}

impl Default for ColorInterpolationMethod {
    fn default() -> Self {
        Self::oklab()
    }
}

impl Parse for ColorInterpolationMethod {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        input.expect_ident_matching("in")?;
        let space = ColorSpace::parse(input)?;
        // https://drafts.csswg.org/css-color-4/#hue-interpolation
        //     Unless otherwise specified, if no specific hue interpolation
        //     algorithm is selected by the host syntax, the default is shorter.
        let hue = if space.is_polar() {
            input
                .try_parse(|input| -> Result<_, ParseError<'i>> {
                    let hue = HueInterpolationMethod::parse(input)?;
                    input.expect_ident_matching("hue")?;
                    Ok(hue)
                })
                .unwrap_or(HueInterpolationMethod::Shorter)
        } else {
            HueInterpolationMethod::Shorter
        };
        Ok(Self { space, hue })
    }
}

impl ToCss for ColorInterpolationMethod {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str("in ")?;
        self.space.to_css(dest)?;
        if self.hue != HueInterpolationMethod::Shorter {
            dest.write_char(' ')?;
            self.hue.to_css(dest)?;
            dest.write_str(" hue")?;
        }
        Ok(())
    }
}

/// A color and its weight for use in a color mix.
pub struct ColorMixItem {
    /// The color being mixed.
    pub color: AbsoluteColor,
    /// How much this color contributes to the final mix.
    pub weight: f32,
}

impl ColorMixItem {
    /// Create a new color item for mixing.
    #[inline]
    pub fn new(color: AbsoluteColor, weight: f32) -> Self {
        Self { color, weight }
    }
}

/// Mix N colors into one (left-to-right fold).
pub fn mix_many(
    interpolation: ColorInterpolationMethod,
    items: impl IntoIterator<Item = ColorMixItem>,
    flags: ColorMixFlags,
) -> AbsoluteColor {
    let items = items.into_iter().collect::<ColorMixItemList<_>>();

    if items.is_empty() {
        return AbsoluteColor::TRANSPARENT_BLACK.to_color_space(interpolation.space);
    }

    let normalize = flags.contains(ColorMixFlags::NORMALIZE_WEIGHTS);
    let mut weight_scale = 1.0;
    let mut alpha_multiplier = 1.0;
    if normalize {
        // https://drafts.csswg.org/css-color-5/#color-mix-percent-norm
        let sum: f32 = items.iter().map(|item| item.weight).sum();
        if sum == 0.0 {
            // The colors are still mixed; only the resulting alpha is zero.
            alpha_multiplier = 0.0;
        } else if (sum - 1.0).abs() > f32::EPSILON {
            weight_scale = 1.0 / sum;
            if sum < 1.0 {
                alpha_multiplier = sum;
            }
        }
    }

    // We can unwrap here, because we already checked for no items.
    let (first, rest) = items.split_first().unwrap();
    let mut accumulated_color = convert_for_mix(&first.color, interpolation.space);
    let mut accumulated_weight = first.weight * weight_scale;

    for item in rest {
        let weight = item.weight * weight_scale;
        let combined = accumulated_weight + weight;
        if combined == 0.0 && !normalize {
            continue;
        }
        let right = convert_for_mix(&item.color, interpolation.space);

        let (left_weight, right_weight) = if normalize {
            if combined == 0.0 {
                (0.5, 0.5)
            } else {
                (accumulated_weight / combined, weight / combined)
            }
        } else {
            (accumulated_weight, weight)
        };

        accumulated_color = mix_with_weights(
            &accumulated_color,
            left_weight,
            &right,
            right_weight,
            interpolation.hue,
        );
        accumulated_weight = combined;
    }

    let components = accumulated_color.raw_components();
    let alpha = components[3] * alpha_multiplier;

    // FIXME: In rare cases we end up with 0.999995 in the alpha channel,
    //        so we reduce the precision to avoid serializing to
    //        rgba(?, ?, ?, 1).  This is not ideal, so we should look into
    //        ways to avoid it. Maybe pre-multiply all color components and
    //        then divide after calculations?
    let alpha = (alpha.clamp(0.0, 1.0) * 1000.0).round() / 1000.0;

    let mut result = AbsoluteColor::new(
        interpolation.space,
        components[0],
        components[1],
        components[2],
        alpha,
    );
    result.flags = accumulated_color.flags;

    if flags.contains(ColorMixFlags::RESULT_IN_MODERN_SYNTAX) {
        // HSL and HWB results with missing components already serialize in
        // modern syntax. Converting them to sRGB would erase the missing
        // components that later interpolation needs to retain.
        let has_missing_component = result.flags.intersects(
            ColorFlags::C0_IS_NONE
                | ColorFlags::C1_IS_NONE
                | ColorFlags::C2_IS_NONE
                | ColorFlags::ALPHA_IS_NONE,
        );
        if result.is_legacy_syntax() && !has_missing_component {
            result.to_color_space(ColorSpace::Srgb)
        } else {
            result
        }
    } else if items.iter().all(|item| item.color.is_legacy_syntax()) {
        // If both sides of the mix is legacy then convert the result back into
        // legacy.
        result.into_srgb_legacy()
    } else {
        result
    }
}

/// What the outcome of each component should be in a mix result.
#[derive(Clone, Copy, PartialEq)]
#[repr(u8)]
enum ComponentMixOutcome {
    /// Mix the left and right sides to give the result.
    Mix,
    /// Carry the left side forward to the result.
    UseLeft,
    /// Carry the right side forward to the result.
    UseRight,
    /// The resulting component should also be none.
    None,
}

impl ComponentMixOutcome {
    fn from_colors(
        left: &AbsoluteColor,
        right: &AbsoluteColor,
        flags_to_check: ColorFlags,
    ) -> Self {
        match (
            left.flags.contains(flags_to_check),
            right.flags.contains(flags_to_check),
        ) {
            (true, true) => Self::None,
            (true, false) => Self::UseRight,
            (false, true) => Self::UseLeft,
            (false, false) => Self::Mix,
        }
    }
}

impl AbsoluteColor {
    /// Calculate the flags that should be carried forward a color before converting
    /// it to the interpolation color space according to:
    /// <https://drafts.csswg.org/css-color-4/#interpolation-missing>
    pub(crate) fn carry_forward_analogous_missing_components(&mut self, source: &AbsoluteColor) {
        use ColorFlags as F;
        use ColorSpace as S;

        if source.color_space == self.color_space {
            return;
        }

        let all_components = F::C0_IS_NONE | F::C1_IS_NONE | F::C2_IS_NONE;
        if source.flags.contains(all_components) {
            self.flags.insert(all_components);
            return;
        }

        // With neither lightness nor chroma specified, a cylindrical color
        // supplies no white or black component to an HWB interpolation.
        if matches!(source.color_space, S::Lch | S::Oklch)
            && self.color_space == S::Hwb
            && source.flags.contains(F::C0_IS_NONE | F::C1_IS_NONE)
        {
            self.flags.insert(all_components);
            return;
        }

        // Reds             r, x
        // Greens           g, y
        // Blues            b, z
        if source.color_space.is_rgb_or_xyz_like() && self.color_space.is_rgb_or_xyz_like() {
            self.flags |= source.flags & all_components;
            return;
        }

        // Lightness        L
        if matches!(source.color_space, S::Lab | S::Lch | S::Oklab | S::Oklch) {
            if matches!(self.color_space, S::Lab | S::Lch | S::Oklab | S::Oklch) {
                self.flags |= source.flags & F::C0_IS_NONE;
            } else if matches!(self.color_space, S::Hsl) {
                if source.flags.contains(F::C0_IS_NONE) {
                    self.flags.insert(F::C2_IS_NONE)
                }
            }
        } else if matches!(source.color_space, S::Hsl)
            && matches!(self.color_space, S::Lab | S::Lch | S::Oklab | S::Oklch)
        {
            if source.flags.contains(F::C2_IS_NONE) {
                self.flags.insert(F::C0_IS_NONE)
            }
        }

        // Colorfulness     C, S
        if matches!(source.color_space, S::Hsl | S::Lch | S::Oklch)
            && matches!(self.color_space, S::Hsl | S::Lch | S::Oklch)
        {
            self.flags |= source.flags & F::C1_IS_NONE;
        }

        // Hue              H
        if matches!(source.color_space, S::Hsl | S::Hwb) {
            if matches!(self.color_space, S::Hsl | S::Hwb) {
                self.flags |= source.flags & F::C0_IS_NONE;
            } else if matches!(self.color_space, S::Lch | S::Oklch) {
                if source.flags.contains(F::C0_IS_NONE) {
                    self.flags.insert(F::C2_IS_NONE)
                }
            }
        } else if matches!(source.color_space, S::Lch | S::Oklch) {
            if matches!(self.color_space, S::Hsl | S::Hwb) {
                if source.flags.contains(F::C2_IS_NONE) {
                    self.flags.insert(F::C0_IS_NONE)
                }
            } else if matches!(self.color_space, S::Lch | S::Oklch) {
                self.flags |= source.flags & F::C2_IS_NONE;
            }
        }

        // Opponent         a, a
        // Opponent         b, b
        if matches!(source.color_space, S::Lab | S::Oklab)
            && matches!(self.color_space, S::Lab | S::Oklab)
        {
            self.flags |= source.flags & F::C1_IS_NONE;
            self.flags |= source.flags & F::C2_IS_NONE;
        } else if matches!(source.color_space, S::Lab | S::Oklab)
            && matches!(self.color_space, S::Lch | S::Oklch)
            && source.flags.contains(F::C1_IS_NONE | F::C2_IS_NONE)
        {
            self.flags.insert(F::C1_IS_NONE);
        } else if matches!(source.color_space, S::Lch | S::Oklch)
            && matches!(self.color_space, S::Lab | S::Oklab)
            && source.flags.contains(F::C1_IS_NONE)
        {
            self.flags.insert(F::C1_IS_NONE | F::C2_IS_NONE);
        }
    }
}

/// Mix two colors already in the interpolation color space.
fn mix_with_weights(
    left: &AbsoluteColor,
    left_weight: f32,
    right: &AbsoluteColor,
    right_weight: f32,
    hue_interpolation: HueInterpolationMethod,
) -> AbsoluteColor {
    debug_assert!(right.color_space == left.color_space);
    let color_space = left.color_space;

    let outcomes = [
        ComponentMixOutcome::from_colors(&left, &right, ColorFlags::C0_IS_NONE),
        ComponentMixOutcome::from_colors(&left, &right, ColorFlags::C1_IS_NONE),
        ComponentMixOutcome::from_colors(&left, &right, ColorFlags::C2_IS_NONE),
        ComponentMixOutcome::from_colors(&left, &right, ColorFlags::ALPHA_IS_NONE),
    ];

    // Convert both sides into just components.
    let left = left.raw_components();
    let right = right.raw_components();

    let (result, result_flags) = interpolate_premultiplied(
        &left,
        left_weight,
        &right,
        right_weight,
        color_space.hue_index(),
        hue_interpolation,
        &outcomes,
    );

    let mut result = AbsoluteColor::new(color_space, result[0], result[1], result[2], result[3]);
    result.flags = result_flags;
    result
}

fn convert_for_mix(color: &AbsoluteColor, color_space: ColorSpace) -> AbsoluteColor {
    color.to_color_space_with_missing(color_space)
}

fn interpolate_premultiplied_component(
    left: f32,
    left_weight: f32,
    left_alpha: f32,
    right: f32,
    right_weight: f32,
    right_alpha: f32,
) -> f32 {
    left * left_weight * left_alpha + right * right_weight * right_alpha
}

// Normalize hue into [0, 360)
#[inline]
fn normalize_hue(v: f32) -> f32 {
    v - 360. * (v / 360.).floor()
}

fn adjust_hue(left: &mut f32, right: &mut f32, hue_interpolation: HueInterpolationMethod) {
    // Adjust the hue angle as per
    // https://drafts.csswg.org/css-color/#hue-interpolation.
    //
    // If both hue angles are NAN, they should be set to 0. Otherwise, if a
    // single hue angle is NAN, it should use the other hue angle.
    if left.is_nan() {
        if right.is_nan() {
            *left = 0.;
            *right = 0.;
        } else {
            *left = *right;
        }
    } else if right.is_nan() {
        *right = *left;
    }

    if hue_interpolation == HueInterpolationMethod::Specified {
        // Angles are not adjusted. They are interpolated like any other
        // component.
        return;
    }

    *left = normalize_hue(*left);
    *right = normalize_hue(*right);

    match hue_interpolation {
        // https://drafts.csswg.org/css-color/#shorter
        HueInterpolationMethod::Shorter => {
            let delta = *right - *left;

            if delta > 180. {
                *left += 360.;
            } else if delta < -180. {
                *right += 360.;
            }
        },
        // https://drafts.csswg.org/css-color/#longer
        HueInterpolationMethod::Longer => {
            let delta = *right - *left;
            if 0. < delta && delta < 180. {
                *left += 360.;
            } else if -180. < delta && delta <= 0. {
                *right += 360.;
            }
        },
        // https://drafts.csswg.org/css-color/#increasing
        HueInterpolationMethod::Increasing => {
            if *right < *left {
                *right += 360.;
            }
        },
        // https://drafts.csswg.org/css-color/#decreasing
        HueInterpolationMethod::Decreasing => {
            if *left < *right {
                *left += 360.;
            }
        },
        HueInterpolationMethod::Specified => unreachable!("Handled above"),
    }
}

fn interpolate_hue(
    mut left: f32,
    left_weight: f32,
    mut right: f32,
    right_weight: f32,
    hue_interpolation: HueInterpolationMethod,
) -> f32 {
    adjust_hue(&mut left, &mut right, hue_interpolation);
    left * left_weight + right * right_weight
}

struct InterpolatedAlpha {
    /// The adjusted left alpha value.
    left: f32,
    /// The adjusted right alpha value.
    right: f32,
    /// The interpolated alpha value.
    interpolated: f32,
    /// Whether the alpha component should be `none`.
    is_none: bool,
}

fn interpolate_alpha(
    left: f32,
    left_weight: f32,
    right: f32,
    right_weight: f32,
    outcome: ComponentMixOutcome,
) -> InterpolatedAlpha {
    // <https://drafts.csswg.org/css-color-4/#interpolation-missing>
    let mut result = match outcome {
        ComponentMixOutcome::Mix => {
            let interpolated = left * left_weight + right * right_weight;
            InterpolatedAlpha {
                left,
                right,
                interpolated,
                is_none: false,
            }
        },
        ComponentMixOutcome::UseLeft => InterpolatedAlpha {
            left,
            right: left,
            interpolated: left,
            is_none: false,
        },
        ComponentMixOutcome::UseRight => InterpolatedAlpha {
            left: right,
            right,
            interpolated: right,
            is_none: false,
        },
        ComponentMixOutcome::None => InterpolatedAlpha {
            left: 1.0,
            right: 1.0,
            interpolated: 0.0,
            is_none: true,
        },
    };

    // Clip all alpha values to [0.0..1.0].
    result.left = result.left.clamp(0.0, 1.0);
    result.right = result.right.clamp(0.0, 1.0);
    result.interpolated = result.interpolated.clamp(0.0, 1.0);

    result
}

fn interpolate_premultiplied(
    left: &[f32; 4],
    left_weight: f32,
    right: &[f32; 4],
    right_weight: f32,
    hue_index: Option<usize>,
    hue_interpolation: HueInterpolationMethod,
    outcomes: &[ComponentMixOutcome; 4],
) -> ([f32; 4], ColorFlags) {
    let alpha = interpolate_alpha(left[3], left_weight, right[3], right_weight, outcomes[3]);
    let mut flags = if alpha.is_none {
        ColorFlags::ALPHA_IS_NONE
    } else {
        ColorFlags::empty()
    };

    let mut result = [0.; 4];

    for i in 0..3 {
        match outcomes[i] {
            ComponentMixOutcome::Mix => {
                let is_hue = hue_index == Some(i);
                result[i] = if is_hue {
                    normalize_hue(interpolate_hue(
                        left[i],
                        left_weight,
                        right[i],
                        right_weight,
                        hue_interpolation,
                    ))
                } else {
                    let interpolated = interpolate_premultiplied_component(
                        left[i],
                        left_weight,
                        alpha.left,
                        right[i],
                        right_weight,
                        alpha.right,
                    );

                    if alpha.interpolated == 0.0 {
                        interpolated
                    } else {
                        interpolated / alpha.interpolated
                    }
                };
            },
            ComponentMixOutcome::UseLeft | ComponentMixOutcome::UseRight => {
                let used_component = if outcomes[i] == ComponentMixOutcome::UseLeft {
                    left[i]
                } else {
                    right[i]
                };
                result[i] = if hue_interpolation == HueInterpolationMethod::Longer
                    && hue_index == Some(i)
                {
                    // If "longer hue" interpolation is required, we have to actually do
                    // the computation even if we're using the same value at both ends,
                    // so that interpolating from the starting hue back to the same value
                    // produces a full cycle, rather than a constant hue.
                    normalize_hue(interpolate_hue(
                        used_component,
                        left_weight,
                        used_component,
                        right_weight,
                        hue_interpolation,
                    ))
                } else {
                    used_component
                };
            },
            ComponentMixOutcome::None => {
                result[i] = 0.0;
                match i {
                    0 => flags.insert(ColorFlags::C0_IS_NONE),
                    1 => flags.insert(ColorFlags::C1_IS_NONE),
                    2 => flags.insert(ColorFlags::C2_IS_NONE),
                    _ => unreachable!(),
                }
            },
        }
    }
    result[3] = alpha.interpolated;

    (result, flags)
}

#[cfg(test)]
mod tests {
    use super::{mix_many, ColorInterpolationMethod, ColorMixItem};
    use crate::color::{AbsoluteColor, ColorSpace};
    use crate::values::generics::color::ColorMixFlags;

    #[test]
    fn modern_hsl_mix_keeps_missing_hue() {
        let flags = ColorMixFlags::NORMALIZE_WEIGHTS | ColorMixFlags::RESULT_IN_MODERN_SYNTAX;
        let mixed = mix_many(
            ColorInterpolationMethod {
                space: ColorSpace::Hsl,
                hue: super::HueInterpolationMethod::Shorter,
            },
            [ColorMixItem::new(
                AbsoluteColor::new(ColorSpace::Hsl, None::<f32>, 50.0, 50.0, 1.0),
                1.0,
            )],
            flags,
        );
        assert_eq!(mixed.color_space, ColorSpace::Hsl);
        assert_eq!(mixed.c0(), None);

        let numeric = mix_many(
            ColorInterpolationMethod {
                space: ColorSpace::Hsl,
                hue: super::HueInterpolationMethod::Shorter,
            },
            [ColorMixItem::new(
                AbsoluteColor::new(ColorSpace::Hsl, 180.0, 50.0, 50.0, 1.0),
                1.0,
            )],
            flags,
        );
        assert_eq!(numeric.color_space, ColorSpace::Srgb);

        let gray = mix_many(
            ColorInterpolationMethod {
                space: ColorSpace::Hsl,
                hue: super::HueInterpolationMethod::Shorter,
            },
            [ColorMixItem::new(
                AbsoluteColor::srgb_legacy(128, 128, 128, 1.0),
                1.0,
            )],
            flags,
        );
        assert_eq!(gray.color_space, ColorSpace::Hsl);
        assert_eq!(gray.c0(), None);
    }

    #[test]
    fn hsl_mix_keeps_hue_missing_after_powerless_hwb_conversion() {
        let mixed = mix_many(
            ColorInterpolationMethod {
                space: ColorSpace::Hsl,
                hue: super::HueInterpolationMethod::Shorter,
            },
            [ColorMixItem::new(
                AbsoluteColor::new(ColorSpace::Hwb, 180.0, 100.0, 25.0, 1.0),
                1.0,
            )],
            ColorMixFlags::NORMALIZE_WEIGHTS | ColorMixFlags::RESULT_IN_MODERN_SYNTAX,
        );
        assert_eq!(mixed.color_space, ColorSpace::Hsl);
        assert_eq!(mixed.c0(), None);
    }

    #[test]
    fn zero_weight_items_keep_the_mixed_channels_with_zero_alpha() {
        let interpolation = ColorInterpolationMethod {
            space: ColorSpace::Srgb,
            hue: super::HueInterpolationMethod::Shorter,
        };
        let red = AbsoluteColor::new(ColorSpace::Srgb, 1.0, 0.0, 0.0, 1.0);
        let green = AbsoluteColor::new(ColorSpace::Srgb, 0.0, 1.0, 0.0, 1.0);
        let blue = AbsoluteColor::new(ColorSpace::Srgb, 0.0, 0.0, 1.0, 1.0);
        let mixed = mix_many(
            interpolation,
            [
                ColorMixItem::new(red, 0.0),
                ColorMixItem::new(green, 0.0),
                ColorMixItem::new(blue, 0.0),
            ],
            ColorMixFlags::NORMALIZE_WEIGHTS,
        );
        assert_eq!(mixed.c0(), Some(0.25));
        assert_eq!(mixed.c1(), Some(0.25));
        assert_eq!(mixed.c2(), Some(0.5));
        assert_eq!(mixed.alpha(), Some(0.0));
    }
}
