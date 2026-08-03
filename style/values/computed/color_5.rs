/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for CSS Color Module Level 5 — `output-color-model`.
//!
//! Identity-computed; the specified types derive `ToComputedValue` or
//! re-use the same payload (the value carries an `AtomIdent`, which
//! computes to itself).

use crate::derives::*;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::color_5 as specified;
use to_shmem::ToShmem;

pub use specified::PredefinedOutputColourSpace;

/// Computed value of the `output-color-model` property. Identity to
/// the specified value — the payload is a discriminant plus optionally
/// an `AtomIdent`, neither of which require resolution against a
/// computed-value context.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum OutputColorModel {
    /// `auto`.
    Auto,
    /// `<predefined>`.
    Predefined(PredefinedOutputColourSpace),
    /// `<dashed-ident>` — references an `@color-profile` block.
    Custom(crate::values::AtomIdent),
}

impl OutputColorModel {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl ToComputedValue for specified::OutputColorModel {
    type ComputedValue = OutputColorModel;

    fn to_computed_value(&self, _ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::OutputColorModel::Auto => OutputColorModel::Auto,
            specified::OutputColorModel::Predefined(p) => OutputColorModel::Predefined(*p),
            specified::OutputColorModel::Custom(name) => OutputColorModel::Custom(name.clone()),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            OutputColorModel::Auto => specified::OutputColorModel::Auto,
            OutputColorModel::Predefined(p) => specified::OutputColorModel::Predefined(*p),
            OutputColorModel::Custom(name) => specified::OutputColorModel::Custom(name.clone()),
        }
    }
}

// `ToShmem` is implemented manually; `AtomIdent` participates via its
// own `ToShmem` impl.
impl ToShmem for OutputColorModel {
    fn to_shmem(&self, builder: &mut to_shmem::SharedMemoryBuilder) -> to_shmem::Result<Self> {
        Ok(std::mem::ManuallyDrop::new(match self {
            Self::Auto => Self::Auto,
            Self::Predefined(p) => Self::Predefined(*p),
            Self::Custom(name) => {
                Self::Custom(std::mem::ManuallyDrop::into_inner(name.to_shmem(builder)?))
            },
        }))
    }
}
