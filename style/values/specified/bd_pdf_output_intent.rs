/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe PDF/X output intent + colour-conversion CSS surface (F1).
//!
//! These document-level properties select an ICC profile and colour
//! handling policy for a PDF/X workflow. The moegoe renderer applies
//! them when the cascaded `-bd-pdf-conformance` selects a PDF/X
//! flavour, but the properties parse and compute independently so
//! authors can declare them ahead of the conformance toggle. As with
//! the existing G4 properties, the renderer honours only `:root`
//! declarations.
//!
//! Properties:
//!
//! * `-bd-pdf-output-intent` — `auto | none | <icc-profile-name> |
//!   url(<profile-url>)`. Registers the OutputIntent dictionary in
//!   the document catalog. Mandatory for PDF/X conformance.
//!   (PDFreactor OutputIntent API, `pdfreactor.md:3204`;
//!   Prince `-prince-pdf-output-intent`, `prince.md:8911`.)
//! * `-bd-pdf-fallback-cmyk-profile` — `none | url(...)`. Fallback
//!   ICC profile applied to `device-cmyk()` colours when no
//!   OutputIntent is registered. (Prince
//!   `-prince-fallback-cmyk-profile`, `prince.md:7007`.)
//! * `-bd-pdf-colour-conversion` — `auto | none | content-only |
//!   force-cmyk | force-rgb | force-grey | force-spot`. Instructs
//!   the renderer how to project source colours into the output
//!   intent's working space. (Prince
//!   `-prince-pdf-color-conversion`, `prince.md:8582`.)
//! * `-bd-pdf-colour-options` — `[ use-true-black | preserve-black ]#`.
//!   Per-flag PDF/X colour handling options. `use-true-black` carries
//!   the Prince spec warrant (`-prince-pdf-color-options`,
//!   `prince.md:8647`); `preserve-black` is the moegoe extension that
//!   prevents black-channel conversion in the colour-management
//!   pipeline. `use-true-white` and `preserve-overprint` were dropped
//!   on 2026-05-17 — the former had no upstream warrant in any
//!   reference renderer, and the latter collided with the dedicated
//!   `-bd-pdf-overprint: preserve` longhand (one fact, one place).

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::url::SpecifiedUrl;
use crate::values::AtomIdent;
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// Specified value of `-bd-pdf-output-intent`.
///
/// `auto` (initial) registers the bladsy validator's recommended
/// profile when one exists; `none` clears any registered profile.
/// `Named` carries a registered colour-space identifier
/// (`sRGB IEC61966-2.1`, `FOGRA39`, ...); `Url` references an ICC
/// profile blob the renderer is expected to fetch and embed.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfOutputIntent {
    /// `auto` — defer to the conformance default.
    Auto,
    /// `none` — clear any registered OutputIntent.
    None,
    /// `<icc-profile-name>` — well-known profile identifier.
    Named(AtomIdent),
    /// `url(<profile-url>)` — fetched and embedded as an OutputIntent.
    Url(SpecifiedUrl),
}

impl BdPdfOutputIntent {
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

impl ToCss for BdPdfOutputIntent {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::None => dest.write_str("none"),
            Self::Named(name) => name.to_css(dest),
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl Parse for BdPdfOutputIntent {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(url) = input.try_parse(|i| SpecifiedUrl::parse(context, i)) {
            return Ok(Self::Url(url));
        }
        if let Ok(s) = input.try_parse(|i| i.expect_string().map(|s| s.as_ref().to_owned())) {
            return Ok(Self::Named(AtomIdent::from(s.as_str())));
        }
        let ident = input.expect_ident()?;
        let ident_value = ident.as_ref().to_owned();
        Ok(match_ignore_ascii_case! { &ident_value,
            "auto" => Self::Auto,
            "none" => Self::None,
            _ => Self::Named(AtomIdent::from(ident_value.as_str()))
        })
    }
}

/// Specified value of `-bd-pdf-fallback-cmyk-profile`.
///
/// `none` (initial) leaves CMYK colours unconverted. `Url` provides
/// an ICC profile blob the renderer embeds and applies to all
/// `device-cmyk()` values when no OutputIntent profile is present.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfFallbackCmykProfile {
    /// `none` — no fallback profile.
    None,
    /// `url(<profile-url>)` — fetched, embedded, and applied to
    /// CMYK colours.
    Url(SpecifiedUrl),
}

