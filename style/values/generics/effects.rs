/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Generic types for CSS values related to effects.
use crate::derives::*;

use crate::values::animated::{Animate, Procedure, ToAnimatedZero};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use crate::values::generics::size::Size2D;
use crate::values::generics::{NonNegative, ZeroToOne};
use crate::Zero;

/// One entry in the `box-shadow-offset` list.
#[derive(
    Clone,
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
#[repr(C, u8)]
pub enum GenericBoxShadowOffset<L> {
    /// No shadow for this entry.
    None,
    /// Horizontal and vertical offsets.
    Offset(#[css(field_bound)] Size2D<L>),
}

impl<L: Zero + Clone> GenericBoxShadowOffset<L> {
    fn offsets_or_zero(&self) -> Size2D<L> {
        match self {
            Self::None => Size2D::zero(),
            Self::Offset(value) => value.clone(),
        }
    }
}

impl<L: Animate + Zero + Clone> Animate for GenericBoxShadowOffset<L> {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        if matches!((self, other), (Self::None, Self::None)) {
            return Ok(Self::None);
        }
        self.offsets_or_zero()
            .animate(&other.offsets_or_zero(), procedure)
            .map(Self::Offset)
    }
}

impl<L: ComputeSquaredDistance + Zero + Clone> ComputeSquaredDistance
    for GenericBoxShadowOffset<L>
{
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        self.offsets_or_zero()
            .compute_squared_distance(&other.offsets_or_zero())
    }
}

impl<L: ToAnimatedZero> ToAnimatedZero for GenericBoxShadowOffset<L> {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        match self {
            Self::None => Ok(Self::None),
            Self::Offset(value) => value.to_animated_zero().map(Self::Offset),
        }
    }
}

pub use self::GenericBoxShadowOffset as BoxShadowOffset;

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
