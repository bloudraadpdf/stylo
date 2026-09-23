/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Color support functions.

/// cbindgen:ignore
pub mod convert;

mod color_function;
pub mod component;
pub mod mix;
pub mod parsing;
mod to_css;

use self::parsing::ChannelKeyword;
use crate::derives::*;
pub use color_function::*;
use component::ColorComponent;
use cssparser::color::PredefinedColorSpace;

/// Number of color-mix items to reserve on the stack to avoid heap allocations.
pub const PRE_ALLOCATED_COLOR_MIX_ITEMS: usize = 3;

/// Conveniece type to use for collecting color mix items.
pub type ColorMixItemList<T> = smallvec::SmallVec<[T; PRE_ALLOCATED_COLOR_MIX_ITEMS]>;

/// The 3 components that make up a color.  (Does not include the alpha component)
#[derive(Copy, Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
#[cfg_attr(feature = "servo", derive(Deserialize, Serialize))]
#[repr(C)]
pub struct ColorComponents(pub f32, pub f32, pub f32);

impl ColorComponents {
    /// Apply a function to each of the 3 components of the color.
    #[must_use]
    pub fn map(self, f: impl Fn(f32) -> f32) -> Self {
        Self(f(self.0), f(self.1), f(self.2))
    }
}

impl std::ops::Mul for ColorComponents {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0, self.1 * rhs.1, self.2 * rhs.2)
    }
}

impl std::ops::Div for ColorComponents {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0, self.1 / rhs.1, self.2 / rhs.2)
    }
}

/// A color space representation in the CSS specification.
///
/// https://drafts.csswg.org/css-color-4/#typedef-color-space
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
#[cfg_attr(feature = "servo", derive(Deserialize, Serialize))]
#[repr(u8)]
pub enum ColorSpace {
    /// A color specified in the sRGB color space with either the rgb/rgba(..)
    /// functions or the newer color(srgb ..) function. If the color(..)
    /// function is used, the AS_COLOR_FUNCTION flag will be set. Examples:
    /// "color(srgb 0.691 0.139 0.259)", "rgb(176, 35, 66)"
    Srgb = 0,
    /// A color specified in the Hsl notation in the sRGB color space, e.g.
    /// "hsl(289.18 93.136% 65.531%)"
    /// https://drafts.csswg.org/css-color-4/#the-hsl-notation
    Hsl,
    /// A color specified in the Hwb notation in the sRGB color space, e.g.
    /// "hwb(740deg 20% 30%)"
    /// https://drafts.csswg.org/css-color-4/#the-hwb-notation
    Hwb,
    /// A color specified in the Lab color format, e.g.
    /// "lab(29.2345% 39.3825 20.0664)".
    /// https://w3c.github.io/csswg-drafts/css-color-4/#lab-colors
    Lab,
    /// A color specified in the Lch color format, e.g.
    /// "lch(29.2345% 44.2 27)".
    /// https://w3c.github.io/csswg-drafts/css-color-4/#lch-colors
    Lch,
    /// A color specified in the Oklab color format, e.g.
    /// "oklab(40.101% 0.1147 0.0453)".
    /// https://w3c.github.io/csswg-drafts/css-color-4/#lab-colors
    Oklab,
    /// A color specified in the Oklch color format, e.g.
    /// "oklch(40.101% 0.12332 21.555)".
    /// https://w3c.github.io/csswg-drafts/css-color-4/#lch-colors
    Oklch,
    /// A color specified with the color(..) function and the "srgb-linear"
    /// color space, e.g. "color(srgb-linear 0.435 0.017 0.055)".
    SrgbLinear,
    /// A color specified with the color(..) function and the "display-p3"
    /// color space, e.g. "color(display-p3 0.84 0.19 0.72)".
    DisplayP3,
    /// A color specified with the color(..) function and the "display-p3-linear"
    /// color space.
    DisplayP3Linear,
    /// A color specified with the color(..) function and the "a98-rgb" color
    /// space, e.g. "color(a98-rgb 0.44091 0.49971 0.37408)".
    A98Rgb,
    /// A color specified with the color(..) function and the "prophoto-rgb"
    /// color space, e.g. "color(prophoto-rgb 0.36589 0.41717 0.31333)".
    ProphotoRgb,
    /// A color specified with the color(..) function and the "rec2020" color
    /// space, e.g. "color(rec2020 0.42210 0.47580 0.35605)".
    Rec2020,
    /// A color specified with the color(..) function and the "xyz-d50" color
    /// space, e.g. "color(xyz-d50 0.2005 0.14089 0.4472)".
    XyzD50,
    /// A color specified with the color(..) function and the "xyz-d65" or "xyz"
    /// color space, e.g. "color(xyz-d65 0.21661 0.14602 0.59452)".
    /// NOTE: https://drafts.csswg.org/css-color-4/#resolving-color-function-values
    ///       specifies that `xyz` is an alias for the `xyz-d65` color space.
    #[parse(aliases = "xyz")]
    XyzD65,
    /// CSS Color HDR Module Level 1 §4 — `color(rec2100-pq …)`. ITU-R BT.2100
    /// primaries (identical to Rec. 2020) with the SMPTE ST 2084 perceptual
    /// quantiser transfer function. Peak luminance 10 000 cd/m².
    /// https://drafts.csswg.org/css-color-hdr/#rec2100-pq
    Rec2100Pq,
    /// CSS Color HDR Module Level 1 §4 — `color(rec2100-hlg …)`. ITU-R BT.2100
    /// primaries with the ARIB STD-B67 hybrid log-gamma transfer function.
    /// https://drafts.csswg.org/css-color-hdr/#rec2100-hlg
    Rec2100Hlg,
    /// CSS Color HDR Module Level 1 §4 — `color(rec2100-linear …)`. ITU-R BT.2100
    /// primaries with a linear (gamma = 1.0) transfer function — the
    /// scene-linear representation used for compositing pipelines.
    /// https://drafts.csswg.org/css-color-hdr/#rec2100-linear
    Rec2100Linear,
}

