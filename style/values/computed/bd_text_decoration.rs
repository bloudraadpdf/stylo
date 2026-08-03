/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-text-{overline,underline,linethrough}-*`.

use crate::derives::*;
use crate::values::computed::color::Color;
use crate::values::computed::length::{Length, LengthPercentage};
use crate::values::computed::{Context, ToComputedValue};
use crate::values::generics::text::GenericTextDecorationLength;
use crate::values::specified::bd_text_decoration as specified;
use crate::OwnedSlice;
use to_shmem::ToShmem;

pub use specified::{
    BdTextDecorationLineStyle, BdTextDecorationSkipCategory, BdTextEmphasisSkip,
    BdTextUnderlinePosition,
};

/// Computed value of `-bd-text-{position}-color`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTextDecorationLineColour {
    /// `auto` — defer to `text-decoration-color`.
    Auto,
    /// Explicit colour override.
    Colour(Color),
}

impl BdTextDecorationLineColour {
    /// `auto` value.
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

impl ToComputedValue for specified::BdTextDecorationLineColour {
    type ComputedValue = BdTextDecorationLineColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdTextDecorationLineColour::Auto => BdTextDecorationLineColour::Auto,
            specified::BdTextDecorationLineColour::Colour(c) => {
                BdTextDecorationLineColour::Colour(c.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdTextDecorationLineColour::Auto => specified::BdTextDecorationLineColour::Auto,
            BdTextDecorationLineColour::Colour(c) => specified::BdTextDecorationLineColour::Colour(
                ToComputedValue::from_computed_value(c),
            ),
        }
    }
}

/// Computed value of `-bd-text-{position}-thickness`.
///
/// See specified counterpart for the rationale behind the newtype wrapper
/// and the manual `ToComputedValue` / `ToShmem` impls below.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdTextDecorationLineThickness(pub GenericTextDecorationLength<LengthPercentage>);

impl BdTextDecorationLineThickness {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        Self(GenericTextDecorationLength::Auto)
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self.0, GenericTextDecorationLength::Auto)
    }
}

impl ToComputedValue for specified::BdTextDecorationLineThickness {
    type ComputedValue = BdTextDecorationLineThickness;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdTextDecorationLineThickness(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdTextDecorationLineThickness(ToComputedValue::from_computed_value(&computed.0))
    }
}

// `ToShmem` is implemented manually. The auto-derive would require
// `LengthPercentage` (the computed-side type) to implement `ToShmem`,
// which it does not. `Copy::clone()` produces the same value into the
// caller's allocator, which is the only correctness requirement for
// transparent newtype wrappers around `Copy`-equivalent payloads.
impl ToShmem for BdTextDecorationLineThickness {
    fn to_shmem(&self, _: &mut to_shmem::SharedMemoryBuilder) -> to_shmem::Result<Self> {
        Ok(std::mem::ManuallyDrop::new(self.clone()))
    }
}

/// Computed value of `-bd-text-underline-offset`.
///
/// Wraps the computed `LengthPercentageOrAuto` so the cascade reader
/// can distinguish the per-position override from the global
/// `text-underline-offset` computed value.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdTextUnderlineOffset(pub crate::values::computed::LengthPercentageOrAuto);

impl BdTextUnderlineOffset {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        Self(crate::values::computed::LengthPercentageOrAuto::Auto)
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(
            self.0,
            crate::values::computed::LengthPercentageOrAuto::Auto
        )
    }
}

impl ToComputedValue for specified::BdTextUnderlineOffset {
    type ComputedValue = BdTextUnderlineOffset;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdTextUnderlineOffset(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdTextUnderlineOffset(ToComputedValue::from_computed_value(&computed.0))
    }
}

// `ToShmem` is implemented manually for the same reason as
// `BdTextDecorationLineThickness` above.
impl ToShmem for BdTextUnderlineOffset {
    fn to_shmem(&self, _: &mut to_shmem::SharedMemoryBuilder) -> to_shmem::Result<Self> {
        Ok(std::mem::ManuallyDrop::new(self.clone()))
    }
}

/// Computed value of `-bd-text-decoration-trim` (moegoe fork).
///
/// The specified `Length` payload resolves to a computed-side
/// `Length`. `ToShmem` is implemented manually because the computed
/// `Length` does not implement `ToShmem` (cf. the existing
/// `BdTextDecorationLineThickness` pattern).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdTextDecorationTrim {
    /// `auto` — defer to `text-decoration-trim` (initial).
    Auto,
    /// `none` — explicitly disable trimming on this element.
    None,
    /// `<length>` — apply symmetric trim of this length.
    Length(Length),
}

impl BdTextDecorationTrim {
    /// `auto` value.
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

impl ToComputedValue for specified::BdTextDecorationTrim {
    type ComputedValue = BdTextDecorationTrim;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdTextDecorationTrim::Auto => BdTextDecorationTrim::Auto,
            specified::BdTextDecorationTrim::None => BdTextDecorationTrim::None,
            specified::BdTextDecorationTrim::Length(len) => {
                BdTextDecorationTrim::Length(len.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdTextDecorationTrim::Auto => specified::BdTextDecorationTrim::Auto,
            BdTextDecorationTrim::None => specified::BdTextDecorationTrim::None,
            BdTextDecorationTrim::Length(len) => {
                specified::BdTextDecorationTrim::Length(ToComputedValue::from_computed_value(len))
            },
        }
    }
}

// `Length` (computed) does not implement `ToShmem`, so the auto-derive
// cannot fire. The clone is safe — `Length` is a `Copy`-equivalent
// payload.
impl ToShmem for BdTextDecorationTrim {
    fn to_shmem(&self, _: &mut to_shmem::SharedMemoryBuilder) -> to_shmem::Result<Self> {
        Ok(std::mem::ManuallyDrop::new(self.clone()))
    }
}

/// Computed value of `-bd-text-decoration-skip` (moegoe fork).
///
/// Identity-computed mirror of the specified-side enum — the contained
/// `OwnedSlice<BdTextDecorationSkipCategory>` already lives on the
/// shared allocator and the per-category enum is `Copy`-equivalent, so
/// `ToComputedValue` is a structural clone. `ToCss` is hand-rolled
/// because the derive cannot synthesise a serialiser for `OwnedSlice`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTextDecorationSkip {
    /// `none` — defer to the standard `text-decoration-skip-*` cascade.
    None,
    /// Comma-separated category list authored by `-bd-`.
    Categories(OwnedSlice<BdTextDecorationSkipCategory>),
}

impl BdTextDecorationSkip {
    /// `none` value (initial).
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

impl style_traits::ToCss for BdTextDecorationSkip {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> std::fmt::Result
    where
        W: std::fmt::Write,
    {
        use std::fmt::Write;
        match self {
            Self::None => dest.write_str("none"),
            Self::Categories(cats) => {
                let mut first = true;
                for cat in cats.iter() {
                    if !first {
                        dest.write_str(", ")?;
                    }
                    cat.to_css(dest)?;
                    first = false;
                }
                Ok(())
            },
        }
    }
}

impl ToComputedValue for specified::BdTextDecorationSkip {
    type ComputedValue = BdTextDecorationSkip;

    fn to_computed_value(&self, _: &Context) -> Self::ComputedValue {
        match self {
            specified::BdTextDecorationSkip::None => BdTextDecorationSkip::None,
            specified::BdTextDecorationSkip::Categories(cats) => {
                BdTextDecorationSkip::Categories(cats.clone())
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdTextDecorationSkip::None => specified::BdTextDecorationSkip::None,
            BdTextDecorationSkip::Categories(cats) => {
                specified::BdTextDecorationSkip::Categories(cats.clone())
            },
        }
    }
}
