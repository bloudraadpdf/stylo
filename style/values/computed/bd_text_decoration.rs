/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-text-{overline,underline,linethrough}-*`.

use crate::derives::*;
use crate::values::computed::color::Color;
use crate::values::computed::length::LengthPercentage;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::generics::text::GenericTextDecorationLength;
use crate::values::specified::bd_text_decoration as specified;
use to_shmem::ToShmem;

pub use specified::BdTextDecorationLineStyle;

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
            }
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
        specified::BdTextDecorationLineThickness(
            ToComputedValue::from_computed_value(&computed.0),
        )
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