impl ColorSpace {
    /// Returns whether this is a `<rectangular-color-space>`.
    #[inline]
    pub fn is_rectangular(&self) -> bool {
        !self.is_polar()
    }

    /// Returns whether this is a `<polar-color-space>`.
    #[inline]
    pub fn is_polar(&self) -> bool {
        matches!(self, Self::Hsl | Self::Hwb | Self::Lch | Self::Oklch)
    }

    /// Returns true if the color has RGB or XYZ components.
    #[inline]
    pub fn is_rgb_or_xyz_like(&self) -> bool {
        match self {
            Self::Srgb
            | Self::SrgbLinear
            | Self::DisplayP3
            | Self::DisplayP3Linear
            | Self::A98Rgb
            | Self::ProphotoRgb
            | Self::Rec2020
            | Self::Rec2100Pq
            | Self::Rec2100Hlg
            | Self::Rec2100Linear
            | Self::XyzD50
            | Self::XyzD65 => true,
            _ => false,
        }
    }

    /// Returns an index of the hue component in the color space, otherwise
    /// `None`.
    #[inline]
    pub fn hue_index(&self) -> Option<usize> {
        match self {
            Self::Hsl | Self::Hwb => Some(0),
            Self::Lch | Self::Oklch => Some(2),

            _ => {
                debug_assert!(!self.is_polar());
                None
            },
        }
    }
}

/// Flags used when serializing colors.
#[derive(Clone, Copy, Debug, Default, MallocSizeOf, PartialEq, ToShmem)]
#[cfg_attr(feature = "serde", derive(Deserialize, Serialize))]
#[repr(C)]
pub struct ColorFlags(u8);
bitflags! {
    impl ColorFlags : u8 {
        /// Whether the 1st color component is `none`.
        const C0_IS_NONE = 1 << 0;
        /// Whether the 2nd color component is `none`.
        const C1_IS_NONE = 1 << 1;
        /// Whether the 3rd color component is `none`.
        const C2_IS_NONE = 1 << 2;
        /// Whether the alpha component is `none`.
        const ALPHA_IS_NONE = 1 << 3;
        /// Marks that this color is in the legacy color format. This flag is
        /// only valid for the `Srgb` color space.
        const IS_LEGACY_SRGB = 1 << 4;
        /// Serialize a modern direct rgb() with missing channels in legacy form
        /// without changing how its missing channels participate in interpolation.
        const SERIALIZE_AS_LEGACY_SRGB = 1 << 5;
    }
}

