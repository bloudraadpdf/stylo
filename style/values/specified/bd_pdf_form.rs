/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-form-field-*` and `-bd-pdf-signature-field-*`
//! AcroForm widget properties (G3 — Family 26).
//!
//! Native moegoe fork-extension surface for the form-field flag /
//! length / signature widgets that the initial G3 landing
//! (`-bd-pdf-format`) did not plumb. These cover the
//! `-ro-pdf-form-field-flags`, `-ro-pdf-form-field-maxlength`,
//! `-ro-pdf-signature-field-lock`, and `-ro-pdf-signature-field-name`
//! surfaces in PDFreactor (see
//! `docs/reference-manuals/pdfreactor.md:17139–17301`). They apply
//! to elements that produce a widget annotation; they are not
//! inherited; the renderer consumes them via the form-widget
//! conversion arm in `moegoe-css`.
//!
//! `flags` is a bit-flag set of ISO 32000-2 §12.7 widget field
//! flags. `maxlength` is a non-negative integer cap on text-field
//! input. `lock` controls the lock behaviour of a signature widget.
//! `name` declares the signature-field's name in the AcroForm
//! dictionary.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::color::Color;
use crate::values::specified::url::SpecifiedUrl;
use crate::values::specified::Percentage;
use crate::values::CustomIdent;
use crate::{OwnedSlice, OwnedStr};
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::Write;
use style_traits::{ParseError, StyleParseErrorKind};

#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[css(bitflags(
    single = "none",
    mixed = "read-only,required,no-export,multiline,password,file-select,do-not-spell-check,no-scroll,comb,rich-text",
))]
#[repr(C)]
/// Specified value of `-bd-pdf-form-field-flags`.
///
/// Mirrors ISO 32000-2 §12.7.4 field-flag bit positions for
/// `Tx` (text), `Ch` (choice), and `Btn` (button) widget fields.
pub struct BdPdfFormFieldFlags(u32);
bitflags! {
    impl BdPdfFormFieldFlags: u32 {
        /// Empty set (`none` keyword).
        const NONE = 0;
        /// `read-only` — field is not editable.
        const READ_ONLY = 1 << 0;
        /// `required` — field must have a value at submission.
        const REQUIRED = 1 << 1;
        /// `no-export` — field is omitted from submission data.
        const NO_EXPORT = 1 << 2;
        /// `multiline` — text fields accept newline characters.
        const MULTILINE = 1 << 12;
        /// `password` — text input is rendered as bullets.
        const PASSWORD = 1 << 13;
        /// `file-select` — text field's value is the pathname of a file
        /// (ISO 32000-2 §12.7.4.4 Table 231, `Tx`-only). Mutually
        /// exclusive with `multiline`, `password`, and `comb` at the
        /// PDF level; the renderer projects the bit unconditionally and
        /// leaves the exclusivity check to PDF consumers.
        const FILE_SELECT = 1 << 20;
        /// `do-not-spell-check` — disables spell-check.
        const DO_NOT_SPELL_CHECK = 1 << 22;
        /// `no-scroll` — text fields disable scrolling.
        const NO_SCROLL = 1 << 23;
        /// `comb` — text split into equal-width cells.
        const COMB = 1 << 24;
        /// `rich-text` — text interpreted as rich text.
        const RICH_TEXT = 1 << 25;
    }
}

impl Default for BdPdfFormFieldFlags {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl BdPdfFormFieldFlags {
    /// Whether the set is empty (`none`).
    #[inline]
    pub fn is_none(&self) -> bool {
        self.is_empty()
    }
}

/// Specified value of `-bd-pdf-form-field-maxlength`.
///
/// `none` (initial) — no length cap; `<integer>` — non-negative
/// cap on the text-field input.
#[derive(
    Clone,
    Copy,
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
pub enum BdPdfFormFieldMaxLength {
    /// No length cap.
    None,
    /// Explicit non-negative cap.
    Length(u32),
}

impl Default for BdPdfFormFieldMaxLength {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdPdfFormFieldMaxLength {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfFormFieldMaxLength {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let n = input.expect_integer()?;
        if n < 0 {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Length(n as u32))
    }
}

/// Specified value of `-bd-pdf-annotation-hidden`.
///
/// Mirrors the annotation-level `/F 2` (`HIDDEN`) flag per
/// ISO 32000-2 §12.5.3 Table 165. Distinct from the widget *field*
/// flags carried by [`BdPdfFormFieldFlags`] (which write `/Ff`).
///
/// `auto` (initial) — the annotation's hidden bit is derived from
/// HTML semantics. For widget annotations the existing
/// `<input type="hidden">` path stays unchanged.
/// `hidden` — force `/F 2` on the annotation regardless of source
/// markup. On widget annotations the field still participates in
/// `/AcroForm /Fields` and carries `/V`, but the viewer renders no
/// appearance.
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
#[allow(missing_docs)]
pub enum BdPdfAnnotationHidden {
    #[default]
    Auto,
    Hidden,
}

impl BdPdfAnnotationHidden {
    /// Whether the keyword is `auto` (the initial value, derives the
    /// hidden bit from HTML semantics).
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// Specified value of `-bd-pdf-signature-field-lock`.
///
/// Mirrors ISO 32000-2 §12.8.2.3 SigFieldLock /Action entries.
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
#[allow(missing_docs)]
pub enum BdPdfSignatureFieldLock {
    #[default]
    None,
    All,
    Include,
    Exclude,
}

/// Specified value of `-bd-pdf-signature-field-lock-fields`.
///
/// `none` (initial) — no explicit field list; `<custom-ident>+` —
/// space-separated list of fully qualified field names. The list is
/// only meaningful when `-bd-pdf-signature-field-lock` is `include`
/// or `exclude`; with `all` / `none` the list is ignored by the
/// renderer per ISO 32000-2 §12.7.4.5 Table 232 (the `/Fields` entry
/// is only emitted with `/Action /Include` or `/Action /Exclude`).
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
pub enum BdPdfSignatureFieldLockFields {
    /// No explicit field list — projects as the empty `Vec<String>`
    /// in the IR; the renderer emits `/Fields [ ]` for `Include` /
    /// `Exclude` lock variants.
    None,
    /// `<custom-ident>+` — one or more space-separated field names.
    Names(#[css(iterable)] OwnedSlice<CustomIdent>),
}

impl Default for BdPdfSignatureFieldLockFields {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdPdfSignatureFieldLockFields {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfSignatureFieldLockFields {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let mut names: Vec<CustomIdent> = Vec::new();
        loop {
            let result = input.try_parse(|i| -> Result<CustomIdent, ParseError<'i>> {
                let location = i.current_source_location();
                let ident = i.expect_ident()?.clone();
                CustomIdent::from_ident(location, &ident, &["none"])
            });
            match result {
                Ok(name) => names.push(name),
                Err(_) => break,
            }
        }
        if names.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Names(OwnedSlice::from(names)))
    }
}

/// Specified value of `-bd-pdf-signature-field-name`.
///
/// `none` (initial) — no explicit name; `<string>` — the AcroForm
/// dictionary `/T` entry for the signature widget.
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
pub enum BdPdfSignatureFieldName {
    /// No explicit name.
    None,
    /// Explicit `/T` string.
    Literal(OwnedStr),
}

impl Default for BdPdfSignatureFieldName {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdPdfSignatureFieldName {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfSignatureFieldName {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

// ---------------------------------------------------------------------
// `-bd-pdf-form-field-mk-*` — `/MK` Appearance Characteristics
// (ISO 32000-2 §12.5.6.19 Table 167) longhands for AcroForm widget
// annotations. Per-element; not inherited; consumed by the
// form-widget conversion arm in `moegoe-css`. Pushbutton-only entries
// (`RC`, `AC`, `RI`, `IX`, `IF`, `TP`) are still parsed on any
// element; the renderer applies them only to pushbutton widgets per
// the spec.
// ---------------------------------------------------------------------

/// Specified value of `-bd-pdf-form-field-mk-border-colour` and
/// `-bd-pdf-form-field-mk-background-colour`.
///
/// `none` (initial) omits the corresponding `/MK /BC` or `/MK /BG`
/// entry — the viewer falls back to its default. A `<color>` value
/// writes the device-space colour array per Table 167. Wide-gamut
/// colours collapse to a three-component DeviceRGB array because
/// `/MK` colour entries are device-space only.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfFormFieldMkColour {
    /// `none` — entry omitted from `/MK`.
    None,
    /// Explicit colour.
    Colour(Color),
}

impl Default for BdPdfFormFieldMkColour {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdPdfFormFieldMkColour {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfFormFieldMkColour {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        Ok(Self::Colour(Color::parse(context, input)?))
    }
}

/// Specified value of `-bd-pdf-form-field-mk-rotation`.
///
/// `/MK /R` per ISO 32000-2 §12.5.6.19 Table 167 — the widget
/// rotation in degrees, restricted to multiples of 90 in `[0, 360)`.
/// `0` is the initial value and is treated identically to omission
/// (no `/R` entry emitted).
///
/// Accepts either the CSS integer form (`0`, `90`, `180`, `270`) or
/// keyword aliases (`zero`, `quarter`, `half`, `three-quarter`).
/// The Stylo `Parse` derive only matches ident tokens, so integer
/// parsing is handled by a manual `Parse` impl that tries the integer
/// path first.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
pub enum BdPdfFormFieldMkRotation {
    /// `0` / `zero` — no rotation (initial, omits `/R`).
    #[default]
    Zero,
    /// `90` / `quarter` — 90° counter-clockwise.
    Quarter,
    /// `180` / `half` — upside-down.
    Half,
    /// `270` / `three-quarter` — 90° clockwise.
    ThreeQuarter,
}

impl Parse for BdPdfFormFieldMkRotation {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        // Try integer tokens first (`90`, `180`, `270`, `0`).
        if let Ok(n) = input.try_parse(|i| i.expect_integer()) {
            return match n {
                0 => Ok(Self::Zero),
                90 => Ok(Self::Quarter),
                180 => Ok(Self::Half),
                270 => Ok(Self::ThreeQuarter),
                _ => Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
            };
        }
        // Fall back to keyword aliases.
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        match_ignore_ascii_case! { &ident,
            "zero" => Ok(Self::Zero),
            "quarter" => Ok(Self::Quarter),
            "half" => Ok(Self::Half),
            "three-quarter" => Ok(Self::ThreeQuarter),
            _ => Err(location.new_unexpected_token_error(
                cssparser::Token::Ident(ident.clone())
            ))
        }
    }
}

impl BdPdfFormFieldMkRotation {
    /// Whether the value is the initial (`0`, omits `/R`).
    #[inline]
    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Zero)
    }
}

/// Specified value of `-bd-pdf-form-field-mk-rollover-caption` and
/// `-bd-pdf-form-field-mk-down-caption`.
///
/// `none` (initial) omits the corresponding `/MK /RC` or `/MK /AC`
/// entry. A `<string>` value writes the caption verbatim; meaningful
/// only on pushbutton widgets.
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
pub enum BdPdfFormFieldMkCaption {
    /// `none` — entry omitted from `/MK`.
    None,
    /// Explicit caption string.
    Literal(OwnedStr),
}

impl Default for BdPdfFormFieldMkCaption {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdPdfFormFieldMkCaption {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfFormFieldMkCaption {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-form-field-mk-rollover-icon` and
/// `-bd-pdf-form-field-mk-alternate-icon`.
///
/// `none` (initial) omits the corresponding `/MK /RI` or `/MK /IX`
/// entry. `url(<href>)` declares the image source; the cascade
/// reader fetches the bytes through the standard moegoe
/// [`ResourceLoader`] and constructs a `FormXObject` for embedding
/// per ISO 32000-2 §12.5.6.19 Table 167. Meaningful only on
/// pushbutton widgets.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfFormFieldMkIcon {
    /// `none` — entry omitted from `/MK`.
    None,
    /// `url(<href>)` — external icon image.
    Url(SpecifiedUrl),
}

