use cssparser::{Parser, ParserInput};
use style::{
    media_queries::{MediaList, MediaQuery},
    parser::ParserContext,
    stylesheets::{CssRuleType, Origin, UrlExtraData},
};
use style_traits::{ParsingMode, ToCss};

/// A CSSOM media-query list parsed and compared by the CSS engine.
#[derive(Clone)]
pub struct CssomMediaList(MediaList);

impl std::fmt::Debug for CssomMediaList {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("CssomMediaList")
            .field(&self.serialization())
            .finish()
    }
}

impl CssomMediaList {
    #[must_use]
    pub fn parse(source: &str) -> Self {
        Self(with_parser(source, |context, input| {
            MediaList::parse(context, input)
        }))
    }

    #[must_use]
    pub fn items(&self) -> Vec<String> {
        self.0
            .media_queries
            .iter()
            .map(ToCss::to_css_string)
            .collect()
    }

    #[must_use]
    pub fn serialization(&self) -> String {
        self.0.to_css_string()
    }

    pub fn append(&mut self, medium: &str) -> bool {
        let Some(query) = parse_single(medium) else {
            return false;
        };
        if !self.0.media_queries.contains(&query) {
            self.0.media_queries.push(query);
        }
        true
    }

    /// Returns `None` for invalid input, `Some(false)` when no matching query
    /// exists, and `Some(true)` after removing the matching query.
    pub fn delete(&mut self, medium: &str) -> Option<bool> {
        let query = parse_single(medium)?;
        let old_len = self.0.media_queries.len();
        self.0.media_queries.retain(|candidate| candidate != &query);
        Some(self.0.media_queries.len() != old_len)
    }
}

fn parse_single(source: &str) -> Option<MediaQuery> {
    with_parser(source, |context, input| {
        input
            .parse_entirely(|input| MediaQuery::parse(context, input))
            .ok()
    })
}

fn with_parser<T>(
    source: &str,
    parse: impl FnOnce(&ParserContext<'_>, &mut Parser<'_, '_>) -> T,
) -> T {
    with_parser_context(|context| {
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        parse(context, &mut parser)
    })
}

fn with_parser_context<T>(use_context: impl FnOnce(&ParserContext<'_>) -> T) -> T {
    let url_data: UrlExtraData = crate::context::ABOUT_BLANK.clone().into();
    let context = ParserContext::new(
        Origin::Author,
        &url_data,
        Some(CssRuleType::Style),
        ParsingMode::DEFAULT,
        selectors::matching::QuirksMode::NoQuirks,
        Default::default(),
        None,
        None,
    );
    use_context(&context)
}

#[cfg(test)]
mod tests {
    use super::CssomMediaList;

    #[test]
    fn media_lists_use_css_syntax_and_canonical_serialization() {
        let mut list = CssomMediaList::parse("screen and (min-width: 480px), print");
        assert_eq!(list.items(), ["screen and (min-width: 480px)", "print"]);
        assert!(list.append("projection"));
        assert_eq!(
            list.serialization(),
            "screen and (min-width: 480px), print, projection"
        );
        assert_eq!(list.delete("print"), Some(true));
        assert_eq!(list.delete("speech"), Some(false));
        assert_eq!(list.delete("screen, print"), None);
    }
}
