/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A top-level `@color-profile --name { … }` rule (CSS Color 5 §7).
//!
//! Authors register a custom ICC profile against a `<dashed-ident>`
//! name so subsequent `color(<dashed-ident> ...)` references and the
//! standard `output-color-model: <dashed-ident>` value resolve against
//! the declared profile.
//!
//! ```css
//! @color-profile --my-icc {
//!   src: url("./icc/CoatedFOGRA39.icc");
//!   rendering-intent: relative-colorimetric;
//!   components: cyan magenta yellow black;
//! }
//! ```
//!
//! ## Rationale for hand-rolled descriptor parsing
//!
//! The descriptor space is small and tightly bounded (three descriptors
//! today), and none of the existing property-longhand machinery applies
//! cleanly — `src` accepts a single `<url>`, `rendering-intent` is a
//! restricted keyword set, and `components` is a space-separated
//! `<custom-ident>` list bracketed by literal `[` / `]`. A hand-rolled
//! `RuleBodyItemParser` keeps the at-rule self-contained inside this
//! module and avoids polluting `longhands.toml` with descriptor-only
//! entries.

use crate::derives::*;
use crate::error_reporting::ContextualParseError;
use crate::parser::{Parse, ParserContext};
use crate::shared_lock::{DeepCloneWithLock, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::values::specified::url::SpecifiedUrl;
use crate::values::{AtomIdent, CustomIdent};
use cssparser::{
    match_ignore_ascii_case, AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser,
    ParserState, QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, SourceLocation,
};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOf, MallocSizeOfOps};
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, StyleParseErrorKind};

/// The rendering intent declared inside an `@color-profile` block.
///
/// The four ICC rendering intents per ICC.1:2010 §6.1.5. `auto` means
/// "no intent declared" so the conversion pipeline picks the engine
/// default (typically `relative-colorimetric` for proof / print work
/// and `perceptual` for display).
#[derive(Clone, Copy, Debug, PartialEq, ToShmem)]
pub enum ColorProfileRenderingIntent {
    /// Authors did not declare an intent; the IR boundary derives one.
    Auto,
    /// `perceptual` — preserve overall colour appearance.
    Perceptual,
    /// `relative-colorimetric` — preserve in-gamut colours exactly;
    /// clamp out-of-gamut colours.
    RelativeColorimetric,
    /// `saturation` — preserve saturation, sacrifice hue / lightness.
    Saturation,
    /// `absolute-colorimetric` — preserve in-gamut colours exactly
    /// against the source white point.
    AbsoluteColorimetric,
}

impl ColorProfileRenderingIntent {
    fn parse_keyword<'i, 't>(
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i, StyleParseErrorKind<'i>>> {
        let location = input.current_source_location();
        let ident = input.expect_ident_cloned()?;
        match_ignore_ascii_case! { &ident,
            "perceptual" => Ok(Self::Perceptual),
            "relative-colorimetric" => Ok(Self::RelativeColorimetric),
            "saturation" => Ok(Self::Saturation),
            "absolute-colorimetric" => Ok(Self::AbsoluteColorimetric),
            _ => Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
        }
    }
}

impl style_traits::ToCss for ColorProfileRenderingIntent {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str(match self {
            Self::Auto => "auto",
            Self::Perceptual => "perceptual",
            Self::RelativeColorimetric => "relative-colorimetric",
            Self::Saturation => "saturation",
            Self::AbsoluteColorimetric => "absolute-colorimetric",
        })
    }
}