/// An absolutely specified color, using either rgb(), rgba(), lab(), lch(),
/// oklab(), oklch() or color().
#[derive(Copy, Clone, Debug, MallocSizeOf, PartialEq, ToShmem, ToTyped)]
#[cfg_attr(feature = "servo", derive(Deserialize, Serialize))]
#[repr(C)]
pub struct AbsoluteColor {
    /// The 3 components that make up colors in any color space.
    pub components: ColorComponents,
    /// The alpha component of the color.
    pub alpha: f32,
    /// The current color space that the components represent.
    pub color_space: ColorSpace,
    /// Extra flags used durring serialization of this color.
    pub flags: ColorFlags,
}

/// Given an [`AbsoluteColor`], return the 4 float components as the type given,
/// e.g.:
///
/// ```rust
/// let srgb = AbsoluteColor::new(ColorSpace::Srgb, 1.0, 0.0, 0.0, 0.0);
/// let floats = color_components_as!(&srgb, [f32; 4]); // [1.0, 0.0, 0.0, 0.0]
/// ```
macro_rules! color_components_as {
    ($c:expr, $t:ty) => {{
        // This macro is not an inline function, because we can't use the
        // generic  type ($t) in a constant expression as per:
        // https://github.com/rust-lang/rust/issues/76560
        const_assert_eq!(std::mem::size_of::<$t>(), std::mem::size_of::<[f32; 4]>());
        const_assert_eq!(std::mem::align_of::<$t>(), std::mem::align_of::<[f32; 4]>());
        const_assert!(std::mem::size_of::<AbsoluteColor>() >= std::mem::size_of::<$t>());
        const_assert_eq!(
            std::mem::align_of::<AbsoluteColor>(),
            std::mem::align_of::<$t>()
        );

        std::mem::transmute::<&ColorComponents, &$t>(&$c.components)
    }};
}

/// Holds details about each component passed into creating a new [`AbsoluteColor`].
pub struct ComponentDetails {
    value: f32,
    is_none: bool,
}

impl From<f32> for ComponentDetails {
    fn from(value: f32) -> Self {
        Self {
            value,
            is_none: false,
        }
    }
}

impl From<u8> for ComponentDetails {
    fn from(value: u8) -> Self {
        Self {
            value: value as f32 / 255.0,
            is_none: false,
        }
    }
}

impl From<Option<f32>> for ComponentDetails {
    fn from(value: Option<f32>) -> Self {
        if let Some(value) = value {
            Self {
                value,
                is_none: false,
            }
        } else {
            Self {
                value: 0.0,
                is_none: true,
            }
        }
    }
}

impl From<ColorComponent<f32>> for ComponentDetails {
    fn from(value: ColorComponent<f32>) -> Self {
        if let ColorComponent::Value(value) = value {
            Self {
                value,
                is_none: false,
            }
        } else {
            Self {
                value: 0.0,
                is_none: true,
            }
        }
    }
}

impl AbsoluteColor {
    /// A fully transparent color in the legacy syntax.
    pub const TRANSPARENT_BLACK: Self = Self {
        components: ColorComponents(0.0, 0.0, 0.0),
        alpha: 0.0,
        color_space: ColorSpace::Srgb,
        flags: ColorFlags::IS_LEGACY_SRGB,
    };

    /// An opaque black color in the legacy syntax.
    pub const BLACK: Self = Self {
        components: ColorComponents(0.0, 0.0, 0.0),
        alpha: 1.0,
        color_space: ColorSpace::Srgb,
        flags: ColorFlags::IS_LEGACY_SRGB,
    };

    /// An opaque white color in the legacy syntax.
    pub const WHITE: Self = Self {
        components: ColorComponents(1.0, 1.0, 1.0),
        alpha: 1.0,
        color_space: ColorSpace::Srgb,
        flags: ColorFlags::IS_LEGACY_SRGB,
    };

    /// Create a new [`AbsoluteColor`] with the given [`ColorSpace`] and
    /// components.
    pub fn new(
        color_space: ColorSpace,
        c1: impl Into<ComponentDetails>,
        c2: impl Into<ComponentDetails>,
        c3: impl Into<ComponentDetails>,
        alpha: impl Into<ComponentDetails>,
    ) -> Self {
        Self::new_impl(color_space, c1, c2, c3, alpha, true)
    }

    pub(crate) fn new_unclamped(
        color_space: ColorSpace,
        c1: impl Into<ComponentDetails>,
        c2: impl Into<ComponentDetails>,
        c3: impl Into<ComponentDetails>,
        alpha: impl Into<ComponentDetails>,
    ) -> Self {
        Self::new_impl(color_space, c1, c2, c3, alpha, false)
    }

