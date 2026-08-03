/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for Bloudraad page-mark and print-shop properties.
//!
//! F20 — `BdColorBarPosition` + `BdPrintMarkSet` for the
//! `-bd-page-colorbar-*` / `-bd-page-print-mark-set` print-shop
//! tooling. Keyword-only specified types reuse the specified
//! module's enums. The URL-bearing `BdColorBarPosition` lifts
//! through a manual `ToComputedValue` so the inner URL is converted
//! to the computed `CssUrl`. `BdColorBarOffset` is the computed
//! `Length`.

use crate::derives::*;
use crate::values::computed::length::{LengthPercentage, NonNegativeLength};
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Color, Context, ToComputedValue};
use crate::values::generics::url::GenericUrlOrNone;
use crate::values::specified::bd_page_marks as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use specified::{
    BdColourBarPositionSide, BdPageMarkEnabled, BdPrintMarkSet, BdRegistrationPosition,
    BdSidenoteGlyph,
};

/// Computed value of a `-bd-page-*-mark-length` / `-offset` property.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPageMarkLength(pub NonNegativeLength);

impl BdPageMarkLength {
    /// Build a value from a points constant.
    #[inline]
    pub fn from_pt(pt: f32) -> Self {
        // CSS px = 1/96in, pt = 1/72in. Convert at the boundary.
        Self(NonNegativeLength::new(pt * 96.0 / 72.0))
    }
}

impl ToComputedValue for specified::BdPageMarkLength {
    type ComputedValue = BdPageMarkLength;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdPageMarkLength(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self(ToComputedValue::from_computed_value(&computed.0))
    }
}

/// Computed `auto | <non-negative-length>` mark extent.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPageMarkLengthOrAuto(pub crate::values::computed::length::NonNegativeLengthOrAuto);

impl BdPageMarkLengthOrAuto {
    /// Initial automatic extent.
    #[inline]
    pub fn auto() -> Self {
        Self(crate::values::generics::length::LengthPercentageOrAuto::Auto)
    }
}

impl ToComputedValue for specified::BdPageMarkLengthOrAuto {
    type ComputedValue = BdPageMarkLengthOrAuto;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdPageMarkLengthOrAuto(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self(ToComputedValue::from_computed_value(&computed.0))
    }
}

/// Computed non-negative length-percentage printer-mark offset.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdPageMarkOffset(pub crate::values::computed::length::NonNegativeLengthPercentage);

impl BdPageMarkOffset {
    /// Initial 100% offset.
    #[inline]
    pub fn hundred_percent() -> Self {
        Self(crate::values::generics::NonNegative(
            LengthPercentage::new_percent(crate::values::computed::Percentage::hundred()),
        ))
    }
}

impl ToComputedValue for specified::BdPageMarkOffset {
    type ComputedValue = BdPageMarkOffset;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdPageMarkOffset(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self(ToComputedValue::from_computed_value(&computed.0))
    }
}

/// Computed `none | <non-negative-length>` printer-mark width.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPageMarkWidth {
    /// Do not paint mark strokes.
    None,
    /// Absolute mark stroke width.
    Length(NonNegativeLength),
}

impl BdPageMarkWidth {
    /// Build an absolute value from points.
    #[inline]
    pub fn from_pt(pt: f32) -> Self {
        Self::Length(NonNegativeLength::new(pt * 96.0 / 72.0))
    }
}

impl ToComputedValue for specified::BdPageMarkWidth {
    type ComputedValue = BdPageMarkWidth;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPageMarkWidth::None => BdPageMarkWidth::None,
            specified::BdPageMarkWidth::Length(length) => {
                BdPageMarkWidth::Length(length.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPageMarkWidth::None => Self::None,
            BdPageMarkWidth::Length(length) => {
                Self::Length(ToComputedValue::from_computed_value(length))
            },
        }
    }
}

/// Computed value of `-bd-page-marks-colour`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPageMarksColour {
    /// `auto`.
    Auto,
    /// Concrete computed colour.
    Colour(Color),
}

impl BdPageMarksColour {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToCss for BdPageMarksColour {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Colour(c) => c.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdPageMarksColour {
    type ComputedValue = BdPageMarksColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdPageMarksColour::Auto,
            Self::Colour(c) => BdPageMarksColour::Colour(c.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPageMarksColour::Auto => Self::Auto,
            BdPageMarksColour::Colour(c) => Self::Colour(ToComputedValue::from_computed_value(c)),
        }
    }
}

// ===== F20 — colour bar / print-mark-set =================================

/// Computed value of `-bd-page-colorbar-*`.
///
/// Note: `ComputedUrl` is not `ToShmem` (it carries an `Arc`); the
/// derive is therefore omitted here.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdColorBarPosition {
    /// `none`.
    None,
    /// `auto`.
    Auto,
    /// Bloudraad `gradient-tint` process-control wedge.
    GradientTint,
    /// Bloudraad `progressive-color` process-control wedge.
    ProgressiveColor,
    /// One or more authored computed colour swatches.
    Colours(crate::OwnedSlice<Color>),
    /// `<url>`.
    Url(GenericUrlOrNone<ComputedUrl>),
}

