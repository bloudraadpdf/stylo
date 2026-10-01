use std::sync::Arc;

use cssparser::{Parser, ParserInput, ToCss, Token, TokenSerializationType};
pub use stylo_cssom_model::{
    SyntaxBlockKind, SyntaxBodyKind, SyntaxOptions, SyntaxParseError, SyntaxRule, SyntaxValue,
};

#[derive(Clone, Debug)]
enum Component {
    Atom {
        kind: AtomKind,
        text: Arc<str>,
    },
    Block {
        kind: SyntaxBlockKind,
        body: Vec<Self>,
    },
    Function {
        name: Arc<str>,
        body: Vec<Self>,
    },
}

#[derive(Clone, Debug, PartialEq)]
enum AtomKind {
    Ident(Arc<str>),
    AtKeyword(Arc<str>),
    Whitespace,
    Colon,
    Semicolon,
    Comma,
    Cdo,
    Cdc,
    Other,
}

impl Component {
    fn is(&self, expected: &AtomKind) -> bool {
        matches!(self, Self::Atom { kind, .. } if kind == expected)
    }

    fn value(&self) -> SyntaxValue {
        match self {
            Self::Atom { text, .. } => SyntaxValue::Token(text.clone()),
            Self::Block { kind, body } => SyntaxValue::Block {
                kind: *kind,
                body: values(body),
            },
            Self::Function { name, body } => SyntaxValue::Function {
                name: name.clone(),
                args: comma_groups(body),
            },
        }
    }
}

fn values(components: &[Component]) -> Vec<SyntaxValue> {
    components.iter().map(Component::value).collect()
}

fn comma_groups(components: &[Component]) -> Vec<Vec<SyntaxValue>> {
    if components.is_empty() {
        return Vec::new();
    }
    components
        .split(|value| value.is(&AtomKind::Comma))
        .map(values)
        .collect()
}

fn trim(mut components: &[Component]) -> &[Component] {
    while components
        .first()
        .is_some_and(|value| value.is(&AtomKind::Whitespace))
    {
        components = &components[1..];
    }
    while components
        .last()
        .is_some_and(|value| value.is(&AtomKind::Whitespace))
    {
        components = &components[..components.len() - 1];
    }
    components
}

fn tokenize(source: &str) -> Result<Vec<Component>, SyntaxParseError> {
    let source = source
        .replace("\r\n", "\n")
        .replace(['\r', '\x0c'], "\n")
        .replace('\0', "�");
    let mut input = ParserInput::new(&source);
    consume_components(&mut Parser::new(&mut input), 0)
}

fn consume_components(
    parser: &mut Parser<'_, '_>,
    depth: usize,
) -> Result<Vec<Component>, SyntaxParseError> {
    if depth > 128 {
        return Err(SyntaxParseError::NestingLimit);
    }
    let mut components = Vec::new();
    while let Ok(token) = parser.next_including_whitespace() {
        let token = token.clone();
        let component = match token {
            Token::Function(ref name) => Component::Function {
                name: Arc::from(name.as_ref()),
                body: nested_components(parser, depth)?,
            },
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                let kind = match token {
                    Token::ParenthesisBlock => SyntaxBlockKind::Parentheses,
                    Token::SquareBracketBlock => SyntaxBlockKind::Brackets,
                    _ => SyntaxBlockKind::Braces,
                };
                let body = nested_components(parser, depth)?;
                Component::Block { kind, body }
            },
            _ => {
                let kind = match &token {
                    Token::Ident(name) => AtomKind::Ident(Arc::from(name.as_ref())),
                    Token::AtKeyword(name) => AtomKind::AtKeyword(Arc::from(name.as_ref())),
                    Token::WhiteSpace(_) => AtomKind::Whitespace,
                    Token::Colon => AtomKind::Colon,
                    Token::Semicolon => AtomKind::Semicolon,
                    Token::Comma => AtomKind::Comma,
                    Token::CDO => AtomKind::Cdo,
                    Token::CDC => AtomKind::Cdc,
                    _ => AtomKind::Other,
                };
                Component::Atom {
                    kind,
                    text: token.to_css_string().into(),
                }
            },
        };
        components.push(component);
    }
    Ok(components)
}

fn nested_components(
    parser: &mut Parser<'_, '_>,
    depth: usize,
) -> Result<Vec<Component>, SyntaxParseError> {
    parser
        .parse_nested_block(|nested| {
            consume_components(nested, depth + 1).map_err(|error| nested.new_custom_error(error))
        })
        .map_err(|error| match error.kind {
            cssparser::ParseErrorKind::Custom(error) => error,
            _ => SyntaxParseError::InvalidValue,
        })
}