    pub(crate) fn new_impl(
        color_space: ColorSpace,
        c1: impl Into<ComponentDetails>,
        c2: impl Into<ComponentDetails>,
        c3: impl Into<ComponentDetails>,
        alpha: impl Into<ComponentDetails>,
        clamp_components: bool,
    ) -> Self {
        let mut flags = ColorFlags::empty();

        macro_rules! cd {
            ($c:expr,$flag:expr) => {{
                let component_details = $c.into();
                if component_details.is_none {
                    flags |= $flag;
                }
                component_details.value
            }};
        }

        let mut components = ColorComponents(
            cd!(c1, ColorFlags::C0_IS_NONE),
            cd!(c2, ColorFlags::C1_IS_NONE),
            cd!(c3, ColorFlags::C2_IS_NONE),
        );

        let alpha = cd!(alpha, ColorFlags::ALPHA_IS_NONE);

        // Lightness for Lab and Lch is clamped to [0..100].
        if clamp_components && matches!(color_space, ColorSpace::Lab | ColorSpace::Lch) {
            components.0 = components.0.clamp(0.0, 100.0);
        }

        // Lightness for Oklab and Oklch is clamped to [0..1].
        if clamp_components && matches!(color_space, ColorSpace::Oklab | ColorSpace::Oklch) {
            components.0 = components.0.clamp(0.0, 1.0);
        }

        // Chroma must not be less than 0.
        if clamp_components && matches!(color_space, ColorSpace::Lch | ColorSpace::Oklch) {
            components.1 = components.1.max(0.0);
        }

        // Alpha is always clamped to [0..1].
        let alpha = alpha.clamp(0.0, 1.0);

        Self {
            components,
            alpha,
            color_space,
            flags,
        }
    }

    /// Convert this color into the sRGB color space and set it to the legacy
    /// syntax.
    #[inline]
    #[must_use]
    pub fn into_srgb_legacy(self) -> Self {
        let mut result = if !matches!(self.color_space, ColorSpace::Srgb) {
            self.to_color_space(ColorSpace::Srgb)
        } else {
            self
        };

        // Explicitly set the flags to IS_LEGACY_SRGB only to clear out the
        // *_IS_NONE flags, because the legacy syntax doesn't allow "none".
        result.flags = ColorFlags::IS_LEGACY_SRGB;

        result
    }

    /// Create a new [`AbsoluteColor`] from rgba legacy syntax values in the sRGB color space.
    pub fn srgb_legacy(red: u8, green: u8, blue: u8, alpha: f32) -> Self {
        let mut result = Self::new(ColorSpace::Srgb, red, green, blue, alpha);
        result.flags = ColorFlags::IS_LEGACY_SRGB;
        result
    }

    /// Return all the components of the color in an array.  (Includes alpha)
    #[inline]
    pub fn raw_components(&self) -> &[f32; 4] {
        unsafe { color_components_as!(self, [f32; 4]) }
    }

    /// Returns true if this color is in the legacy color syntax.
    #[inline]
    pub fn is_legacy_syntax(&self) -> bool {
        // rgb(), rgba(), hsl(), hsla(), hwb(), hwba()
        match self.color_space {
            ColorSpace::Srgb => self.flags.contains(ColorFlags::IS_LEGACY_SRGB),
            ColorSpace::Hsl | ColorSpace::Hwb => true,
            _ => false,
        }
    }

    /// Returns true if this color is fully transparent.
    #[inline]
    pub fn is_transparent(&self) -> bool {
        self.flags.contains(ColorFlags::ALPHA_IS_NONE) || self.alpha == 0.0
    }

    /// Return an optional first component.
    #[inline]
    pub fn c0(&self) -> Option<f32> {
        if self.flags.contains(ColorFlags::C0_IS_NONE) {
            None
        } else {
            Some(self.components.0)
        }
    }

    /// Return an optional second component.
    #[inline]
    pub fn c1(&self) -> Option<f32> {
        if self.flags.contains(ColorFlags::C1_IS_NONE) {
            None
        } else {
            Some(self.components.1)
        }
    }

    /// Return an optional second component.
    #[inline]
    pub fn c2(&self) -> Option<f32> {
        if self.flags.contains(ColorFlags::C2_IS_NONE) {
            None
        } else {
            Some(self.components.2)
        }
    }

    /// Return an optional alpha component.
    #[inline]
    pub fn alpha(&self) -> Option<f32> {
        if self.flags.contains(ColorFlags::ALPHA_IS_NONE) {
            None
        } else {
            Some(self.alpha)
        }
    }

