/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Color Module Level 5 longhands.
//!
//! Implements the document-level `output-color-model` property (§7).
//! <https://drafts.csswg.org/css-color-5/#output-color-model>
//!
//! Grammar: `auto | <name>` where `<name>` is one of the well-known
//! predefined colour spaces (`srgb`, `srgb-linear`, `display-p3`,
//! `a98-rgb`, `prophoto-rgb`, `rec2020`) or a `<dashed-ident>` naming
//! an `@color-profile` block declared in the document. Selects the
//! preferred output colour space for the rendered document.
//!
//! `auto` (initial) — the renderer picks the output colour space from
//! the active `-bd-pdf-conformance` profile and the document's
//! `@color-profile` declarations.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::AtomIdent;
use cssparser::{match_ignore_ascii_case, Parser};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// One of the well-known predefined colour spaces recognised by
/// `output-color-model`. The list mirrors the predefined RGB
/// colour-space tokens accepted by `color(<colorspace> ...)` in CSS
/// Color 4 §10.
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
pub enum PredefinedOutputColourSpace {
    /// `srgb`.
    Srgb,
    /// `srgb-linear`.
    SrgbLinear,
    /// `display-p3`.
    DisplayP3,
    /// `a98-rgb`.
    A98Rgb,
    /// `prophoto-rgb`.
    ProphotoRgb,
    /// `rec2020`.
    Rec2020,
}

/// Specified value of the `output-color-model` property (§7).
///
/// `auto` (initial) — defer to the active conformance profile and any
/// declared `@color-profile`. `Predefined(<name>)` selects one of the
/// well-known colour spaces. `Custom(--name)` references an
/// `@color-profile --name { … }` block declared in the document.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum OutputColorModel {
    /// `auto` — defer to the conformance profile.
    Auto,
    /// One of the predefined RGB colour spaces.
    Predefined(PredefinedOutputColourSpace),
    /// `<dashed-ident>` — references a custom `@color-profile`.
    /// The ident is preserved with its leading `--` so cascade
    /// readers can match the declared profile name verbatim.
    Custom(AtomIdent),
}

impl OutputColorModel {
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

impl ToCss for OutputColorModel {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Predefined(p) => p.to_css(dest),
            Self::Custom(name) => dest.write_str(name.as_ref()),
        }
    }
}

impl Parse for OutputColorModel {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let ident = input.expect_ident_cloned()?;
        match_ignore_ascii_case! { &ident,
            "auto" => return Ok(Self::Auto),
            "srgb" => return Ok(Self::Predefined(PredefinedOutputColourSpace::Srgb)),
            "srgb-linear" => return Ok(Self::Predefined(PredefinedOutputColourSpace::SrgbLinear)),
            "display-p3" => return Ok(Self::Predefined(PredefinedOutputColourSpace::DisplayP3)),
            "a98-rgb" => return Ok(Self::Predefined(PredefinedOutputColourSpace::A98Rgb)),
            "prophoto-rgb" => return Ok(Self::Predefined(PredefinedOutputColourSpace::ProphotoRgb)),
            "rec2020" => return Ok(Self::Predefined(PredefinedOutputColourSpace::Rec2020)),
            _ => {},
        }
        if ident.starts_with("--") {
            return Ok(Self::Custom(AtomIdent::from(&*ident)));
        }
        Err(location.new_unexpected_token_error(cssparser::Token::Ident(ident)))
    }
}
