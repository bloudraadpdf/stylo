/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe PDF rubber-stamp annotation CSS surface (V1-20).
//!
//! Five sibling longhands forming the `-bd-pdf-stamp-*` cluster
//! per ISO 32000-2 §12.5.6.13:
//!
//! - `-bd-pdf-stamp-icon` — gating longhand selecting the predefined
//!   `/Name` keyword or an author-defined `custom(<string>)` glyph.
//!   When `none` (default), no stamp is emitted.
//! - `-bd-pdf-stamp-contents` — `/Contents` body text.
//! - `-bd-pdf-stamp-title` — `/T` popup title (author).
//! - `-bd-pdf-stamp-subject` — `/Subj` subject line.
//! - `-bd-pdf-stamp-intent` — `/IT` intent keyword
//!   (`stamp-image` / `stamp-snapshot` per ISO 32000-2 §12.5.6.2
//!   Table 168, or a custom string).
//!
//! See `~/dev/bloudraad/moegoe/V1-AUDIT.md` item 20 for the closure
//! audit; the matching IR/conversion plumbing lives in
//! `moegoe-ir::pdf_annotation::PdfStampAnnotation` and the renderer
//! dispatch in `moegoe-pdf::renderer::annotations`.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::OwnedStr;
use cssparser::{match_ignore_ascii_case, Parser};
use style_traits::ParseError;

/// Specified value of `-bd-pdf-stamp-icon`.
///
/// Mirrors the fifteen predefined keywords from ISO 32000-2
/// §12.5.6.13 Table 184 plus an author-defined `custom(<string>)`
/// function value and the gating `none` keyword.
///
/// `none` (the initial) suppresses stamp emission entirely; every
/// other value gates the renderer to produce one `/Subtype /Stamp`
/// annotation over the bearer element's painted bbox.
///
/// Default predefined stamps (e.g. `draft`, `approved`) project onto
/// krilla's [`StampIcon`] variants; viewers supply the appearance.
/// `custom("MyHouseStamp")` projects onto `StampIcon::Custom` and
/// emits the literal name verbatim — the embedder is responsible
/// for supplying an `/AP` appearance stream if the receiving viewer
/// does not recognise the name.
///
/// [`StampIcon`]: ../../../../../../krilla/src/interactive/annotation.rs
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
#[allow(missing_docs)]
pub enum BdPdfStampIcon {
    /// `none` — no stamp annotation emitted (initial).
    None,
    /// `/Approved` — green checkmark.
    Approved,
    /// `/AsIs` — slanted "AS IS" label.
    AsIs,
    /// `/Confidential` — red "CONFIDENTIAL" label.
    Confidential,
    /// `/Departmental` — blue "DEPARTMENTAL" label.
    Departmental,
    /// `/Draft` — red "DRAFT" label.
    Draft,
    /// `/Experimental` — blue "EXPERIMENTAL" label.
    Experimental,
    /// `/Expired` — red "EXPIRED" label.
    Expired,
    /// `/Final` — green "FINAL" label.
    Final,
    /// `/ForComment` — green "FOR COMMENT" label.
    ForComment,
    /// `/ForPublicRelease` — green "FOR PUBLIC RELEASE" label.
    ForPublicRelease,
    /// `/NotApproved` — red "NOT APPROVED" label.
    NotApproved,
    /// `/NotForPublicRelease` — red "NOT FOR PUBLIC RELEASE" label.
    NotForPublicRelease,
    /// `/Sold` — blue "SOLD" label.
    Sold,
    /// `/TopSecret` — red "TOP SECRET" label.
    TopSecret,
    /// `custom(<string>)` — author-defined name emitted verbatim
    /// in the annotation's `/Name` entry.
    Custom(OwnedStr),
}