    /// Return the value of a component by its channel keyword.
    pub fn get_component_by_channel_keyword(
        &self,
        channel_keyword: ChannelKeyword,
    ) -> Result<Option<f32>, ()> {
        if channel_keyword == ChannelKeyword::Alpha {
            return Ok(self.alpha());
        }

        Ok(match self.color_space {
            ColorSpace::Srgb => {
                if self.flags.contains(ColorFlags::IS_LEGACY_SRGB) {
                    match channel_keyword {
                        ChannelKeyword::R => self.c0().map(|v| v * 255.0),
                        ChannelKeyword::G => self.c1().map(|v| v * 255.0),
                        ChannelKeyword::B => self.c2().map(|v| v * 255.0),
                        _ => return Err(()),
                    }
                } else {
                    match channel_keyword {
                        ChannelKeyword::R => self.c0(),
                        ChannelKeyword::G => self.c1(),
                        ChannelKeyword::B => self.c2(),
                        _ => return Err(()),
                    }
                }
            },
            ColorSpace::Hsl => match channel_keyword {
                ChannelKeyword::H => self.c0(),
                ChannelKeyword::S => self.c1(),
                ChannelKeyword::L => self.c2(),
                _ => return Err(()),
            },
            ColorSpace::Hwb => match channel_keyword {
                ChannelKeyword::H => self.c0(),
                ChannelKeyword::W => self.c1(),
                ChannelKeyword::B => self.c2(),
                _ => return Err(()),
            },
            ColorSpace::Lab | ColorSpace::Oklab => match channel_keyword {
                ChannelKeyword::L => self.c0(),
                ChannelKeyword::A => self.c1(),
                ChannelKeyword::B => self.c2(),
                _ => return Err(()),
            },
            ColorSpace::Lch | ColorSpace::Oklch => match channel_keyword {
                ChannelKeyword::L => self.c0(),
                ChannelKeyword::C => self.c1(),
                ChannelKeyword::H => self.c2(),
                _ => return Err(()),
            },
            ColorSpace::SrgbLinear
            | ColorSpace::DisplayP3
            | ColorSpace::DisplayP3Linear
            | ColorSpace::A98Rgb
            | ColorSpace::ProphotoRgb
            | ColorSpace::Rec2020
            | ColorSpace::Rec2100Pq
            | ColorSpace::Rec2100Hlg
            | ColorSpace::Rec2100Linear => match channel_keyword {
                ChannelKeyword::R => self.c0(),
                ChannelKeyword::G => self.c1(),
                ChannelKeyword::B => self.c2(),
                _ => return Err(()),
            },
            ColorSpace::XyzD50 | ColorSpace::XyzD65 => match channel_keyword {
                ChannelKeyword::X => self.c0(),
                ChannelKeyword::Y => self.c1(),
                ChannelKeyword::Z => self.c2(),
                _ => return Err(()),
            },
        })
    }

    /// Convert this color to the specified color space.
    pub fn to_color_space(&self, color_space: ColorSpace) -> Self {
        self.to_color_space_impl(color_space)
    }

    pub(crate) fn to_color_space_for_relative(&self, color_space: ColorSpace) -> Self {
        self.to_color_space_with_missing(color_space)
    }

