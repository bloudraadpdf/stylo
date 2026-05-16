/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A top-level `@-bd-colour <name> { … }` rule.
//!
//! moegoe Family 2 — named-spot colour declaration. Authors register a
//! colorant name and its tint/alternate colour-space against the document
//! so subsequent `-bd-spot(<name>)` / `-bd-separation(<name>)` references
//! resolve to a PDF Separation colour space (ISO 32000-2 §8.6.6.4).
//!
//! ```css
//! @-bd-colour PANTONE-185 {
//!   colour-values: device-cmyk(0 1 0.55 0);
//!   alternate: cmyk;          /* optional, defaults to "cmyk" */
//! }
//! ```
//!
//! ## Rationale for hand-rolled descriptor parsing
//!
//! The descriptor space is tiny (two recognised descriptors today), and
//! none of the existing property-longhand machinery applies cleanly —
//! `colour-values` accepts a single `<color>` and `alternate` accepts a
//! restricted keyword set. A hand-rolled `RuleBodyItemParser` keeps the
//! at-rule self-contained inside this module and avoids polluting
//! `longhands.toml` with `restricted_to_at_rule = "bd-colour"` entries
//! that would only be parsed inside this single context.
//!
//! ## Prince / PDFreactor authoring compatibility
//!
//! - Prince's `@prince-color <name> { … }` and PDFreactor's
//!   `@-ro-spot-color <name> { … }` are translated into `@-bd-colour`
//!   in `moegoe-css/src/compat/translate.rs` (planned). The native
//!   form is the canonical surface — translation is a thin
//!   text-rewrite, not a parallel AST path.
//! - The descriptor name is `colour-values` (British spelling matches
//!   the surrounding `-bd-colour` family); the American
//!   `color-values` alias is accepted to ease migration from
//!   Prince/PDFreactor authoring conventions.

use crate::derives::*;
use crate::error_reporting::ContextualParseError;
use crate::parser::{Parse, ParserContext};
use crate::shared_lock::{DeepCloneWithLock, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::values::specified::Color as SpecifiedColor;
use crate::values::AtomIdent;
use cssparser::{
    match_ignore_ascii_case, AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser,
    ParserState, QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, SourceLocation,
};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOf, MallocSizeOfOps};
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, StyleParseErrorKind};

/// The alternate colour space declared for a spot colour.
///
/// PDF 32000-2 §8.6.6.4 requires every Separation colour space to name
/// an alternate colour space that consumers fall back to when the named
/// colorant is unavailable. moegoe currently always emits CMYK as the
/// alternate (matching `@-bd-colour <name> { colour-values: device-cmyk(...) }`
/// authoring), but the enum is kept open so future authoring can declare
/// an sRGB-defined separation without breaking the rule shape.
#[derive(Clone, Copy, Debug, PartialEq, ToShmem)]
pub enum BdColourAlternateKind {
    /// Authors did not declare an alternate; the IR boundary derives one
    /// from `colour-values` (CMYK from `device-cmyk()`, otherwise sRGB).
    Auto,
    /// `alternate: cmyk` — emit a `DeviceCMYK` alternate. This is the
    /// default when `colour-values` is a CMYK colour.
    Cmyk,
    /// `alternate: rgb` — emit a `DeviceRGB` alternate (currently
    /// rejected by the IR boundary with a diagnostic; PDF/X workflows
    /// rarely want RGB alternates).
    Rgb,
}

impl BdColourAlternateKind {
    fn parse_keyword<'i, 't>(
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i, StyleParseErrorKind<'i>>> {
        let location = input.current_source_location();
        let ident = input.expect_ident_cloned()?;
        match_ignore_ascii_case! { &ident,
            "cmyk" => Ok(BdColourAlternateKind::Cmyk),
            "rgb" => Ok(BdColourAlternateKind::Rgb),
            _ => Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
        }
    }
}

impl style_traits::ToCss for BdColourAlternateKind {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str(match self {
            BdColourAlternateKind::Auto => "auto",
            BdColourAlternateKind::Cmyk => "cmyk",
            BdColourAlternateKind::Rgb => "rgb",
        })
    }
}

/// A `@-bd-colour <name> { … }` rule.
///
/// `name` is the colorant name registered against the document — PDF
/// 32000-2 §8.6.6.4 requires this to match the `/N` colorant entry in
/// the Separation colour space dictionary verbatim, so the parser
/// preserves the authored case (no ASCII-lowercasing).
///
/// `values` is the typed `<color>` declared via `colour-values:`. The
/// IR boundary projects this to a CMYK tint (for `device-cmyk()`),
/// preserves the sRGB fallback, and indexes the entry under `name`
/// inside [`moegoe_ir::SpotColourRegistry`].
///
/// `alternate` selects the PDF alternate colour space:
/// `BdColourAlternateKind::Auto` (the parser default) lets the IR
/// boundary derive the alternate from the `colour-values` colour
/// space. `Cmyk` / `Rgb` force the declared alternate regardless of
/// the tint colour space.
#[derive(Clone, Debug, ToShmem)]
pub struct BdColourRule {
    /// The colorant name. Stored as an `AtomIdent` so equality
    /// matches the cascade reader's lookup key and the OM serialisation
    /// round-trip preserves authored case.
    pub name: AtomIdent,
    /// The tint colour declared via the `colour-values:` descriptor.
    /// `None` when the descriptor was omitted (an empty `@-bd-colour`
    /// block is a parse error but the field is `Option` so the parser
    /// can hold a partial value during descriptor accumulation).
    pub values: Option<SpecifiedColor>,
    /// The alternate colour space declared via the `alternate:`
    /// descriptor. Defaults to [`BdColourAlternateKind::Auto`] (derived
    /// at IR time from the `values` colour space).
    pub alternate: BdColourAlternateKind,
    /// The source position of the at-rule for diagnostics.
    pub source_location: SourceLocation,
}

