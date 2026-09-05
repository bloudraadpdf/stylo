use std::borrow::Cow;

use cssparser::{Parser, ParserInput};
use style::values::CustomIdent;

use crate::css_scan::{
    advance_past_string_or_comment, find_matching_delimiter, hex_encode, is_ident_continue,
    rewrite_style_rules,
};

const ACTIVE_PSEUDO: &[u8] = b":active-view-transition";
const ACTIVE_TYPE_PSEUDO: &[u8] = b":active-view-transition-type";
pub const ACTIVE_STATE: &str = "--moegoe-view-transition-active";
pub const TYPE_STATE_PREFIX: &str = "--moegoe-view-transition-type-";

pub fn rewrite_stylesheet(css: &str) -> Cow<'_, str> {
    if !contains_active_selector(css) {
        return Cow::Borrowed(css);
    }
    let rewritten = rewrite_style_rules(css, &|prelude, body| {
        let mut rule = rewrite_selector(prelude).into_owned();
        rule.push('{');
        rule.push_str(body);
        rule.push('}');
        rule
    });
    if rewritten == css {
        Cow::Borrowed(css)
    } else {
        Cow::Owned(rewritten)
    }
}

pub fn rewrite_selector(selector: &str) -> Cow<'_, str> {
    if !contains_active_selector(selector) {
        return Cow::Borrowed(selector);
    }
    let bytes = selector.as_bytes();
    let mut output = String::with_capacity(selector.len());
    let mut copied = 0;
    let mut cursor = 0;
    while cursor < bytes.len() {
        if advance_past_string_or_comment(bytes, &mut cursor) {
            continue;
        }
        if matches_token(bytes, cursor, ACTIVE_TYPE_PSEUDO)
            && bytes.get(cursor + ACTIVE_TYPE_PSEUDO.len()) == Some(&b'(')
        {
            let arguments_start = cursor + ACTIVE_TYPE_PSEUDO.len() + 1;
            let Some(close) = find_matching_delimiter(bytes, arguments_start, b'(', b')') else {
                cursor += 1;
                continue;
            };
            let Some(types) = parse_types(&selector[arguments_start..close]) else {
                cursor = close + 1;
                continue;
            };
            output.push_str(&selector[copied..cursor]);
            output.push_str(":is(");
            for (index, value) in types.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                output.push_str(":state(");
                output.push_str(&encode_type(value));
                output.push(')');
            }
            output.push(')');
            cursor = close + 1;
            copied = cursor;
            continue;
        }
        if matches_token(bytes, cursor, ACTIVE_PSEUDO)
            && !bytes
                .get(cursor + ACTIVE_PSEUDO.len())
                .is_some_and(|byte| is_ident_continue(*byte) || *byte == b'(')
        {
            output.push_str(&selector[copied..cursor]);
            output.push_str(":state(");
            output.push_str(ACTIVE_STATE);
            output.push(')');
            cursor += ACTIVE_PSEUDO.len();
            copied = cursor;
            continue;
        }
        cursor += 1;
    }
    if copied == 0 {
        Cow::Borrowed(selector)
    } else {
        output.push_str(&selector[copied..]);
        Cow::Owned(output)
    }
}

fn contains_active_selector(css: &str) -> bool {
    let bytes = css.as_bytes();
    bytes
        .windows(ACTIVE_PSEUDO.len())
        .any(|window| window.eq_ignore_ascii_case(ACTIVE_PSEUDO))
}

fn matches_token(bytes: &[u8], cursor: usize, expected: &[u8]) -> bool {
    cursor + expected.len() <= bytes.len()
        && bytes[cursor..cursor + expected.len()].eq_ignore_ascii_case(expected)
}

fn parse_types(arguments: &str) -> Option<Vec<String>> {
    let mut input = ParserInput::new(arguments);
    let mut parser = Parser::new(&mut input);
    let types = parser
        .parse_entirely(|input| {
            input.parse_comma_separated(|item| CustomIdent::parse(item, &["none"]))
        })
        .ok()?;
    let types = types
        .into_iter()
        .map(|ident| ident.0.as_ref().to_owned())
        .collect::<Vec<_>>();
    (!types.is_empty()
        && !types
            .iter()
            .any(|value| value.to_ascii_lowercase().starts_with("-ua-")))
    .then_some(types)
}

fn encode_type(value: &str) -> String {
    format!("{TYPE_STATE_PREFIX}{}", hex_encode(value))
}

pub fn decode_type(encoded: &str) -> Option<String> {
    if !encoded.len().is_multiple_of(2) {
        return None;
    }
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let pair = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect::<Option<Vec<_>>>()?;
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::{ACTIVE_STATE, rewrite_selector, rewrite_stylesheet};

    #[test]
    fn rewrites_valid_active_selectors_and_preserves_specificity_shape() {
        assert_eq!(
            rewrite_selector("html:active-view-transition-type(foo, Bar)::before"),
            "html:is(:state(--moegoe-view-transition-type-666f6f),:state(--moegoe-view-transition-type-426172))::before"
        );
        assert_eq!(
            rewrite_selector(":root:active-view-transition"),
            format!(":root:state({ACTIVE_STATE})")
        );
    }

    #[test]
    fn keeps_invalid_strings_comments_and_declarations_unchanged() {
        let css = concat!(
            ":active-view-transition-type(foo,){color:red}",
            ".literal{content:':active-view-transition'}",
            "/* :active-view-transition */",
        );
        assert_eq!(rewrite_stylesheet(css), css);
    }
}