    fn to_color_space_impl(&self, color_space: ColorSpace) -> Self {
        use ColorSpace::*;

        if self.color_space == color_space {
            return self.clone();
        }

        // Missing components act as zero while converting between color spaces.
        // Interpolation carries analogous missing components forward separately.
        macro_rules! missing_to_zero {
            ($c:expr) => {{
                if let Some(v) = $c {
                    crate::values::normalize(v)
                } else {
                    0.0
                }
            }};
        }

        let components = ColorComponents(
            missing_to_zero!(self.c0()),
            missing_to_zero!(self.c1()),
            missing_to_zero!(self.c2()),
        );

        let result = match (self.color_space, color_space) {
            // We have simplified conversions that do not need to convert to XYZ
            // first. This improves performance, because it skips at least 2
            // matrix multiplications and reduces float rounding errors.
            (Srgb, Hsl) => convert::rgb_to_hsl(&components),
            (Srgb, Hwb) => convert::rgb_to_hwb(&components),
            (Hsl, Srgb) => convert::hsl_to_rgb(&components),
            (Hwb, Srgb) => convert::hwb_to_rgb(&components),
            (Hwb, Hsl) => convert::rgb_to_hsl(&convert::hwb_to_rgb(&components)),
            (Hsl, Hwb) => convert::rgb_to_hwb(&convert::hsl_to_rgb(&components)),
            (Lab, Lch) | (Oklab, Oklch) => convert::orthogonal_to_polar(
                &components,
                if color_space == Lch {
                    convert::LCH_HUE_EPSILON
                } else {
                    convert::OKLCH_HUE_EPSILON
                },
            ),
            (Lch, Lab) | (Oklch, Oklab) => convert::polar_to_orthogonal(&components),

            // All other conversions need to convert to XYZ first.
            _ => {
                let (xyz, white_point) = match self.color_space {
                    Lab => convert::to_xyz::<convert::Lab>(&components),
                    Lch => convert::to_xyz::<convert::Lch>(&components),
                    Oklab => convert::to_xyz::<convert::Oklab>(&components),
                    Oklch => convert::to_xyz::<convert::Oklch>(&components),
                    Srgb => convert::to_xyz::<convert::Srgb>(&components),
                    Hsl => convert::to_xyz::<convert::Hsl>(&components),
                    Hwb => convert::to_xyz::<convert::Hwb>(&components),
                    SrgbLinear => convert::to_xyz::<convert::SrgbLinear>(&components),
                    DisplayP3 => convert::to_xyz::<convert::DisplayP3>(&components),
                    DisplayP3Linear => convert::to_xyz::<convert::DisplayP3Linear>(&components),
                    A98Rgb => convert::to_xyz::<convert::A98Rgb>(&components),
                    ProphotoRgb => convert::to_xyz::<convert::ProphotoRgb>(&components),
                    Rec2020 => convert::to_xyz::<convert::Rec2020>(&components),
                    Rec2100Pq => convert::to_xyz::<convert::Rec2100Pq>(&components),
                    Rec2100Hlg => convert::to_xyz::<convert::Rec2100Hlg>(&components),
                    Rec2100Linear => convert::to_xyz::<convert::Rec2100Linear>(&components),
                    XyzD50 => convert::to_xyz::<convert::XyzD50>(&components),
                    XyzD65 => convert::to_xyz::<convert::XyzD65>(&components),
                };

                match color_space {
                    Lab => convert::from_xyz::<convert::Lab>(&xyz, white_point),
                    Lch => convert::from_xyz::<convert::Lch>(&xyz, white_point),
                    Oklab => convert::from_xyz::<convert::Oklab>(&xyz, white_point),
                    Oklch => convert::from_xyz::<convert::Oklch>(&xyz, white_point),
                    Srgb => convert::from_xyz::<convert::Srgb>(&xyz, white_point),
                    Hsl => convert::from_xyz::<convert::Hsl>(&xyz, white_point),
                    Hwb => convert::from_xyz::<convert::Hwb>(&xyz, white_point),
                    SrgbLinear => convert::from_xyz::<convert::SrgbLinear>(&xyz, white_point),
                    DisplayP3 => convert::from_xyz::<convert::DisplayP3>(&xyz, white_point),
                    DisplayP3Linear => {
                        convert::from_xyz::<convert::DisplayP3Linear>(&xyz, white_point)
                    },
                    A98Rgb => convert::from_xyz::<convert::A98Rgb>(&xyz, white_point),
                    ProphotoRgb => convert::from_xyz::<convert::ProphotoRgb>(&xyz, white_point),
                    Rec2020 => convert::from_xyz::<convert::Rec2020>(&xyz, white_point),
                    Rec2100Pq => convert::from_xyz::<convert::Rec2100Pq>(&xyz, white_point),
                    Rec2100Hlg => convert::from_xyz::<convert::Rec2100Hlg>(&xyz, white_point),
                    Rec2100Linear => convert::from_xyz::<convert::Rec2100Linear>(&xyz, white_point),
                    XyzD50 => convert::from_xyz::<convert::XyzD50>(&xyz, white_point),
                    XyzD65 => convert::from_xyz::<convert::XyzD65>(&xyz, white_point),
                }
            },
        };

        // A NAN value coming from a conversion function means the the component
        // is missing, so we convert it to None.
        macro_rules! nan_to_missing {
            ($v:expr) => {{
                if $v.is_nan() {
                    None
                } else {
                    Some($v)
                }
            }};
        }

        let mut c0 = nan_to_missing!(result.0);
        let mut c1 = nan_to_missing!(result.1);
        let mut c2 = nan_to_missing!(result.2);

        let source_is_neutral = match self.color_space {
            Hsl => components.1 == 0.0 || components.2 == 0.0 || components.2 == 100.0,
            Hwb => components.1 + components.2 >= 100.0,
            Lab | Oklab => components.1 == 0.0 && components.2 == 0.0,
            Lch | Oklch => components.1 == 0.0,
            Srgb => components.0 == components.1 && components.1 == components.2,
            _ => false,
        };
        if source_is_neutral && matches!(color_space, Lch | Oklch) && c1.is_some() {
            c1 = Some(0.0);
            c2 = None;
        }

        // Check the source because finite-precision conversion can cross the
        // target's powerless threshold.
        let source_hue_powerless = match self.color_space {
            Hsl => self
                .c1()
                .is_some_and(|saturation| saturation.abs() <= 0.001),
            Hwb => self
                .c1()
                .zip(self.c2())
                .is_some_and(|(white, black)| white + black >= 99.999),
            Lch => self
                .c1()
                .is_some_and(|chroma| chroma <= convert::LCH_HUE_EPSILON),
            Oklch => self
                .c1()
                .is_some_and(|chroma| chroma <= convert::OKLCH_HUE_EPSILON),
            _ => false,
        };
        match color_space {
            Hsl if source_hue_powerless
                || c1.is_some_and(|saturation| saturation.abs() <= 0.001) =>
            {
                c0 = None
            },
            Hwb if source_hue_powerless
                || c1
                    .zip(c2)
                    .is_some_and(|(white, black)| white + black >= 99.999) =>
            {
                c0 = None
            },
            Lch if source_hue_powerless
                || c1.is_some_and(|chroma| chroma <= convert::LCH_HUE_EPSILON) =>
            {
                c2 = None
            },
            Oklch
                if source_hue_powerless
                    || c1.is_some_and(|chroma| chroma <= convert::OKLCH_HUE_EPSILON) =>
            {
                c2 = None
            },
            _ => {},
        }

        Self::new_unclamped(color_space, c0, c1, c2, self.alpha())
    }

