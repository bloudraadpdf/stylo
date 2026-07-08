/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-tag*` tagged-PDF role override properties (G8).
//!
//! Native fork-extension surface for tagged-PDF authoring control:
//! override the structure-tree role assigned to an element, mark
//! a subtree as Artifact (excluded from accessibility), suppress
//! tagging for a wrapper, or supply per-tag attributes
//! (alt/actual-text/title/lang/expanded). Per-element; not
//! inherited. Initial `auto` everywhere.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::CustomIdent;
use crate::OwnedStr;
use cssparser::{match_ignore_ascii_case, Parser};

/// PDF/UA structure-tree standard roles. One Rust variant per
/// krilla `TagKind` variant — keep aligned with
/// `~/github/krilla/crates/krilla/src/interchange/tagging/generated.rs`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
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
pub enum BdPdfStandardRole {
    Part,
    Article,
    Section,
    Div,
    BlockQuote,
    Caption,
    Toc,
    Toci,
    Index,
    P,
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
    L,
    Li,
    Lbl,
    LBody,
    Table,
    Tr,
    Th,
    Td,
    #[css(keyword = "thead")]
    THead,
    #[css(keyword = "tbody")]
    TBody,
    #[css(keyword = "tfoot")]
    TFoot,
    Span,
    InlineQuote,
    Note,
    Reference,
    BibEntry,
    Code,
    Link,
    Annot,
    Figure,
    Formula,
    Form,
    NonStruct,
    Datetime,
    Terms,
    Title,
    Strong,
    Em,
    /// PDF 2.0 `Aside` structure type (ISO 32000-2 §14.8.4.3).
    /// Content distinct from the surrounding flow — callouts,
    /// sidebars, commentary, background information. Honoured under
    /// PDF/UA-2 / WTPDF; downgrades to `Div` on PDF 1.7 output.
    Aside,
    /// PDF 2.0 `Sub` structure type (ISO 32000-2 §14.8.4.6).
    /// Inline subdivision inside a block-level element — typically
    /// the subscript / superscript context exposed by HTML's
    /// `<sub>` / `<sup>` elements. Honoured under PDF/UA-2 / WTPDF;
    /// downgrades to a `/RoleMap` custom name on PDF 1.7 output.
    Sub,
}

/// Artifact subtype keyword for `-bd-pdf-tag: artifact(<kind>)`.
///
/// ISO 32000-2 §14.8.2.2 classifies artifacts into four broad
/// categories. The bare `artifact` keyword resolves to `Layout`
/// (the most common case for purely decorative typography or
/// design elements).
///
/// Maps onto krilla's `ArtifactType` at the PDF emission boundary:
/// `Layout` -> `ArtifactType::Layout`, `Page` -> `ArtifactType::Page`,
/// `Background` -> `ArtifactType::Background`, `Pagination` ->
/// `ArtifactType::PaginationOther` (the generic pagination kind;
/// per-margin-box `Header` / `Footer` / `PageNumber` subtypes are
/// chosen by the renderer, not the author).
#[repr(u8)]
#[derive(
    Clone,
    Copy,
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
pub enum BdPdfArtifactKind {
    /// `layout` — cosmetic typographical or design element. Default
    /// for the bare `artifact` keyword.
    #[default]
    Layout,
    /// `page` — page artifact (cut marks, colour bars, &c.).
    Page,
    /// `background` — background of a page or graphical element.
    Background,
    /// `pagination` — pagination-related (headers, footers, page
    /// numbers). Per-subtype attribution (`Header` / `Footer` /
    /// `PageNumber`) is chosen by the renderer based on the
    /// margin-box position; CSS authors only select the generic
    /// kind here.
    Pagination,
}

/// Specified value of `-bd-pdf-tag`.
///
/// `auto` (initial) — derive from HTML semantics. `none` — this
/// element produces no structure entry but descendants attach to
/// the parent group (transparent wrapper). `artifact` (or
/// `artifact(layout|page|background|pagination)`) — exclude this
/// element and its subtree from the structure tree and mark the
/// content as a PDF `/Artifact` of the named kind (`layout` by
/// default). `<standard-role>` — explicit krilla `TagKind`.
/// `<custom-ident>` — author-named role; falls back to `Span` or
/// `Div` (block-display) with a warning until the krilla
/// `RoleMap` API lands.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfTagValue {
    /// `auto` — derive from HTML semantics.
    Auto,
    /// `none` — emit no structure entry for this element.
    None,
    /// `artifact(<kind>)` — exclude from the structure tree and
    /// mark as a PDF artifact of the named kind. The bare
    /// `artifact` keyword resolves to
    /// [`BdPdfArtifactKind::Layout`].
    Artifact(BdPdfArtifactKind),
    /// `<standard-role>` — explicit standard role.
    Standard(BdPdfStandardRole),
    /// `<custom-ident>` — custom role name.
    Custom(CustomIdent),
}

impl BdPdfTagValue {
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

impl Parse for BdPdfTagValue {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        // `artifact(<kind>)` — functional notation with one
        // BdPdfArtifactKind keyword argument.
        if let Ok(value) = input.try_parse(|i| {
            let location = i.current_source_location();
            let function = i.expect_function()?.clone();
            if !function.eq_ignore_ascii_case("artifact") {
                return Err(location.new_custom_error::<_, style_traits::StyleParseErrorKind>(
                    style_traits::StyleParseErrorKind::UnexpectedFunction(function.clone()),
                ));
            }
            i.parse_nested_block(|i| {
                let kind = BdPdfArtifactKind::parse(i)?;
                Ok(Self::Artifact(kind))
            })
        }) {
            return Ok(value);
        }
        // Bare keywords: auto | none | artifact (= artifact(layout)).
        if let Ok(value) = input.try_parse(|i| {
            let ident = i.expect_ident()?;
            match_ignore_ascii_case! { ident,
                "auto" => Ok(Self::Auto),
                "none" => Ok(Self::None),
                "artifact" => Ok(Self::Artifact(BdPdfArtifactKind::Layout)),
                _ => Err(i.new_custom_error::<_, style_traits::StyleParseErrorKind>(
                    style_traits::StyleParseErrorKind::UnspecifiedError,
                )),
            }
        }) {
            return Ok(value);
        }
        if let Ok(role) = input.try_parse(BdPdfStandardRole::parse) {
            return Ok(Self::Standard(role));
        }
        Ok(Self::Custom(CustomIdent::parse(
            input,
            &["auto", "none", "artifact"],
        )?))
    }
}

/// Specified value of `-bd-pdf-tag-{alt,actual-text,lang}`.
///
/// `auto` — fall back to HTML attribute (`alt`, ARIA, `lang`) or
/// document default. `none` — explicit empty slot (suppress
/// fallback). `<string>` — literal value.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfTagStringAuto {
    /// `auto` — defer to HTML/inheritance.
    Auto,
    /// `none` — explicit empty.
    None,
    /// `<string>` — literal value.
    Literal(OwnedStr),
}

impl BdPdfTagStringAuto {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl Parse for BdPdfTagStringAuto {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if let Ok(value) = input.try_parse(|i| {
            let ident = i.expect_ident()?;
            match_ignore_ascii_case! { ident,
                "auto" => Ok(Self::Auto),
                "none" => Ok(Self::None),
                _ => Err(i.new_custom_error::<_, style_traits::StyleParseErrorKind>(
                    style_traits::StyleParseErrorKind::UnspecifiedError,
                )),
            }
        }) {
            return Ok(value);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-tag-{title,expanded}`.
