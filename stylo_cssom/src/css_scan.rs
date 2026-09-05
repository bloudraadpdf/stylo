#[inline]
pub fn utf8_char_width(b: u8) -> usize {
    match b {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => unreachable!("continuation byte at char boundary — input is not valid UTF-8"),
    }
}

#[inline]
pub fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

#[inline]
pub fn is_css_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 0x0C)
}

pub fn trim_ascii(s: &[u8]) -> &[u8] {
    let start = s
        .iter()
        .position(|b| !is_css_whitespace(*b))
        .unwrap_or(s.len());
    let end = s
        .iter()
        .rposition(|b| !is_css_whitespace(*b))
        .map_or(start, |p| p + 1);
    &s[start..end]
}

pub fn parse_component_as_f32(token: &str) -> Option<f32> {
    if token.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    if let Some(percent) = token.strip_suffix('%') {
        return percent.trim().parse::<f32>().ok().map(|v| v / 100.0);
    }
    token.parse::<f32>().ok()
}

pub fn skip_separators(bytes: &[u8], idx: &mut usize) {
    while *idx < bytes.len() {
        let b = bytes[*idx];
        if b.is_ascii_whitespace() || b == b',' {
            *idx += 1;
        } else {
            break;
        }
    }
}

pub fn contains_any_ascii_ci(css: &str, needles: &[&[u8]]) -> bool {
    let bytes = css.as_bytes();
    needles.iter().any(|needle| {
        bytes
            .windows(needle.len())
            .any(|w| w.eq_ignore_ascii_case(needle))
    })
}

pub fn hex_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len() * 2);
    for byte in value.as_bytes() {
        use std::fmt::Write;
        write!(encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

pub fn match_any_ascii_ci(
    bytes: &[u8],
    offset: usize,
    needles: &[&[u8]],
) -> Option<(usize, usize)> {
    needles.iter().enumerate().find_map(|(index, needle)| {
        (offset + needle.len() <= bytes.len()
            && bytes[offset..offset + needle.len()].eq_ignore_ascii_case(needle))
        .then_some((index, needle.len()))
    })
}

pub fn find_matching_close_paren(bytes: &[u8], after_open: usize) -> Option<usize> {
    let mut depth = 1usize;
    for (offset, byte) in bytes.get(after_open..)?.iter().enumerate() {
        match byte {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(after_open + offset);
                }
            },
            _ => {},
        }
    }
    None
}

pub fn skip_string_or_comment(bytes: &[u8], cursor: usize) -> Option<usize> {
    let escaped = bytes[..cursor]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
        % 2
        == 1;
    if !escaped && bytes.get(cursor) == Some(&b'/') && bytes.get(cursor + 1) == Some(&b'*') {
        let mut end = cursor + 2;
        while end + 1 < bytes.len() && bytes[end..end + 2] != *b"*/" {
            end += 1;
        }
        return Some((end + 2).min(bytes.len()));
    }
    let quote = *bytes.get(cursor)?;
    if escaped || !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let mut end = cursor + 1;
    while end < bytes.len() {
        if bytes[end] == b'\\' {
            end = (end + 2).min(bytes.len());
        } else if bytes[end] == quote {
            return Some(end + 1);
        } else {
            end += 1;
        }
    }
    Some(end)
}

pub fn advance_past_string_or_comment(bytes: &[u8], cursor: &mut usize) -> bool {
    let Some(after) = skip_string_or_comment(bytes, *cursor) else {
        return false;
    };
    *cursor = after;
    true
}

fn advance_past_escape(bytes: &[u8], cursor: &mut usize) -> bool {
    if bytes.get(*cursor) != Some(&b'\\') {
        return false;
    }
    *cursor = (*cursor + 2).min(bytes.len());
    true
}

pub fn named_function_is_closed(value: &str, function: &[u8]) -> bool {
    let bytes = value.as_bytes();
    let mut expected_closers = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes.get(cursor..cursor + 2) == Some(b"/*") {
            let Some(end) = bytes[cursor + 2..]
                .windows(2)
                .position(|window| window == b"*/")
            else {
                return !expected_closers.iter().any(|(_, named)| *named);
            };
            cursor += end + 4;
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'"') {
            let quote = bytes[cursor];
            cursor += 1;
            let mut closed = false;
            while cursor < bytes.len() {
                match bytes[cursor] {
                    b'\\' => cursor = (cursor + 2).min(bytes.len()),
                    byte if byte == quote => {
                        cursor += 1;
                        closed = true;
                        break;
                    },
                    _ => cursor += 1,
                }
            }
            if !closed {
                return false;
            }
            continue;
        }
        if advance_past_escape(bytes, &mut cursor) {
            continue;
        }
        match bytes[cursor] {
            b'(' => {
                let named = cursor >= function.len()
                    && bytes[cursor - function.len()..cursor].eq_ignore_ascii_case(function)
                    && cursor.checked_sub(function.len() + 1).is_none_or(|before| {
                        let byte = bytes[before];
                        !is_ident_continue(byte) && byte.is_ascii() && byte != b'\\'
                    });
                expected_closers.push((b')', named));
            },
            b'[' => expected_closers.push((b']', false)),
            b'{' => expected_closers.push((b'}', false)),
            b')' | b']' | b'}' => {
                let Some((expected, named)) = expected_closers.pop() else {
                    cursor += 1;
                    continue;
                };
                if expected != bytes[cursor] {
                    return !named && !expected_closers.iter().any(|(_, named)| *named);
                }
            },
            _ => {},
        }
        cursor += 1;
    }
    !expected_closers.iter().any(|(_, named)| *named)
}

