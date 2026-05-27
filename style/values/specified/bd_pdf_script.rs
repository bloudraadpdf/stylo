/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-script` and `-bd-pdf-event-scripts` properties (G64).
//!
//! Native moegoe fork-extension surface for PDF document-level
//! JavaScript. These properties register JavaScript that the PDF
//! reader executes — moegoe does NOT invoke a JavaScript engine at
//! render time for these; the bytes are embedded into the resulting
//! PDF and run by the viewer at the appropriate event.
//!
//! - [`BdPdfScript`] (`-bd-pdf-script`) — `none | <string>+`. Each
//!   string is registered as a uniquely-named entry on the document
//!   catalogue's `/Names /JavaScript` name tree (ISO 32000-2
//!   §12.6.4.16). Accumulates across declarations in document order.
//!   Mirrors Prince's `prince-pdf-script: <JavaScript>`.
//! - [`BdPdfEventScripts`] (`-bd-pdf-event-scripts`) — `none |
//!   <event-spec>#`, where `<event-spec> = <event-name>(<string>)`
//!   and `<event-name>` is one of `wc` (Will Close), `ws` (Will
//!   Save), `ds` (Did Save), `wp` (Will Print), `dp` (Did Print).
//!   Each spec attaches to the document catalogue's `/AA`
//!   additional-actions dictionary under the corresponding PDF
//!   event key (ISO 32000-2 §12.6.3 Table 200). Mirrors Prince's
//!   `prince-pdf-event-scripts: <event> <JavaScript>, ...`.
//!
//! The cascade reader only honours declarations on `:root`. The
//! renderer is byte-passthrough — script source is written verbatim
//! into the PDF action dictionary's `/JS` entry without any
//! evaluation or sanitisation.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::{OwnedSlice, OwnedStr};
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::Write;
use style_traits::{ParseError, StyleParseErrorKind};

/// Specified value of `-bd-pdf-script`.
///
/// `none` clears the slot. `<string>+` contributes one or more
/// JavaScript source strings; the renderer registers each one on
/// the PDF catalogue's `/Names /JavaScript` name tree under a
/// uniquely synthesised name and accumulates across declarations
/// in document order.
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
pub enum BdPdfScript {
    /// `none` — no document-level scripts contributed.
    None,
    /// `<string>+` — one or more JavaScript source strings.
    Strings(#[css(iterable)] OwnedSlice<OwnedStr>),
}

impl BdPdfScript {
    /// `none` value (initial).
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

impl Parse for BdPdfScript {
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
        let mut strings: Vec<OwnedStr> = Vec::new();
        loop {
            match input.try_parse(|i| -> Result<OwnedStr, ParseError<'i>> {
                let s = i.expect_string()?;
                Ok(s.as_ref().to_owned().into())
            }) {
                Ok(s) => strings.push(s),
                Err(_) => break,
            }
        }
        if strings.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Strings(OwnedSlice::from(strings)))
    }
}

/// PDF document-level additional-action event keys (ISO 32000-2
/// §12.6.3 Table 200). The keyword form (`wc`, `ws`, ...) matches
/// the PDF event-key letters exactly (lowercased) so the cascade
/// reader does not need a translation table.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum BdPdfEventKind {
    /// `wc` — Will Close (`/WC`).
    Wc,
    /// `ws` — Will Save (`/WS`).
    Ws,
    /// `ds` — Did Save (`/DS`).
    Ds,
    /// `wp` — Will Print (`/WP`).
    Wp,
    /// `dp` — Did Print (`/DP`).
    Dp,
}

impl BdPdfEventKind {
    /// Lowercase event name as written in CSS.
    #[inline]
    pub fn as_ident(self) -> &'static str {
        match self {
            Self::Wc => "wc",
            Self::Ws => "ws",
            Self::Ds => "ds",
            Self::Wp => "wp",
            Self::Dp => "dp",
        }
    }
}

impl style_traits::ToCss for BdPdfEventKind {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> std::fmt::Result
    where
        W: std::fmt::Write,
    {
        dest.write_str(self.as_ident())
    }
}

/// One event-spec — pairs an event keyword with the JavaScript source
/// to register under that event in the document catalogue's `/AA`.
///
/// Serialises as `wc("source")` — function token with one quoted
/// string argument. Parsing accepts the same shape; the function
/// name must match a known event keyword case-insensitively.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C)]
pub struct BdPdfEventScript {
    /// PDF event key (`/WC` / `/WS` / `/DS` / `/WP` / `/DP`).
    pub event: BdPdfEventKind,
    /// JavaScript source to attach to the event.
    pub script: OwnedStr,
}