impl BdColorBarPosition {
    /// Initial value.
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

impl ToCss for BdColorBarPosition {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Auto => dest.write_str("auto"),
            Self::GradientTint => dest.write_str("gradient-tint"),
            Self::ProgressiveColor => dest.write_str("progressive-color"),
            Self::Colours(list) => {
                let mut first = true;
                for colour in list.iter() {
                    if !first {
                        dest.write_str(" ")?;
                    }
                    first = false;
                    colour.to_css(dest)?;
                }
                Ok(())
            },
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdColorBarPosition {
    type ComputedValue = BdColorBarPosition;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdColorBarPosition::None => BdColorBarPosition::None,
            specified::BdColorBarPosition::Auto => BdColorBarPosition::Auto,
            specified::BdColorBarPosition::GradientTint => BdColorBarPosition::GradientTint,
            specified::BdColorBarPosition::ProgressiveColor => BdColorBarPosition::ProgressiveColor,
            specified::BdColorBarPosition::Colours(list) => {
                let colours = list
                    .iter()
                    .map(|colour| colour.to_computed_value(ctx))
                    .collect::<Vec<_>>();
                BdColorBarPosition::Colours(crate::OwnedSlice::from(colours))
            },
            specified::BdColorBarPosition::Url(u) => {
                BdColorBarPosition::Url(u.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(c: &Self::ComputedValue) -> Self {
        match c {
            BdColorBarPosition::None => specified::BdColorBarPosition::None,
            BdColorBarPosition::Auto => specified::BdColorBarPosition::Auto,
            BdColorBarPosition::GradientTint => specified::BdColorBarPosition::GradientTint,
            BdColorBarPosition::ProgressiveColor => specified::BdColorBarPosition::ProgressiveColor,
            BdColorBarPosition::Colours(list) => {
                let colours = list
                    .iter()
                    .map(ToComputedValue::from_computed_value)
                    .collect::<Vec<_>>();
                specified::BdColorBarPosition::Colours(crate::OwnedSlice::from(colours))
            },
            BdColorBarPosition::Url(u) => {
                specified::BdColorBarPosition::Url(ToComputedValue::from_computed_value(u))
            },
        }
    }
}

// `-bd-page-colorbar-offset` resolves to the predefined
// `computed::Length` type directly.

// ===== Tier 4 §A.4.7 — marker variants ==================================

/// Computed value of `-bd-pdf-mark-registration-color`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdRegistrationColour {
    /// `auto`.
    Auto,
    /// Concrete computed colour.
    Colour(Color),
}

impl BdRegistrationColour {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToCss for BdRegistrationColour {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Colour(c) => c.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdRegistrationColour {
    type ComputedValue = BdRegistrationColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdRegistrationColour::Auto,
            Self::Colour(c) => BdRegistrationColour::Colour(c.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdRegistrationColour::Auto => Self::Auto,
            BdRegistrationColour::Colour(c) => {
                Self::Colour(ToComputedValue::from_computed_value(c))
            },
        }
    }
}

/// Computed value of `-bd-pdf-mark-crop-color`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdCropColour {
    /// `auto`.
    Auto,
    /// Concrete computed colour.
    Colour(Color),
}

impl BdCropColour {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToCss for BdCropColour {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Colour(c) => c.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdCropColour {
    type ComputedValue = BdCropColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdCropColour::Auto,
            Self::Colour(c) => BdCropColour::Colour(c.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdCropColour::Auto => Self::Auto,
            BdCropColour::Colour(c) => Self::Colour(ToComputedValue::from_computed_value(c)),
        }
    }
}

/// Computed value of `-bd-pdf-mark-bleed-color`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdBleedColour {
    /// `auto`.
    Auto,
    /// Concrete computed colour.
    Colour(Color),
}

impl BdBleedColour {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToCss for BdBleedColour {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Colour(c) => c.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdBleedColour {
    type ComputedValue = BdBleedColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdBleedColour::Auto,
            Self::Colour(c) => BdBleedColour::Colour(c.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdBleedColour::Auto => Self::Auto,
            BdBleedColour::Colour(c) => Self::Colour(ToComputedValue::from_computed_value(c)),
        }
    }
}

/// Computed value of `-bd-pdf-mark-colour-bar-swatches`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdColourBarSwatches {
    /// `none`.
    None,
    /// One or more `<color>` swatches.
    Colours(crate::OwnedSlice<Color>),
}

impl Default for BdColourBarSwatches {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdColourBarSwatches {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for BdColourBarSwatches {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Colours(list) => {
                let mut first = true;
                for c in list.iter() {
                    if !first {
                        dest.write_str(" ")?;
                    }
                    first = false;
                    c.to_css(dest)?;
                }
                Ok(())
            },
        }
    }
}

impl ToComputedValue for specified::BdColourBarSwatches {
    type ComputedValue = BdColourBarSwatches;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::None => BdColourBarSwatches::None,
            Self::Colours(list) => {
                let mut out: Vec<Color> = Vec::with_capacity(list.len());
                for c in list.iter() {
                    out.push(c.to_computed_value(ctx));
                }
                BdColourBarSwatches::Colours(crate::OwnedSlice::from(out))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdColourBarSwatches::None => Self::None,
            BdColourBarSwatches::Colours(list) => {
                let mut out: Vec<crate::values::specified::Color> = Vec::with_capacity(list.len());
                for c in list.iter() {
                    out.push(ToComputedValue::from_computed_value(c));
                }
                Self::Colours(crate::OwnedSlice::from(out))
            },
        }
    }
}

/// Computed value of `-bd-pdf-mark-sidenote-offset`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdSidenoteMarkerOffset(pub LengthPercentage);

impl BdSidenoteMarkerOffset {
    /// Initial value (`0`).
    #[inline]
    pub fn zero() -> Self {
        Self(LengthPercentage::zero_percent())
    }
}

impl ToComputedValue for specified::BdSidenoteMarkerOffset {
    type ComputedValue = BdSidenoteMarkerOffset;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdSidenoteMarkerOffset(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self(ToComputedValue::from_computed_value(&computed.0))
    }
}
