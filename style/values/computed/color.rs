/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed color values.

use crate::color::{AbsoluteColor, ColorFloat};
use crate::values::animated::ToAnimatedZero;
use crate::values::computed::percentage::Percentage;
use crate::values::generics::color::{
    GenericCaretColor, GenericColor, GenericColorLayers, GenericColorMix, GenericColorOrAuto,
};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::color::{ColorScheme, ForcedColorAdjust, PrintColorAdjust};

/// The computed value of the `color` property.
pub type ColorPropertyValue = AbsoluteColor;

/// A computed value for `<color>`.
pub type Color = GenericColor<Percentage>;

/// A computed color-mix().
pub type ColorMix = GenericColorMix<Color, Percentage>;

/// A computed color-layers().
pub type ColorLayers = GenericColorLayers<Color>;

impl ToCss for Color {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: fmt::Write,
    {
        match *self {
            Self::Absolute(ref c) => c.to_css(dest),
            Self::ColorFunction(ref color_function) => color_function.to_css(dest),
            Self::CurrentColor => dest.write_str("currentcolor"),
            Self::ColorMix(ref m) => m.to_css(dest),
            Self::ColorLayers(ref layers) => layers.to_css(dest),
            Self::ContrastColor(ref c) => {
                dest.write_str("contrast-color(")?;
                c.to_css(dest)?;
                dest.write_char(')')
            },
        }
    }
}

impl Color {
    /// A fully transparent color.
    pub const TRANSPARENT_BLACK: Self = Self::Absolute(AbsoluteColor::TRANSPARENT_BLACK);

    /// An opaque black color.
    pub const BLACK: Self = Self::Absolute(AbsoluteColor::BLACK);

    /// An opaque white color.
    pub const WHITE: Self = Self::Absolute(AbsoluteColor::WHITE);

    /// Create a new computed [`Color`] from a given color-mix, simplifying it to an absolute color
    /// if possible.
    pub fn from_color_mix(color_mix: ColorMix) -> Self {
        if let Some(absolute) = color_mix.mix_to_absolute() {
            Self::Absolute(absolute)
        } else {
            Self::ColorMix(Box::new(color_mix))
        }
    }

    /// Resolve color layers without currentcolor at computed-value time.
    pub fn from_color_layers(layers: ColorLayers) -> Self {
        let mut colors = layers.colors.iter().rev();
        let Some(mut result) = colors.next().and_then(Self::as_absolute).copied() else {
            return Self::ColorLayers(Box::new(layers));
        };
        for color in colors {
            let Some(source) = color.as_absolute() else {
                return Self::ColorLayers(Box::new(layers));
            };
            result = crate::color::layers::composite(&result, source, layers.blend_mode);
        }
        Self::Absolute(result)
    }

    /// Resolve contrast against an absolute background at computed-value time.
    /// Keep colors depending on currentcolor unresolved for inherited values.
    pub fn from_contrast_color(background: Self) -> Self {
        match background {
            Self::Absolute(color) => Self::Absolute(Self::contrasting_color(&color)),
            other => Self::ContrastColor(Box::new(other)),
        }
    }

    /// Combine this complex color with the given foreground color into an
    /// absolute color.
    pub fn resolve_to_absolute(&self, current_color: &AbsoluteColor) -> AbsoluteColor {
        use crate::values::specified::percentage::ToPercentage;

        match *self {
            Self::Absolute(c) => c,
            Self::ColorFunction(ref color_function) => {
                color_function.resolve_to_absolute(current_color)
            },
            Self::CurrentColor => *current_color,
            Self::ColorMix(ref mix) => {
                use crate::color::mix;

                mix::mix_many(
                    mix.interpolation,
                    mix.items().iter().map(|item| {
                        mix::ColorMixItem::new(
                            item.color.resolve_to_absolute(current_color),
                            ColorFloat::from(item.percentage.value().to_percentage()),
                        )
                    }),
                    mix.flags,
                )
            },
            Self::ColorLayers(ref layers) => {
                let mut colors = layers.colors.iter().rev();
                let first = colors.next().expect("a parsed color-layers has a color");
                let mut result = first.resolve_to_absolute(current_color);
                for color in colors {
                    result = crate::color::layers::composite(
                        &result,
                        &color.resolve_to_absolute(current_color),
                        layers.blend_mode,
                    );
                }
                result
            },
            Self::ContrastColor(ref c) => {
                Self::contrasting_color(&c.resolve_to_absolute(current_color))
            },
        }
    }

    fn contrasting_color(background: &AbsoluteColor) -> AbsoluteColor {
        if Self::contrast_ratio(background, &AbsoluteColor::BLACK)
            > Self::contrast_ratio(background, &AbsoluteColor::WHITE)
        {
            AbsoluteColor::BLACK
        } else {
            AbsoluteColor::WHITE
        }
    }

    fn contrast_ratio(a: &AbsoluteColor, b: &AbsoluteColor) -> ColorFloat {
        // TODO: This just implements the WCAG 2.1 algorithm,
        // https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio
        // Consider using a more sophisticated contrast algorithm, e.g. see
        // https://apcacontrast.com
        let compute = |c: ColorFloat| -> ColorFloat {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ColorFloat::powf((c + 0.055) / 1.055, 2.4)
            }
        };
        let luminance = |r, g, b| -> ColorFloat { 0.2126 * r + 0.7152 * g + 0.0722 * b };
        let a = a.into_srgb_legacy();
        let b = b.into_srgb_legacy();
        let a = a.raw_components();
        let b = b.raw_components();
        let la = luminance(compute(a[0]), compute(a[1]), compute(a[2])) + 0.05;
        let lb = luminance(compute(b[0]), compute(b[1]), compute(b[2])) + 0.05;
        if la > lb {
            la / lb
        } else {
            lb / la
        }
    }
}

impl ToAnimatedZero for AbsoluteColor {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Ok(Self::TRANSPARENT_BLACK)
    }
}

/// auto | <color>
pub type ColorOrAuto = GenericColorOrAuto<Color>;

/// caret-color
pub type CaretColor = GenericCaretColor<Color>;