impl Default for BdPdfFormFieldMkIcon {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdPdfFormFieldMkIcon {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfFormFieldMkIcon {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let url = SpecifiedUrl::parse(context, input)?;
        Ok(Self::Url(url))
    }
}

/// `/MK /IF /S` scale type per ISO 32000-2 §12.5.6.19 Table 188.
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
#[allow(missing_docs)]
pub enum BdPdfFormFieldMkIconFitScaleType {
    /// `always` — anamorphic scaling (`/S /A`, the spec default).
    #[default]
    Always,
    /// `proportional` — proportional scaling (`/S /P`).
    Proportional,
    /// `never` — never scale; matches `scale-when: never` (`/SW /N`)
    /// at the renderer.
    Never,
    /// `bigger` — only scale when the content is bigger; matches
    /// `scale-when: content-bigger` (`/SW /B`).
    Bigger,
}

/// `/MK /IF /SW` scale-when keyword per ISO 32000-2 §12.5.6.19
/// Table 189.
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
#[allow(missing_docs)]
pub enum BdPdfFormFieldMkIconFitScaleWhen {
    /// `always` — `/SW /A` (the spec default).
    #[default]
    Always,
    /// `content-smaller` — `/SW /S`.
    ContentSmaller,
    /// `content-bigger` — `/SW /B`.
    ContentBigger,
    /// `never` — `/SW /N`.
    Never,
}

/// Specified value of `-bd-pdf-form-field-mk-icon-fit`.
///
/// `/MK /IF` icon-fit dictionary per ISO 32000-2 §12.5.6.19 Table 187.
/// `none` (initial) omits the entry; otherwise the value is a triple
/// `<scale-type> || <scale-when> || <alignment>` where alignment is a
/// `<percentage> <percentage>` pair (`/A` array). Each component is
/// optional in any order; missing components fall back to their
/// initial values (`always`, `always`, `50% 50%`).
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfFormFieldMkIconFit {
    /// `none` — entry omitted from `/MK`.
    None,
    /// Explicit icon-fit triple.
    Fit(BdPdfFormFieldMkIconFitValue),
}

/// The non-`none` payload of [`BdPdfFormFieldMkIconFit`]. Carries the
/// three orderable components of the `<scale-type> || <scale-when> ||
/// <alignment>` value-list.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdPdfFormFieldMkIconFitValue {
    /// `/IF /S` scale-type keyword.
    pub scale_type: BdPdfFormFieldMkIconFitScaleType,
    /// `/IF /SW` scale-when keyword.
    pub scale_when: BdPdfFormFieldMkIconFitScaleWhen,
    /// `/IF /A[0]` horizontal alignment percentage in `[0%, 100%]`.
    pub align_x: Percentage,
    /// `/IF /A[1]` vertical alignment percentage in `[0%, 100%]`.
    pub align_y: Percentage,
}

