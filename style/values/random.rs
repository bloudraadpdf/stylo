/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Random cache names and document-lifetime random base values (CSS Values 5 §9).

use crate::derives::*;
use crate::parser::ParserContext;
use crate::Atom;
use cssparser::{Parser, SourcePosition, Token};
use std::collections::hash_map::{DefaultHasher, RandomState};
use std::fmt::{self, Write};
use std::hash::{BuildHasher, Hash, Hasher};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// Validate a fixed base and keep it below one in the engine's CSS numeric precision.
pub fn normalized_base(value: f64) -> Option<f64> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return None;
    }
    Some(value.min(f32::from_bits(1.0f32.to_bits() - 1) as f64))
}

/// Original declaration identity, assigned before shorthand expansion and parser backtracking.
#[derive(Clone, Debug, Deserialize, MallocSizeOf, PartialEq, Serialize, ToShmem)]
pub struct RandomFunctionIdentity {
    /// The original property name, including a custom function scope when present.
    pub property: Atom,
    /// One-based lexical occurrence among both kinds of random function.
    pub index: usize,
}

impl RandomFunctionIdentity {
    /// Whether this occurrence belongs to an original declaration.
    pub fn is_bound(&self) -> bool {
        !self.property.is_empty()
    }

    /// The distinct namespace used by random functions evaluated during arbitrary substitution.
    pub fn for_early_substitution(&self) -> Self {
        if !self.is_bound() {
            return self.clone();
        }
        Self {
            property: Atom::from(format!("early-{}", self.property)),
            index: self.index,
        }
    }
}

/// Parse-time positions of random function opening tokens in one declaration.
#[derive(Clone, Debug)]
pub struct RandomDeclarationContext {
    /// The declaration's original property name.
    pub property: Atom,
    /// Positions immediately after each opening function token.
    pub positions: Vec<usize>,
    /// Early arbitrary random-item substitutions have their own lexical stream.
    pub early_positions: Vec<usize>,
    /// Lexical functions already assigned in an enclosing early evaluation.
    pub index_offset: usize,
}

impl RandomDeclarationContext {
    /// Scan without changing the parser's state or depending on successful parse branches.
    pub fn new<'i, 't>(property: Atom, input: &mut Parser<'i, 't>) -> Self {
        fn scan<'i>(
            input: &mut Parser<'i, '_>,
            positions: &mut Vec<usize>,
            early: &mut Vec<usize>,
            in_early: bool,
            key_only: bool,
            depth: usize,
        ) {
            while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
                if key_only && matches!(token, Token::Comma) {
                    return;
                }
                let mut is_item = false;
                let nested = match token {
                    Token::Function(ref name) => {
                        if name.eq_ignore_ascii_case("random")
                            || name.eq_ignore_ascii_case("random-item")
                        {
                            positions.push(input.position().byte_index());
                        }
                        is_item = name.eq_ignore_ascii_case("random-item");
                        if is_item || in_early && name.eq_ignore_ascii_case("random") {
                            early.push(input.position().byte_index());
                        }
                        true
                    },
                    Token::ParenthesisBlock
                    | Token::SquareBracketBlock
                    | Token::CurlyBracketBlock => true,
                    _ => false,
                };
                // Uninspected nested blocks are skipped by cssparser without consuming the Rust stack.
                if nested && depth < 128 {
                    let _ = input.parse_nested_block(|input| {
                        if is_item {
                            scan(input, positions, early, true, true, depth + 1);
                            scan(input, positions, early, in_early, false, depth + 1);
                        } else {
                            scan(input, positions, early, in_early, false, depth + 1);
                        }
                        Ok::<_, ParseError<'i>>(())
                    });
                }
            }
        }
        let state = input.state();
        let mut positions = Vec::new();
        let mut early_positions = Vec::new();
        scan(input, &mut positions, &mut early_positions, false, false, 0);
        input.reset(&state);
        Self {
            property,
            positions,
            early_positions,
            index_offset: 0,
        }
    }

    /// Resolve an occurrence using its source position, independent of parse attempt order.
    pub fn identity(&self, byte: usize) -> RandomFunctionIdentity {
        let Ok(index) = self.positions.binary_search(&byte) else {
            return RandomFunctionIdentity {
                property: Atom::from(""),
                index: 0,
            };
        };
        RandomFunctionIdentity {
            property: self.property.clone(),
            index: self.index_offset + index + 1,
        }
    }

    /// The separate early-substitution stream for retained random-item references.
    pub fn early_identity(&self, byte: usize) -> RandomFunctionIdentity {
        let Ok(index) = self.early_positions.binary_search(&byte) else {
            return RandomFunctionIdentity {
                property: Atom::from(""),
                index: 0,
            };
        };
        RandomFunctionIdentity {
            property: self.property.clone(),
            index: self.index_offset + index + 1,
        }
        .for_early_substitution()
    }
}

