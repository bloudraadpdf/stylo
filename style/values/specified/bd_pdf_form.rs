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
use crate::values::CustomIdent;
use crate::{OwnedSlice, OwnedStr};
use cssparser::Parser;
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
    Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
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
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
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
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
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
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
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
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
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