impl BdPdfFallbackCmykProfile {
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

impl ToCss for BdPdfFallbackCmykProfile {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl Parse for BdPdfFallbackCmykProfile {
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

/// Specified value of `-bd-pdf-colour-conversion`.
///
/// Each keyword maps to a renderer-internal colour-conversion mode.
/// `auto` (initial) lets the conformance dictate the policy;
/// `content-only` converts only declared content colours, leaving
/// page-mark colours alone; the `force-*` family overrides every
/// pipeline colour into the named working space.
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
pub enum BdPdfColourConversion {
    #[default]
    Auto,
    None,
    ContentOnly,
    ForceCmyk,
    ForceRgb,
    ForceGrey,
    ForceSpot,
}

/// One option flag in `-bd-pdf-colour-options`.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
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
pub enum BdPdfColourOption {
    UseTrueBlack,
    PreserveBlack,
}

/// Specified value of `-bd-pdf-colour-options`.
///
/// Bitset of [`BdPdfColourOption`] entries. `none` (initial) clears
/// the set; otherwise the property accepts a comma-separated list of
/// option keywords.
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
pub struct BdPdfColourOptions {
    /// Bitset of `BdPdfColourOption` (1 << variant).
    pub flags: u8,
}

impl BdPdfColourOptions {
    /// Initial value (no flags set).
    #[inline]
    pub fn none() -> Self {
        Self { flags: 0 }
    }

    /// Whether the bitset has no flags set.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.flags == 0
    }

    /// Whether the bitset includes the given option.
    #[inline]
    pub fn contains(&self, option: BdPdfColourOption) -> bool {
        (self.flags & flag_bit(option)) != 0
    }

    fn insert(&mut self, option: BdPdfColourOption) {
        self.flags |= flag_bit(option);
    }
}

#[inline]
fn flag_bit(option: BdPdfColourOption) -> u8 {
    1 << (option as u8)
}

impl ToCss for BdPdfColourOptions {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        if self.is_empty() {
            return dest.write_str("none");
        }
        let mut first = true;
        for option in [
            BdPdfColourOption::UseTrueBlack,
            BdPdfColourOption::PreserveBlack,
        ] {
            if self.contains(option) {
                if !first {
                    dest.write_str(", ")?;
                }
                option.to_css(dest)?;
                first = false;
            }
        }
        Ok(())
    }
}

impl Parse for BdPdfColourOptions {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::none());
        }
        let mut value = Self::none();
        let first = BdPdfColourOption::parse(input)?;
        value.insert(first);
        while input.try_parse(|i| i.expect_comma()).is_ok() {
            let option = BdPdfColourOption::parse(input)?;
            if value.contains(option) {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            value.insert(option);
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::ParsingMode;

    fn parse_options(css: &str) -> BdPdfColourOptions {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = ParserContext::new(
            Origin::Author,
            &url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        );
        let mut input = ParserInput::new(css);
        let mut parser = Parser::new(&mut input);
        parser
            .parse_entirely(|input| BdPdfColourOptions::parse(&context, input))
            .expect("colour options should parse")
    }

    #[test]
    fn colour_options_round_trip() {
        for (css, expected) in [
            ("none", "none"),
            ("use-true-black", "use-true-black"),
            ("preserve-black", "preserve-black"),
            (
                "use-true-black, preserve-black",
                "use-true-black, preserve-black",
            ),
        ] {
            let value = parse_options(css);
            assert_eq!(value.to_css_string(), expected);
        }
    }
}
