use std::{collections::BTreeMap, ops::Range};

use cssparser::{Parser, ParserInput, SourceLocation, Token};
use style::{shared_lock::SharedRwLockReadGuard, stylesheets::CssRule};

pub fn stylesheet_parser_input(css: &str) -> ParserInput<'_> {
    ParserInput::new_with_url_error_recovery(css, cssparser::UrlErrorRecovery::Css2)
}

pub struct ScannedRuleSource {
    pub text: String,
    pub location: SourceLocation,
}

pub(super) struct RuleSourceSpan {
    range: Range<usize>,
    pub block_end: Option<usize>,
}

pub(super) struct AuthoredSource<'a> {
    text: &'a str,
    recovery: cssparser::UrlErrorRecovery,
    positions: BTreeMap<(u32, u32), usize>,
}

impl<'a> AuthoredSource<'a> {
    pub fn new(text: &'a str, recovery: cssparser::UrlErrorRecovery) -> Self {
        let mut positions = BTreeMap::new();
        let mut input = ParserInput::new_with_url_error_recovery(text, recovery);
        index_positions(&mut Parser::new(&mut input), &mut positions);
        Self {
            text,
            recovery,
            positions,
        }
    }

    pub const fn text(&self) -> &'a str {
        self.text
    }

    pub fn position(&self, location: SourceLocation) -> Option<usize> {
        self.positions
            .get(&(location.line, location.column))
            .copied()
    }

    pub fn span(&self, location: SourceLocation) -> Option<RuleSourceSpan> {
        Some(span_at(self.text, self.position(location)?, self.recovery))
    }
}

impl RuleSourceSpan {
    pub fn text<'a>(&self, css: &'a str) -> &'a str {
        &css[self.range.clone()]
    }
}

pub(super) fn location(rule: &CssRule, guard: &SharedRwLockReadGuard<'_>) -> SourceLocation {
    match rule {
        CssRule::Style(rule) => rule.read_with(guard).source_location,
        CssRule::Namespace(rule) => rule.source_location,
        CssRule::Import(rule) => rule.read_with(guard).source_location,
        CssRule::Media(rule) => rule.source_location,
        CssRule::CustomMedia(rule) => rule.source_location,
        CssRule::Container(rule) => rule.source_location,
        CssRule::FontFace(rule) => rule.read_with(guard).source_location,
        CssRule::FontFeatureValues(rule) => rule.source_location,
        CssRule::FontPaletteValues(rule) => rule.source_location,
        CssRule::CounterStyle(rule) => rule.read_with(guard).source_location,
        CssRule::Keyframes(rule) => rule.read_with(guard).source_location,
        CssRule::Margin(rule) => rule.source_location,
        CssRule::Footnote(rule) => rule.source_location,
        CssRule::Sidenote(rule) => rule.source_location,
        CssRule::BdColour(rule) => rule.source_location,
        CssRule::ColorProfile(rule) => rule.source_location,
        CssRule::Region(rule) => rule.source_location,
        CssRule::Supports(rule) => rule.source_location,
        CssRule::When(rule) => rule.source_location,
        CssRule::Else(rule) => rule.source_location,
        CssRule::Page(rule) => rule.read_with(guard).source_location,
        CssRule::Property(rule) => rule.source_location,
        CssRule::Document(rule) => rule.source_location,
        CssRule::LayerBlock(rule) => rule.source_location,
        CssRule::LayerStatement(rule) => rule.source_location,
        CssRule::Scope(rule) => rule.source_location,
        CssRule::StartingStyle(rule) => rule.source_location,
        CssRule::PositionTry(rule) => rule.read_with(guard).source_location,
        CssRule::NestedDeclarations(rule) => rule.read_with(guard).source_location,
    }
}

fn index_positions(input: &mut Parser<'_, '_>, positions: &mut BTreeMap<(u32, u32), usize>) {
    while !input.is_exhausted() {
        input.skip_whitespace();
        let position = input.position();
        let current = input.current_source_location();
        let token = match input.next().cloned() {
            Ok(token) => token,
            Err(_) => continue,
        };
        positions.insert((current.line, current.column), position.byte_index());
        if opens_block(&token) {
            let _ = input.parse_nested_block(|input| {
                index_positions(input, positions);
                Ok::<_, cssparser::ParseError<'_, ()>>(())
            });
        }
    }
}

fn opens_block(token: &Token<'_>) -> bool {
    matches!(
        token,
        Token::CurlyBracketBlock
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::Function(_)
    )
}

fn exhaust(input: &mut Parser<'_, '_>) {
    while !input.is_exhausted() {
        let _ = input.next_including_whitespace_and_comments();
    }
}

fn consume_rule(input: &mut Parser<'_, '_>) -> Option<Range<usize>> {
    while !input.is_exhausted() {
        let token = match input.next_including_whitespace_and_comments().cloned() {
            Ok(token) => token,
            Err(_) => continue,
        };
        match token {
            Token::Semicolon => return None,
            Token::CurlyBracketBlock => {
                let start = input.position().byte_index();
                return input
                    .parse_nested_block(|input| {
                        exhaust(input);
                        Ok::<_, cssparser::ParseError<'_, ()>>(start..input.position().byte_index())
                    })
                    .ok();
            },
            _ => {},
        }
    }
    None
}

fn span_at(css: &str, start: usize, recovery: cssparser::UrlErrorRecovery) -> RuleSourceSpan {
    let mut input = ParserInput::new_with_url_error_recovery(&css[start..], recovery);
    let mut input = Parser::new(&mut input);
    let block = consume_rule(&mut input);
    RuleSourceSpan {
        range: start..start + input.position().byte_index(),
        block_end: block.map(|block| start + block.end),
    }
}

pub(super) fn canonical_group_header(css: &str) -> stylo_cssom_model::RuleGroupHeader {
    let mut input = stylesheet_parser_input(css);
    let mut parser = Parser::new(&mut input);
    loop {
        let start = parser.position();
        if matches!(
            parser
                .next_including_whitespace_and_comments()
                .expect("a native grouping rule has a block"),
            Token::CurlyBracketBlock
        ) {
            return stylo_cssom_model::RuleGroupHeader::new(css[..start.byte_index()].trim_end());
        }
    }
}

pub fn forgiving_rule_sources(css: &str) -> Vec<ScannedRuleSource> {
    let mut input = stylesheet_parser_input(css);
    let mut input = Parser::new(&mut input);
    let mut rules = Vec::new();
    while !input.is_exhausted() {
        let source_start = input.position();
        input.skip_whitespace();
        let start = input.state();
        match input.next().cloned() {
            Err(_) | Ok(Token::CDO | Token::CDC) => continue,
            Ok(_) => input.reset(&start),
        }
        consume_rule(&mut input);
        let rule = css[source_start.byte_index()..input.position().byte_index()].trim();
        if !rule.is_empty() {
            rules.push(ScannedRuleSource {
                text: rule.to_owned(),
                location: start.source_location(),
            });
        }
    }
    rules
}
