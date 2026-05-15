/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe PDF attachment + destination + page-label CSS surface (F9).
//!
//! See audit `docs/audits/CSS-COVERAGE-AUDIT-2026-05-14/stylo-push-plan.md`
//! family 9 for source-vendor citations. Properties:
//! - `-bd-pdf-attachment-{description,location,mime-type,name,url}`
//! - `-bd-pdf-destination`
//! - `-bd-pdf-page-label`
//! - `-bd-anchor`
//! - `-bd-destination-area`
//!
//! These describe the document-level PDF structures that link
//! elements to file attachments, named destinations, and viewer
//! navigation hints.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::url::SpecifiedUrl;
use crate::OwnedStr;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value for an attachment string slot (`<auto|none> | <string>`).
///
/// Used by `-bd-pdf-attachment-{description,mime-type,name}` and
/// `-bd-pdf-page-label` / `-bd-pdf-destination` / `-bd-anchor`.
/// Variants:
/// - `Auto` — viewer / renderer default.
/// - `None` — explicitly empty slot.
/// - `Literal(...)` — `<string>` value.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfStringSlot {
    /// `auto` — viewer / renderer default.
    Auto,
    /// `none` — explicitly empty.
    None,
    /// `<string>` — literal value.
    Literal(OwnedStr),
}

impl BdPdfStringSlot {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// `none` value.
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfStringSlot {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::Literal(s.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-pdf-attachment-location`.
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
pub enum BdPdfAttachmentLocation {
    #[default]
    Before,
    After,
}

/// Specified value of `-bd-pdf-attachment-url`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfAttachmentUrl {
    /// `none` — no embedded file.
    None,
    /// `url(<file-url>)` — embedded file source.
    Url(SpecifiedUrl),
}

impl BdPdfAttachmentUrl {
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

impl Parse for BdPdfAttachmentUrl {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        Ok(Self::Url(SpecifiedUrl::parse(context, input)?))
    }
}

/// Specified value of `-bd-destination-area`.
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
pub enum BdDestinationArea {
    #[default]
    Auto,
    Element,
    FitPage,
    FitWidth,
    FitHeight,
}
