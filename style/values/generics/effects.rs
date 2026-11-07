/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Generic types for CSS values related to effects.
use crate::derives::*;
use std::fmt::Write;

use crate::values::generics::{NonNegative, ZeroToOne};

/// A generic value for a single `box-shadow`.
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToAnimatedZero,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C)]
pub struct GenericBoxShadow<Color, SizeLength, BlurShapeLength, ShapeLength> {
    /// The base shadow.
    pub base: GenericSimpleShadow<Color, SizeLength, BlurShapeLength>,
    /// The spread radius.
    pub spread: ShapeLength,
    /// Whether this is an inset box shadow.
    #[animation(constant)]
    #[css(represents_keyword)]
    pub inset: bool,
}

pub use self::GenericBoxShadow as BoxShadow;

/// A generic value for a single `filter`.
#[cfg_attr(feature = "servo", derive(Deserialize, Serialize))]
#[derive(
    Clone,
    ComputeSquaredDistance,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
#[animation(no_bound(U))]
#[repr(C, u8)]
pub enum GenericFilter<Angle, Factor, Length, Shadow, U> {
    /// `blur(<length>)`
    #[css(function)]
    Blur(#[animation(field_bound)] NonNegative<Length>),
    /// `brightness(<factor>)`
    #[css(function)]
    Brightness(#[animation(field_bound)] NonNegative<Factor>),
    /// `contrast(<factor>)`
    #[css(function)]
    Contrast(#[animation(field_bound)] NonNegative<Factor>),
    /// `grayscale(<factor>)`
    #[css(function)]
    Grayscale(
        #[animation(field_bound)]
        #[compute(field_bound)]
        ZeroToOne<Factor>,
    ),
    /// `hue-rotate(<angle>)`
    #[css(function)]
    HueRotate(Angle),
    /// `invert(<factor>)`
    #[css(function)]
    Invert(
        #[animation(field_bound)]
        #[compute(field_bound)]
        ZeroToOne<Factor>,
    ),
    /// `opacity(<factor>)`
    #[css(function)]
    Opacity(
        #[animation(field_bound)]
        #[compute(field_bound)]
        ZeroToOne<Factor>,
    ),
    /// `saturate(<factor>)`
    #[css(function)]
    Saturate(#[animation(field_bound)] NonNegative<Factor>),
    /// `sepia(<factor>)`
    #[css(function)]
    Sepia(
        #[animation(field_bound)]
        #[compute(field_bound)]
        ZeroToOne<Factor>,
    ),
    /// `drop-shadow(...)`
    #[css(function)]
    DropShadow(Shadow),
    /// `<url>`
    #[animation(error)]
    Url(U),
}

pub use self::GenericFilter as Filter;

/// A generic value for the `drop-shadow()` filter and the `text-shadow` property.
///
/// Contrary to the canonical order from the spec, the color is serialised
/// first, like in Gecko and Webkit.
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToAnimatedZero,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C)]
pub struct GenericSimpleShadow<Color, SizeLength, ShapeLength> {
    /// Color.
    pub color: Color,
    /// Horizontal radius.
    pub horizontal: SizeLength,
    /// Vertical radius.
    pub vertical: SizeLength,
    /// Blur radius.
    pub blur: ShapeLength,
}

pub use self::GenericSimpleShadow as SimpleShadow;

/// One shadow offset; `none` suppresses its layer.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToAnimatedZero,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C, u8)]
pub enum GenericBoxShadowOffset<L> {
    /// A suppressed shadow.
    None,
    /// Horizontal and vertical offsets.
    Offset(L, L),
}

impl<L: style_traits::ToCss + PartialEq> style_traits::ToCss for GenericBoxShadowOffset<L> {
    fn to_css<W: std::fmt::Write>(
        &self,
        dest: &mut style_traits::CssWriter<W>,
    ) -> std::fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Offset(x, y) => {
                x.to_css(dest)?;
                if x != y {
                    dest.write_char(' ')?;
                    y.to_css(dest)?;
                }
                Ok(())
            },
        }
    }
}

impl<L: Clone + crate::Zero> GenericBoxShadowOffset<L> {
    fn components(&self) -> (L, L) {
        match self {
            Self::None => (L::zero(), L::zero()),
            Self::Offset(x, y) => (x.clone(), y.clone()),
        }
    }
}

impl<L: Clone + crate::Zero + crate::values::animated::Animate> crate::values::animated::Animate
    for GenericBoxShadowOffset<L>
{
    fn animate(
        &self,
        other: &Self,
        procedure: crate::values::animated::Procedure,
    ) -> Result<Self, ()> {
        if matches!((self, other), (Self::None, Self::None)) {
            return Ok(Self::None);
        }
        let (x, y) = self.components();
        let (other_x, other_y) = other.components();
        Ok(Self::Offset(
            x.animate(&other_x, procedure)?,
            y.animate(&other_y, procedure)?,
        ))
    }
}

impl<L: Clone + crate::Zero + crate::values::distance::ComputeSquaredDistance>
    crate::values::distance::ComputeSquaredDistance for GenericBoxShadowOffset<L>
{
    fn compute_squared_distance(
        &self,
        other: &Self,
    ) -> Result<crate::values::distance::SquaredDistance, ()> {
        let (x, y) = self.components();
        let (other_x, other_y) = other.components();
        Ok(x.compute_squared_distance(&other_x)? + y.compute_squared_distance(&other_y)?)
    }
}
