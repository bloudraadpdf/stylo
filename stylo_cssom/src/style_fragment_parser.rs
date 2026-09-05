use cssparser::{Parser, ParserInput};
use style::{
    parser::ParserContext,
    stylesheets::{CssRuleType, Origin, UrlExtraData},
};
use style_traits::{ParseError, ParsingMode};

pub fn parse_style_fragment_with<T, F>(css: &str, f: F) -> Option<T>
where
    F: for<'i, 't> FnOnce(&ParserContext<'_>, &mut Parser<'i, 't>) -> Result<T, ParseError<'i>>,
{
    parse_fragment_with_rule_type(css, CssRuleType::Style, f)
}

pub fn parse_fragment_with_rule_type<T, F>(css: &str, rule_type: CssRuleType, f: F) -> Option<T>
where
    F: for<'i, 't> FnOnce(&ParserContext<'_>, &mut Parser<'i, 't>) -> Result<T, ParseError<'i>>,
{
    let url_data: UrlExtraData = crate::context::ABOUT_BLANK.clone().into();
    let context = ParserContext::new(
        Origin::Author,
        &url_data,
        Some(rule_type),
        ParsingMode::DEFAULT,
        selectors::matching::QuirksMode::NoQuirks,
        #[allow(clippy::default_trait_access)]
        Default::default(),
        None,
        None,
    );
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    parser.parse_entirely(|input| f(&context, input)).ok()
}
