/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The `@function` rule.
//!
//! <https://drafts.csswg.org/css-mixins-1/#function-rule>

use crate::custom_properties::{self, Name, VariableValue};
use crate::derives::*;
use crate::parser::ParserContext;
use crate::properties_and_values::syntax::Descriptor;
use crate::properties_and_values::value::{
    AllowComputationallyDependent, SpecifiedValue as SpecifiedRegisteredValue,
};
use crate::shared_lock::{
    DeepCloneWithLock, Locked, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard,
};
use crate::stylesheets::CssRules;
use crate::values::{serialize_atom_identifier, serialize_atom_name, DashedIdent};
use crate::Atom;
use cssparser::{CowRcStr, Parser, ParserInput, SourceLocation, Token};
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// A function parameter.
///
/// <https://drafts.csswg.org/css-mixins-1/#function-parameter>
#[derive(Clone, Debug, ToShmem)]
pub struct FunctionParameter {
    /// The parameter name, without the leading dashes.
    pub name: Name,
    /// The parameter type; universal when omitted.
    pub syntax: Descriptor,
    /// The default value.
    pub default: Option<Arc<VariableValue>>,
}

impl ToCss for FunctionParameter {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        dest.write_str("--")?;
        serialize_atom_name(&self.name, dest)?;
        if !self.syntax.is_universal() {
            dest.write_char(' ')?;
            self.syntax.to_css_type(dest)?;
        }
        if let Some(ref default) = self.default {
            dest.write_str(": ")?;
            default.to_css(dest)?;
        }
        Ok(())
    }
}

/// The prelude of a `@function` rule.
pub struct FunctionPrelude {
    name: DashedIdent,
    parameters: Box<[FunctionParameter]>,
    return_type: Descriptor,
}

impl FunctionPrelude {
    /// Parse `<function-token> <function-parameter>#? ) [ returns <css-type> ]?`.
    pub fn parse<'i>(
        context: &ParserContext,
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        let name = match *input.next()? {
            Token::Function(ref name) if name.starts_with("--") => {
                DashedIdent(Atom::from(name.as_ref()))
            },
            ref token => return Err(location.new_unexpected_token_error(token.clone())),
        };
        let parameters = input.parse_nested_block(|input| parse_parameters(context, input))?;
        let return_type = if input
            .try_parse(|input| input.expect_ident_matching("returns"))
            .is_ok()
        {
            Descriptor::parse_css_type(input)?
        } else {
            Descriptor::universal()
        };
        input.expect_exhausted()?;
        Ok(Self {
            name,
            parameters,
            return_type,
        })
    }
}

fn parse_parameters<'i>(
    context: &ParserContext,
    input: &mut Parser<'i, '_>,
) -> Result<Box<[FunctionParameter]>, ParseError<'i>> {
    input.skip_whitespace();
    if input.is_exhausted() {
        return Ok(Box::default());
    }
    let parameters = input.parse_comma_separated(|input| parse_parameter(context, input))?;
    for (index, parameter) in parameters.iter().enumerate() {
        if parameters[..index]
            .iter()
            .any(|earlier| earlier.name == parameter.name)
        {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
    }
    Ok(parameters.into_boxed_slice())
}

fn parse_parameter<'i>(
    context: &ParserContext,
    input: &mut Parser<'i, '_>,
) -> Result<FunctionParameter, ParseError<'i>> {
    let location = input.current_source_location();
    let name = input.expect_ident_cloned()?;
    let name = custom_properties::parse_name(&name).map_err(|()| {
        location.new_custom_error(StyleParseErrorKind::UnexpectedIdent(name.clone()))
    })?;
    let syntax = input
        .try_parse(Descriptor::parse_css_type)
        .unwrap_or_else(|_| Descriptor::universal());
    let default = if input.try_parse(|input| input.expect_colon()).is_ok() {
        input.skip_whitespace();
        let value = VariableValue::parse(input, &context.url_data, &context.namespaces)?;
        if value.css.is_empty() || !default_matches(&value, &syntax) {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Some(Arc::new(value))
    } else {
        None
    };
    input.expect_exhausted()?;
    Ok(FunctionParameter {
        name: Atom::from(name),
        syntax,
        default,
    })
}

fn default_matches(value: &VariableValue, syntax: &Descriptor) -> bool {
    if syntax.is_universal() || value.has_references() {
        return true;
    }
    let mut input = ParserInput::new(&value.css);
    let mut input = Parser::new(&mut input);
    input
        .parse_entirely(|input| {
            SpecifiedRegisteredValue::parse(
                input,
                syntax,
                &value.url_data,
                AllowComputationallyDependent::Yes,
            )
        })
        .is_ok()
}

/// A `@function` rule.
#[derive(Debug, ToShmem)]
pub struct FunctionRule {
    /// The function name, with its leading dashes.
    pub name: DashedIdent,
    /// The parameters, with unique names.
    pub parameters: Box<[FunctionParameter]>,
    /// The return type; universal when omitted.
    pub return_type: Descriptor,
    /// The body: function declarations and conditional group rules.
    pub rules: Arc<Locked<CssRules>>,
    /// The source position this rule was found at.
    pub source_location: SourceLocation,
}

