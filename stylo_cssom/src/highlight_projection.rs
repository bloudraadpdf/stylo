pub const CUSTOM_HIGHLIGHT_ATTRIBUTE: &str = "data-moegoe-custom-highlight";
pub const CUSTOM_HIGHLIGHT_ELEMENT: &str = "moegoe-highlight";
pub const SELECTION_ATTRIBUTE: &str = "data-moegoe-selection";
pub const TARGET_TEXT_ATTRIBUTE: &str = "data-moegoe-target-text";

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

/// Rewrite each highlight selector to also match the projection wrappers of the highlighted text, keeping the
/// pseudo-element selector, which the highlight cascade of the originating element matches (CSS Pseudo 4 §3.5).
pub fn project_highlight_selectors(css: &str) -> String {
    const PSEUDO: &[u8] = b"::highlight(";
    const SIMPLE_HIGHLIGHTS: &[(&[u8], &str)] = &[
        (b"::selection", SELECTION_ATTRIBUTE),
        (b"::target-text", TARGET_TEXT_ATTRIBUTE),
    ];

    let bytes = css.as_bytes();
    let mut output = String::with_capacity(css.len());
    let mut index = 0;
    let mut quote = None;
    let mut comment = false;
    let mut selector_start = 0;
    let mut source_selector_start = 0;
    let mut projected = false;
    let mut depth = 0_usize;
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
                let mut filter = format!("[{CUSTOM_HIGHLIGHT_ATTRIBUTE}=\"");
                push_css_string(&mut filter, &name);
                filter.push_str("\"]");
                push_projected_highlight(&mut output, selector_start, &filter);
                projected = true;
                index = name_end + 1;
                continue;
            }
        }
        if let Some((pseudo, attribute)) = SIMPLE_HIGHLIGHTS.iter().find(|(pseudo, _)| {
            bytes
                .get(index..index.saturating_add(pseudo.len()))
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(pseudo))
                && bytes
                    .get(index.saturating_add(pseudo.len()))
                    .is_none_or(|byte| {
                        !byte.is_ascii_alphanumeric() && !matches!(byte, b'-' | b'_')
                    })
        }) {
            push_projected_highlight(&mut output, selector_start, &format!("[{attribute}]"));
            projected = true;
            index += pseudo.len();
            continue;
        }
        let byte = bytes[index];
        if projected && depth == 0 && matches!(byte, b'{' | b',') {
            push_pseudo_element_selector(&mut output, css[source_selector_start..index].trim());
        }
        copy_next_char(css, &mut output, &mut index);
        match byte {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b'{' | b'}' | b';' | b',' if depth == 0 => {
                selector_start = output.len();
                source_selector_start = index;
                projected = false;
            },
            _ => {},
        }
    }
    output
}

/// Rewrite the selector since `selector_start` to match the projection wrappers of the text that its
/// originating element owns: the wrappers that are its children, and the wrappers nested in those. A wrapper
/// holds only wrappers and text, and is never an originating element, so ancestors of the originating element
/// do not reach descendant text (CSS Pseudo 4 §3.5).
fn push_projected_highlight(output: &mut String, selector_start: usize, filter: &str) {
    let selector = &output[selector_start..];
    let origin_start = selector_start + selector.len() - selector.trim_start().len();
    let origin = output[origin_start..].to_owned();
    output.truncate(origin_start);
    let trimmed = origin.trim_end();
    if trimmed.is_empty() {
        output.push_str(CUSTOM_HIGHLIGHT_ELEMENT);
    } else {
        let element = CUSTOM_HIGHLIGHT_ELEMENT;
        let separator =
            if trimmed.len() != origin.len() || trimmed.ends_with(['>', '+', '~']) { " " } else { "" };
        let origin = format!("{trimmed}{separator}:where(:not({element}))");
        output.push_str(&format!(
            ":is({origin} > {element}, {origin} > {element} {element})"
        ));
    }
    output.push_str(":where(");
    output.push_str(filter);
    output.push(')');
}

