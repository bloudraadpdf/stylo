use std::{
    borrow::Cow,
    hash::{DefaultHasher, Hash, Hasher},
};

use cssparser::{Parser, ParserInput, serialize_string};

use crate::css_scan::{current_property_name, is_ident_continue, utf8_char_width};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnonymousSystem {
    Cyclic,
    Numeric,
    Alphabetic,
    Symbolic,
    Fixed,
}

impl AnonymousSystem {
    const fn css(self) -> &'static str {
        match self {
            Self::Cyclic => "cyclic",
            Self::Numeric => "numeric",
            Self::Alphabetic => "alphabetic",
            Self::Symbolic => "symbolic",
            Self::Fixed => "fixed",
        }
    }
}

struct ParsedSymbols {
    system: AnonymousSystem,
    symbols: Vec<String>,
    end: usize,
}

fn parse_symbols_call(css: &str, start: usize) -> Option<ParsedSymbols> {
    const OPEN: usize = "symbols(".len();
    let bytes = css.as_bytes();
    let mut cursor = start + OPEN;
    let mut depth = 1_usize;
    let mut quote = None;
    let mut escaped = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == delimiter {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                },
                _ => {},
            }
        }
        cursor += 1;
    }
    if depth != 0 || quote.is_some() {
        return None;
    }

    let mut input = ParserInput::new(&css[start + OPEN..cursor]);
    let mut parser = Parser::new(&mut input);
    let parsed = parser
        .parse_entirely(|input| {
            let system = input
                .try_parse(
                    |input| -> Result<AnonymousSystem, cssparser::ParseError<'_, ()>> {
                        let location = input.current_source_location();
                        let ident = input.expect_ident()?;
                        match ident.as_ref().to_ascii_lowercase().as_str() {
                            "cyclic" => Ok(AnonymousSystem::Cyclic),
                            "numeric" => Ok(AnonymousSystem::Numeric),
                            "alphabetic" => Ok(AnonymousSystem::Alphabetic),
                            "symbolic" => Ok(AnonymousSystem::Symbolic),
                            "fixed" => Ok(AnonymousSystem::Fixed),
                            _ => Err(location.new_custom_error(())),
                        }
                    },
                )
                .unwrap_or(AnonymousSystem::Symbolic);
            let mut symbols = Vec::new();
            while !input.is_exhausted() {
                symbols.push(input.expect_string()?.as_ref().to_owned());
            }
            if symbols.is_empty()
                || matches!(
                    system,
                    AnonymousSystem::Numeric | AnonymousSystem::Alphabetic
                ) && symbols.len() < 2
            {
                return Err(input.new_custom_error::<(), ()>(()));
            }
            Ok((system, symbols))
        })
        .ok()?;
    Some(ParsedSymbols {
        system: parsed.0,
        symbols: parsed.1,
        end: cursor + 1,
    })
}

fn eligible_property(out: &str) -> bool {
    current_property_name(out)
        .is_some_and(|property| matches!(property.as_str(), "list-style-type" | "content"))
}

fn generated_name(system: AnonymousSystem, symbols: &[String]) -> String {
    let mut hasher = DefaultHasher::new();
    system.css().hash(&mut hasher);
    symbols.hash(&mut hasher);
    format!("--__moegoe-anonymous-symbols-{:016x}", hasher.finish())
}

fn generated_rule(
    name: &str,
    system: AnonymousSystem,
    symbols: &[String],
) -> stylo_cssom_model::RuleNode {
    let mut serialised_symbols = String::new();
    for symbol in symbols {
        if !serialised_symbols.is_empty() {
            serialised_symbols.push(' ');
        }
        serialize_string(symbol, &mut serialised_symbols)
            .expect("writing CSS to a string is infallible");
    }
    stylo_cssom_model::RuleNode::counter_style(
        name,
        [
            stylo_cssom_model::RuleDeclaration::new("system", system.css()),
            stylo_cssom_model::RuleDeclaration::new("symbols", serialised_symbols),
            stylo_cssom_model::RuleDeclaration::new("suffix", r#"" ""#),
        ],
    )
}

pub struct SymbolsFunctionProjection<'a> {
    pub rewritten: Cow<'a, str>,
    pub counter_styles: Vec<stylo_cssom_model::RuleNode>,
}

