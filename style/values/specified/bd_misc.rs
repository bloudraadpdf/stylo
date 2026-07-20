/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-*` miscellaneous declarative-tuning properties (F32).
//!
//! Catch-all module for native `-bd-*` longhands that capture a single
//! PDFreactor / Prince proprietary tuning knob. Each property carries
//! a small keyword enum or numeric/string newtype and is wired through
//! a longhand entry in `longhands.toml`. Compat translation lives in
//! `moegoe-css/src/compat/translate.rs`; cascade readers and IR
//! plumbing live in `moegoe-css/src/computed_to_ir/`.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::{
    Context as ComputedContext, ToComputedValue as ToComputedValueTrait,
};
use crate::values::specified::{Integer, Percentage};
use crate::values::CSSFloat;
use crate::OwnedStr;
use cssparser::Parser;
use style_traits::{ParseError, StyleParseErrorKind};

/// Specified value of `-bd-lang` (F32.1).
///
/// Mirrors Prince `-prince-lang` — overrides the language tag used
/// for hyphenation, justification, locale-aware quoting, and
/// font-language fall-through. `auto` defers to the standard
/// HTML/XML `lang` attribute. `<string>` supplies a BCP 47 tag
/// directly.
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
pub enum BdLang {
    /// `auto` — defer to the HTML `lang` attribute.
    Auto,
    /// `<string>` — BCP 47 language tag literal.
    Literal(OwnedStr),
}

impl BdLang {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl Parse for BdLang {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-tab-snap` (T5 §A.5.9).
///
/// Native counterpart to Prince's `-prince-tab-size: nearest` /
/// PDFreactor's `-ro-tab-size: nearest` extension. Modifies how the
/// standard CSS `tab-size` interval is consumed at line layout:
///
/// - `next-greater` (CSS Text 3 §3 default) — a tab advances the
///   cursor to the next multiple of `tab-size` strictly greater than
///   the current inline position.
/// - `nearest` — Prince-style — a tab advances the cursor to the
///   nearest multiple of `tab-size`. When the cursor is already at a
///   multiple of `tab-size`, the tab consumes one full interval.
///
/// The interval itself continues to come from standard `tab-size`.
/// This longhand only carries the *policy*, so authors who set
/// `-prince-tab-size: nearest` along with a numeric `tab-size` get
/// the expected combined behaviour after compat translation.
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
pub enum BdTabSnap {
    /// CSS Text 3 default — snap to the next multiple strictly greater
    /// than the cursor.
    #[default]
    NextGreater,
    /// Prince extension — snap to the nearest multiple of `tab-size`.
    Nearest,
}

/// Specified value of `-bd-shrink-to-fit` (F32.2).
///
/// `@page` descriptor mirroring Prince `-prince-shrink-to-fit`.
/// `auto` (initial) — block content overflowing the page area is
/// shrunk uniformly until it fits. `none` — no shrinking.
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
pub enum BdShrinkToFit {
    #[default]
    Auto,
    None,
}

/// Specified value of `-bd-table-column-span` / `-bd-rowspan` /
/// `-bd-table-row-span` / `-bd-listitem-value` (F32.3, F32.4, F32.9).
///
/// Single-integer overrides for HTML attributes that would normally
/// supply the value. `auto` (initial) defers to HTML; a positive
/// integer overrides.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdIntegerAuto {
    /// `auto` — defer to the HTML attribute.
    Auto,
    /// `<integer>` — explicit override.
    Value(Integer),
}

impl BdIntegerAuto {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }
}

impl Parse for BdIntegerAuto {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        let v = Integer::parse(context, input)?;
        Ok(Self::Value(v))
    }
}

/// Computed value of `-bd-table-column-span` / `-bd-rowspan` /
/// `-bd-table-row-span` / `-bd-listitem-value`.
///
/// Round-trips identically to the specified value, but the inner
/// integer becomes a plain `i32` (mirroring `Integer`'s
/// `ToComputedValue` impl).
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum ComputedBdIntegerAuto {
    /// `auto`.
    Auto,
    /// `<integer>`.
    Value(i32),
}

impl ToComputedValueTrait for BdIntegerAuto {
    type ComputedValue = ComputedBdIntegerAuto;

    fn to_computed_value(&self, context: &ComputedContext) -> ComputedBdIntegerAuto {
        match *self {
            Self::Auto => ComputedBdIntegerAuto::Auto,
            Self::Value(ref v) => ComputedBdIntegerAuto::Value(v.to_computed_value(context)),
        }
    }

    fn from_computed_value(computed: &ComputedBdIntegerAuto) -> Self {
        match *computed {
            ComputedBdIntegerAuto::Auto => Self::Auto,
            ComputedBdIntegerAuto::Value(v) => {
                Self::Value(<Integer as ToComputedValueTrait>::from_computed_value(&v))
            }
        }
    }
}

