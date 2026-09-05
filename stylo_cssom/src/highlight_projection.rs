pub const CUSTOM_HIGHLIGHT_ATTRIBUTE: &str = "data-moegoe-custom-highlight";
pub const CUSTOM_HIGHLIGHT_ELEMENT: &str = "moegoe-highlight";
pub const SELECTION_ATTRIBUTE: &str = "data-moegoe-selection";

use stylo_cssom_model::InternalStylesheetRoot;

#[must_use]
pub fn project_custom_highlight_root(root: &InternalStylesheetRoot) -> InternalStylesheetRoot {
    InternalStylesheetRoot::new(
        root.origin(),
        crate::author_rule_projection::project_rule_sources(
            root.rules(),
            &mut project_highlight_selectors,
        ),
    )
}

pub fn project_highlight_selectors(css: &str) -> String {
    const PSEUDO: &[u8] = b"::highlight(";
    const SELECTION: &[u8] = b"::selection";

    let bytes = css.as_bytes();
    let mut output = String::with_capacity(css.len());
    let mut index = 0;
    let mut quote = None;
    let mut comment = false;
    while index < bytes.len() {
        if comment {
            if bytes.get(index..index.saturating_add(2)) == Some(b"*/") {
                output.push_str("*/");
                index += 2;
                comment = false;
            } else {
                copy_next_char(css, &mut output, &mut index);
            }
            continue;
        }
        if let Some(delimiter) = quote {
            let byte = bytes[index];
            copy_next_char(css, &mut output, &mut index);
            if byte == b'\\' && index < bytes.len() {
                copy_next_char(css, &mut output, &mut index);
            } else if byte == delimiter {
                quote = None;
            }
            continue;
        }
        if bytes.get(index..index.saturating_add(2)) == Some(b"/*") {
            output.push_str("/*");
            index += 2;
            comment = true;
            continue;
        }
        if matches!(bytes[index], b'\'' | b'"') {
            quote = Some(bytes[index]);
            copy_next_char(css, &mut output, &mut index);
            continue;
        }
        if bytes
            .get(index..index.saturating_add(PSEUDO.len()))
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(PSEUDO))
            && let Some(close) = bytes[index + PSEUDO.len()..]
                .iter()
                .position(|byte| *byte == b')')
        {
            let name_start = index + PSEUDO.len();
            let name_end = name_start + close;
            let name = css[name_start..name_end].trim();
            if let Some(name) = decode_css_identifier(name).filter(|name| !name.is_empty()) {
                push_projected_selector_origin(&mut output);
                output.push_str(CUSTOM_HIGHLIGHT_ELEMENT);
                output.push_str(":where([");
                output.push_str(CUSTOM_HIGHLIGHT_ATTRIBUTE);
                output.push_str("=\"");
                push_css_string(&mut output, &name);
                output.push_str("\"])");
                index = name_end + 1;
                continue;
            }
        }
        if bytes
            .get(index..index.saturating_add(SELECTION.len()))
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(SELECTION))
            && bytes
                .get(index.saturating_add(SELECTION.len()))
                .is_none_or(|byte| !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_'))
        {
            push_projected_selector_origin(&mut output);
            output.push_str(CUSTOM_HIGHLIGHT_ELEMENT);
            output.push_str(":where([");
            output.push_str(SELECTION_ATTRIBUTE);
            output.push_str("])");
            index += SELECTION.len();
            continue;
        }
        copy_next_char(css, &mut output, &mut index);
    }
    output
}

fn push_projected_selector_origin(output: &mut String) {
    let selector_has_origin = output
        .chars()
        .rev()
        .find(|ch| !ch.is_whitespace())
        .is_some_and(|ch| !matches!(ch, '{' | ','));
    if selector_has_origin && !output.ends_with(char::is_whitespace) {
        output.push(' ');
    }
}

fn copy_next_char(source: &str, output: &mut String, offset: &mut usize) {
    let ch = source[*offset..]
        .chars()
        .next()
        .expect("a source offset below the byte length must start a character");
    output.push(ch);
    *offset += ch.len_utf8();
}

fn decode_css_identifier(source: &str) -> Option<String> {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            output.push(ch);
            continue;
        }
        let next = chars.peek().copied()?;
        if next.is_ascii_hexdigit() {
            let mut value = 0_u32;
            for _ in 0..6 {
                let Some(digit) = chars.peek().and_then(|ch| ch.to_digit(16)) else {
                    break;
                };
                value = value.saturating_mul(16).saturating_add(digit);
                chars.next();
            }
            if chars.peek().is_some_and(char::is_ascii_whitespace) {
                chars.next();
            }
            let scalar = (value != 0 && !(0xd800..=0xdfff).contains(&value))
                .then(|| char::from_u32(value))
                .flatten()
                .unwrap_or('\u{fffd}');
            output.push(scalar);
        } else if matches!(next, '\n' | '\r' | '\u{c}') {
            return None;
        } else {
            output.push(
                chars
                    .next()
                    .expect("a peeked escaped character must remain available"),
            );
        }
    }
    Some(output)
}

