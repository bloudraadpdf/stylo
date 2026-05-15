/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for the F3 PDF page-box descriptors.

use crate::derives::*;
use crate::values::computed::page::PageSize;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_page_boxes as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::bd_page_boxes::BdPdfPageClip;

/// Computed value of `-bd-pdf-{media,crop,art}-size`.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfPageBoxSize {
    /// `auto`
    Auto,
    /// Concrete page-box dimensions.
    Page(PageSize),
}

impl BdPdfPageBoxSize {
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

impl ToCss for BdPdfPageBoxSize {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Page(p) => p.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdPdfPageBoxSize {
    type ComputedValue = BdPdfPageBoxSize;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdPdfPageBoxSize::Auto,
            Self::Page(p) => BdPdfPageBoxSize::Page(p.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfPageBoxSize::Auto => Self::Auto,
            BdPdfPageBoxSize::Page(p) => {
                Self::Page(ToComputedValue::from_computed_value(p))
            },
        }
    }
}

macro_rules! computed_size_wrapper {
    ($name:ident, $specified:path) => {
        /// Computed value of the corresponding page-box descriptor.
        #[derive(
            Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped,
        )]
        #[repr(C)]
        pub struct $name(pub BdPdfPageBoxSize);

        impl $name {
            /// Initial value (`auto`).
            #[inline]
            pub fn auto() -> Self {
                Self(BdPdfPageBoxSize::auto())
            }
        }

        impl ToComputedValue for $specified {
            type ComputedValue = $name;

            fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
                $name(self.0.to_computed_value(ctx))
            }

            fn from_computed_value(computed: &Self::ComputedValue) -> Self {
                Self(ToComputedValue::from_computed_value(&computed.0))
            }
        }
    };
}

computed_size_wrapper!(BdPdfMediaSize, specified::BdPdfMediaSize);
computed_size_wrapper!(BdPdfCropSize, specified::BdPdfCropSize);
computed_size_wrapper!(BdPdfArtSize, specified::BdPdfArtSize);
