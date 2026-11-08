/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The context within which CSS code is parsed.

use crate::context::QuirksMode;
use crate::custom_properties::AttrTaintedRange;
use crate::error_reporting::{ContextualParseError, ParseErrorReporter};
use crate::stylesheets::{CssRuleType, CssRuleTypes, Namespaces, Origin, UrlExtraData};
use crate::use_counters::UseCounters;
use cssparser::{Parser, SourceLocation, SourcePosition, UnicodeRange};
use selectors::parser::ParseRelative;
use std::borrow::Cow;
use style_traits::{OneOrMoreSeparated, ParseError, ParsingMode, Separator};

/// Nesting context for parsing rules.
#[derive(Clone, Copy)]
pub struct NestingContext {
    /// All rule types we've nested into, if any.
    pub rule_types: CssRuleTypes,
    /// Whether or not parsing relative selector syntax should be allowed.
    pub parse_relative: ParseRelative,
    /// The innermost rule that supplies a property declaration grammar.
    declaration_rule_type: Option<CssRuleType>,
}

impl NestingContext {
    fn declaration_rule_for(rule_type: CssRuleType) -> Option<CssRuleType> {
        match rule_type {
            CssRuleType::Scope
            | CssRuleType::Style
            | CssRuleType::NestedDeclarations
            | CssRuleType::Result
            | CssRuleType::Apply
            | CssRuleType::Contents => Some(CssRuleType::Style),
            CssRuleType::Keyframe
            | CssRuleType::PositionTry
            | CssRuleType::Page
            | CssRuleType::Margin
            | CssRuleType::Footnote
            | CssRuleType::Sidenote => Some(rule_type),
            _ => None,
        }
    }

    fn parse_relative_for(rule_type: CssRuleType) -> ParseRelative {
        match rule_type {
            CssRuleType::Scope => ParseRelative::ForScope,
            CssRuleType::Style
            | CssRuleType::Result
            | CssRuleType::Apply
            | CssRuleType::Contents => ParseRelative::ForNesting,
            _ => ParseRelative::No,
        }
    }

    /// Create a new nesting context.
    pub fn new(rule_types: CssRuleTypes, parse_nested_rule_type: Option<CssRuleType>) -> Self {
        // CSSOM insertion supplies ancestor kinds rather than an ordered stack.
        // Restricted declaration rules cannot contain style or scope rules;
        // page margin rules are the only nested restricted declaration grammar.
        let declaration_rule_type = [
            CssRuleType::Margin,
            CssRuleType::Footnote,
            CssRuleType::Sidenote,
            CssRuleType::PositionTry,
            CssRuleType::Keyframe,
            CssRuleType::Page,
            CssRuleType::Style,
            CssRuleType::Scope,
            CssRuleType::NestedDeclarations,
            CssRuleType::Result,
            CssRuleType::Apply,
            CssRuleType::Contents,
        ]
        .into_iter()
        .find(|kind| rule_types.contains(*kind))
        .and_then(Self::declaration_rule_for);
        Self {
            rule_types,
            declaration_rule_type,
            parse_relative: parse_nested_rule_type
                .map_or(ParseRelative::No, Self::parse_relative_for),
        }
    }

    /// Create a new nesting context based on the given rule.
    pub fn new_from_rule(rule_type: Option<CssRuleType>) -> Self {
        Self {
            rule_types: rule_type.map(CssRuleTypes::from).unwrap_or_default(),
            declaration_rule_type: rule_type.and_then(Self::declaration_rule_for),
            parse_relative: rule_type
                .map(Self::parse_relative_for)
                .unwrap_or(ParseRelative::No),
        }
    }

    /// Save the current nesting context.
    pub fn save(&mut self, rule_type: CssRuleType) -> Self {
        let old = *self;
        self.rule_types.insert(rule_type);
        if let Some(kind) = Self::declaration_rule_for(rule_type) {
            self.declaration_rule_type = Some(kind);
        }
        let new_parse_relative = Self::parse_relative_for(rule_type);
        if new_parse_relative != ParseRelative::No {
            self.parse_relative = new_parse_relative;
        }
        old
    }

    /// Load the saved nesting context.
    pub fn restore(&mut self, saved: Self) {
        *self = saved;
    }
}

