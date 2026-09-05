use std::{borrow::Cow, ops::Range};

use crate::css_scan::{is_css_whitespace, is_ident_continue};

pub const INTERNAL_OVERLAY_TRANSITION_PROPERTY: &str = "--moegoe-overlay-transition";

fn declaration_value_start(bytes: &[u8], property_end: usize) -> Option<usize> {
    let mut colon = property_end;
    while colon < bytes.len() && is_css_whitespace(bytes[colon]) {
        colon += 1;
    }
    (bytes.get(colon) == Some(&b':')).then_some(colon + 1)
}

fn declaration_end(bytes: &[u8], value_start: usize) -> usize {
    let mut cursor = value_start;
    let mut quote = None;
    let mut nesting = 0_u32;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active_quote) = quote {
            if byte == b'\\' {
                cursor = (cursor + 2).min(bytes.len());
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
        } else {
            match byte {
                b'\'' | b'"' => quote = Some(byte),
                b'(' | b'[' => nesting = nesting.saturating_add(1),
                b')' | b']' => nesting = nesting.saturating_sub(1),
                b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                    cursor += 2;
                    while cursor + 1 < bytes.len() && bytes[cursor..cursor + 2] != *b"*/" {
                        cursor += 1;
                    }
                    cursor = (cursor + 2).min(bytes.len());
                    continue;
                },
                b';' | b'}' if nesting == 0 => break,
                _ => {},
            }
        }
        cursor += 1;
    }
    cursor
}

fn starts_property(bytes: &[u8], cursor: usize, property: &[u8]) -> Option<usize> {
    let property_end = cursor + property.len();
    (property_end <= bytes.len()
        && bytes[cursor..property_end].eq_ignore_ascii_case(property)
        && (cursor == 0 || !is_ident_continue(bytes[cursor - 1])))
    .then_some(property_end)
}

fn overlay_identifiers(bytes: &[u8], value: Range<usize>) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut cursor = value.start;
    while cursor < value.end {
        match bytes[cursor] {
            quote @ (b'\'' | b'"') => {
                cursor += 1;
                while cursor < value.end {
                    if bytes[cursor] == b'\\' {
                        cursor = (cursor + 2).min(value.end);
                    } else if bytes[cursor] == quote {
                        cursor += 1;
                        break;
                    } else {
                        cursor += 1;
                    }
                }
            },
            b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                cursor += 2;
                while cursor + 1 < value.end && bytes[cursor..cursor + 2] != *b"*/" {
                    cursor += 1;
                }
                cursor = (cursor + 2).min(value.end);
            },
            _ if cursor + b"overlay".len() <= value.end
                && bytes[cursor..cursor + b"overlay".len()].eq_ignore_ascii_case(b"overlay")
                && (cursor == value.start || !is_ident_continue(bytes[cursor - 1]))
                && (cursor + b"overlay".len() == value.end
                    || !is_ident_continue(bytes[cursor + b"overlay".len()])) =>
            {
                ranges.push(cursor..cursor + b"overlay".len());
                cursor += b"overlay".len();
            },
            _ => cursor += 1,
        }
    }
    ranges
}

/// Preserve the author-visible `overlay` transition endpoint through Stylo's
/// Servo parser, where `overlay` is computed but is not accepted as a
/// transition-property identifier.
pub fn rewrite_overlay_transition_property(css: &str) -> Cow<'_, str> {
    if !css
        .as_bytes()
        .windows(b"overlay".len())
        .any(|window| window.eq_ignore_ascii_case(b"overlay"))
    {
        return Cow::Borrowed(css);
    }

    let bytes = css.as_bytes();
    let mut replacements = Vec::new();
    let mut declaration_start = true;
    let mut cursor = 0;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                cursor += 2;
                while cursor + 1 < bytes.len() && bytes[cursor..cursor + 2] != *b"*/" {
                    cursor += 1;
                }
                cursor = (cursor + 2).min(bytes.len());
            },
            quote @ (b'\'' | b'"') => {
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
                declaration_start = false;
            },
            b'{' | b';' => {
                declaration_start = true;
                cursor += 1;
            },
            b'}' => {
                declaration_start = false;
                cursor += 1;
            },
            byte if declaration_start && is_css_whitespace(byte) => cursor += 1,
            _ if declaration_start => {
                let property_end = starts_property(bytes, cursor, b"transition-property")
                    .or_else(|| starts_property(bytes, cursor, b"transition"));
                let Some(value_start) =
                    property_end.and_then(|end| declaration_value_start(bytes, end))
                else {
                    declaration_start = false;
                    cursor += 1;
                    continue;
                };
                let end = declaration_end(bytes, value_start);
                replacements.extend(overlay_identifiers(bytes, value_start..end));
                declaration_start = false;
                cursor = end;
            },
            _ => {
                declaration_start = false;
                cursor += 1;
            },
        }
    }

    if replacements.is_empty() {
        return Cow::Borrowed(css);
    }
    let mut rewritten = css.to_owned();
    for range in replacements.into_iter().rev() {
        rewritten.replace_range(range, INTERNAL_OVERLAY_TRANSITION_PROPERTY);
    }
    Cow::Owned(rewritten)
}

#[cfg(test)]
mod tests {
    use super::{INTERNAL_OVERLAY_TRANSITION_PROPERTY, rewrite_overlay_transition_property};

    #[test]
    fn rewrites_only_overlay_transition_identifiers() {
        let css = concat!(
            ".a{transition: opacity 1s, OVERLAY 2s;content:'transition: overlay'}",
            ".b{transition-property:overlay,display;overlay:none}",
            ".overlay{color:green}/* transition: overlay */",
        );

        assert_eq!(
            rewrite_overlay_transition_property(css),
            format!(
                concat!(
                    ".a{{transition: opacity 1s, {0} 2s;content:'transition: overlay'}}",
                    ".b{{transition-property:{0},display;overlay:none}}",
                    ".overlay{{color:green}}/* transition: overlay */",
                ),
                INTERNAL_OVERLAY_TRANSITION_PROPERTY,
            )
        );
    }
}