impl FunctionRule {
    /// Build the rule from its prelude and body.
    pub fn new(
        prelude: FunctionPrelude,
        rules: Arc<Locked<CssRules>>,
        source_location: SourceLocation,
    ) -> Self {
        Self {
            name: prelude.name,
            parameters: prelude.parameters,
            return_type: prelude.return_type,
            rules,
            source_location,
        }
    }
}

impl DeepCloneWithLock for FunctionRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        Self {
            name: self.name.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            rules: Arc::new(
                lock.wrap(
                    self.rules
                        .read_with(guard)
                        .deep_clone_with_lock(lock, guard),
                ),
            ),
            source_location: self.source_location.clone(),
        }
    }
}

impl ToCss for FunctionRule {
    /// The prelude, from `@function` to the return type.
    ///
    /// <https://drafts.csswg.org/css-mixins-1/#serialize-a-cssfunctionrule>
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        dest.write_str("@function ")?;
        serialize_atom_identifier(&self.name.0, dest)?;
        dest.write_char('(')?;
        for (index, parameter) in self.parameters.iter().enumerate() {
            if index != 0 {
                dest.write_str(", ")?;
            }
            parameter.to_css(dest)?;
        }
        dest.write_char(')')?;
        if !self.return_type.is_universal() {
            dest.write_str(" returns ")?;
            self.return_type.to_css_type(dest)?;
        }
        Ok(())
    }
}

impl ToCssWithGuard for FunctionRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        ToCss::to_css(self, &mut CssWriter::new(dest))?;
        self.rules.read_with(guard).to_css_block(guard, dest)
    }
}

/// The name of a descriptor in a function body.
#[derive(Clone, Debug, Eq, PartialEq, ToShmem)]
pub enum FunctionDescriptorName {
    /// A local variable, without its leading dashes.
    Local(Name),
    /// The `result` descriptor.
    Result,
}

impl FunctionDescriptorName {
    /// Parse a descriptor name: a custom property name, or `result`.
    pub fn parse<'i>(name: &CowRcStr<'i>) -> Option<Self> {
        if name.eq_ignore_ascii_case("result") {
            return Some(Self::Result);
        }
        custom_properties::parse_name(name)
            .ok()
            .map(|name| Self::Local(Atom::from(name)))
    }
}

impl ToCss for FunctionDescriptorName {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match *self {
            Self::Local(ref name) => {
                dest.write_str("--")?;
                serialize_atom_name(name, dest)
            },
            Self::Result => dest.write_str("result"),
        }
    }
}

/// A descriptor in a function body.
#[derive(Clone, Debug, ToShmem)]
pub struct FunctionDescriptor {
    /// The descriptor name.
    pub name: FunctionDescriptorName,
    /// The unsubstituted value.
    pub value: Arc<VariableValue>,
}

impl FunctionDescriptor {
    /// Parse one descriptor; `!important` makes it invalid.
    pub fn parse<'i>(
        context: &ParserContext,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, '_>,
    ) -> Result<Self, ParseError<'i>> {
        let Some(name) = FunctionDescriptorName::parse(&name) else {
            return Err(input.new_custom_error(StyleParseErrorKind::UnknownProperty(name)));
        };
        input.skip_whitespace();
        let value = VariableValue::parse(input, &context.url_data, &context.namespaces)?;
        input.expect_exhausted()?;
        Ok(Self {
            name,
            value: Arc::new(value),
        })
    }
}

/// The declarations of a function body, in specified order, with later
/// declarations replacing earlier ones of the same name.
#[derive(Clone, Debug, Default, ToShmem)]
pub struct FunctionDescriptors(Vec<FunctionDescriptor>);

impl FunctionDescriptors {
    /// Append a descriptor, replacing an earlier one of the same name.
    pub fn push(&mut self, descriptor: FunctionDescriptor) {
        self.0.retain(|existing| existing.name != descriptor.name);
        self.0.push(descriptor);
    }

    /// Whether no descriptor was parsed.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The descriptors in specified order.
    pub fn iter(&self) -> std::slice::Iter<'_, FunctionDescriptor> {
        self.0.iter()
    }
}

impl ToCss for FunctionDescriptors {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        for (index, descriptor) in self.0.iter().enumerate() {
            if index != 0 {
                dest.write_char(' ')?;
            }
            descriptor.name.to_css(dest)?;
            dest.write_str(": ")?;
            descriptor.value.to_css(dest)?;
            dest.write_char(';')?;
        }
        Ok(())
    }
}

/// A run of consecutive declarations in a function body.
///
/// <https://drafts.csswg.org/css-mixins-1/#cssfunctiondeclarations>
#[derive(Clone, Debug, ToShmem)]
pub struct FunctionDeclarationsRule {
    /// The declarations.
    pub descriptors: FunctionDescriptors,
    /// The source position this rule was found at.
    pub source_location: SourceLocation,
}

