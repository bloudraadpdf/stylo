/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-pdf-page-rotation` and `-bd-rotate-body` (F19).

use crate::derives::*;
use crate::values::computed::{Angle, Context, ToComputedValue};
use crate::values::specified::bd_page_rotation as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::bd_page_rotation::BdPdfPageRotation;

/// Computed value of `-bd-rotate-body`.
///
/// Computed `Angle` does not implement `ToShmem`, mirroring
/// `computed::OffsetRotate`; the resolved-value walk handles
/// shared-memory representations.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdRotateBody {
    /// `none` — body is not rotated.
    None,
    /// `<angle>` — rotate the body content by the given angle.
    Angle(Angle),
}

impl BdRotateBody {
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

impl ToCss for BdRotateBody {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Angle(a) => a.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdRotateBody {
    type ComputedValue = BdRotateBody;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::None => BdRotateBody::None,
            Self::Angle(a) => BdRotateBody::Angle(a.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdRotateBody::None => Self::None,
            BdRotateBody::Angle(a) => {
                Self::Angle(ToComputedValue::from_computed_value(a))
            },
        }
    }
}