    /// Convert an origin used for interpolation or relative channel references.
    /// Its missing components are carried to analogous channels after the
    /// numeric conversion has treated them as zero.
    pub(crate) fn to_color_space_with_missing(&self, color_space: ColorSpace) -> Self {
        let mut converted = self.to_color_space(color_space);
        converted.carry_forward_analogous_missing_components(self);
        converted
    }
}

impl From<PredefinedColorSpace> for ColorSpace {
    fn from(value: PredefinedColorSpace) -> Self {
        match value {
            PredefinedColorSpace::Srgb => ColorSpace::Srgb,
            PredefinedColorSpace::SrgbLinear => ColorSpace::SrgbLinear,
            PredefinedColorSpace::DisplayP3 => ColorSpace::DisplayP3,
            PredefinedColorSpace::DisplayP3Linear => ColorSpace::DisplayP3Linear,
            PredefinedColorSpace::A98Rgb => ColorSpace::A98Rgb,
            PredefinedColorSpace::ProphotoRgb => ColorSpace::ProphotoRgb,
            PredefinedColorSpace::Rec2020 => ColorSpace::Rec2020,
            PredefinedColorSpace::XyzD50 => ColorSpace::XyzD50,
            PredefinedColorSpace::XyzD65 => ColorSpace::XyzD65,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AbsoluteColor, ColorSpace};

    #[test]
    fn hwb_to_hsl_to_srgb_keeps_zero_channel_exact() {
        let hwb = AbsoluteColor::new(ColorSpace::Hwb, 180.0, None::<f32>, 25.0, 1.0);
        let srgb = hwb
            .to_color_space(ColorSpace::Hsl)
            .to_color_space(ColorSpace::Srgb);
        assert_eq!(srgb.components.0, 0.0);
    }

    #[test]
    fn conversion_to_hsl_marks_powerless_hue_missing() {
        let boundary = AbsoluteColor::new(ColorSpace::Hwb, 180.0, 49.999, 50.0, 1.0);
        assert_eq!(boundary.to_color_space(ColorSpace::Hsl).c0(), None);

        let hwb = AbsoluteColor::new(ColorSpace::Hwb, 180.0, 100.0, 25.0, 1.0);
        let hsl = hwb.to_color_space(ColorSpace::Hsl);
        assert_eq!(hsl.c0(), None);
        assert_eq!(hsl.c1(), Some(0.0));

        let lch = AbsoluteColor::new(ColorSpace::Lch, 20.0, 0.0, 180.0, 1.0);
        assert_eq!(lch.to_color_space(ColorSpace::Hsl).c0(), None);

        let out_of_gamut = AbsoluteColor::new(ColorSpace::Lch, 0.0, 20.0, 180.0, 1.0);
        assert!(out_of_gamut.to_color_space(ColorSpace::Hsl).c0().is_some());
    }

    #[test]
    fn missing_lightness_is_zero_during_cross_space_conversion() {
        let lch = AbsoluteColor::new(ColorSpace::Lch, None::<f32>, 20.0, 180.0, 1.0);
        let hsl = lch.to_color_space(ColorSpace::Hsl);
        assert!(hsl.c0().is_some());
        assert!(hsl.c1().is_some());
        assert!(hsl.c2().is_some());
    }

    #[test]
    fn interpolation_carries_missing_channels_after_zero_based_conversion() {
        let all_missing =
            AbsoluteColor::new(ColorSpace::Srgb, None::<f32>, None::<f32>, None::<f32>, 1.0);
        let lab = all_missing.to_color_space_with_missing(ColorSpace::Lab);
        assert_eq!((lab.c0(), lab.c1(), lab.c2()), (None, None, None));

        let missing_lightness = AbsoluteColor::new(ColorSpace::Lch, None::<f32>, 20.0, 180.0, 1.0);
        let hsl = missing_lightness.to_color_space_with_missing(ColorSpace::Hsl);
        assert!(hsl.c0().is_some());
        assert!(hsl.c1().is_some());
        assert_eq!(hsl.c2(), None);

        let missing_lightness_and_chroma =
            AbsoluteColor::new(ColorSpace::Lch, None::<f32>, None::<f32>, 180.0, 1.0);
        let hwb = missing_lightness_and_chroma.to_color_space_with_missing(ColorSpace::Hwb);
        assert_eq!((hwb.c0(), hwb.c1(), hwb.c2()), (None, None, None));
    }

    #[test]
    fn conversion_carries_paired_achromatic_components() {
        let hwb = AbsoluteColor::new(ColorSpace::Hwb, 180.0, None::<f32>, None::<f32>, 1.0);
        let hsl = hwb.to_color_space_with_missing(ColorSpace::Hsl);
        assert_eq!((hsl.c0(), hsl.c1(), hsl.c2()), (Some(180.0), None, None));

        let lch = hwb.to_color_space_with_missing(ColorSpace::Lch);
        assert_eq!((lch.c0(), lch.c1()), (None, None));
        assert!(lch.c2().is_some());

        let hsl = AbsoluteColor::new(ColorSpace::Hsl, 180.0, None::<f32>, None::<f32>, 1.0);
        let hwb = hsl.to_color_space_with_missing(ColorSpace::Hwb);
        assert_eq!((hwb.c0(), hwb.c1(), hwb.c2()), (None, None, None));

        let lab = AbsoluteColor::new(ColorSpace::Lab, 50.0, None::<f32>, None::<f32>, 1.0);
        let hsl = lab.to_color_space_with_missing(ColorSpace::Hsl);
        assert_eq!(hsl.c1(), None);
    }

    #[test]
    fn exact_neutral_origins_convert_to_zero_polar_chroma() {
        for origin in [
            AbsoluteColor::new(ColorSpace::Hsl, 180.0, 0.0, 50.0, 1.0),
            AbsoluteColor::new(ColorSpace::Hwb, 180.0, 100.0, 25.0, 1.0),
            AbsoluteColor::new(ColorSpace::Lch, 20.0, 0.0, 180.0, 1.0),
        ] {
            let converted = origin.to_color_space(ColorSpace::Oklch);
            assert_eq!(converted.c1(), Some(0.0), "{origin:?}");
            assert_eq!(converted.c2(), None, "{origin:?}");
        }
    }

    #[test]
    fn out_of_gamut_conversion_keeps_negative_oklch_lightness() {
        let lch = AbsoluteColor::new(ColorSpace::Lch, 0.0, 20.0, 180.0, 1.0);
        let oklch = lch.to_color_space(ColorSpace::Oklch);
        assert!(oklch.c0().expect("lightness") < 0.0);
    }
}