impl Default for BdPdfFormFieldMkIconFit {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdPdfFormFieldMkIconFit {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfFormFieldMkIconFit {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        // `<scale-type> || <scale-when> || <align-x> <align-y>` —
        // each component is optional; at least one must parse. The
        // alignment pair is the only multi-token component, so we
        // accept the first percentage we see, then require the
        // second percentage to follow before yielding control back.
        let mut scale_type: Option<BdPdfFormFieldMkIconFitScaleType> = None;
        let mut scale_when: Option<BdPdfFormFieldMkIconFitScaleWhen> = None;
        let mut align: Option<(Percentage, Percentage)> = None;
        loop {
            if scale_type.is_none() {
                if let Ok(v) = input.try_parse(BdPdfFormFieldMkIconFitScaleType::parse) {
                    scale_type = Some(v);
                    continue;
                }
            }
            if scale_when.is_none() {
                if let Ok(v) = input.try_parse(BdPdfFormFieldMkIconFitScaleWhen::parse) {
                    scale_when = Some(v);
                    continue;
                }
            }
            if align.is_none() {
                if let Ok(pair) =
                    input.try_parse(|i| -> Result<(Percentage, Percentage), ParseError<'i>> {
                        let x = Percentage::parse(context, i)?;
                        let y = Percentage::parse(context, i)?;
                        Ok((x, y))
                    })
                {
                    align = Some(pair);
                    continue;
                }
            }
            break;
        }
        if scale_type.is_none() && scale_when.is_none() && align.is_none() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        let (align_x, align_y) =
            align.unwrap_or_else(|| (Percentage::new(0.5), Percentage::new(0.5)));
        Ok(Self::Fit(BdPdfFormFieldMkIconFitValue {
            scale_type: scale_type.unwrap_or_default(),
            scale_when: scale_when.unwrap_or_default(),
            align_x,
            align_y,
        }))
    }
}

