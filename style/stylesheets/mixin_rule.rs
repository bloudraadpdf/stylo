/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Typed mixin rules. CSS Mixins 1 §5.5 retains the legacy @result algorithm
//! pending its reconciliation with the newer direct-declaration grammar.

use crate::custom_properties::VariableValue;
use crate::derives::*;
use crate::parser::ParserContext;
use crate::shared_lock::{
    DeepCloneWithLock, Locked, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard,
};
use crate::stylesheets::function_rule::{parse_parameters, FunctionParameter};
use crate::stylesheets::{CssRuleType, CssRules};
use crate::values::{serialize_atom_identifier, DashedIdent};
use crate::Atom;
use cssparser::{Parser, SourceLocation, Token};
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, CssWriter, ParseError, ToCss};

/// Grammar and role of a rule in the mixin family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ToShmem)]
pub enum MixinRuleKind {
    /// Named definition.
    Definition,
    /// Call site, optionally supplying contents.
    Apply,
    /// Placeholder, optionally supplying fallback contents.
    Contents,
    /// Legacy expansion body specified in §5.5.
    Result,
    /// Lexically scoped custom property group.
    Private,
}

impl MixinRuleKind {
    /// At-keyword without its leading @.
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Definition => "mixin",
            Self::Apply => "apply",
            Self::Contents => "contents",
            Self::Result => "result",
            Self::Private => "private",
        }
    }
    /// Parser nesting context.
    pub fn rule_type(self) -> CssRuleType {
        match self {
            Self::Definition => CssRuleType::Mixin,
            Self::Apply => CssRuleType::Apply,
            Self::Contents => CssRuleType::Contents,
            Self::Result => CssRuleType::Result,
            Self::Private => CssRuleType::Private,
        }
    }
}

/// Parsed header shared by statement and block forms.
#[derive(Clone, Debug, ToShmem)]
pub struct MixinPrelude {
    /// Rule role.
    pub kind: MixinRuleKind,
    /// Definition or application name.
    pub name: Option<DashedIdent>,
    /// Definition parameters.
    pub parameters: Box<[FunctionParameter]>,
    /// Validated dashed-function invocation including its name.
    pub invocation: Option<Arc<VariableValue>>,
}

impl MixinPrelude {
    /// Canonical inline private declarations, excluding nested group rules.
    pub fn private_declarations_css(&self) -> String {
        if self.kind != MixinRuleKind::Private {
            return String::new();
        }
        self.parameters
            .iter()
            .map(|parameter| format!("{};", parameter.to_css_string()))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Parse the grammar appropriate to the rule kind.
    pub fn parse<'i>(
        kind: MixinRuleKind,
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, ParseError<'i>> {
        let mut result = Self {
            kind,
            name: None,
            parameters: Box::default(),
            invocation: None,
        };
        if matches!(kind, MixinRuleKind::Definition | MixinRuleKind::Apply) {
            let state = input.state();
            let location = input.current_source_location();
            let (name, function) = match input.next()?.clone() {
                Token::Ident(name) if name.starts_with("--") => (name, false),
                Token::Function(name) if name.starts_with("--") => (name, true),
                token => return Err(location.new_unexpected_token_error(token)),
            };
            result.name = Some(DashedIdent(Atom::from(name.as_ref())));
            if function {
                if kind == MixinRuleKind::Definition {
                    result.parameters =
                        input.parse_nested_block(|input| parse_parameters(context, input))?;
                } else {
                    input.parse_nested_block(|input| {
                        while input.next_including_whitespace_and_comments().is_ok() {}
                        Ok::<_, ParseError<'i>>(())
                    })?;
                    input.expect_exhausted()?;
                    input.reset(&state);
                    result.invocation = Some(Arc::new(VariableValue::parse(
                        input,
                        &context.url_data,
                        &context.namespaces,
                    )?));
                }
            }
        }
        input.expect_exhausted()?;
        Ok(result)
    }
}