impl ToCssWithGuard for FunctionDeclarationsRule {
    fn to_css(&self, _: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        self.descriptors.to_css(&mut CssWriter::new(dest))
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use crate::shared_lock::ToCssWithGuard;
    use crate::stylesheets::{CssRule, StylesheetInDocument};
    use crate::test_support::parse_stylesheet;

    fn serialized_rules(css: &str) -> Vec<String> {
        let sheet = parse_stylesheet(css);
        let guard = sheet.shared_lock.read();
        sheet
            .contents(&guard)
            .rules(&guard)
            .iter()
            .map(|rule| rule.to_css_string(&guard))
            .collect()
    }

    #[test]
    fn preludes_follow_the_function_grammar() {
        for (prelude, valid) in [
            ("--foo()", true),
            ("--foo( --x )", true),
            ("--foo (--x)", false),
            ("--foo(--x auto)", true),
            ("--foo(--x <transform-list>)", true),
            ("--foo(--x type(<length> | auto) : auto)", true),
            ("--foo(--x:1px, --y, --z:2px)", true),
            ("--foo(--x: var(--y, 10px), --z: 2px)", true),
            ("--foo(--x: {1px, 2px})", true),
            ("--foo(--x: 10px !important)", false),
            ("--foo(--x <length>: 10deg)", false),
            ("--foo(--x type(auto | none): thing)", false),
            ("--foo(--x <transform-list>#)", false),
            ("--foo(--x *)", false),
            ("--foo(--x 50px)", false),
            ("--foo(--x <length> | auto)", false),
            ("--foo(--x, --x)", false),
            ("--foo(,)", false),
            ("--foo(--x, ;)", false),
            ("--foo(--x) returns <length>+", true),
            ("--foo(--x) returns type(foo | bar)", true),
            ("--foo(--x) returns", false),
            ("--foo(--x) returns *", false),
            ("--foo(--x) returns <length>!", false),
            ("--foo(--x): <length>", false),
            ("foo()", false),
        ] {
            let rules = serialized_rules(&format!("@function {prelude} {{}}"));
            assert_eq!(rules.len() == 1, valid, "{prelude}");
        }
    }

    #[test]
    fn preludes_serialize_canonically() {
        for (authored, expected) in [
            (
                "--f(--x type(<length>), --y type(*): 10px) returns type(*)",
                "@function --f(--x <length>, --y: 10px) {\n}",
            ),
            (
                "--f(--a I\\ dent) returns type(I\\ dent)",
                "@function --f(--a I\\ dent) returns I\\ dent {\n}",
            ),
            (
                "--f() returns type(<length> | auto)",
                "@function --f() returns type(<length> | auto) {\n}",
            ),
        ] {
            assert_eq!(
                serialized_rules(&format!("@function {authored} {{}}")),
                [expected]
            );
        }
    }

    #[test]
    fn bodies_keep_locals_result_and_conditional_rules() {
        let sheet = parse_stylesheet(
            "@function --f() { --x: 1px; --x: 2px !important; color: red; result: 3px; \
             result: 4px; .a { --y: 1px; } @supports (width: 1px) { result: 5px; } --y: 6px; }",
        );
        let guard = sheet.shared_lock.read();
        let rules = sheet.contents(&guard).rules(&guard);
        let CssRule::Function(ref function) = rules[0] else {
            panic!("expected @function");
        };
        let body = function
            .rules
            .read_with(&guard)
            .0
            .iter()
            .map(|rule| rule.to_css_string(&guard))
            .collect::<Vec<_>>();
        assert_eq!(
            body,
            [
                "--x: 1px; result: 4px;",
                "@supports (width: 1px) {\n  result: 5px;\n}",
                "--y: 6px;"
            ]
        );
    }

    #[test]
    fn dashed_function_arguments_are_declaration_values() {
        for (value, valid) in [
            ("--func()", true),
            ("--func(auto ,100px ,#fff)", true),
            ("--func(--bar(), --baz(--fez()))", true),
            ("--func({1}, 2)", true),
            ("--func({,},{4})", true),
            ("--func({{}},{4})", true),
            ("--func(--)", true),
            ("--func(50px --myident:)", true),
            ("--func({ --myident : })", true),
            ("--func(!)", false),
            ("--func(})", false),
            ("--func(red !important)", false),
            ("--func({red} !important)", false),
            ("--func(asdf,)", false),
            ("--func(a, ,b)", false),
            ("--func(1{})", false),
            ("--func({} 1)", false),
            ("--func({})", false),
            ("--func(1, { })", false),
            ("--func(--myident:)", false),
            ("--func(10px, --myident : )", false),
        ] {
            let rules = serialized_rules(&format!("p {{ top: {value}; }}"));
            assert_eq!(rules[0] != "p { }", valid, "{value}: {}", rules[0]);
        }
    }
}
