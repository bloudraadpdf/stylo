/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Named URL patterns for CSS Navigation 1.
use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::shared_lock::{DeepCloneWithLock, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard};
use crate::stylesheets::UrlExtraData;
use crate::values::{specified::url::SpecifiedUrl, DashedIdent};
use cssparser::{
    AtRuleParser, CowRcStr, DeclarationParser, Parser, ParserState, QualifiedRuleParser,
    RuleBodyItemParser, RuleBodyParser, SourceLocation,
};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// The base against which a named URL pattern is resolved.
#[derive(Clone, Debug, Default, MallocSizeOf, PartialEq, ToShmem)]
pub enum LocationBase {
    /// The containing stylesheet's resource base URL.
    #[default]
    Stylesheet,
    /// The current document base URL.
    Document,
    /// An explicit CSS URL.
    Url(SpecifiedUrl),
}

/// A named URL pattern and its declaration context.
#[derive(Clone, Debug, MallocSizeOf, ToShmem)]
pub struct LocationRule {
    /// The author-defined location name.
    pub name: DashedIdent,
    /// The last valid value of each URLPatternInit component descriptor.
    pub components: Vec<(String, String)>,
    /// The argument to the alternative url-pattern() descriptor.
    pub pattern: Option<String>,
    /// The base URL descriptor.
    pub base: Option<LocationBase>,
    /// The stylesheet's URL context.
    pub url_data: UrlExtraData,
    /// The source position of the rule.
    #[ignore_malloc_size_of = "inline source coordinates"]
    pub source_location: SourceLocation,
}

impl LocationRule {
    /// Parse a descriptor list, recovering invalid declarations independently.
    pub fn parse<'i, 't>(
        name: DashedIdent,
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
        source_location: SourceLocation,
    ) -> Result<Self, ParseError<'i>> {
        let mut rule = Self {
            name,
            components: Vec::new(),
            pattern: None,
            base: None,
            url_data: context.url_data.clone(),
            source_location,
        };
        {
            let mut parser = LocationParser {
                context,
                rule: &mut rule,
            };
            for _ in RuleBodyParser::new(input, &mut parser) {}
        }
        if rule.pattern.is_some() && !rule.components.is_empty() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        #[cfg(feature = "servo")]
        if rule.matcher(Some(context.url_data.as_str())).is_none() {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(rule)
    }

    /// Whether the effective pattern is unchanged, independent of source coordinates.
    pub fn same_pattern(&self, other: &Self) -> bool {
        self.name == other.name
            && self.pattern == other.pattern
            && self.base.as_ref().unwrap_or(&LocationBase::Stylesheet)
                == other.base.as_ref().unwrap_or(&LocationBase::Stylesheet)
            && self.url_data == other.url_data
            && self.components.len() == other.components.len()
            && self
                .components
                .iter()
                .all(|component| other.components.contains(component))
    }

    #[cfg(feature = "servo")]
    fn matcher(&self, document_base: Option<&str>) -> Option<urlpattern::UrlPattern<regex::Regex>> {
        let base_url = match self.base.as_ref().unwrap_or(&LocationBase::Stylesheet) {
            LocationBase::Stylesheet => self.url_data.0.as_ref().clone(),
            LocationBase::Document => document_base?.parse().ok()?,
            LocationBase::Url(url) => url.url()?.as_ref().clone(),
        };
        let init = if let Some(pattern) = &self.pattern {
            urlpattern::UrlPatternInit::parse_constructor_string::<regex::Regex>(
                pattern,
                Some(base_url),
            )
            .ok()?
        } else {
            let mut init = urlpattern::UrlPatternInit {
                base_url: Some(base_url),
                ..Default::default()
            };
            for (name, value) in &self.components {
                let target = match name.as_str() {
                    "protocol" => &mut init.protocol,
                    "hostname" => &mut init.hostname,
                    "port" => &mut init.port,
                    "pathname" => &mut init.pathname,
                    "search" => &mut init.search,
                    "hash" => &mut init.hash,
                    _ => return None,
                };
                *target = Some(value.clone());
            }
            init
        };
        urlpattern::UrlPattern::parse(init, Default::default()).ok()
    }