/// Specified value of `-bd-pdf-form-field-mk-text-position`.
///
/// `/MK /TP` integer keyword per ISO 32000-2 §12.5.6.19 Table 192.
/// Selects the position of the pushbutton caption relative to the
/// icon. `caption-only` (initial) matches the absent-`/TP` default
/// and is treated identically to omission.
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
#[allow(missing_docs)]
pub enum BdPdfFormFieldMkTextPosition {
    /// `caption-only` — `/TP 0` (initial, omits `/TP`).
    #[default]
    CaptionOnly,
    /// `icon-only` — `/TP 1`.
    IconOnly,
    /// `caption-below-icon` — `/TP 2`.
    CaptionBelowIcon,
    /// `caption-above-icon` — `/TP 3`.
    CaptionAboveIcon,
    /// `caption-right-of-icon` — `/TP 4`.
    CaptionRightOfIcon,
    /// `caption-left-of-icon` — `/TP 5`.
    CaptionLeftOfIcon,
    /// `caption-overlaid-on-icon` — `/TP 6`.
    CaptionOverlaidOnIcon,
}

impl BdPdfFormFieldMkTextPosition {
    /// Whether the value is the initial (`caption-only`, omits
    /// `/TP`).
    #[inline]
    pub fn is_caption_only(&self) -> bool {
        matches!(self, Self::CaptionOnly)
    }
}
