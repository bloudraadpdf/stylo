/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-pdf-viewer-*` properties.
//!
//! Specified-to-computed is the identity for the enum-valued
//! variants; the numeric-valued variants (`BdInitialPage`,
//! `BdPagesCounterOffset`, `BdInitialZoom::Percentage`) project
//! `Integer`/`Percentage` through `ToComputedValue` explicitly.

use crate::derives::*;
use crate::values::computed::{Context, Integer, Percentage, ToComputedValue};
use crate::values::specified::bd_pdf_viewer as specified;
use crate::OwnedSlice;

pub use crate::values::specified::bd_pdf_viewer::{
    BdFirstPageSide, BdPdfTriState, BdPdfViewerDirection, BdPdfViewerDuplex,
    BdPdfViewerNonFullscreenPageMode, BdPdfViewerPageBox, BdPdfViewerPageLayout,
    BdPdfViewerPageMode, BdPdfViewerPrintScaling,
};

/// Computed value of `-bd-initial-page`. Identity over `Integer`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdInitialPage(pub Integer);

impl BdInitialPage {
    /// Initial value (`1`).
    #[inline]
    pub fn one() -> Self {
        Self(1)
    }
}

impl ToComputedValue for specified::BdInitialPage {
    type ComputedValue = BdInitialPage;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdInitialPage(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdInitialPage(ToComputedValue::from_computed_value(&computed.0))
    }
}

/// Computed value of `-bd-pages-counter-offset`. Identity over
/// `Integer`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPagesCounterOffset(pub Integer);

impl BdPagesCounterOffset {
    /// Initial value (`0`).
    #[inline]
    pub fn zero() -> Self {
        Self(0)
    }
}

impl ToComputedValue for specified::BdPagesCounterOffset {
    type ComputedValue = BdPagesCounterOffset;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdPagesCounterOffset(self.0.to_computed_value(ctx))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdPagesCounterOffset(ToComputedValue::from_computed_value(&computed.0))
    }
}

/// Computed value of `-bd-initial-zoom`.
///
/// Identity over keyword variants; `Percentage` is projected via
/// `ToComputedValue`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
#[allow(missing_docs)]
pub enum BdInitialZoom {
    Auto,
    Percentage(Percentage),
    FitPage,
    FitPageHeight,
    FitPageWidth,
    FitContent,
    FitContentHeight,
    FitContentWidth,
}

impl BdInitialZoom {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl ToComputedValue for specified::BdInitialZoom {
    type ComputedValue = BdInitialZoom;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdInitialZoom::Auto => BdInitialZoom::Auto,
            specified::BdInitialZoom::Percentage(p) => {
                BdInitialZoom::Percentage(p.to_computed_value(ctx))
            }
            specified::BdInitialZoom::FitPage => BdInitialZoom::FitPage,
            specified::BdInitialZoom::FitPageHeight => BdInitialZoom::FitPageHeight,
            specified::BdInitialZoom::FitPageWidth => BdInitialZoom::FitPageWidth,
            specified::BdInitialZoom::FitContent => BdInitialZoom::FitContent,
            specified::BdInitialZoom::FitContentHeight => BdInitialZoom::FitContentHeight,
            specified::BdInitialZoom::FitContentWidth => BdInitialZoom::FitContentWidth,
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdInitialZoom::Auto => specified::BdInitialZoom::Auto,
            BdInitialZoom::Percentage(p) => {
                specified::BdInitialZoom::Percentage(ToComputedValue::from_computed_value(p))
            }
            BdInitialZoom::FitPage => specified::BdInitialZoom::FitPage,
            BdInitialZoom::FitPageHeight => specified::BdInitialZoom::FitPageHeight,
            BdInitialZoom::FitPageWidth => specified::BdInitialZoom::FitPageWidth,
            BdInitialZoom::FitContent => specified::BdInitialZoom::FitContent,
            BdInitialZoom::FitContentHeight => specified::BdInitialZoom::FitContentHeight,
            BdInitialZoom::FitContentWidth => specified::BdInitialZoom::FitContentWidth,
        }
    }
}

/// Computed value of `-bd-pdf-viewer-num-copies` (K13).
///
/// Identity over `Auto`; the `Count(Integer)` variant is projected
/// through `ToComputedValue` so any `calc()` expression collapses
/// to its rounded literal here.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
#[allow(missing_docs)]
pub enum BdPdfViewerNumCopies {
    Auto,
    Count(Integer),
}

impl BdPdfViewerNumCopies {
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

impl ToComputedValue for specified::BdPdfViewerNumCopies {
    type ComputedValue = BdPdfViewerNumCopies;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfViewerNumCopies::Auto => BdPdfViewerNumCopies::Auto,
            specified::BdPdfViewerNumCopies::Count(i) => {
                BdPdfViewerNumCopies::Count(i.to_computed_value(ctx))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfViewerNumCopies::Auto => specified::BdPdfViewerNumCopies::Auto,
            BdPdfViewerNumCopies::Count(i) => {
                specified::BdPdfViewerNumCopies::Count(ToComputedValue::from_computed_value(i))
            }
        }
    }
}

/// Computed value of `-bd-pdf-viewer-print-page-range` (K13).
///
/// Identity over `Auto`; the `Pages(OwnedSlice<Integer>)` variant
/// projects each entry through `ToComputedValue`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
#[allow(missing_docs)]
pub enum BdPdfViewerPrintPageRange {
    Auto,
    Pages(#[css(iterable)] OwnedSlice<Integer>),
}

impl BdPdfViewerPrintPageRange {
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

impl ToComputedValue for specified::BdPdfViewerPrintPageRange {
    type ComputedValue = BdPdfViewerPrintPageRange;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfViewerPrintPageRange::Auto => BdPdfViewerPrintPageRange::Auto,
            specified::BdPdfViewerPrintPageRange::Pages(items) => {
                BdPdfViewerPrintPageRange::Pages(OwnedSlice::from(
                    items
                        .iter()
                        .map(|i| i.to_computed_value(ctx))
                        .collect::<Vec<_>>(),
                ))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfViewerPrintPageRange::Auto => specified::BdPdfViewerPrintPageRange::Auto,
            BdPdfViewerPrintPageRange::Pages(items) => {
                specified::BdPdfViewerPrintPageRange::Pages(OwnedSlice::from(
                    items
                        .iter()
                        .map(ToComputedValue::from_computed_value)
                        .collect::<Vec<_>>(),
                ))
            }
        }
    }
}
