/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-viewer-*` PDF viewer preference properties (G5).
//!
//! Native fork-extension surface for ISO 32000-2 §12.2
//! ViewerPreferences dictionary entries (PageLayout, PageMode,
//! NonFullScreenPageMode, Direction, PrintScaling, Duplex, plus
//! seven boolean flags HideToolbar, HideMenubar, HideWindowUI,
//! FitWindow, CenterWindow, DisplayDocTitle, PickTrayByPDFSize,
//! and the K13 cluster NumCopies, PrintPageRange, ViewArea, ViewClip,
//! PrintArea, PrintClip).
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
use crate::OwnedSlice;
use cssparser::{match_ignore_ascii_case, Parser};
use style_traits::{ParseError, StyleParseErrorKind};

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

/// Specified value of `-bd-pdf-viewer-num-copies` (K13).
///
/// `auto | <integer [1, 99]>`. `auto` (initial) emits no
/// `/NumCopies` slot; an integer in `[1, 99]` projects directly
/// onto `/ViewerPreferences /NumCopies` per ISO 32000-2 §12.2
/// Table 153. Out-of-range integers (`< 1` or `> 99`) are
/// rejected at parse time so authors cannot accidentally smuggle
/// an invalid PDF dictionary entry past the cascade.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfViewerNumCopies {
    /// `auto` (initial) — no `/NumCopies` slot is emitted.
    Auto,
    /// `<integer [1, 99]>` — explicit copy count.
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

impl Default for BdPdfViewerNumCopies {
    fn default() -> Self {
        Self::Auto
    }
}

impl Parse for BdPdfViewerNumCopies {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let value = Integer::parse(context, input)?;
        if value.value() < 1 || value.value() > 99 {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Count(value))
    }
}

/// Specified value of `-bd-pdf-viewer-print-page-range` (K13).
///
/// `auto | <integer>+` — a flat space-separated list of 1-indexed
/// page numbers (mirroring the `bd-hyphenate-lines` shape). The
/// list is interpreted pairwise: `n1 n2 n3 n4 …` projects onto
/// the PDF `/ViewerPreferences /PrintPageRange` array
/// `[first1 last1 first2 last2 …]` (ISO 32000-2 §12.2 Table 153,
/// §14.11.2). The cascade reader emits the slot only when the
/// list has an even cardinality with `first[i] <= last[i]`;
/// downstream odd-length lists are rejected at the IR boundary
/// rather than in CSS.
///
/// `auto` (initial) emits no `/PrintPageRange` slot. Each integer
/// must be `>= 1`; non-positive values are rejected at parse time
/// so authors cannot smuggle a malformed range past the cascade.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfViewerPrintPageRange {
    /// `auto` (initial) — no `/PrintPageRange` slot is emitted.
    Auto,
    /// `<integer>+` — flat space-separated page-number list.
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

impl Default for BdPdfViewerPrintPageRange {
    fn default() -> Self {
        Self::Auto
    }
}

impl Parse for BdPdfViewerPrintPageRange {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let mut pages: Vec<Integer> = Vec::new();
        loop {
            let value = match input.try_parse(|i| Integer::parse(context, i)) {
                Ok(v) => v,
                Err(_) => break,
            };
            if value.value() < 1 {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            pages.push(value);
        }
        if pages.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Pages(OwnedSlice::from(pages)))
    }
}

/// Shared specified value for the four `-bd-pdf-viewer-{view,print}-{area,clip}`
/// longhands (K13). All four select from the same PDF page-box
/// vocabulary (ISO 32000-2 §14.11.2). Initial value is `auto` so the
/// renderer can defer to its own default and emit no
/// `/ViewerPreferences /ViewArea | ViewClip | PrintArea | PrintClip`
/// slot.
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
pub enum BdPdfViewerPageBox {
    #[default]
    Auto,
    MediaBox,
    CropBox,
    BleedBox,
    TrimBox,
    ArtBox,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::{ParsingMode, ToCss};

    fn make_context(url_data: &UrlExtraData) -> ParserContext {
        ParserContext::new(
            Origin::Author,
            url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        )
    }

    fn parse_num_copies(css: &str) -> Result<BdPdfViewerNumCopies, ()> {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = make_context(&url_data);
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfViewerNumCopies::parse(&context, input))
            .map_err(|_| ())
    }

    fn parse_print_page_range(css: &str) -> Result<BdPdfViewerPrintPageRange, ()> {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = make_context(&url_data);
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfViewerPrintPageRange::parse(&context, input))
            .map_err(|_| ())
    }

    fn parse_page_box(css: &str) -> Result<BdPdfViewerPageBox, ()> {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let _context = make_context(&url_data);
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfViewerPageBox::parse(input))
            .map_err(|_| ())
    }

    #[test]
    fn bd_pdf_viewer_num_copies_initial_is_auto() {
        assert!(BdPdfViewerNumCopies::default().is_auto());
        assert_eq!(BdPdfViewerNumCopies::auto(), BdPdfViewerNumCopies::Auto);
    }

    #[test]
    fn bd_pdf_viewer_num_copies_auto_parses() {
        let value = parse_num_copies("auto").expect("auto should parse");
        assert_eq!(value, BdPdfViewerNumCopies::Auto);
        assert_eq!(value.to_css_string(), "auto");
    }

    #[test]
    fn bd_pdf_viewer_num_copies_one_through_ninety_nine_parse() {
        for n in [1, 2, 5, 25, 50, 99] {
            let value = parse_num_copies(&n.to_string()).expect("integer in [1, 99] should parse");
            assert!(matches!(value, BdPdfViewerNumCopies::Count(_)));
            assert_eq!(value.to_css_string(), n.to_string());
        }
    }

    #[test]
    fn bd_pdf_viewer_num_copies_rejects_zero_and_negative() {
        assert!(parse_num_copies("0").is_err());
        assert!(parse_num_copies("-1").is_err());
    }

    #[test]
    fn bd_pdf_viewer_num_copies_rejects_above_ninety_nine() {
        assert!(parse_num_copies("100").is_err());
        assert!(parse_num_copies("9999").is_err());
    }

    #[test]
    fn bd_pdf_viewer_print_page_range_initial_is_auto() {
        assert!(BdPdfViewerPrintPageRange::default().is_auto());
        assert_eq!(
            BdPdfViewerPrintPageRange::auto(),
            BdPdfViewerPrintPageRange::Auto,
        );
    }

    #[test]
    fn bd_pdf_viewer_print_page_range_auto_parses() {
        let value = parse_print_page_range("auto").expect("auto should parse");
        assert!(value.is_auto());
        assert_eq!(value.to_css_string(), "auto");
    }

    #[test]
    fn bd_pdf_viewer_print_page_range_single_integer_parses() {
        let value = parse_print_page_range("5").expect("single integer should parse");
        match &value {
            BdPdfViewerPrintPageRange::Pages(items) => assert_eq!(items.len(), 1),
            _ => panic!("expected Pages variant"),
        }
        assert_eq!(value.to_css_string(), "5");
    }

    #[test]
    fn bd_pdf_viewer_print_page_range_multi_integer_parses() {
        let value = parse_print_page_range("1 5 7 9")
            .expect("space-separated integer list should parse");
        match &value {
            BdPdfViewerPrintPageRange::Pages(items) => assert_eq!(items.len(), 4),
            _ => panic!("expected Pages variant"),
        }
        assert_eq!(value.to_css_string(), "1 5 7 9");
    }

    #[test]
    fn bd_pdf_viewer_print_page_range_rejects_zero() {
        assert!(parse_print_page_range("0").is_err());
        assert!(parse_print_page_range("1 0 3").is_err());
    }

    #[test]
    fn bd_pdf_viewer_print_page_range_rejects_negative() {
        assert!(parse_print_page_range("-1").is_err());
    }

    #[test]
    fn bd_pdf_viewer_print_page_range_rejects_empty() {
        // Bare empty value (no `auto`, no integer) must fail.
        assert!(parse_print_page_range("").is_err());
    }

    #[test]
    fn bd_pdf_viewer_page_box_initial_is_auto() {
        assert_eq!(BdPdfViewerPageBox::default(), BdPdfViewerPageBox::Auto);
    }

    #[test]
    fn bd_pdf_viewer_page_box_all_variants_round_trip() {
        for (css, expected) in [
            ("auto", BdPdfViewerPageBox::Auto),
            ("media-box", BdPdfViewerPageBox::MediaBox),
            ("crop-box", BdPdfViewerPageBox::CropBox),
            ("bleed-box", BdPdfViewerPageBox::BleedBox),
            ("trim-box", BdPdfViewerPageBox::TrimBox),
            ("art-box", BdPdfViewerPageBox::ArtBox),
        ] {
            let value = parse_page_box(css).expect("page-box keyword should parse");
            assert_eq!(value, expected);
            assert_eq!(value.to_css_string(), css);
        }
    }

    #[test]
    fn bd_pdf_viewer_page_box_rejects_unknown_keyword() {
        assert!(parse_page_box("crop").is_err());
        assert!(parse_page_box("xyz-box").is_err());
    }
}