/// Specified value of `-bd-caption-page` (F32.6).
///
/// Prince `caption-page*` — which page in a multi-page table the
/// `<caption>` element appears on. `all` is the spec default for
/// HTML tables; PDFreactor / Prince allow restricting to the first
/// or following pages.
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
pub enum BdCaptionPage {
    #[default]
    All,
    First,
    Following,
}

/// Specified value of `-bd-target-candidate` (F32.7).
///
/// PDFreactor `-ro-target-candidate` — opts an element in / out of
/// being a candidate target for cross-reference resolution
/// (`target-counter()`, `target-text()`). `auto` (initial) — the
/// renderer chooses; `yes` / `no` force the answer.
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
pub enum BdTargetCandidate {
    #[default]
    Auto,
    Yes,
    No,
}

/// Specified value of `-bd-truncate-margin-after-break` (F32.8).
///
/// PDFreactor `-ro-truncate-margin-after-break` — controls whether
/// block margins are collapsed away when a break leaves them at the
/// top of a fresh fragmentainer. `auto` defers to standard CSS
/// margin-collapse rules (truncate at unforced breaks, keep at
/// forced breaks and at the first page); `none` retains the margin
/// in full; `always` truncates in every case, including forced
/// breaks and the first page (PDFreactor manual, "Between Blocks").
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
pub enum BdTruncateMarginAfterBreak {
    #[default]
    Auto,
    None,
    Always,
}

/// Specified value of `-bd-replacedelement` (F32.10).
///
/// PDFreactor `-ro-replacedelement` — marks an element to behave as
/// a synthetic replaced element for layout purposes. `none` (initial)
/// — normal flow. `auto` lets the renderer infer from `content:`.
/// `image` declares the element as an image-style replaced element.
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
pub enum BdReplacedElement {
    #[default]
    None,
    Auto,
    Image,
}

/// Specified value of `-bd-scale-content` (F36; supersedes the
/// earlier F32.12 number-typed prototype).
///
/// PDFreactor `-ro-scale-content` (matrix line 17962) — page-level
/// uniform visual scale applied to the entire content stream of
/// every page via a `[s 0 0 s 0 0] cm` transform pushed at content-
/// stream open. Authored on `:root`; non-root declarations have no
/// effect on the renderer.
///
/// `Percentage(p)` (initial `100%`) — every page paints at `p` of
/// natural size. `1.0` (100%) is the identity — the renderer elides
/// the `cm` transform entirely. `fit-page` (per-page) — each page's
/// scale is `min(1.0, page_content_height / natural_content_height)`
/// so any page whose natural content exceeds the page content box
/// shrinks uniformly to fit, while pages already within the content
/// box paint at full size. Layout (line breaks, page breaks,
/// widows / orphans, multicol balance) is unaffected — only the
/// visual paint scales.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdScaleContent {
    /// `<percentage>` — uniform scale factor expressed as a
    /// percentage. Stored as the parsed `Percentage` so `to_css`
    /// round-trips the authored token. Initial value: `100%`.
    Percentage(Percentage),
    /// `fit-page` — per-page shrink-to-fit when natural content
    /// overflows the page content box.
    FitPage,
}

impl BdScaleContent {
    /// Initial value (`100%`).
    #[inline]
    pub fn initial() -> Self {
        Self::Percentage(Percentage::new(1.0))
    }
}

impl Parse for BdScaleContent {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("fit-page"))
            .is_ok()
        {
            return Ok(Self::FitPage);
        }
        Ok(Self::Percentage(Percentage::parse(context, input)?))
    }
}

/// Computed value of `-bd-scale-content`.
///
/// The percentage is computed to a plain `CSSFloat` factor (e.g.
/// `0.5` for `50%`) so downstream consumers can multiply directly
/// without re-deriving the unit base.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum ComputedBdScaleContent {
    /// `<percentage>` — resolved to a multiplicative factor.
    Percentage(CSSFloat),
    /// `fit-page` — per-page shrink-to-fit.
    FitPage,
}

impl ComputedBdScaleContent {
    /// Initial computed value (`100%` → factor `1.0`).
    #[inline]
    pub fn initial() -> Self {
        Self::Percentage(1.0)
    }
}

impl ToComputedValueTrait for BdScaleContent {
    type ComputedValue = ComputedBdScaleContent;

    fn to_computed_value(&self, context: &ComputedContext) -> ComputedBdScaleContent {
        match *self {
            Self::Percentage(ref p) => {
                ComputedBdScaleContent::Percentage(p.to_computed_value(context).0)
            }
            Self::FitPage => ComputedBdScaleContent::FitPage,
        }
    }