impl style_traits::ToCss for BdPdfEventScript {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> std::fmt::Result
    where
        W: std::fmt::Write,
    {
        dest.write_str(self.event.as_ident())?;
        dest.write_char('(')?;
        style_traits::ToCss::to_css(&self.script, dest)?;
        dest.write_char(')')
    }
}

/// Specified value of `-bd-pdf-event-scripts`.
///
/// `none` clears the slot. The non-`none` form is a comma-separated
/// list of `<event-name>(<string>)` specs; multiple specs may target
/// the same event, in which case later declarations win (PDF's `/AA`
/// is a dictionary keyed by event).
///
/// `ToCss` is implemented manually because `derive(ToCss)` with
/// `#[css(iterable)]` separates items by whitespace, whereas the
/// grammar requires comma separation. The struct-level
/// `#[css(comma)]` attribute is reserved for tuple-like inputs.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BdPdfEventScripts {
    /// `none` — no event scripts contributed.
    None,
    /// `<event-spec>#` — comma-separated list of event/script pairs.
    Specs(OwnedSlice<BdPdfEventScript>),
}

impl style_traits::ToCss for BdPdfEventScripts {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> std::fmt::Result
    where
        W: std::fmt::Write,
    {
        match self {
            Self::None => dest.write_str("none"),
            Self::Specs(specs) => {
                let mut first = true;
                for spec in specs.iter() {
                    if !first {
                        dest.write_str(", ")?;
                    }
                    style_traits::ToCss::to_css(spec, dest)?;
                    first = false;
                }
                Ok(())
            }
        }
    }
}

impl BdPdfEventScripts {
    /// `none` value (initial).
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

impl Parse for BdPdfEventScripts {
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
        let specs = input.parse_comma_separated(|inner| -> Result<BdPdfEventScript, ParseError<'i>> {
            let location = inner.current_source_location();
            let function = inner.expect_function()?.clone();
            let event = match_ignore_ascii_case! { &function,
                "wc" => BdPdfEventKind::Wc,
                "ws" => BdPdfEventKind::Ws,
                "ds" => BdPdfEventKind::Ds,
                "wp" => BdPdfEventKind::Wp,
                "dp" => BdPdfEventKind::Dp,
                _ => return Err(location.new_custom_error::<_, StyleParseErrorKind>(
                    StyleParseErrorKind::UnexpectedFunction(function.clone()),
                )),
            };
            let script = inner.parse_nested_block(|nested| -> Result<OwnedStr, ParseError<'i>> {
                let s = nested.expect_string()?;
                Ok(s.as_ref().to_owned().into())
            })?;
            Ok(BdPdfEventScript { event, script })
        })?;
        if specs.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Specs(OwnedSlice::from(specs)))
    }
}

/// Specified value of the per-widget AcroForm `/AA` JavaScript
/// trio extension — `-bd-pdf-calculate`, `-bd-pdf-focus`, and
/// `-bd-pdf-blur`. These cascade onto AcroForm widget annotations
/// and project onto krilla's `WidgetAnnotation::with_*_action`
/// setters at PDF emission time, populating the `/AA /C`, `/AA /Fo`,
/// and `/AA /Bl` entries per ISO 32000-2 §12.6.4.16 Table 230.
///
/// `none` (initial) leaves the corresponding `/AA` slot unset.
/// A `<string>` value is written verbatim as the
/// `Action /JavaScript /JS` body — moegoe does NOT invoke a
/// JavaScript engine here; the bytes are passed through to the
/// PDF viewer which executes them at the corresponding event.
///
/// Non-inherited: each widget reads the value at its own host
/// element. Author-provided values override any HTML-input-type
/// defaults the convert layer assigns (`AFNumber_Keystroke` etc.).
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
pub enum BdPdfWidgetActionScript {
    /// `none` — the `/AA` slot is omitted from the widget's
    /// additional-actions dictionary.
    None,
    /// `<string>` — JavaScript source written verbatim into the
    /// `/AA /<key> /JS` entry.
    Literal(OwnedStr),
}

impl BdPdfWidgetActionScript {
    /// `none` value (initial).
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

impl Parse for BdPdfWidgetActionScript {
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