/// A `@color-profile --name { … }` rule.
///
/// `name` is the `<dashed-ident>` profile name (preserved verbatim,
/// including its leading `--`, so cascade readers can match the
/// `output-color-model: <dashed-ident>` value byte-for-byte).
///
/// `src` is the URL of the ICC profile blob the renderer is expected
/// to fetch and embed. The body parser rejects rules without a `src:`
/// descriptor — a profile reference cannot resolve without a fetchable
/// payload.
///
/// `rendering_intent` and `components` are optional descriptors.
#[derive(Clone, Debug, ToShmem)]
pub struct ColorProfileRule {
    /// `<dashed-ident>` profile name. Stored as an `AtomIdent` whose
    /// string preserves the leading `--`.
    pub name: AtomIdent,
    /// URL of the ICC profile blob declared via `src:`. `None` only
    /// during parser accumulation — the body parser rejects rules
    /// with no `src` descriptor.
    pub src: Option<SpecifiedUrl>,
    /// Rendering intent declared via `rendering-intent:`. Defaults to
    /// [`ColorProfileRenderingIntent::Auto`] (derived by the IR
    /// boundary from the profile class).
    pub rendering_intent: ColorProfileRenderingIntent,
    /// Channel labels declared via `components:`. Empty when the
    /// descriptor is omitted; the IR boundary derives default labels
    /// from the profile's colour-space class.
    pub components: Vec<CustomIdent>,
    /// The source position of the at-rule for diagnostics.
    pub source_location: SourceLocation,
}

impl ColorProfileRule {
    /// Construct an empty rule used as a parser accumulator.
    fn empty(name: AtomIdent, source_location: SourceLocation) -> Self {
        Self {
            name,
            src: None,
            rendering_intent: ColorProfileRenderingIntent::Auto,
            components: Vec::new(),
            source_location,
        }
    }

    /// Gets the CSS rule name for this rule.
    #[inline]
    pub fn name_token(&self) -> &'static str {
        "color-profile"
    }

    /// Heap-size measurement.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, _guard: &SharedRwLockReadGuard, _ops: &mut MallocSizeOfOps) -> usize {
        0
    }
}

impl ToCssWithGuard for ColorProfileRule {
    fn to_css(&self, _guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        use style_traits::ToCss;

        dest.write_str("@color-profile ")?;
        // `AtomIdent`'s `Display` would re-serialise but
        // `serialize_atom_identifier` is the canonical hook for
        // `<dashed-ident>` round-tripping.
        crate::values::serialize_atom_identifier(&self.name.0, dest)?;
        dest.write_str(" { ")?;
        if let Some(ref src) = self.src {
            dest.write_str("src: ")?;
            src.to_css(&mut style_traits::CssWriter::new(dest))?;
            dest.write_str("; ")?;
        }
        if !matches!(self.rendering_intent, ColorProfileRenderingIntent::Auto) {
            dest.write_str("rendering-intent: ")?;
            self.rendering_intent
                .to_css(&mut style_traits::CssWriter::new(dest))?;
            dest.write_str("; ")?;
        }
        if !self.components.is_empty() {
            dest.write_str("components: ")?;
            let mut first = true;
            for ident in &self.components {
                if !first {
                    dest.write_char(' ')?;
                }
                first = false;
                ident.to_css(&mut style_traits::CssWriter::new(dest))?;
            }
            dest.write_str("; ")?;
        }
        dest.write_char('}')
    }
}

impl DeepCloneWithLock for ColorProfileRule {
    fn deep_clone_with_lock(&self, _lock: &SharedRwLock, _guard: &SharedRwLockReadGuard) -> Self {
        // No `Locked<>` payloads — `Clone` is enough.
        self.clone()
    }
}

/// Parse the prelude of an `@color-profile` rule. Returns the
/// `<dashed-ident>` profile name and rejects any tokens after it.
///
/// The spec requires the prelude to be a single `<dashed-ident>`. The
/// special `device-cmyk` reserved name is intentionally accepted here
/// (mirrors Chromium's parsing) — the IR boundary inspects the name to
/// route the `device-cmyk()` fallback profile when authored.
pub fn parse_color_profile_name<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<AtomIdent, ParseError<'i, StyleParseErrorKind<'i>>> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?.clone();
    if !ident.starts_with("--") && !ident.eq_ignore_ascii_case("device-cmyk") {
        return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    let name = AtomIdent::from(&*ident);
    input.expect_exhausted()?;
    Ok(name)
}