pub fn parse_value_list(source: &str) -> Result<Vec<SyntaxValue>, SyntaxParseError> {
    tokenize(source).map(|components| values(&components))
}

pub fn parse_value(source: &str) -> Result<SyntaxValue, SyntaxParseError> {
    let components = tokenize(source)?;
    match trim(&components) {
        [value] => Ok(value.value()),
        _ => Err(SyntaxParseError::InvalidValue),
    }
}

pub fn parse_comma_value_list(source: &str) -> Result<Vec<Vec<SyntaxValue>>, SyntaxParseError> {
    tokenize(source).map(|components| comma_groups(&components))
}

pub fn parse_stylesheet(
    source: &str,
    options: &SyntaxOptions,
) -> Result<Vec<SyntaxRule>, SyntaxParseError> {
    tokenize(source).map(|components| rule_list(&components, options, true))
}

pub fn parse_rule_list(
    source: &str,
    options: &SyntaxOptions,
) -> Result<Vec<SyntaxRule>, SyntaxParseError> {
    tokenize(source).map(|components| rule_list(&components, options, false))
}

pub fn parse_rule(source: &str, options: &SyntaxOptions) -> Result<SyntaxRule, SyntaxParseError> {
    let components = tokenize(source)?;
    let mut input = trim(&components);
    let rule = consume_rule(&mut input, options).ok_or(SyntaxParseError::InvalidRule)?;
    if trim(input).is_empty() {
        Ok(rule)
    } else {
        Err(SyntaxParseError::InvalidRule)
    }
}

pub fn parse_declaration(source: &str) -> Result<SyntaxRule, SyntaxParseError> {
    let components = tokenize(source)?;
    let end = components
        .iter()
        .position(|value| value.is(&AtomKind::Semicolon))
        .unwrap_or(components.len());
    declaration(&components[..end]).ok_or(SyntaxParseError::InvalidDeclaration)
}

pub fn parse_declaration_list(
    source: &str,
    options: &SyntaxOptions,
) -> Result<Vec<SyntaxRule>, SyntaxParseError> {
    tokenize(source).map(|components| declaration_list(&components, options))
}

fn declaration(components: &[Component]) -> Option<SyntaxRule> {
    let components = trim(components);
    let (
        Component::Atom {
            kind: AtomKind::Ident(name),
            ..
        },
        rest,
    ) = components.split_first()?
    else {
        return None;
    };
    let rest = trim(rest);
    if !rest.first()?.is(&AtomKind::Colon) {
        return None;
    }
    let body = trim(&rest[1..]);
    let braces = body
        .iter()
        .filter(|value| {
            matches!(
                value,
                Component::Block {
                    kind: SyntaxBlockKind::Braces,
                    ..
                }
            )
        })
        .count();
    if !name.starts_with("--") && braces > 0 && trim(body).len() != 1 {
        return None;
    }
    Some(SyntaxRule::Declaration {
        name: name.clone(),
        body: values(body),
    })
}

fn declaration_list(mut input: &[Component], options: &SyntaxOptions) -> Vec<SyntaxRule> {
    let mut rules = Vec::new();
    while !input.is_empty() {
        if input[0].is(&AtomKind::Whitespace) || input[0].is(&AtomKind::Semicolon) {
            input = &input[1..];
        } else if matches!(
            input[0],
            Component::Atom {
                kind: AtomKind::AtKeyword(_),
                ..
            }
        ) {
            if let Some(rule) = consume_rule(&mut input, options) {
                rules.push(rule)
            }
        } else {
            let end = input
                .iter()
                .position(|value| value.is(&AtomKind::Semicolon))
                .unwrap_or(input.len());
            if let Some(rule) = declaration(&input[..end]) {
                rules.push(rule)
            }
            input = &input[(end + 1).min(input.len())..];
        }
    }
    rules
}

fn rule_list(
    mut input: &[Component],
    options: &SyntaxOptions,
    stylesheet: bool,
) -> Vec<SyntaxRule> {
    let mut rules = Vec::new();
    while !input.is_empty() {
        if input[0].is(&AtomKind::Whitespace)
            || (stylesheet && (input[0].is(&AtomKind::Cdo) || input[0].is(&AtomKind::Cdc)))
        {
            input = &input[1..];
        } else if let Some(rule) = consume_rule(&mut input, options) {
            rules.push(rule);
        }
    }
    rules
}