    fn from_computed_value(computed: &ComputedBdScaleContent) -> Self {
        match *computed {
            ComputedBdScaleContent::Percentage(f) => Self::Percentage(Percentage::new(f)),
            ComputedBdScaleContent::FitPage => Self::FitPage,
        }
    }
}

/// Specified value of `-bd-position-origin` (F32.13).
///
/// PDFreactor `-ro-position-origin` — selects the reference box
/// against which `position: absolute` offsets are resolved. `border`
/// (initial) — standard CSS behaviour. `padding` — content-box edge
/// of the containing block including padding. `content` — content
/// box only.
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
pub enum BdPositionOrigin {
    #[default]
    Border,
    Padding,
    Content,
    #[css(keyword = "-bd-page-box")]
    PageBox,
    #[css(keyword = "-bd-bleed-box")]
    BleedBox,
}

/// Specified value of `-bd-line-break-opportunity` (F32.14).
///
/// PDFreactor `-ro-line-break-opportunity` — declares whether the
/// element introduces a line-break opportunity at its position in
/// the inline run. `auto` defers to standard CSS rules; `before` /
/// `after` force the opportunity on the named edge; `none`
/// suppresses both.
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
pub enum BdLineBreakOpportunity {
    #[default]
    Auto,
    Before,
    After,
    None,
}

/// Specified value of `-bd-object-slice` (F32.15).
///
/// PDFreactor `-ro-object-slice` — whether a block-level replaced
/// element (typically an image) may be sliced across a page break.
/// `auto` defers to standard behaviour (no slicing); `none` is the
/// initial / standard. `slice` requests fragmentation.
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
pub enum BdObjectSlice {
    #[default]
    None,
    Slice,
    Auto,
}

/// Specified value of `-bd-flow` (F32.21).
///
/// Prince `flow*` / `-prince-flow` — removes the element from
/// normal flow and routes it into a named region or static area.
/// `none` (initial) — element remains in normal flow.
/// `<custom-ident>` — name of the target flow.
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
pub enum BdFlow {
    /// `none` — element stays in normal flow.
    None,
    /// `<custom-ident>` — name of the destination flow.
    Name(OwnedStr),
}

impl BdFlow {
    /// Initial value (`none`).
    #[inline]
    pub fn none() -> Self {
        Self::None
    }
}

impl Parse for BdFlow {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let ident = input.expect_ident()?;
        // Reserved words guard — `none` is the only one reserved
        // because the flow registry treats every other identifier
        // as a free name.
        if ident.eq_ignore_ascii_case("none") {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Name(ident.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-column-clip` (K20).
///
/// `normal | clip` (default `normal`). Mirrors PDFreactor's
/// `-ro-column-clip` and Prince's overflow control for multicol
/// columns. `normal` (initial) preserves the CSS Multi-column
/// spec's overflow behaviour — content that exceeds the column's
/// inline extent is permitted to ink-overflow into the column
/// gap, matching CSS Multi-column Level 2 §3.4. `clip` forces
/// the renderer to clip overflowing inline content at the
/// column's inline edge (paint-time intersection with the column
/// box), suppressing the bleed into the gap that some authors
/// find undesirable in print.
///
/// This is paint-time scoped; layout-time geometry is unchanged.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdColumnClip {
    #[default]
    Normal,
    Clip,
}

impl BdColumnClip {
    /// Whether the value is `normal`.
    #[inline]
    pub fn is_normal(&self) -> bool {
        matches!(self, Self::Normal)
    }

    /// Whether the value is `clip`.
    #[inline]
    pub fn is_clip(&self) -> bool {
        matches!(self, Self::Clip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::{Parser, ParserInput};
    use style_traits::ToCss;

    fn parse_column_clip(css: &str) -> Result<BdColumnClip, ()> {
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdColumnClip::parse(input))
            .map_err(|_| ())
    }

    #[test]
    fn bd_column_clip_initial_is_normal() {
        assert_eq!(BdColumnClip::default(), BdColumnClip::Normal);
        assert!(BdColumnClip::default().is_normal());
    }

    #[test]
    fn bd_column_clip_normal_parses() {
        let value = parse_column_clip("normal").expect("normal should parse");
        assert_eq!(value, BdColumnClip::Normal);
        assert_eq!(value.to_css_string(), "normal");
    }

    #[test]
    fn bd_column_clip_clip_parses() {
        let value = parse_column_clip("clip").expect("clip should parse");
        assert_eq!(value, BdColumnClip::Clip);
        assert!(value.is_clip());
        assert_eq!(value.to_css_string(), "clip");
    }

    #[test]
    fn bd_column_clip_rejects_unknown() {
        assert!(parse_column_clip("hidden").is_err());
        assert!(parse_column_clip("visible").is_err());
        assert!(parse_column_clip("").is_err());
    }
}
