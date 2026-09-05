#![allow(clippy::elidable_lifetime_names)]
#![allow(clippy::needless_continue)]
#![allow(clippy::single_match_else)]
#![allow(clippy::match_same_arms)]

use cssparser::{Parser, ParserInput, Token};

use super::CompatMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MetadataKind {
    Title,

    Author,

    Subject,

    Keywords,

    Xmp,

    Creator,

    Producer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataDeclaration {
    pub kind: MetadataKind,

    pub values: Vec<String>,

    pub url: Option<String>,

    pub source_property: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataWarningKind {
    UnknownVendor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataWarning {
    pub kind: MetadataWarningKind,

    pub property: String,

    pub line: u32,

    pub column: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ExtractedMetadata {
    pub declarations: Vec<MetadataDeclaration>,

    pub warnings: Vec<MetadataWarning>,
}

impl ExtractedMetadata {
    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty() && self.warnings.is_empty()
    }
}

pub fn extract_metadata(css: &str, compat: CompatMode) -> ExtractedMetadata {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut out = ExtractedMetadata::default();
    walk_top_level(&mut parser, compat, &mut out);
    out
}

fn walk_top_level<'i>(
    parser: &mut Parser<'i, '_>,
    compat: CompatMode,
    out: &mut ExtractedMetadata,
) {
    loop {
        parser.skip_whitespace();
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::AtKeyword(_) => {
                    consume_at_rule(parser, compat, out);
                },
                Token::WhiteSpace(_) | Token::Comment(_) => {},
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return;
                },
                _ => {
                    parser.reset(&snapshot);
                    if walk_qualified_rule(parser, compat, out).is_err() {
                        return;
                    }
                },
            },
            Err(_) => return,
        }
    }
}

fn consume_at_rule<'i>(
    parser: &mut Parser<'i, '_>,
    compat: CompatMode,
    out: &mut ExtractedMetadata,
) {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Semicolon => return,
                Token::CurlyBracketBlock => {
                    let _ = parser.parse_nested_block(
                        |inner| -> Result<(), cssparser::ParseError<'i, ()>> {
                            walk_top_level(inner, compat, out);
                            Ok(())
                        },
                    );
                    return;
                },
                _ => continue,
            },
            Err(_) => return,
        }
    }
}

fn walk_qualified_rule<'i>(
    parser: &mut Parser<'i, '_>,
    compat: CompatMode,
    out: &mut ExtractedMetadata,
) -> Result<(), ()> {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::CurlyBracketBlock => {
                    let _ = parser.parse_nested_block(
                        |inner| -> Result<(), cssparser::ParseError<'i, ()>> {
                            scan_block(inner, compat, out);
                            Ok(())
                        },
                    );
                    return Ok(());
                },
                Token::Semicolon => return Ok(()),
                _ => continue,
            },
            Err(_) => return Err(()),
        }
    }
}

fn scan_block<'i>(parser: &mut Parser<'i, '_>, compat: CompatMode, out: &mut ExtractedMetadata) {
    loop {
        parser.skip_whitespace();
        let decl_start = parser.current_source_location();
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Ident(ident) => {
                    let name = ident.as_ref();
                    let classified = classify(name, compat);
                    let is_foreign = classified.is_none() && is_known_vendor_metadata(name);

                    match parser.next() {
                        Ok(Token::Colon) => {},
                        _ => {
                            skip_to_decl_end(parser);
                            continue;
                        },
                    }

                    if let Some(kind) = classified {
                        let values = parse_string_value(parser);
                        if !values.is_empty() {
                            out.declarations.push(MetadataDeclaration {
                                kind,
                                values,
                                url: None,
                                source_property: name.to_string(),
                            });
                        }
                    } else if is_foreign {
                        out.warnings.push(MetadataWarning {
                            kind: MetadataWarningKind::UnknownVendor,
                            property: name.to_string(),
                            line: decl_start.line + 1,
                            column: decl_start.column,
                        });
                        skip_to_decl_end(parser);
                    } else {
                        skip_to_decl_end(parser);
                    }
                },
                Token::AtKeyword(_) => {
                    consume_at_rule(parser, compat, out);
                },
                Token::CurlyBracketBlock => {
                    let _ = parser.parse_nested_block(
                        |inner| -> Result<(), cssparser::ParseError<'i, ()>> {
                            scan_block(inner, compat, out);
                            Ok(())
                        },
                    );
                },
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return;
                },
                Token::Semicolon | Token::WhiteSpace(_) | Token::Comment(_) => {},
                _ => {
                    skip_to_decl_end(parser);
                },
            },
            Err(_) => return,
        }
    }
}

fn parse_string_value<'i>(parser: &mut Parser<'i, '_>) -> Vec<String> {
    let mut strings: Vec<String> = Vec::new();
    let mut saw_none = false;
    loop {
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::QuotedString(ref s) => {
                    strings.push(s.as_ref().to_string());
                },
                Token::Ident(ref ident) => {
                    if ident.eq_ignore_ascii_case("none") {
                        saw_none = true;
                    } else if ident.eq_ignore_ascii_case("important") {
                    } else {
                        skip_to_decl_end_from_state(parser, &snapshot);
                        return if saw_none { Vec::new() } else { strings };
                    }
                },
                Token::Delim('!') => {},
                Token::Comma | Token::WhiteSpace(_) | Token::Comment(_) => {},
                Token::Semicolon => {
                    return if saw_none { Vec::new() } else { strings };
                },
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return if saw_none { Vec::new() } else { strings };
                },
                _ => {
                    skip_to_decl_end_from_state(parser, &snapshot);
                    return if saw_none { Vec::new() } else { strings };
                },
            },
            Err(_) => return if saw_none { Vec::new() } else { strings },
        }
    }
}

pub(super) fn skip_to_decl_end<'i>(parser: &mut Parser<'i, '_>) {
    loop {
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Semicolon => return,
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return;
                },
                _ => continue,
            },
            Err(_) => return,
        }
    }
}

fn skip_to_decl_end_from_state<'i>(parser: &mut Parser<'i, '_>, initial: &cssparser::ParserState) {
    parser.reset(initial);
    skip_to_decl_end(parser);
}

fn classify(name: &str, compat: CompatMode) -> Option<MetadataKind> {
    let lower = ascii_lower(name);
    match lower.as_str() {
        "-ro-title" if compat == CompatMode::PdfReactor => Some(MetadataKind::Title),
        "-ro-author" if compat == CompatMode::PdfReactor => Some(MetadataKind::Author),
        "-ro-subject" if compat == CompatMode::PdfReactor => Some(MetadataKind::Subject),
        "-ro-keywords" if compat == CompatMode::PdfReactor => Some(MetadataKind::Keywords),
        _ => None,
    }
}

fn is_known_vendor_metadata(name: &str) -> bool {
    let lower = ascii_lower(name);
    matches!(
        lower.as_str(),
        "-ro-title" | "-ro-author" | "-ro-subject" | "-ro-keywords"
    )
}

fn ascii_lower(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push(c.to_ascii_lowercase());
    }
    out
}