/// The data that the parser needs from outside in order to parse a stylesheet.
#[derive(Clone)]
pub struct ParserContext<'a> {
    /// The `Origin` of the stylesheet, whether it's a user, author or
    /// user-agent stylesheet.
    pub stylesheet_origin: Origin,
    /// The extra data we need for resolving url values.
    pub url_data: &'a UrlExtraData,
    /// The mode to use when parsing.
    pub parsing_mode: ParsingMode,
    /// The quirks mode of this stylesheet.
    pub quirks_mode: QuirksMode,
    /// The active error reporter, or none if error reporting is disabled.
    error_reporter: Option<&'a dyn ParseErrorReporter>,
    /// The currently active namespaces.
    pub namespaces: Cow<'a, Namespaces>,
    /// The use counters we want to record while parsing style rules, if any.
    pub use_counters: Option<&'a UseCounters>,
    /// Current nesting context.
    pub nesting_context: NestingContext,
    /// The ranges of the parsed text that `attr()` substitution produced.
    attr_tainted: &'a [AttrTaintedRange],
    random_declaration: Option<std::sync::Arc<crate::values::random::RandomDeclarationContext>>,
}

impl<'a> ParserContext<'a> {
    /// Create a parser context.
    #[inline]
    pub fn new(
        stylesheet_origin: Origin,
        url_data: &'a UrlExtraData,
        rule_type: Option<CssRuleType>,
        parsing_mode: ParsingMode,
        quirks_mode: QuirksMode,
        namespaces: Cow<'a, Namespaces>,
        error_reporter: Option<&'a dyn ParseErrorReporter>,
        use_counters: Option<&'a UseCounters>,
    ) -> Self {
        Self {
            stylesheet_origin,
            url_data,
            parsing_mode,
            quirks_mode,
            error_reporter,
            namespaces,
            use_counters,
            nesting_context: NestingContext::new_from_rule(rule_type),
            attr_tainted: &[],
            random_declaration: None,
        }
    }