pub fn project_symbols_functions(css: &str) -> SymbolsFunctionProjection<'_> {
    if !css
        .as_bytes()
        .windows("symbols(".len())
        .any(|window| window.eq_ignore_ascii_case(b"symbols("))
    {
        return SymbolsFunctionProjection {
            rewritten: Cow::Borrowed(css),
            counter_styles: Vec::new(),
        };
    }

    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len() + 128);
    let mut counter_styles = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            let start = cursor;
            cursor += 2;
            while cursor + 1 < bytes.len() && bytes[cursor..cursor + 2] != *b"*/" {
                cursor += 1;
            }
            cursor = (cursor + 2).min(bytes.len());
            out.push_str(&css[start..cursor]);
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'"') {
            let start = cursor;
            let quote = bytes[cursor];
            cursor += 1;
            while cursor < bytes.len() {
                if bytes[cursor] == b'\\' {
                    cursor = (cursor + 2).min(bytes.len());
                } else if bytes[cursor] == quote {
                    cursor += 1;
                    break;
                } else {
                    cursor += 1;
                }
            }
            out.push_str(&css[start..cursor]);
            continue;
        }
        let matches = cursor + "symbols(".len() <= bytes.len()
            && bytes[cursor..cursor + "symbols(".len()].eq_ignore_ascii_case(b"symbols(")
            && (cursor == 0 || !is_ident_continue(bytes[cursor - 1]));
        if matches
            && eligible_property(&out)
            && let Some(parsed) = parse_symbols_call(css, cursor)
        {
            let name = generated_name(parsed.system, &parsed.symbols);
            out.push_str(&name);
            if !counter_styles.iter().any(|(existing, _)| existing == &name) {
                counter_styles.push((
                    name.clone(),
                    generated_rule(&name, parsed.system, &parsed.symbols),
                ));
            }
            cursor = parsed.end;
            continue;
        }

        let width = utf8_char_width(bytes[cursor]);
        out.push_str(&css[cursor..cursor + width]);
        cursor += width;
    }

    if counter_styles.is_empty() {
        return SymbolsFunctionProjection {
            rewritten: Cow::Borrowed(css),
            counter_styles: Vec::new(),
        };
    }
    SymbolsFunctionProjection {
        rewritten: Cow::Owned(out),
        counter_styles: counter_styles.into_iter().map(|(_, rule)| rule).collect(),
    }
}

pub fn value_contains_valid_symbols_function(property: &str, value: &str) -> bool {
    if !matches!(property, "list-style-type" | "content") {
        return false;
    }
    let probe = format!("x {{ {property}: {value}; }}");
    !project_symbols_functions(&probe).counter_styles.is_empty()
}

#[cfg(test)]
mod tests {
    use super::project_symbols_functions;

    #[test]
    fn valid_anonymous_styles_become_named_rules_in_supported_properties() {
        let projection = project_symbols_functions(
            ".list { list-style-type: symbols(numeric '0' '1') } .item::after { content: counter(n, symbols('*')) }",
        );
        let rewritten = projection.rewritten;
        assert!(!rewritten.contains("symbols(numeric"));
        assert!(!rewritten.contains("symbols('*')"));
        assert_eq!(projection.counter_styles.len(), 2);
        assert!(
            projection.counter_styles[0]
                .serialization()
                .contains("system: numeric; symbols: \"0\" \"1\"; suffix: \" \";")
        );
        assert!(
            projection.counter_styles[1]
                .serialization()
                .contains("system: symbolic; symbols: \"*\"; suffix: \" \";")
        );
    }

    #[test]
    fn invalid_or_unrelated_symbols_functions_are_unchanged() {
        for css in [
            ".x { list-style-type: symbols(numeric '0') }",
            ".x { list-style-type: symbols('*' ident) }",
            ".x { background: symbols('*') }",
            ".x::after { content: \"symbols('*')\" }",
        ] {
            let projection = project_symbols_functions(css);
            assert_eq!(projection.rewritten, css);
            assert!(projection.counter_styles.is_empty());
        }
    }
}