fn consume_rule(input: &mut &[Component], options: &SyntaxOptions) -> Option<SyntaxRule> {
    let name = match input.first()? {
        Component::Atom {
            kind: AtomKind::AtKeyword(name),
            ..
        } => Some(name.clone()),
        _ => None,
    };
    if name.is_some() {
        *input = &input[1..]
    }
    let end = input
        .iter()
        .position(|component| {
            matches!(
                component,
                Component::Block {
                    kind: SyntaxBlockKind::Braces,
                    ..
                }
            ) || (name.is_some() && component.is(&AtomKind::Semicolon))
        })
        .unwrap_or(input.len());
    let prelude = values(&input[..end]);
    let block = match input.get(end) {
        Some(Component::Block { body, .. }) => Some(body.as_slice()),
        _ => None,
    };
    *input = &input[(end + 1).min(input.len())..];
    match name {
        Some(name) => {
            let body = block.map(|body| match options.at_rule_body(&name) {
                SyntaxBodyKind::Rules => rule_list(body, options, false),
                SyntaxBodyKind::Declarations => declaration_list(body, options),
            });
            Some(SyntaxRule::AtRule {
                name,
                prelude,
                body,
            })
        },
        None => block.map(|body| SyntaxRule::QualifiedRule {
            prelude,
            body: declaration_list(body, options),
        }),
    }
}

fn identifier(name: &str) -> String {
    let mut result = String::new();
    cssparser::serialize_identifier(name, &mut result).expect("writing to a string cannot fail");
    result
}

pub fn serialize_value(value: &SyntaxValue) -> String {
    match value {
        SyntaxValue::Token(text) => text.to_string(),
        SyntaxValue::Block { kind, body } => {
            let name = kind.name().as_bytes();
            format!(
                "{}{}{}",
                char::from(name[0]),
                serialize_values(body),
                char::from(name[1])
            )
        },
        SyntaxValue::Function { name, args } => {
            let body = args
                .iter()
                .map(|values| serialize_values(values))
                .collect::<Vec<_>>()
                .join(",");
            format!("{}({body})", identifier(name))
        },
    }
}

fn boundaries(source: &str) -> (TokenSerializationType, TokenSerializationType) {
    let mut input = ParserInput::new(source);
    let mut parser = Parser::new(&mut input);
    let mut first = TokenSerializationType::Nothing;
    let mut last = TokenSerializationType::Nothing;
    while let Ok(token) = parser.next_including_whitespace() {
        let kind = token.serialization_type();
        first.set_if_nothing(kind);
        last = match token {
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => TokenSerializationType::Other,
            _ => kind,
        };
    }
    (first, last)
}

pub fn serialize_values(values: &[SyntaxValue]) -> String {
    let mut result = String::new();
    let mut previous = TokenSerializationType::Nothing;
    for value in values {
        let text = serialize_value(value);
        let (first, last) = boundaries(&text);
        if previous.needs_separator_when_before(first) {
            result.push_str("/**/")
        }
        result.push_str(&text);
        if last != TokenSerializationType::Nothing {
            previous = last
        }
    }
    result
}

pub fn serialize_rule(rule: &SyntaxRule) -> String {
    match rule {
        SyntaxRule::Declaration { name, body } => {
            format!("{}: {};", identifier(name), serialize_values(body))
        },
        SyntaxRule::QualifiedRule { prelude, body } => {
            format!("{}{{{}}}", serialize_values(prelude), serialize_rules(body))
        },
        SyntaxRule::AtRule {
            name,
            prelude,
            body,
        } => {
            let prefix =
                serialize_values(&[SyntaxValue::Token(format!("@{}", identifier(name)).into())]);
            let prelude = serialize_values(prelude);
            let separator = if boundaries(&prefix)
                .1
                .needs_separator_when_before(boundaries(&prelude).0)
            {
                "/**/"
            } else {
                ""
            };
            let suffix = body.as_ref().map_or_else(
                || ";".to_string(),
                |body| format!("{{{}}}", serialize_rules(body)),
            );
            format!("{prefix}{separator}{prelude}{suffix}")
        },
    }
}

fn serialize_rules(rules: &[SyntaxRule]) -> String {
    rules
        .iter()
        .map(serialize_rule)
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
#[path = "parser_api_tests.rs"]
mod tests;