    /// Match with the stylesheet or an explicit base URL.
    #[cfg(feature = "servo")]
    pub fn matches(&self, address: &str) -> bool {
        self.matches_with_document_base(address, None)
    }

    /// Match a URL using the document base supplied by the style device when requested.
    #[cfg(feature = "servo")]
    pub fn matches_with_document_base(&self, address: &str, document_base: Option<&str>) -> bool {
        let Some(pattern) = self.matcher(document_base) else {
            return false;
        };
        let Ok(url) = address.parse() else {
            return false;
        };
        pattern
            .test(urlpattern::UrlPatternMatchInput::Url(url))
            .unwrap_or(false)
    }
}

struct LocationParser<'a, 'b: 'a> {
    context: &'a ParserContext<'b>,
    rule: &'a mut LocationRule,
}

impl<'a, 'b, 'i> DeclarationParser<'i> for LocationParser<'a, 'b> {
    type Declaration = ();
    type Error = StyleParseErrorKind<'i>;

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _: &ParserState,
    ) -> Result<(), ParseError<'i>> {
        let name = name.to_ascii_lowercase();
        match name.as_str() {
            "protocol" | "hostname" | "port" | "pathname" | "search" | "hash" => {
                let value = input.expect_string_cloned()?.to_string();
                input.expect_exhausted()?;
                self.rule
                    .components
                    .retain(|(component, _)| component != &name);
                self.rule.components.push((name, value));
            },
            "pattern" => {
                input.expect_function_matching("url-pattern")?;
                let pattern = input
                    .parse_nested_block(|input| Ok(input.expect_string_cloned()?.to_string()))?;
                input.expect_exhausted()?;
                self.rule.pattern = Some(pattern);
            },
            "base-url" => {
                let base = if input
                    .try_parse(|input| input.expect_ident_matching("stylesheet"))
                    .is_ok()
                {
                    LocationBase::Stylesheet
                } else if input
                    .try_parse(|input| input.expect_ident_matching("document"))
                    .is_ok()
                {
                    LocationBase::Document
                } else {
                    LocationBase::Url(SpecifiedUrl::parse(self.context, input)?)
                };
                input.expect_exhausted()?;
                self.rule.base = Some(base);
            },
            _ => return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
        }
        Ok(())
    }
}

impl<'a, 'b, 'i> AtRuleParser<'i> for LocationParser<'a, 'b> {
    type Prelude = ();
    type AtRule = ();
    type Error = StyleParseErrorKind<'i>;
}
impl<'a, 'b, 'i> QualifiedRuleParser<'i> for LocationParser<'a, 'b> {
    type Prelude = ();
    type QualifiedRule = ();
    type Error = StyleParseErrorKind<'i>;
}
impl<'a, 'b, 'i> RuleBodyItemParser<'i, (), StyleParseErrorKind<'i>> for LocationParser<'a, 'b> {
    fn parse_qualified(&self) -> bool {
        false
    }
    fn parse_declarations(&self) -> bool {
        true
    }
}

impl DeepCloneWithLock for LocationRule {
    fn deep_clone_with_lock(&self, _: &SharedRwLock, _: &SharedRwLockReadGuard) -> Self {
        self.clone()
    }
}
impl ToCssWithGuard for LocationRule {
    fn to_css(
        &self,
        _: &SharedRwLockReadGuard,
        dest: &mut style_traits::CssStringWriter,
    ) -> fmt::Result {
        dest.write_str("@location ")?;
        self.name.to_css(&mut CssWriter::new(dest))?;
        dest.write_str(" { ")?;
        for (name, value) in &self.components {
            write!(dest, "{name}: ")?;
            cssparser::serialize_string(value, dest)?;
            dest.write_str("; ")?;
        }
        if let Some(pattern) = &self.pattern {
            dest.write_str("pattern: url-pattern(")?;
            cssparser::serialize_string(pattern, dest)?;
            dest.write_str("); ")?;
        }
        match &self.base {
            None => {},
            Some(LocationBase::Stylesheet) => dest.write_str("base-url: stylesheet; ")?,
            Some(LocationBase::Document) => dest.write_str("base-url: document; ")?,
            Some(LocationBase::Url(url)) => {
                dest.write_str("base-url: ")?;
                url.to_css(&mut CssWriter::new(dest))?;
                dest.write_str("; ")?;
            },
        }
        dest.write_char('}')
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin};
    use cssparser::ParserInput;
    use style_traits::ParsingMode;