///
/// `none` — no value. `<string>` — literal value. (No `auto` —
/// there is no implicit source to fall back to for these.)
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfTagStringPlain {
    /// `none` — no value.
    None,
    /// `<string>` — literal value.
    Literal(OwnedStr),
}

impl BdPdfTagStringPlain {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

impl Parse for BdPdfTagStringPlain {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-tag-header-cell-scope` (T1-D2).
///
/// PDF/UA-1 §7.5.1 / PDF/UA-2 §8.8 / ISO 32000-2 §14.7.4.4 Table
/// 359 require every `Tag::TH` structure element on a tagged PDF
/// to carry an explicit `/Scope` attribute indicating which axis
/// the header cell labels. `none` accepts the renderer's
/// structural default (Column for `<thead>` headers, Row
/// otherwise — matching HTML5 §4.9.10.1 "header cell implicit
/// scope"). Authors can pin a specific scope on a per-element
/// basis with `row`, `column`, or `both`; the value lands on
/// krilla `Tag::TH::with_scope(...)` at PDF emission time.
/// Mirrors the IR-side `IrTableHeaderScope` enum so authoring
/// stays direct from CSS to the structure tree without an HTML
/// `scope=` attribute.
///
/// Initial `none`. Per-element; not inherited.
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
pub enum BdPdfTagHeaderCellScope {
    /// `none` (initial) — defer to the renderer's structural
    /// default and any HTML `scope=` attribute on the bearing
    /// `<th>`.
    #[default]
    None,
    /// `row` — the header cell labels its row.
    Row,
    /// `column` — the header cell labels its column.
    Column,
    /// `both` — the header cell labels both row and column
    /// (PDF `/Both`). Also covers HTML5 `scope="rowgroup"` and
    /// `scope="colgroup"` which PDF collapses onto `/Both`.
    Both,
}

impl BdPdfTagHeaderCellScope {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

/// Specified value of `-bd-pdf-tag-table-summary` (T1-D2).
///
/// ISO 32000-2 §14.7.4.4 Table 359 defines the `/Summary` entry
/// on the `Table` structure attribute object as a free-form
/// string supplying an accessible description of the table's
/// purpose, structure, or content. PDF/UA-2 §8.8 / WTPDF clause
/// 5 honour the entry as the canonical equivalent of HTML's
/// legacy `<table summary="...">` attribute. Cascade values
/// land on krilla `Tag::Table::with_summary(...)` at PDF
/// emission time.
///
/// `none` — no summary. `<string>` — literal summary text.
/// Initial `none`. Per-element; not inherited.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfTagTableSummary {
    /// `none` (initial) — no `/Summary` entry, unless an HTML
    /// `summary=` attribute supplies one.
    None,
    /// `<string>` — literal summary text.
    Literal(OwnedStr),
}

impl BdPdfTagTableSummary {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

impl Parse for BdPdfTagTableSummary {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-tag-form` (T1-D2).
///
/// PDFreactor's `-ro-pdf-tag-form` (pdfreactor.md:17421) is the
/// `/Role` entry on the `/PrintField` attribute owner attached to a
/// `Form` structure element (ISO 32000-2 §14.7.4.4 Table 359).
/// PDF/UA-1 §7.18 / PDF/UA-2 §8.13 require non-interactive form
/// controls in tagged PDF to declare their role so assistive
/// technology can announce them. The cascade reader projects this
/// onto krilla's `FormFieldRole` enum at PDF emission time.
///
/// `none` (initial) — no `/Role` entry; the renderer leaves the
/// `Form` structure element without a role. `text | button |
/// radiobutton | checkbox | listbox` — explicit roles. Per-element;
/// not inherited.
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
pub enum BdPdfTagForm {
    /// `none` (initial) — no `/Role` entry.
    #[default]
    None,
    /// `text` — text field (`/Role /tv`).
    Text,
    /// `button` — push-button (`/Role /pb`).
    Button,
    /// `radiobutton` — radio button (`/Role /rb`). Single-token
    /// keyword matches the PDFreactor surface
    /// (`-ro-pdf-tag-form: radiobutton`).
    #[css(keyword = "radiobutton")]
    RadioButton,
    /// `checkbox` — checkbox (`/Role /cb`). Single-token keyword
    /// matches the PDFreactor surface
    /// (`-ro-pdf-tag-form: checkbox`).
    #[css(keyword = "checkbox")]
    CheckBox,
    /// `listbox` — list-box (`/Role /lb`); PDF 2.0+. Single-token
    /// keyword matches the PDFreactor surface.
    #[css(keyword = "listbox")]
    ListBox,
}

impl BdPdfTagForm {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

/// Specified value of `-bd-pdf-tag-form-checked` (T1-D2).
///
/// PDFreactor's `-ro-pdf-tag-form-checked` (pdfreactor.md:17450) is
/// the `/checked` (PDF 1.x) / `/Checked` (PDF 2.0+) entry on the
/// `/PrintField` attribute owner attached to a `Form` structure
/// element representing a checkbox or radio button (ISO 32000-2
/// §14.7.4.4 Table 359). The cascade reader projects this onto
/// krilla's `FormFieldState` enum at PDF emission time. The `mixed`
/// keyword mirrors HTML's `aria-checked: mixed` and PDFreactor's
/// `neutral` keyword; the compat translator rewrites `neutral` to
/// `mixed` so authors targeting the native surface use the
/// `aria-checked` vocabulary.
///
/// `none` (initial) — no `/Checked` entry. `off | on | mixed` —
/// explicit state. Per-element; not inherited.
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
pub enum BdPdfTagFormChecked {
    /// `none` (initial) — no `/Checked` entry.
    #[default]
    None,
    /// `off` — the control is unchecked.
    Off,
    /// `on` — the control is checked.
    On,
    /// `mixed` — the control is in the indeterminate / mixed
    /// state. Mirrors HTML's `aria-checked: mixed`.
    Mixed,
}

impl BdPdfTagFormChecked {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

/// Specified value of `-bd-pdf-tag-form-name` (T1-D2).
///
/// PDFreactor's `-ro-pdf-tag-form-name` (pdfreactor.md:17475) is the
/// `/Desc` entry on the `/PrintField` attribute owner attached to a
/// `Form` structure element (ISO 32000-2 §14.7.4.4 Table 359). The
/// entry carries the descriptive name screen readers announce for
/// the form control. PDFreactor's full grammar accepts a
/// comma-separated list with `auto`, `aria-name`, `aria-description`,
/// and `<string>` parts; the native surface accepts a simpler
/// `none | <string>` grammar — `auto` resolution against ARIA
/// happens in the convert layer, not the parser.
///
/// `none` (initial) — no `/Desc` entry. `<string>` — literal
/// descriptive name. Per-element; not inherited.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfTagFormName {
    /// `none` (initial) — no `/Desc` entry.
    None,
    /// `<string>` — literal descriptive name.
    Literal(OwnedStr),
}

impl BdPdfTagFormName {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

impl Parse for BdPdfTagFormName {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-tag-namespace` (K6).
///
/// PDF 2.0 (ISO 32000-2 §14.8.6) lets a structure element bind to
/// a declared namespace so structure-element names carry meaning
/// relative to a vocabulary. bladsy projects this onto its
/// `TagNamespace` enum.
///
/// `auto` — pick the default for the element's HTML namespace:
/// `<math>` and descendants resolve to `mathml`, every other
/// element resolves to the PDF 2.0 standard structure namespace
/// (no override is set on the per-tag `/NS` slot). `pdf2-ssn` and
/// `bladsy` pin the explicit bladsy-built-in namespaces; `html`
/// and `mathml` request the well-known external namespace URIs
/// (`http://www.w3.org/1999/xhtml` and
/// `http://www.w3.org/1998/Math/MathML` respectively).
/// `<custom-ident>` reserves a slot for a future
/// per-`@-bd-pdf-namespace` registry; v1 of the renderer treats
/// unknown idents the same as `auto` and emits a diagnostic.
///
/// Initial `auto`. Per-element; not inherited (the renderer walks
/// the cascade per-element, and a `<math>` ancestor that already
/// carries a `mathml` binding does not force descendants into the
/// same namespace — descendants pick it up via their own HTML
/// namespace).
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfTagNamespace {
    /// `auto` (initial) — defer to the element's HTML namespace.
    /// `<math>` -> MathML; everything else -> default (no `/NS`).
    Auto,
    /// `pdf2-ssn` — bind to the PDF 2.0 standard structure
    /// namespace (bladsy `TagNamespace::Pdf2Ssn`). Useful when a
    /// custom-ident role is being rewritten back to its standard
    /// name and the author wants to force the SSN binding.
    Pdf2Ssn,
    /// `bladsy` — bind to bladsy's custom namespace
    /// (`TagNamespace::Bladsy`). Reserved for bladsy-defined
    /// custom names (`Datetime`, `Terms`, `Title`, &c.).
    Bladsy,
    /// `mathml` — MathML 3 namespace URI
    /// (`http://www.w3.org/1998/Math/MathML`). The renderer
    /// registers the URI on the bladsy `Document` once per
    /// document and reuses the resulting handle for every
    /// element flagged `mathml`.
    MathMl,
    /// `html` — HTML 4 namespace URI
    /// (`http://www.w3.org/1999/xhtml`). Same registration
    /// behaviour as `mathml`.
    Html,
    /// `<custom-ident>` — author-named namespace key. Reserved
    /// for a future per-`@-bd-pdf-namespace` URI registry. v1 of
    /// the renderer treats unknown idents the same as `auto` and
    /// emits a diagnostic.
    Custom(CustomIdent),
}

impl BdPdfTagNamespace {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl Parse for BdPdfTagNamespace {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if let Ok(value) = input.try_parse(|i| {
            let ident = i.expect_ident()?;
            match_ignore_ascii_case! { ident,
                "auto" => Ok(Self::Auto),
                "pdf2-ssn" => Ok(Self::Pdf2Ssn),
                "bladsy" => Ok(Self::Bladsy),
                "mathml" => Ok(Self::MathMl),
                "html" => Ok(Self::Html),
                _ => Err(i.new_custom_error::<_, style_traits::StyleParseErrorKind>(
                    style_traits::StyleParseErrorKind::UnspecifiedError,
                )),
            }
        }) {
            return Ok(value);
        }
        Ok(Self::Custom(CustomIdent::parse(
            input,
            &["auto", "pdf2-ssn", "bladsy", "mathml", "html"],
        )?))
    }
}