/// Authored mixin-family rule. None distinguishes a statement from an empty block.
#[derive(Debug, ToShmem)]
pub struct MixinRule {
    /// Typed header.
    pub prelude: MixinPrelude,
    /// Authored children, if a block was present.
    pub rules: Option<Arc<Locked<CssRules>>>,
    /// Source position.
    pub source_location: SourceLocation,
}

impl DeepCloneWithLock for MixinRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        Self {
            prelude: self.prelude.clone(),
            rules: self.rules.as_ref().map(|rules| {
                Arc::new(lock.wrap(rules.read_with(guard).deep_clone_with_lock(lock, guard)))
            }),
            source_location: self.source_location,
        }
    }
}

impl ToCss for MixinPrelude {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        dest.write_char('@')?;
        dest.write_str(self.kind.keyword())?;
        if let Some(ref name) = self.name {
            dest.write_char(' ')?;
            if let Some(ref invocation) = self.invocation {
                return invocation.to_css(dest);
            }
            serialize_atom_identifier(&name.0, dest)?;
            if self.kind == MixinRuleKind::Definition && !self.parameters.is_empty() {
                dest.write_char('(')?;
                for (index, parameter) in self.parameters.iter().enumerate() {
                    if index != 0 {
                        dest.write_str(", ")?;
                    }
                    parameter.to_css(dest)?;
                }
                dest.write_char(')')?;
            }
        }
        Ok(())
    }
}

impl ToCssWithGuard for MixinRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        self.prelude.to_css(&mut CssWriter::new(dest))?;
        if self.prelude.kind == MixinRuleKind::Private {
            let rules = self.rules.as_ref().map(|rules| rules.read_with(guard));
            let nested = rules.as_ref().is_some_and(|rules| !rules.0.is_empty());
            dest.write_str(if nested { " {\n  " } else { " { " })?;
            for (index, declaration) in self.prelude.parameters.iter().enumerate() {
                if index != 0 {
                    dest.write_char(' ')?;
                }
                declaration.to_css(&mut CssWriter::new(dest))?;
                dest.write_char(';')?;
            }
            if let Some(rules) = rules {
                for rule in &rules.0 {
                    dest.write_str("\n  ")?;
                    rule.to_css(guard, dest)?;
                }
            }
            if nested {
                dest.write_str("\n}")
            } else if self.prelude.parameters.is_empty() {
                dest.write_char('}')
            } else {
                dest.write_str(" }")
            }
        } else {
            match self.rules {
                Some(ref rules) => {
                    let rules = rules.read_with(guard);
                    if matches!(
                        self.prelude.kind,
                        MixinRuleKind::Apply | MixinRuleKind::Contents
                    ) && (rules.0.is_empty()
                        || matches!(
                            rules.0.as_slice(),
                            [crate::stylesheets::CssRule::NestedDeclarations(_)]
                        ))
                    {
                        dest.write_str(" { ")?;
                        if let Some(rule) = rules.0.first() {
                            rule.to_css(guard, dest)?;
                            dest.write_char(' ')?;
                        }
                        dest.write_char('}')
                    } else {
                        rules.to_css_block(guard, dest)
                    }
                },
                None => dest.write_char(';'),
            }
        }
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use crate::stylesheets::{CssRule, StylesheetInDocument};
    use crate::test_support::parse_stylesheet;
    #[test]
    fn mixin_parameters_and_context_are_validated() {
        for (css, valid) in [
            ("@mixin --m {}", true),
            ("@mixin --m(--a <length>: 2px) {}", true),
            ("@mixin --m(--a, --a) {}", false),
            ("@mixin --m(--a <length>: red) {}", false),
            ("@mixin --m() returns <length> {}", false),
            ("@mixin nope() {}", false),
            ("@apply --m;", false),
            ("@contents;", false),
            ("@private {}", false),
        ] {
            let sheet = parse_stylesheet(css);
            let guard = sheet.shared_lock.read();
            let rules = sheet.contents(&guard).rules(&guard);
            assert_eq!(rules.len() == 1, valid, "{css}");
            if valid {
                assert!(matches!(rules[0], CssRule::Mixin(_)));
            }
        }
    }
}
