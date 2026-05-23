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
//!
//! G5b — PDFreactor `@-ro-preferences` parity surface for the five
//! descriptors that have no direct PDF ViewerPreferences dictionary
//! slot but instead alter sheet-side selection, the document open
//! action, or the `pages` counter total:
//!
//! - `-bd-first-page-side`: which sheet side (`:left`/`:right`)
//!   the first authored page resolves to (PDFreactor manual
//!   §"first-page-side"). Affects page-context selection AND
//!   default `/PageLayout` orientation in two-page modes.
//! - `-bd-first-page-side-view`: identical vocabulary; affects only
//!   the viewer-side `/PageLayout` slot (PDFreactor "view-only"
//!   variant), leaving the layout pass unchanged.
//! - `-bd-initial-page`: 1-indexed page the viewer should open on
//!   (ISO 32000-2 §12.6.4.3 — emits `/OpenAction [<page> /XYZ …]`).
//! - `-bd-initial-zoom`: opening zoom factor (`fit-page`,
//!   `fit-page-height`, `fit-page-width`, `fit-content`,
//!   `fit-content-height`, `fit-content-width`, `<percentage>`,
//!   `auto`); pairs with `-bd-initial-page` in `/OpenAction`.
//! - `-bd-pages-counter-offset`: integer added to the CSS Paged
//!   Media `pages` counter so chapter PDFs that are part of a
//!   larger work can show the global page count.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::{Integer, Percentage};
use cssparser::{match_ignore_ascii_case, Parser};
use style_traits::ParseError;

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

/// Specified value of `-bd-first-page-side` and
/// `-bd-first-page-side-view`.
///
/// Vocabulary matches PDFreactor's `first-page-side` /
/// `first-page-side-view` (PDFreactor manual §"first-page-side").
/// `verso`/`recto` are direction-aware aliases — the cascade reader
/// resolves them against the root's `direction` longhand and projects
/// onto a concrete `left`/`right`; the resolution lives downstream so
/// the value carried through the cascade is the author-written
/// keyword.
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
pub enum BdFirstPageSide {
    /// `auto` — defer to spec default (`recto` modulo
    /// `break-before` on the document element).
    #[default]
    Auto,
    /// `left` — first page is a left sheet.
    Left,
    /// `right` — first page is a right sheet.
    Right,
    /// `verso` — `left` under LTR, `right` under RTL.
    Verso,
    /// `recto` — `right` under LTR, `left` under RTL.
    Recto,
}

/// Specified value of `-bd-initial-page`.
///
/// `<integer>` — the 1-indexed page the viewer should open on.
/// PDFreactor manual §"initial-page". Initial value is `1` (open on
/// the first page).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdInitialPage(pub Integer);

impl BdInitialPage {
    /// Initial value (`1`).
    #[inline]
    pub fn one() -> Self {
        Self(Integer::new(1))
    }
}

impl Parse for BdInitialPage {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(Integer::parse(context, input)?))
    }
}

/// Specified value of `-bd-initial-zoom`.
///
/// `auto | <percentage> | fit-page | fit-page-height |
/// fit-page-width | fit-content | fit-content-height |
/// fit-content-width`. PDFreactor manual §"initial-zoom". Maps to
/// the destination form used in `/OpenAction`
/// (ISO 32000-2 §12.6.4.3):
/// `auto` → no `/OpenAction` slot;
/// `<percentage>` → `/XYZ null null <factor>`;
/// `fit-page` → `/Fit`;
/// `fit-page-height` → `/FitV null`;
/// `fit-page-width` → `/FitH null`;
/// `fit-content` → `/FitB`;
/// `fit-content-height` → `/FitBV null`;
/// `fit-content-width` → `/FitBH null`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdInitialZoom {
    /// `auto` (initial). Viewer default — no `/OpenAction` slot.
    Auto,
    /// `<percentage>` — explicit zoom factor.
    Percentage(Percentage),
    /// `fit-page` → `/Fit` destination.
    FitPage,
    /// `fit-page-height` → `/FitV` destination.
    FitPageHeight,
    /// `fit-page-width` → `/FitH` destination.
    FitPageWidth,
    /// `fit-content` → `/FitB` destination.
    FitContent,
    /// `fit-content-height` → `/FitBV` destination.
    FitContentHeight,
    /// `fit-content-width` → `/FitBH` destination.
    FitContentWidth,
}

impl BdInitialZoom {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl Default for BdInitialZoom {
    fn default() -> Self {
        Self::Auto
    }
}

impl Parse for BdInitialZoom {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(ident) = input.try_parse(|i| i.expect_ident_cloned()) {
            return match_ignore_ascii_case! { &ident,
                "auto" => Ok(Self::Auto),
                "fit-page" => Ok(Self::FitPage),
                "fit-page-height" => Ok(Self::FitPageHeight),
                "fit-page-width" => Ok(Self::FitPageWidth),
                "fit-content" => Ok(Self::FitContent),
                "fit-content-height" => Ok(Self::FitContentHeight),
                "fit-content-width" => Ok(Self::FitContentWidth),
                _ => Err(input.new_custom_error(
                    style_traits::StyleParseErrorKind::UnspecifiedError,
                )),
            };
        }
        Ok(Self::Percentage(Percentage::parse(context, input)?))
    }
}

/// Specified value of `-bd-pages-counter-offset`.
///
/// `<integer>` — added to the CSS Paged Media `pages` counter so a
/// chapter PDF that is part of a larger work can show the global
/// page count (`Page 1 of 250` rather than `Page 1 of 50`). Negative
/// values are accepted; the paginator clamps at the per-page
/// resolved value, never the per-page emission. PDFreactor manual
/// §"pages-counter-offset". Initial value is `0`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPagesCounterOffset(pub Integer);

impl BdPagesCounterOffset {
    /// Initial value (`0`).
    #[inline]
    pub fn zero() -> Self {
        Self(Integer::new(0))
    }
}

impl Parse for BdPagesCounterOffset {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(Self(Integer::parse(context, input)?))
    }
}