impl BdColourRule {
    /// Construct an empty rule used as a parser accumulator.
    fn empty(name: AtomIdent, source_location: SourceLocation) -> Self {
        Self {
            name,
            values: None,
            alternate: BdColourAlternateKind::Auto,
            source_location,
        }
    }

    /// Gets the CSS rule name for this rule.
    #[inline]
    pub fn name_token(&self) -> &'static str {
        "-bd-colour"
    }

    /// Heap-size measurement. The `values` field's `SpecifiedColor`
    /// has no measured heap allocations (currently); future-proof.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, _guard: &SharedRwLockReadGuard, _ops: &mut MallocSizeOfOps) -> usize {
        0
    }
}

impl ToCssWithGuard for BdColourRule {
    fn to_css(&self, _guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        use style_traits::ToCss;

        dest.write_str("@-bd-colour ")?;
        crate::values::serialize_atom_identifier(&self.name.0, dest)?;
        dest.write_str(" { ")?;
        if let Some(ref values) = self.values {
            dest.write_str("colour-values: ")?;
            values.to_css(&mut style_traits::CssWriter::new(dest))?;
            dest.write_str("; ")?;
        }
        if !matches!(self.alternate, BdColourAlternateKind::Auto) {
            dest.write_str("alternate: ")?;
            self.alternate
                .to_css(&mut style_traits::CssWriter::new(dest))?;
            dest.write_str("; ")?;
        }
        dest.write_char('}')
    }
}

impl DeepCloneWithLock for BdColourRule {
    fn deep_clone_with_lock(
        &self,
        _lock: &SharedRwLock,
        _guard: &SharedRwLockReadGuard,
    ) -> Self {
        // `SpecifiedColor` and the rest are `Clone` without lock
        // shenanigans — there's no `Locked<>` payload in this rule.
        self.clone()
    }
}

/// Parse the prelude of an `@-bd-colour` rule. Returns the colorant
/// name and rejects any tokens after it (no media-list or supports
/// conditions in this slot — keeps the surface tight against future
/// extensions).
pub fn parse_bd_colour_name<'i, 't>(
    input: &mut Parser<'i, 't>,
) -> Result<AtomIdent, ParseError<'i, StyleParseErrorKind<'i>>> {
    let location = input.current_source_location();
    let ident = input.expect_ident()?.clone();
    if ident.is_empty() {
        return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    let name = AtomIdent::from(&*ident);
    input.expect_exhausted()?;
    Ok(name)
}

/// Parse the body (inside `{}`) of an `@-bd-colour` rule.
///
/// Recognised descriptors:
/// - `colour-values` (American alias: `color-values`) — `<color>`.
/// - `alternate` — `auto | cmyk | rgb`.
///
/// Unknown descriptors are logged via `ContextualParseError::
/// UnsupportedRule` and discarded, mirroring how `@counter-style`
/// handles unsupported descriptors.
///
/// Returns an error when the body has no `colour-values:` descriptor;
/// a spot reference cannot resolve to a Separation colour space
/// without a tint to use as the PDF tint transform.
pub fn parse_bd_colour_body<'i, 't>(
    context: &ParserContext,
    name: AtomIdent,
    input: &mut Parser<'i, 't>,
    location: SourceLocation,
) -> Result<BdColourRule, ParseError<'i, StyleParseErrorKind<'i>>> {
    let mut rule = BdColourRule::empty(name, location);
    let start = input.current_source_location();
    {
        let mut parser = BdColourRuleParser {
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
    if rule.values.is_none() {
        context.log_css_error(
            start,
            ContextualParseError::UnsupportedRule(
                "@-bd-colour without colour-values descriptor",
                input.new_custom_error::<_, StyleParseErrorKind<'i>>(
                    StyleParseErrorKind::UnspecifiedError,
                ),
            ),
        );
        return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
    }
    Ok(rule)
}

struct BdColourRuleParser<'a, 'b: 'a> {
    context: &'a ParserContext<'b>,
    rule: &'a mut BdColourRule,
}

impl<'a, 'b, 'i> AtRuleParser<'i> for BdColourRuleParser<'a, 'b> {
    type Prelude = ();
    type AtRule = ();
    type Error = StyleParseErrorKind<'i>;
}

impl<'a, 'b, 'i> QualifiedRuleParser<'i> for BdColourRuleParser<'a, 'b> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = StyleParseErrorKind<'i>;
}

impl<'a, 'b, 'i> RuleBodyItemParser<'i, (), StyleParseErrorKind<'i>>
    for BdColourRuleParser<'a, 'b>
{
    fn parse_qualified(&self) -> bool {
        false
    }
    fn parse_declarations(&self) -> bool {
        true
    }
}

impl<'a, 'b, 'i> DeclarationParser<'i> for BdColourRuleParser<'a, 'b> {
    type Declaration = ();
    type Error = StyleParseErrorKind<'i>;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _start: &ParserState,
    ) -> Result<(), ParseError<'i, Self::Error>> {
        match_ignore_ascii_case! { &name,
            "colour-values" | "color-values" => {
                let color = SpecifiedColor::parse(self.context, input)?;
                input.expect_exhausted()?;
                self.rule.values = Some(color);
                Ok(())
            },
            "alternate" => {
                let kind = BdColourAlternateKind::parse_keyword(input)?;
                input.expect_exhausted()?;
                self.rule.alternate = kind;
                Ok(())
            },
            _ => Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
        }
    }
}

#[cfg(feature = "gecko")]
impl MallocSizeOf for BdColourRule {
    fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
        0
    }
}
