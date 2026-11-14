use cssparser::{Parser, ParserInput};
use selectors::matching::QuirksMode;
use style::{
    parser::ParserContext,
    stylesheets::{CssRuleType, Origin, UrlExtraData},
    values::specified::source_size_list::SourceSizeList,
};
use style_traits::ParsingMode;

pub fn parse(source: &str, quirks_mode: QuirksMode) -> SourceSizeList {
    crate::context::initialise_required_servo_style_prefs();
    let url_data: UrlExtraData = crate::context::ABOUT_BLANK.clone().into();
    let context = ParserContext::new(
        Origin::Author,
        &url_data,
        Some(CssRuleType::Style),
        ParsingMode::DEFAULT,
        quirks_mode,
        Default::default(),
        None,
        None,
    );
    let mut input = ParserInput::new(source);
    SourceSizeList::parse(&context, &mut Parser::new(&mut input))
}