pub fn find_matching_delimiter(
    bytes: &[u8],
    after_open: usize,
    open: u8,
    close: u8,
) -> Option<usize> {
    let mut depth = 1_u32;
    let mut cursor = after_open;
    while cursor < bytes.len() {
        if advance_past_string_or_comment(bytes, &mut cursor) {
            continue;
        }
        if bytes[cursor] == open {
            depth += 1;
        } else if bytes[cursor] == close {
            depth -= 1;
            if depth == 0 {
                return Some(cursor);
            }
        }
        cursor += 1;
    }
    None
}

pub fn split_top_level(bytes: &[u8], delimiter: u8) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut cursor = 0;
    let mut nesting = 0_u32;
    while cursor < bytes.len() {
        if let Some(after) = skip_string_or_comment(bytes, cursor) {
            cursor = after;
            continue;
        }
        if advance_past_escape(bytes, &mut cursor) {
            continue;
        }
        match bytes[cursor] {
            b'(' | b'[' | b'{' => nesting += 1,
            b')' | b']' | b'}' => nesting = nesting.saturating_sub(1),
            byte if byte == delimiter && nesting == 0 => {
                ranges.push(start..cursor);
                start = cursor + 1;
            },
            _ => {},
        }
        cursor += 1;
    }
    ranges.push(start..bytes.len());
    ranges
}

pub fn rewrite_style_rules(css: &str, rewrite: &impl Fn(&str, &str) -> String) -> String {
    rewrite_style_rules_with_opaque_at_rules(css, rewrite, &|_| false)
}

pub fn rewrite_style_rules_with_opaque_at_rules(
    css: &str,
    rewrite: &impl Fn(&str, &str) -> String,
    opaque_at_rule: &impl Fn(&str) -> bool,
) -> String {
    let bytes = css.as_bytes();
    let mut output = String::with_capacity(css.len());
    let mut rule_start = 0;
    let mut cursor = 0;
    while cursor < bytes.len() {
        cursor = skip_string_or_comment(bytes, cursor).unwrap_or(cursor);
        match bytes.get(cursor) {
            Some(b';') => {
                output.push_str(&css[rule_start..=cursor]);
                cursor += 1;
                rule_start = cursor;
            },
            Some(b'{') => {
                let Some(close) = find_matching_delimiter(bytes, cursor + 1, b'{', b'}') else {
                    break;
                };
                let prelude = &css[rule_start..cursor];
                let body = &css[cursor + 1..close];
                if prelude.trim_start().starts_with('@') {
                    output.push_str(prelude);
                    output.push('{');
                    if opaque_at_rule(prelude) {
                        output.push_str(body);
                    } else {
                        output.push_str(&rewrite_style_rules_with_opaque_at_rules(
                            body,
                            rewrite,
                            opaque_at_rule,
                        ));
                    }
                    output.push('}');
                } else {
                    output.push_str(&rewrite(prelude, body));
                }
                cursor = close + 1;
                rule_start = cursor;
            },
            Some(_) => cursor += 1,
            None => break,
        }
    }
    output.push_str(&css[rule_start..]);
    output
}

pub fn current_property_name(out: &str) -> Option<String> {
    let bytes = out.as_bytes();
    let colon = bytes.iter().rposition(|&byte| byte == b':')?;
    let boundary = bytes[..colon]
        .iter()
        .rposition(|&byte| matches!(byte, b';' | b'{' | b'}'))
        .map_or(0, |index| index + 1);
    let name = trim_ascii(&bytes[boundary..colon]);
    if name.is_empty()
        || name.starts_with(b"--")
        || !name
            .iter()
            .all(|&byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return None;
    }
    Some(std::str::from_utf8(name).ok()?.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::{rewrite_style_rules, split_top_level};

    #[test]
    fn escaped_delimiters_remain_inside_a_component() {
        let css = br"--a\;b:value";

        assert_eq!(split_top_level(css, b';'), [0..css.len()]);
    }

    #[test]
    fn escaped_identifier_quotes_do_not_hide_later_style_rules() {
        let css = r".a{view-transition-name:secon\'d}.b{color:green}";
        let rewritten = rewrite_style_rules(css, &|prelude, body| {
            format!("{prelude}{{marker:yes;{body}}}")
        });

        assert_eq!(
            rewritten,
            r".a{marker:yes;view-transition-name:secon\'d}.b{marker:yes;color:green}"
        );
    }
}