    /// Bind the original property and lexical random occurrences before any shorthand expansion.
    pub fn for_random_declaration<'i>(&self, property: &str, input: &mut Parser<'i, '_>) -> Self {
        self.for_random_declaration_with_offset(property, 0, input)
    }

    /// Bind an early typed subexpression after preceding functions in the declaration's early stream.
    pub fn for_random_declaration_with_offset<'i>(&self, property: &str, offset: usize, input: &mut Parser<'i, '_>) -> Self {
        let mut context = self.clone();
        let mut declaration = crate::values::random::RandomDeclarationContext::new(crate::Atom::from(property), input);
        declaration.index_offset = offset;
        context.random_declaration = Some(std::sync::Arc::new(declaration));
        context
    }

    /// The stable random occurrence immediately after the opening function token.
    pub fn random_function_identity(&self, position: SourcePosition) -> crate::values::random::RandomFunctionIdentity {
        self.random_function_identity_by_byte(position.byte_index())
    }

    /// Resolve a retained substitution function's absolute source position.
    pub fn random_function_identity_by_byte(&self, byte: usize) -> crate::values::random::RandomFunctionIdentity {
        self.random_declaration.as_ref().map_or_else(
            || crate::values::random::RandomFunctionIdentity { property: crate::Atom::from(""), index: 0 },
            |declaration| declaration.identity(byte),
        )
    }

    /// Whether random auto/property scopes have an original declaration identity.
    pub fn random_declaration_is_bound(&self) -> bool { self.random_declaration.is_some() }

    /// The early-substitution occurrence of a retained arbitrary random function.
    pub fn random_early_function_identity_by_byte(&self, byte: usize) -> crate::values::random::RandomFunctionIdentity {
        self.random_declaration.as_ref().map_or_else(
            || self.random_function_identity_by_byte(byte),
            |declaration| declaration.early_identity(byte),
        )
    }

    /// Random functions are values of declarations, never media/container feature values or descriptors.
    pub fn random_functions_allowed(&self) -> bool {
        let rules = self.rule_types();
        ![
            CssRuleType::FontFace,
            CssRuleType::FontFeatureValues,
            CssRuleType::FontPaletteValues,
            CssRuleType::CounterStyle,
            CssRuleType::Property,
            CssRuleType::ColorProfile,
        ].into_iter().any(|rule| rules.contains(rule))
            && !(self.declaration_rule_types().is_empty()
                && (rules.contains(CssRuleType::Media) || rules.contains(CssRuleType::Container)))
    }

    /// Parse text whose `ranges` came from `attr()` substitution.
    pub fn with_attr_tainted(self, ranges: &'a [AttrTaintedRange]) -> Self {
        Self {
            attr_tainted: ranges,
            ..self
        }
    }

    /// Reject a `<url>` that starts at `start` in `attr()`-tainted text.
    ///
    /// <https://drafts.csswg.org/css-values-5/#attr-security>
    pub fn reject_attr_tainted_url<'i>(
        &self,
        input: &Parser<'i, '_>,
        start: SourcePosition,
    ) -> Result<(), ParseError<'i>> {
        let index = start.byte_index();
        if self
            .attr_tainted
            .iter()
            .any(|range| (range.start..range.end).contains(&index))
        {
            return Err(input.new_custom_error(style_traits::StyleParseErrorKind::UnspecifiedError));
        }
        Ok(())
    }

    /// Temporarily sets the rule_type and executes the callback function, returning its result.
    pub fn nest_for_rule<R>(
        &mut self,
        rule_type: CssRuleType,
        cb: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let old = self.nesting_context.save(rule_type);
        let r = cb(self);
        self.nesting_context.restore(old);
        r
    }

    /// Whether we're in a @page rule.
    #[inline]
    pub fn in_page_rule(&self) -> bool {
        self.nesting_context.rule_types.contains(CssRuleType::Page)
    }

    /// Returns whether !important declarations are forbidden.
    #[inline]
    pub fn allows_important_declarations(&self) -> bool {
        !self
            .nesting_context
            .rule_types
            .intersects(CssRuleTypes::IMPORTANT_FORBIDDEN)
    }

    /// Rule grammar used to admit property declarations, independently of ancestors.
    pub fn declaration_rule_types(&self) -> CssRuleTypes {
        self.nesting_context
            .declaration_rule_type
            .map(CssRuleTypes::from)
            .unwrap_or_default()
    }

    /// Get the rule type, which assumes that one is available.
    pub fn rule_types(&self) -> CssRuleTypes {
        self.nesting_context.rule_types
    }

    /// Returns whether CSS error reporting is enabled.
    #[inline]
    pub fn error_reporting_enabled(&self) -> bool {
        self.error_reporter.is_some()
    }

    /// Record a CSS parse error with this context’s error reporting.
    pub fn log_css_error(&self, location: SourceLocation, error: ContextualParseError) {
        let error_reporter = match self.error_reporter {
            Some(r) => r,
            None => return,
        };

        error_reporter.report_error(self.url_data, location, error)
    }

    /// Whether we're in a user-agent stylesheet.
    #[inline]
    pub fn in_ua_sheet(&self) -> bool {
        self.stylesheet_origin == Origin::UserAgent
    }

    /// Returns whether chrome-only rules should be parsed.
    #[inline]
    pub fn chrome_rules_enabled(&self) -> bool {
        self.url_data.chrome_rules_enabled() || self.stylesheet_origin != Origin::Author
    }

    /// Whether the parsing mode allows units or functions that are not computationally independent.
    #[inline]
    pub fn allows_computational_dependence(&self) -> bool {
        self.parsing_mode.allows_computational_dependence()
    }
}

/// A trait to abstract parsing of a specified value given a `ParserContext` and
/// CSS input.
///
/// This can be derived on keywords with `#[derive(Parse)]`.
///
/// The derive code understands the following attributes on each of the variants:
///
///  * `#[parse(aliases = "foo,bar")]` can be used to alias a value with another
///    at parse-time.
///
///  * `#[parse(condition = "function")]` can be used to make the parsing of the
///    value conditional on `function`, which needs to fulfill
///    `fn(&ParserContext) -> bool`.
///
///  * `#[parse(parse_fn = "function")]` can be used to specify a function other than Parser::parse
///    for a particular variant.
pub trait Parse: Sized {
    /// Parse a value of this type.
    ///
    /// Returns an error on failure.
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>>;
}

impl<T> Parse for Vec<T>
where
    T: Parse + OneOrMoreSeparated,
    <T as OneOrMoreSeparated>::S: Separator,
{
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        <T as OneOrMoreSeparated>::S::parse(input, |i| T::parse(context, i))
    }
}

impl<T> Parse for Box<T>
where
    T: Parse,
{
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        T::parse(context, input).map(Box::new)
    }
}

impl Parse for crate::OwnedStr {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(input.expect_string()?.as_ref().to_owned().into())
    }
}

impl Parse for UnicodeRange {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Ok(UnicodeRange::parse(input)?)
    }
}