/// The simplified tuple that determines which random functions share a base value.
#[derive(
    Clone,
    Debug,
    Deserialize,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    Serialize,
    ToResolvedValue,
    ToShmem,
)]
pub struct RandomCacheKey {
    /// Explicit dashed name, or null when omitted.
    pub name: Option<Atom>,
    /// Explicit or generated `ua-` property/index identifier.
    pub property: Option<Atom>,
    /// Include the real element/pseudo identity in the document cache key.
    pub element_scoped: bool,
}

/// A random key whose fixed numeric expression is owned by the numeric or substitution parser.
#[derive(
    Clone, Debug, Deserialize, MallocSizeOf, PartialEq, Serialize, ToResolvedValue, ToShmem,
)]
pub enum RandomKey<N> {
    /// An explicitly supplied random base expression.
    Fixed(N),
    /// A simplified name that selects a document-lifetime random base.
    Cache(RandomCacheKey),
}

impl<N> crate::values::animated::ToAnimatedZero for RandomKey<N> {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

impl<N> RandomKey<N> {
    /// The default `auto` key in the original declaration.
    pub fn auto(context: &ParserContext, position: SourcePosition) -> Self {
        Self::auto_with_identity(&context.random_function_identity(position))
    }

    /// The default `auto` key for a retained arbitrary substitution function.
    pub fn auto_with_identity(identity: &RandomFunctionIdentity) -> Self {
        Self::Cache(RandomCacheKey {
            name: None,
            property: Some(Atom::from(format!(
                "ua-{}-{}",
                identity.property, identity.index
            ))),
            element_scoped: true,
        })
    }

    /// Parse an explicit random key while preserving the caller's numeric expression representation.
    pub fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        position: SourcePosition,
        fixed: impl FnOnce(&ParserContext, &mut Parser<'i, 't>) -> Result<N, ParseError<'i>>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_with_identity(
            &context.random_function_identity(position),
            input,
            |input| fixed(context, input),
        )
    }

    /// Parse a key after arbitrary substitutions using its retained original declaration identity.
    pub fn parse_with_identity<'i, 't>(
        identity: &RandomFunctionIdentity,
        input: &mut Parser<'i, 't>,
        fixed: impl FnOnce(&mut Parser<'i, 't>) -> Result<N, ParseError<'i>>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("auto"))
            .is_ok()
        {
            if !identity.is_bound() {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            return Ok(Self::auto_with_identity(identity));
        }
        if input
            .try_parse(|input| input.expect_ident_matching("fixed"))
            .is_ok()
        {
            return fixed(input).map(Self::Fixed);
        }
        let mut key = RandomCacheKey {
            name: None,
            property: None,
            element_scoped: false,
        };
        let mut seen = false;
        while let Ok(ident) = input.try_parse(|input| input.expect_ident_cloned()) {
            seen = true;
            if ident.starts_with("--") && key.name.is_none() {
                key.name = Some(Atom::from(ident.as_ref()));
            } else if ident.eq_ignore_ascii_case("element-scoped") && !key.element_scoped {
                key.element_scoped = true;
            } else if key.property.is_none() && ident.eq_ignore_ascii_case("property-scoped") {
                if !identity.is_bound() {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }
                key.property = Some(Atom::from(format!("ua-{}", identity.property)));
            } else if key.property.is_none() && ident.eq_ignore_ascii_case("property-index-scoped")
            {
                if !identity.is_bound() {
                    return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
                }
                key.property = Some(Atom::from(format!(
                    "ua-{}-{}",
                    identity.property, identity.index
                )));
            } else if key.property.is_none() && ident.starts_with("ua-") {
                key.property = Some(Atom::from(ident.as_ref()));
            } else {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
        }
        if !seen {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Cache(key))
    }
}

impl<N: ToCss> ToCss for RandomKey<N> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Fixed(number) => {
                dest.write_str("fixed ")?;
                number.to_css(dest)
            },
            Self::Cache(key) => key.to_css(dest),
        }
    }
}

impl ToCss for RandomCacheKey {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let mut separator = "";
        if let Some(name) = &self.name {
            dest.write_str(separator)?;
            crate::values::serialize_atom_identifier(name, dest)?;
            separator = " ";
        }
        if self.element_scoped {
            dest.write_str(separator)?;
            dest.write_str("element-scoped")?;
            separator = " ";
        }
        if let Some(property) = &self.property {
            dest.write_str(separator)?;
            crate::values::serialize_atom_identifier(property, dest)?;
        }
        Ok(())
    }
}

/// Random state shared for a document's full lifetime, including replacement style devices.
#[derive(Debug)]
pub struct DocumentRandomState {
    seed: u64,
}

impl Default for DocumentRandomState {
    fn default() -> Self {
        Self::with_seed(RandomState::new().build_hasher().finish())
    }
}

