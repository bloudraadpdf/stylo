/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `@view-transition` rule (CSS View Transitions 2 §3).

use crate::derives::*;
use crate::error_reporting::ContextualParseError;
use crate::parser::ParserContext;
use crate::shared_lock::{DeepCloneWithLock, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::values::CustomIdent;
use cssparser::{
    match_ignore_ascii_case, AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser,
    ParserState, QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser, SourceLocation,
};
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOf, MallocSizeOfOps};
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, CssWriter, StyleParseErrorKind, ToCss};

/// The `navigation` descriptor: whether a same-origin navigation starts a view transition.
#[derive(Clone, Copy, Debug, PartialEq, ToShmem)]
pub enum ViewTransitionNavigation {
    /// `auto`.
    Auto,
    /// `none`.
    None,
}

impl ToCss for ViewTransitionNavigation {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        dest.write_str(match self {
            Self::Auto => "auto",
            Self::None => "none",
        })
    }
}

/// The `types` descriptor: `none | <custom-ident>+`, without `default` and `-ua-` prefixed names.
#[derive(Clone, Debug, PartialEq, ToShmem)]
pub enum ViewTransitionTypes {
    /// `none`.
    None,
    /// The listed types.
    List(Vec<CustomIdent>),
}

impl ViewTransitionTypes {
    fn parse<'i, 't>(
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i, StyleParseErrorKind<'i>>> {
        if input
            .try_parse(|input| input.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let mut types = Vec::new();
        while !input.is_exhausted() {
            let location = input.current_source_location();
            let ident = CustomIdent::parse(input, &["none", "default"])?;
            if ident.0.starts_with("-ua-") {
                return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            types.push(ident);
        }
        if types.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::List(types))
    }

    /// The listed types, empty for `none`.
    pub fn names(&self) -> &[CustomIdent] {
        match self {
            Self::None => &[],
            Self::List(types) => types,
        }
    }
}

impl ToCss for ViewTransitionTypes {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::List(types) => {
                for (index, name) in types.iter().enumerate() {
                    if index > 0 {
                        dest.write_char(' ')?;
                    }
                    name.to_css(dest)?;
                }
                Ok(())
            },
        }
    }
}

/// A `@view-transition { … }` rule.
#[derive(Clone, Debug, ToShmem)]
pub struct ViewTransitionRule {
    /// The `navigation` descriptor, when valid.
    pub navigation: Option<ViewTransitionNavigation>,
    /// The `types` descriptor, when valid.
    pub types: Option<ViewTransitionTypes>,
    /// The source position of the at-rule for diagnostics.
    pub source_location: SourceLocation,
}

impl ViewTransitionRule {
    /// Heap-size measurement.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, _guard: &SharedRwLockReadGuard, _ops: &mut MallocSizeOfOps) -> usize {
        0
    }
}

impl ToCssWithGuard for ViewTransitionRule {
    fn to_css(&self, _guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        dest.write_str("@view-transition { ")?;
        if let Some(navigation) = self.navigation {
            dest.write_str("navigation: ")?;
            navigation.to_css(&mut CssWriter::new(dest))?;
            dest.write_str("; ")?;
        }
        if let Some(ref types) = self.types {
            dest.write_str("types: ")?;
            types.to_css(&mut CssWriter::new(dest))?;
            dest.write_str("; ")?;
        }
        dest.write_char('}')
    }
}

impl DeepCloneWithLock for ViewTransitionRule {
    fn deep_clone_with_lock(&self, _lock: &SharedRwLock, _guard: &SharedRwLockReadGuard) -> Self {
        self.clone()
    }
}

/// Parse the body of a `@view-transition` rule. Invalid and unknown descriptors are dropped.
pub fn parse_view_transition_body<'i, 't>(
    context: &ParserContext,
    input: &mut Parser<'i, 't>,
    source_location: SourceLocation,
) -> ViewTransitionRule {
    let mut rule = ViewTransitionRule {
        navigation: None,
        types: None,
        source_location,
    };
    let mut parser = ViewTransitionRuleParser { rule: &mut rule };
    let mut iter = RuleBodyParser::new(input, &mut parser);
    while let Some(declaration) = iter.next() {
        if let Err((error, slice)) = declaration {
            let location = error.location;
            context.log_css_error(
                location,
                ContextualParseError::UnsupportedRule(slice, error),
            );
        }
    }
    rule
}

struct ViewTransitionRuleParser<'a> {
    rule: &'a mut ViewTransitionRule,
}

impl<'a, 'i> AtRuleParser<'i> for ViewTransitionRuleParser<'a> {
    type Prelude = ();
    type AtRule = ();
    type Error = StyleParseErrorKind<'i>;
}

impl<'a, 'i> QualifiedRuleParser<'i> for ViewTransitionRuleParser<'a> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = StyleParseErrorKind<'i>;
}

impl<'a, 'i> RuleBodyItemParser<'i, (), StyleParseErrorKind<'i>> for ViewTransitionRuleParser<'a> {
    fn parse_qualified(&self) -> bool {
        false
    }

    fn parse_declarations(&self) -> bool {
        true
    }
}

impl<'a, 'i> DeclarationParser<'i> for ViewTransitionRuleParser<'a> {
    type Declaration = ();
    type Error = StyleParseErrorKind<'i>;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _start: &ParserState,
    ) -> Result<(), ParseError<'i, Self::Error>> {
        match_ignore_ascii_case! { &name,
            "navigation" => {
                let location = input.current_source_location();
                let ident = input.expect_ident_cloned()?;
                let navigation = match_ignore_ascii_case! { &ident,
                    "auto" => ViewTransitionNavigation::Auto,
                    "none" => ViewTransitionNavigation::None,
                    _ => return Err(location.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
                };
                input.expect_exhausted()?;
                self.rule.navigation = Some(navigation);
                Ok(())
            },
            "types" => {
                let types = ViewTransitionTypes::parse(input)?;
                self.rule.types = Some(types);
                Ok(())
            },
            _ => Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
        }
    }
}

#[cfg(feature = "gecko")]
impl MallocSizeOf for ViewTransitionRule {
    fn size_of(&self, _ops: &mut MallocSizeOfOps) -> usize {
        0
    }
}
