/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-source` / `-bd-source-page` /
//! `-bd-source-area` (F30).
//!
//! `BdSourcePage` is identity-computed (positive-integer). The
//! `BdSource` value swaps its specified `SpecifiedUrl` for the
//! computed `ComputedUrl`. `BdSourceArea` swaps each specified
//! `NonNegativeLength` for the computed equivalent.

use crate::derives::*;
use crate::values::computed::length::NonNegativeLength;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_source as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::bd_source::BdSourcePage;
// `BdSourcePage` derives `ToComputedValue` (identity-computed
// PositiveInteger wrapper) so the computed alias is the specified
// type itself.

/// Computed value of `-bd-source`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdSource {
    /// `none` (initial) — no source PDF.
    None,
    /// `url(<pdf-url>)` — fetched and embedded as a PDF page surface.
    Url(ComputedUrl),
}

impl BdSource {
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

impl ToCss for BdSource {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdSource {
    type ComputedValue = BdSource;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::None => BdSource::None,
            Self::Url(url) => BdSource::Url(url.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdSource::None => Self::None,
            BdSource::Url(url) => Self::Url(ToComputedValue::from_computed_value(url)),
        }
    }
}

/// Computed value of `-bd-source-area`.
///
/// Mirrors the specified-side variants but holds computed
/// `NonNegativeLength` so the renderer reads finalised PDF-point
/// values without re-running unit resolution.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdSourceArea {
    /// `content-box` (initial) — embed the full source page.
    ContentBox,
    /// `inset(<top> <right> <bottom> <left>)` — crop the source page.
    Inset {
        /// Top inset.
        top: NonNegativeLength,
        /// Right inset.
        right: NonNegativeLength,
        /// Bottom inset.
        bottom: NonNegativeLength,
        /// Left inset.
        left: NonNegativeLength,
    },
}

impl BdSourceArea {
    /// Initial value (`content-box`).
    #[inline]
    pub fn content_box() -> Self {
        Self::ContentBox
    }

    /// Whether the value is the initial `content-box`.
    #[inline]
    pub fn is_content_box(&self) -> bool {
        matches!(self, Self::ContentBox)
    }
}

impl ToCss for BdSourceArea {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::ContentBox => dest.write_str("content-box"),
            Self::Inset {
                top,
                right,
                bottom,
                left,
            } => {
                dest.write_str("inset(")?;
                top.to_css(dest)?;
                dest.write_char(' ')?;
                right.to_css(dest)?;
                dest.write_char(' ')?;
                bottom.to_css(dest)?;
                dest.write_char(' ')?;
                left.to_css(dest)?;
                dest.write_char(')')
            }
        }
    }
}

impl ToComputedValue for specified::BdSourceArea {
    type ComputedValue = BdSourceArea;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::ContentBox => BdSourceArea::ContentBox,
            Self::Inset {
                top,
                right,
                bottom,
                left,
            } => BdSourceArea::Inset {
                top: top.to_computed_value(ctx),
                right: right.to_computed_value(ctx),
                bottom: bottom.to_computed_value(ctx),
                left: left.to_computed_value(ctx),
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdSourceArea::ContentBox => Self::ContentBox,
            BdSourceArea::Inset {
                top,
                right,
                bottom,
                left,
            } => Self::Inset {
                top: ToComputedValue::from_computed_value(top),
                right: ToComputedValue::from_computed_value(right),
                bottom: ToComputedValue::from_computed_value(bottom),
                left: ToComputedValue::from_computed_value(left),
            },
        }
    }
}