impl DocumentRandomState {
    /// Rebuild document state from its retained seed without changing any base values.
    pub fn with_seed(seed: u64) -> Self {
        Self { seed }
    }

    /// Resolve a cache tuple; numeric arguments never participate in its identity.
    pub fn resolve(&self, key: &RandomCacheKey, element: usize, pseudo: Option<Atom>) -> f64 {
        self.resolve_in_scope(key, element, pseudo, &[])
    }

    /// Resolve against a hypothetical custom-function element with a stable lexical invocation path.
    pub fn resolve_in_scope(
        &self,
        key: &RandomCacheKey,
        element: usize,
        pseudo: Option<Atom>,
        scope: &[Atom],
    ) -> f64 {
        let mut hash = DefaultHasher::new();
        hash.write_u64(self.seed);
        key.hash(&mut hash);
        key.element_scoped
            .then_some((element, pseudo.as_ref(), scope))
            .hash(&mut hash);
        (hash.finish() >> 40) as f64 / ((1u64 << 24) as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cssparser::ParserInput;

    #[test]
    fn caller_property_is_available_without_a_random_function_at_the_lookup_position() {
        let url = crate::test_support::test_url_data();
        let context = crate::test_support::author_parser_context(&url);
        let mut source = ParserInput::new("  --function()");
        let mut input = Parser::new(&mut source);
        let start = input.position();
        let context = context.for_random_declaration("width", &mut input);
        assert_eq!(context.random_declaration_property(), Some(&Atom::from("width")));
        assert!(!context.random_function_identity(start).is_bound());
    }

    #[test]
    fn declaration_scanning_preserves_parser_state_and_separates_early_keys() {
        let css = "random(0,1) random-item(fixed random(0,1),random(0,1),2) random-item(auto,3,4)";
        let mut source = ParserInput::new(css);
        let mut input = Parser::new(&mut source);
        let before = input.position();
        let scope = RandomDeclarationContext::new(Atom::from("margin"), &mut input);
        assert_eq!(input.position(), before);
        assert_eq!(scope.positions.len(), 5);
        assert_eq!(scope.early_positions.len(), 3);
        for (i, &position) in scope.positions.iter().enumerate() {
            assert_eq!(scope.identity(position).index, i + 1);
            assert_eq!(scope.identity(position).property, Atom::from("margin"));
        }
        let final_item = scope.early_identity(*scope.early_positions.last().unwrap());
        assert_eq!(final_item.property, Atom::from("early-margin"));
        assert_eq!(final_item.index, 3);
        assert!(!scope.identity(0).is_bound());
    }

    #[test]
    fn random_keys_have_canonical_scope_order_and_reject_duplicate_components() {
        let identity = RandomFunctionIdentity {
            property: Atom::from("margin"),
            index: 2,
        };
        for (css, expected) in [
            ("auto", "element-scoped ua-margin-2"),
            (
                "property-scoped --x element-scoped",
                "--x element-scoped ua-margin",
            ),
            ("--x property-index-scoped", "--x ua-margin-2"),
            ("ua-width --x", "--x ua-width"),
            ("--", "--"),
        ] {
            let mut source = ParserInput::new(css);
            let mut input = Parser::new(&mut source);
            let key = input
                .parse_entirely(|input| {
                    RandomKey::<f32>::parse_with_identity(&identity, input, |input| {
                        Ok(input.expect_number()?)
                    })
                })
                .unwrap();
            assert_eq!(key.to_css_string(), expected);
        }
        for css in [
            "element-scoped element-scoped",
            "--x --y",
            "property-scoped ua-width",
            "other",
        ] {
            let mut source = ParserInput::new(css);
            let mut input = Parser::new(&mut source);
            assert!(input
                .parse_entirely(|input| RandomKey::<f32>::parse_with_identity(
                    &identity,
                    input,
                    |input| Ok(input.expect_number()?),
                ))
                .is_err());
        }
    }

    #[test]
    fn document_seed_reconstitutes_bases_without_retaining_element_keys() {
        let state = DocumentRandomState::with_seed(7);
        let recreated = DocumentRandomState::with_seed(7);
        let mut key = RandomCacheKey {
            name: Some(Atom::from("--shared")),
            property: None,
            element_scoped: false,
        };
        let shared = state.resolve(&key, 1, None);
        assert_eq!(
            shared,
            state.resolve(&key, 999, Some(Atom::from("::before")))
        );
        assert_eq!(shared, recreated.resolve(&key, 1, None));
        key.element_scoped = true;
        for element in 0..16 {
            let value = state.resolve(&key, element, None);
            assert_eq!(value, recreated.resolve(&key, element, None));
            assert!(value >= 0.0 && value < 1.0);
        }
        assert_eq!(
            normalized_base(1.0),
            Some(f32::from_bits(1.0f32.to_bits() - 1) as f64)
        );
        assert_eq!(normalized_base(-0.1), None);
        assert_eq!(normalized_base(f64::NAN), None);
    }
}