impl Default for BdPdfStampIcon {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdPdfStampIcon {
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

impl Parse for BdPdfStampIcon {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        // `custom(<string>)` — functional notation with one string
        // argument carrying the author-defined name.
        if let Ok(value) = input.try_parse(|i| {
            let location = i.current_source_location();
            let function = i.expect_function()?.clone();
            if !function.eq_ignore_ascii_case("custom") {
                return Err(location.new_custom_error::<_, style_traits::StyleParseErrorKind>(
                    style_traits::StyleParseErrorKind::UnexpectedFunction(function.clone()),
                ));
            }
            i.parse_nested_block(|i| {
                let s = i.expect_string()?;
                Ok(Self::Custom(s.as_ref().to_owned().into()))
            })
        }) {
            return Ok(value);
        }
        // Bare predefined keywords.
        let location = input.current_source_location();
        let ident = input.expect_ident()?;
        match_ignore_ascii_case! { ident,
            "none" => Ok(Self::None),
            "approved" => Ok(Self::Approved),
            "as-is" => Ok(Self::AsIs),
            "confidential" => Ok(Self::Confidential),
            "departmental" => Ok(Self::Departmental),
            "draft" => Ok(Self::Draft),
            "experimental" => Ok(Self::Experimental),
            "expired" => Ok(Self::Expired),
            "final" => Ok(Self::Final),
            "for-comment" => Ok(Self::ForComment),
            "for-public-release" => Ok(Self::ForPublicRelease),
            "not-approved" => Ok(Self::NotApproved),
            "not-for-public-release" => Ok(Self::NotForPublicRelease),
            "sold" => Ok(Self::Sold),
            "top-secret" => Ok(Self::TopSecret),
            _ => Err(location.new_custom_error::<_, style_traits::StyleParseErrorKind>(
                style_traits::StyleParseErrorKind::UnspecifiedError,
            )),
        }
    }
}

/// Specified value of `-bd-pdf-stamp-{contents,title,subject}`.
///
/// `<auto | none> | <string>`:
///
/// - `auto` (initial) — defer to the renderer's per-slot default
///   (no `/Contents`, `/T`, or `/Subj` entry written).
/// - `none` — explicitly empty slot.
/// - `<string>` — literal annotation text.
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
pub enum BdPdfStampString {
    /// `auto` — defer to renderer default.
    Auto,
    /// `none` — explicitly empty.
    None,
    /// `<string>` — literal annotation text.
    Literal(OwnedStr),
}

impl Default for BdPdfStampString {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

impl BdPdfStampString {
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

    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl Parse for BdPdfStampString {
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

/// Specified value of `-bd-pdf-stamp-intent`.
///
/// `auto | stamp-image | stamp-snapshot | <string>` per ISO 32000-2
/// §12.5.6.2 Table 168 `/IT` keyword space. Custom intent names are
/// admitted (the spec allows author-defined intent identifiers) but
/// only round-trip through viewers that recognise them.
///
/// `auto` (the initial) suppresses the `/IT` entry.
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
#[allow(missing_docs)]
pub enum BdPdfStampIntent {
    /// `auto` — no `/IT` entry written (initial).
    Auto,
    /// `/StampImage` — the stamp renders an image annotation glyph.
    StampImage,
    /// `/StampSnapshot` — the stamp captures a snapshot of the
    /// underlying page content.
    StampSnapshot,
    /// Author-defined intent name — emitted verbatim as a PDF name.
    Custom(OwnedStr),
}

impl Default for BdPdfStampIntent {
    #[inline]
    fn default() -> Self {
        Self::Auto
    }
}

impl BdPdfStampIntent {
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

impl Parse for BdPdfStampIntent {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        if input
            .try_parse(|i| i.expect_ident_matching("stamp-image"))
            .is_ok()
        {
            return Ok(Self::StampImage);
        }
        if input
            .try_parse(|i| i.expect_ident_matching("stamp-snapshot"))
            .is_ok()
        {
            return Ok(Self::StampSnapshot);
        }
        let s = input.expect_string()?;
        Ok(Self::Custom(s.as_ref().to_owned().into()))
    }
}
