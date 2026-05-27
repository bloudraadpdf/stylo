/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for the F5 image properties.

use crate::derives::*;
use crate::values::computed::basic_shape::BasicShape;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, Number, Resolution, ToComputedValue};
use crate::values::specified::bd_image as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::bd_image::{
    BdImageInteractivity, BdImageMagic, BdImageResampling,
};

/// Computed value of `image-resolution` / `-bd-image-resolution`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdImageResolution {
    /// Whether `from-image` was set.
    pub from_image: bool,
    /// Optional explicit resolution.
    pub resolution: Option<Resolution>,
    /// Whether `snap` was set.
    pub snap: bool,
}

impl BdImageResolution {
    /// Initial value (`1dppx`).
    #[inline]
    pub fn one_dppx() -> Self {
        Self {
            from_image: false,
            resolution: Some(Resolution::from_dppx(1.0)),
            snap: false,
        }
    }
}

impl ToCss for BdImageResolution {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        let mut wrote = false;
        if self.from_image {
            dest.write_str("from-image")?;
            wrote = true;
        }
        if let Some(ref res) = self.resolution {
            if wrote {
                dest.write_char(' ')?;
            }
            res.to_css(dest)?;
            wrote = true;
        }
        if self.snap {
            if wrote {
                dest.write_char(' ')?;
            }
            dest.write_str("snap")?;
        }
        Ok(())
    }
}

impl ToComputedValue for specified::BdImageResolution {
    type ComputedValue = BdImageResolution;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdImageResolution {
            from_image: self.from_image,
            resolution: self.resolution.as_ref().map(|r| r.to_computed_value(ctx)),
            snap: self.snap,
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self {
            from_image: computed.from_image,
            resolution: computed
                .resolution
                .as_ref()
                .map(ToComputedValue::from_computed_value),
            snap: computed.snap,
        }
    }
}

/// Computed value of `-bd-image-recompression`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdImageRecompression {
    /// `auto`
    Auto,
    /// `none`
    None,
    /// `lossless`
    Lossless,
    /// `<number>` — JPEG quality.
    Quality(Number),
}

impl BdImageRecompression {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl ToCss for BdImageRecompression {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::None => dest.write_str("none"),
            Self::Lossless => dest.write_str("lossless"),
            Self::Quality(q) => q.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdImageRecompression {
    type ComputedValue = BdImageRecompression;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdImageRecompression::Auto,
            Self::None => BdImageRecompression::None,
            Self::Lossless => BdImageRecompression::Lossless,
            Self::Quality(n) => BdImageRecompression::Quality(n.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdImageRecompression::Auto => Self::Auto,
            BdImageRecompression::None => Self::None,
            BdImageRecompression::Lossless => Self::Lossless,
            BdImageRecompression::Quality(n) => {
                Self::Quality(ToComputedValue::from_computed_value(n))
            },
        }
    }
}

/// Computed value of `-bd-image-clip-path`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdImageClipPath {
    /// `none`
    None,
    /// `<basic-shape>`
    Shape(Box<BasicShape>),
    /// `<url>`
    Url(ComputedUrl),
}

impl BdImageClipPath {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

impl ToCss for BdImageClipPath {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Shape(s) => s.to_css(dest),
            Self::Url(u) => u.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdImageClipPath {
    type ComputedValue = BdImageClipPath;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::None => BdImageClipPath::None,
            Self::Shape(s) => {
                BdImageClipPath::Shape(Box::new(s.as_ref().to_computed_value(ctx)))
            },
            Self::Url(u) => BdImageClipPath::Url(u.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdImageClipPath::None => Self::None,
            BdImageClipPath::Shape(s) => Self::Shape(Box::new(
                ToComputedValue::from_computed_value(s.as_ref()),
            )),
            BdImageClipPath::Url(u) => Self::Url(ToComputedValue::from_computed_value(u)),
        }
    }
}

/// Computed value of `-bd-image-orientation`.
///
/// `Angle(degrees)` carries the rotation as a single `f32` of degrees
/// (already normalised by `ToComputedValue`). Storing degrees rather
/// than the computed `Angle` type sidesteps the latter's lack of a
/// `ToShmem` impl — the property never needs to round-trip back into
/// a specified Angle once cascaded.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdImageOrientation {
    /// `none`
    None,
    /// `from-image`
    FromImage,
    /// `<angle>` normalised to degrees, clockwise.
    Angle(Number),
}

impl BdImageOrientation {
    /// Initial value (`from-image`).
    #[inline]
    pub fn from_image() -> Self {
        Self::FromImage
    }
}

impl ToCss for BdImageOrientation {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::FromImage => dest.write_str("from-image"),
            Self::Angle(deg) => {
                deg.to_css(dest)?;
                dest.write_str("deg")
            },
        }
    }
}

impl ToComputedValue for specified::BdImageOrientation {
    type ComputedValue = BdImageOrientation;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::None => BdImageOrientation::None,
            Self::FromImage => BdImageOrientation::FromImage,
            Self::Angle(a) => {
                BdImageOrientation::Angle(a.to_computed_value(ctx).degrees().into())
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        use crate::values::specified::Angle as SpecifiedAngle;
        match computed {
            BdImageOrientation::None => Self::None,
            BdImageOrientation::FromImage => Self::FromImage,
            BdImageOrientation::Angle(deg) => Self::Angle(SpecifiedAngle::from_degrees(
                ToComputedValue::from_computed_value(deg),
                false,
            )),
        }
    }
}