    fn parse_at_base(body: &str, base: &str) -> Option<LocationRule> {
        let url_data = UrlExtraData::from(url::Url::parse(base).unwrap());
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
        let mut name = ParserInput::new("--route");
        let name = DashedIdent::parse(&context, &mut Parser::new(&mut name)).unwrap();
        let mut body = ParserInput::new(body);
        let mut input = Parser::new(&mut body);
        let location = input.current_source_location();
        input
            .parse_entirely(|input| LocationRule::parse(name, &context, input, location))
            .ok()
    }
    fn parse(body: &str) -> Option<LocationRule> {
        parse_at_base(body, "https://example.test/styles/main.css")
    }

    #[test]
    fn location_descriptors_recover_and_keep_the_last_valid_value() {
        let rule = parse("unknown: url(no); pathname: '/wrong'; pathname: '/movies/:id'; pathname: 7; hostname: 'example.test' !important; @unknown { x: y }").unwrap();
        assert!(rule.matches("https://example.test/movies/123"));
        assert!(!rule.matches("https://other.test/movies/123"));
        assert!(!rule.matches("https://example.test/movies/123/extra"));
        assert!(!rule.matches("https://example.test/wrong"));
        let same = parse("pathname:'/movies/:id'").unwrap();
        assert!(rule.same_pattern(&same));
        assert!(same.same_pattern(&parse("base-url:stylesheet;pathname:'/movies/:id'").unwrap()));
        assert!(!rule.same_pattern(
            &parse_at_base(
                "pathname:'/movies/:id'",
                "https://other.test/styles/main.css"
            )
            .unwrap()
        ));
    }

    #[test]
    fn location_patterns_use_urlpattern_capture_and_regexp_semantics() {
        for body in [
            r#"pathname: '/movies/:id(\\d+)';"#,
            r#"pattern: url-pattern('/movies/:id(\\d+)');"#,
        ] {
            let rule = parse(body).unwrap();
            assert!(rule.matches("https://example.test/movies/123"));
            assert!(!rule.matches("https://example.test/movies/words"));
            assert!(!rule.matches("https://example.test/movies/123/"));
        }
        let wildcard = parse("pathname:'/*/movies'").unwrap();
        assert!(wildcard.matches("https://example.test/en/gb/movies"));
        assert!(parse("pattern:url-pattern('/movies/*');pathname:'/movies/*'").is_none());
        assert!(parse("pathname:'/movies/:id('").is_none());
    }

    #[test]
    fn location_bases_distinguish_stylesheet_document_and_explicit_urls() {
        let stylesheet = parse("pattern:url-pattern('item/:id')").unwrap();
        assert!(stylesheet.matches("https://example.test/styles/item/123"));
        let document = parse("pathname:'item/:id';base-url:document").unwrap();
        assert!(!document.matches("https://example.test/styles/item/123"));
        assert!(document.matches_with_document_base(
            "https://document.test/app/item/123",
            Some("https://document.test/app/")
        ));
        let explicit =
            parse("pattern:url-pattern('item/:id');base-url:url('https://shop.test/catalog/')")
                .unwrap();
        assert!(explicit.matches("https://shop.test/catalog/item/123"));
        assert!(!explicit.matches("https://example.test/styles/item/123"));
        let lock = SharedRwLock::new();
        let css = explicit.to_css_string(&lock.read());
        assert!(css.contains("pattern: url-pattern(\"item/:id\")"));
        assert!(css.contains("base-url: url("));
    }

    #[test]
    fn location_components_use_url_canonicalisation() {
        let rule = parse("protocol:'HTTPS';hostname:'EXAMPLE.TEST';pathname:'/movies/:id';search:'q=:query';hash:':section'").unwrap();
        assert!(rule.matches("https://example.test/movies/123?q=foo#summary"));
        assert!(!rule.matches("https://other.test/movies/123?q=foo#summary"));
        let default_port = parse("protocol:'https';hostname:'example.test';port:'443';pathname:'/movies/:id'").unwrap();
        assert!(default_port.matches("https://example.test/movies/123"));
        assert!(!default_port.matches("https://example.test:444/movies/123"));
    }
}
