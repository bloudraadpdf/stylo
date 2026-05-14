/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-viewer-*` PDF viewer preference properties (G5).
//!
//! Native fork-extension surface for ISO 32000-2 §12.2
//! ViewerPreferences dictionary entries (PageLayout, PageMode,
//! NonFullScreenPageMode, Direction, PrintScaling, Duplex, plus
//! seven boolean flags HideToolbar, HideMenubar, HideWindowUI,
//! FitWindow, CenterWindow, DisplayDocTitle, PickTrayByPDFSize).
//! `NumCopies` and `PrintPageRange` are deferred to a follow-up.
//! All longhands apply to all elements but the moegoe renderer
//! only honours `:root` declarations — viewer prefs are
//! document-level.

use crate::derives::*;

/// Shared three-state value for boolean PDF viewer preference
/// dictionary entries (`HideToolbar`, `HideMenubar`,
/// `HideWindowUI`, `FitWindow`, `CenterWindow`, `DisplayDocTitle`,
/// `PickTrayByPDFSize`).
///
/// `auto` is the initial value — no ViewerPreferences entry is
/// emitted (PDF readers fall back to their default). `yes` / `no`
/// emit `/<Name> true` / `/<Name> false`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfTriState {
    #[default]
    Auto,
    Yes,
    No,
}

impl BdPdfTriState {
    /// Whether the value emits a dictionary entry.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// Specified value of `-bd-pdf-viewer-page-layout`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfViewerPageLayout {
    #[default]
    Auto,
    SinglePage,
    OneColumn,
    TwoColumnLeft,
    TwoColumnRight,
    TwoPageLeft,
    TwoPageRight,
}

/// Specified value of `-bd-pdf-viewer-page-mode`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfViewerPageMode {
    #[default]
    Auto,
    None,
    Outlines,
    Thumbs,
    FullScreen,
    OptionalContent,
    Attachments,
}

/// Specified value of `-bd-pdf-viewer-non-fullscreen-page-mode`.
///
/// Identical to `BdPdfViewerPageMode` minus `full-screen` — PDF
/// §12.2 forbids `FullScreen` in this slot.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfViewerNonFullscreenPageMode {
    #[default]
    Auto,
    None,
    Outlines,
    Thumbs,
    OptionalContent,
}

/// Specified value of `-bd-pdf-viewer-direction`.
///
/// CSS-aligned `ltr` / `rtl` vocabulary; compat translator maps
/// PDFreactor's `L2R` / `R2L` and PDF dictionary `L2R` / `R2L`
/// equivalently.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfViewerDirection {
    #[default]
    Auto,
    Ltr,
    Rtl,
}

/// Specified value of `-bd-pdf-viewer-print-scaling`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfViewerPrintScaling {
    #[default]
    Auto,
    None,
    AppDefault,
}

/// Specified value of `-bd-pdf-viewer-duplex`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BdPdfViewerDuplex {
    #[default]
    Auto,
    Simplex,
    FlipShortEdge,
    FlipLongEdge,
}