fn push_css_string(output: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '"' | '\\' => {
                output.push('\\');
                output.push(ch);
            },
            '\0' => output.push('\u{fffd}'),
            '\n' => output.push_str("\\a "),
            '\r' => output.push_str("\\d "),
            '\u{c}' => output.push_str("\\c "),
            _ => output.push(ch),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{project_custom_highlight_root, project_highlight_selectors};

    #[test]
    fn highlight_selectors_target_render_projection_segments() {
        let css = "::highlight(note), #owner::HIGHLIGHT(mark) { color: blue } p { color: red }";
        assert_eq!(
            project_highlight_selectors(css),
            "moegoe-highlight:where([data-moegoe-custom-highlight=\"note\"]), #owner moegoe-highlight:where([data-moegoe-custom-highlight=\"mark\"]) { color: blue } p { color: red }"
        );
    }

    #[test]
    fn selection_selectors_target_the_typed_selection_projection() {
        let css = "::selection, #owner::SELECTION { color: blue }";
        assert_eq!(
            project_highlight_selectors(css),
            "moegoe-highlight:where([data-moegoe-selection]), #owner moegoe-highlight:where([data-moegoe-selection]) { color: blue }"
        );
    }

    #[test]
    fn grouped_selection_and_custom_highlight_rules_survive_projection() {
        let css = concat!(
            "@container (width >= 400px) {",
            "::selection { color: green }",
            "::highlight(hi) { color: green }",
            "}",
        );
        let parsed =
            crate::ParsedStylesheet::parse(css).expect("the grouped highlight fixture must parse");
        let root = stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::Author,
            parsed.rule_nodes().to_vec(),
        );

        assert_eq!(
            project_custom_highlight_root(&root).projection_serialization(),
            concat!(
                "@container (width >= 400px) {",
                "moegoe-highlight:where([data-moegoe-selection]) { color: green } ",
                "moegoe-highlight:where([data-moegoe-custom-highlight=\"hi\"]) { color: green }}"
            )
        );
    }

    #[test]
    fn separate_top_level_and_nested_highlight_rules_survive_projection() {
        let css = concat!(
            "::selection { color: red; background: transparent; }\n",
            "::highlight(hi) { color: red; background: transparent; }\n",
            "@container (width >= 400px) {\n",
            "::selection { color: green }\n",
            "::highlight(hi) { color: green }\n",
            "}",
        );
        let parsed = crate::ParsedStylesheet::parse(css).expect("highlight fixture must parse");
        let root = stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::Author,
            parsed.rule_nodes().to_vec(),
        );
        let projection = project_custom_highlight_root(&root).projection_serialization();

        assert_eq!(projection.matches("data-moegoe-selection").count(), 2);
        assert_eq!(
            projection.matches("data-moegoe-custom-highlight").count(),
            2
        );
    }

    #[test]
    fn highlight_projection_preserves_top_level_and_nested_named_rule_sources() {
        let parsed = crate::ParsedStylesheet::parse(concat!(
            "@position-try --outer { left: 1px }",
            "@media all { @position-try --inner { top: 2px } ::selection { color: red } }",
            "::highlight(hi) { color: green }",
        ))
        .unwrap();
        let root = stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::Author,
            parsed.rule_nodes().to_vec(),
        );
        let sources = |root: &stylo_cssom_model::InternalStylesheetRoot| {
            [
                root.rules()[0].payload().source_stamp(),
                root.rules()[1].payload().nested()[0]
                    .payload()
                    .source_stamp(),
            ]
        };
        let original = sources(&root);
        assert!(original.iter().all(Option::is_some));
        for projected in [
            project_custom_highlight_root(&root),
            project_custom_highlight_root(&root),
        ] {
            assert_eq!(sources(&projected), original);
        }
    }

    #[test]
    fn highlight_tokens_inside_strings_and_comments_are_untouched() {
        let css = "/* ::highlight(no) — café */ a::before { content: '::highlight(no) 😀' }";
        assert_eq!(project_highlight_selectors(css), css);
    }

    #[test]
    fn escaped_highlight_names_match_the_registry_string() {
        assert_eq!(
            project_highlight_selectors("::highlight(\\66 oo) { color: green }"),
            "moegoe-highlight:where([data-moegoe-custom-highlight=\"foo\"]) { color: green }"
        );
    }
}