/// Append the original pseudo-element `selector` to the projected selector list when it parses on its own, so an
/// unsupported pseudo-element selector does not invalidate the projected rule.
fn push_pseudo_element_selector(output: &mut String, selector: &str) {
    if crate::selector_query::parse_selector(selector).is_ok() {
        let end = output.trim_end().len();
        output.insert_str(end, &format!(", {selector}"));
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
    fn target_text_projection_retains_origin_and_selector_lists() {
        assert_eq!(
            project_highlight_selectors("p::TARGET-TEXT, .marked { color: lime }"),
            ":is(p:where(:not(moegoe-highlight)) > moegoe-highlight, p:where(:not(moegoe-highlight)) > moegoe-highlight moegoe-highlight):where([data-moegoe-target-text]), p::TARGET-TEXT, .marked { color: lime }"
        );
        assert_eq!(
            project_highlight_selectors("::target-textual { color: red }"),
            "::target-textual { color: red }"
        );
        assert_eq!(
            project_highlight_selectors("p::target-text { content: '::target-text' }"),
            ":is(p:where(:not(moegoe-highlight)) > moegoe-highlight, p:where(:not(moegoe-highlight)) > moegoe-highlight moegoe-highlight):where([data-moegoe-target-text]), p::target-text { content: '::target-text' }"
        );
    }

    #[test]
    fn highlight_selectors_target_render_projection_segments() {
        let css = "::highlight(note), #owner::HIGHLIGHT(mark) { color: blue } p { color: red }";
        assert_eq!(
            project_highlight_selectors(css),
            concat!(
                "moegoe-highlight:where([data-moegoe-custom-highlight=\"note\"]), ::highlight(note), ",
                ":is(#owner:where(:not(moegoe-highlight)) > moegoe-highlight, #owner:where(:not(moegoe-highlight)) > moegoe-highlight moegoe-highlight):where([data-moegoe-custom-highlight=\"mark\"]), #owner::HIGHLIGHT(mark) ",
                "{ color: blue } p { color: red }"
            )
        );
    }

    #[test]
    fn selection_selectors_target_the_typed_selection_projection() {
        let css = "::selection, #owner::SELECTION { color: blue }";
        assert_eq!(
            project_highlight_selectors(css),
            concat!(
                "moegoe-highlight:where([data-moegoe-selection]), ::selection, ",
                ":is(#owner:where(:not(moegoe-highlight)) > moegoe-highlight, #owner:where(:not(moegoe-highlight)) > moegoe-highlight moegoe-highlight):where([data-moegoe-selection]), #owner::SELECTION { color: blue }"
            )
        );
    }

    #[test]
    fn highlight_selectors_reach_only_the_text_of_the_originating_element() {
        assert_eq!(
            project_highlight_selectors("main .M::selection, div ::selection, div > ::selection {}"),
            concat!(
                ":is(main .M:where(:not(moegoe-highlight)) > moegoe-highlight, main .M:where(:not(moegoe-highlight)) > moegoe-highlight moegoe-highlight):where([data-moegoe-selection]), main .M::selection, ",
                ":is(div :where(:not(moegoe-highlight)) > moegoe-highlight, div :where(:not(moegoe-highlight)) > moegoe-highlight moegoe-highlight):where([data-moegoe-selection]), div ::selection, ",
                ":is(div > :where(:not(moegoe-highlight)) > moegoe-highlight, div > :where(:not(moegoe-highlight)) > moegoe-highlight moegoe-highlight):where([data-moegoe-selection]), div > ::selection {}"
            )
        );
        assert_eq!(
            project_highlight_selectors(":is(a, b)::selection { color: red }"),
            ":is(:is(a, b):where(:not(moegoe-highlight)) > moegoe-highlight, :is(a, b):where(:not(moegoe-highlight)) > moegoe-highlight moegoe-highlight):where([data-moegoe-selection]), :is(a, b)::selection { color: red }"
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
                "moegoe-highlight:where([data-moegoe-selection]), ::selection { color: green }",
                "moegoe-highlight:where([data-moegoe-custom-highlight=\"hi\"]), ::highlight(hi) { color: green }}"
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
            "moegoe-highlight:where([data-moegoe-custom-highlight=\"foo\"]), ::highlight(\\66 oo) { color: green }"
        );
    }
}