/// Parse the body (inside `{}`) of an `@color-profile` rule.
///
/// Recognised descriptors:
/// - `src` — `<url>`. Mandatory.
/// - `rendering-intent` — `perceptual | relative-colorimetric |
///   saturation | absolute-colorimetric`. Optional.
/// - `components` — `<ident>#` (space-separated channel labels).
///   Optional; CSS Color 5 spec defines the form as `[ <ident># ]?`.
///
/// Unknown descriptors are logged via `ContextualParseError::
/// UnsupportedRule` and discarded, mirroring how `@counter-style`
/// handles unsupported descriptors.
pub fn parse_color_profile_body<'i, 't>(
    context: &ParserContext,
    name: AtomIdent,
    input: &mut Parser<'i, 't>,
    location: SourceLocation,
) -> Result<ColorProfileRule, ParseError<'i, StyleParseErrorKind<'i>>> {
    let mut rule = ColorProfileRule::empty(name, location);
    let start = input.current_source_location();
    {
        let mut parser = ColorProfileRuleParser {
            context,
            rule: &mut rule,
        };
        let mut iter = RuleBodyParser::new(input, &mut parser);
        while let Some(declaration) = iter.next() {
            if let Err((error, slice)) = declaration {
                let loc = error.location;
                let err = ContextualParseError::UnsupportedRule(slice, error);
                context.log_css_error(loc, err);
            }
        }
    }
    if rule.src.is_none() {
        context.log_css_error(
            start,
            ContextualParseError::UnsupportedRule(
                "@color-profile without src descriptor",
                input.new_custom_error::<_, StyleParseErrorKind<'i>>(
                    StyleParseErrorKind::UnspecifiedError,
                ),
            ),
        );
        return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    Ok(rule)
}

struct ColorProfileRuleParser<'a, 'b: 'a> {
    context: &'a ParserContext<'b>,
    rule: &'a mut ColorProfileRule,
}

impl<'a, 'b, 'i> AtRuleParser<'i> for ColorProfileRuleParser<'a, 'b> {
    type Prelude = ();
    type AtRule = ();
    type Error = StyleParseErrorKind<'i>;
}

impl<'a, 'b, 'i> QualifiedRuleParser<'i> for ColorProfileRuleParser<'a, 'b> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = StyleParseErrorKind<'i>;
}

impl<'a, 'b, 'i> RuleBodyItemParser<'i, (), StyleParseErrorKind<'i>>
    for ColorProfileRuleParser<'a, 'b>
{
    fn parse_qualified(&self) -> bool {
        false
    }
    fn parse_declarations(&self) -> bool {
        true
    }
}

impl<'a, 'b, 'i> DeclarationParser<'i> for ColorProfileRuleParser<'a, 'b> {
    type Declaration = ();
    type Error = StyleParseErrorKind<'i>;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _start: &ParserState,
    ) -> Result<(), ParseError<'i, Self::Error>> {
        match_ignore_ascii_case! { &name,
            "src" => {
                let url = SpecifiedUrl::parse(self.context, input)?;
                input.expect_exhausted()?;
                self.rule.src = Some(url);
                Ok(())
            },
            "rendering-intent" => {
                let intent = ColorProfileRenderingIntent::parse_keyword(input)?;
                input.expect_exhausted()?;
                self.rule.rendering_intent = intent;
                Ok(())
            },
            "components" => {
                // `[ <custom-ident># ]?` — a space-separated list of
                // channel labels (e.g. `cyan magenta yellow black` for
                // a CMYK profile). Stored as a `Vec<CustomIdent>` so
                // the IR boundary can validate uniqueness without
                // re-parsing.
                let mut components = Vec::new();
                while !input.is_exhausted() {
                    components.push(CustomIdent::parse(input, &[])?);
                }
                if !components.is_empty() {
                    self.rule.components = components;
                }
                Ok(())
            },
            _ => Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
        }
    }
}

#[cfg(feature = "gecko")]
impl MallocSizeOf for ColorProfileRule {
    fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
        0
    }
}
