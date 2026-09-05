use std::borrow::Cow;

use cssparser::{Parser, ParserInput, serialize_string};

use crate::css_scan::{is_css_whitespace, is_ident_continue};

pub const INTERNAL_DISPLAY_PROPERTY: &str = "--moegoe-webkit-box-display";
pub const INTERNAL_CONTINUE_PROPERTY: &str = "--moegoe-continue";
pub const INTERNAL_LEGACY_TEXT_ALIGN_PROPERTY: &str = "--moegoe-legacy-text-align";
pub const INTERNAL_LEGACY_TEXT_ALIGN_NAME: &str = "moegoe-legacy-text-align";

const AUTHORED_CONTINUE: &[u8] = b"continue";
const AUTOMATIC_LINE_CLAMP: &[u8] = b"line-clamp";
const LEGACY_LINE_CLAMP: &[u8] = b"-webkit-line-clamp";
const TEXT_ALIGN: &[u8] = b"text-align";

type CompatibilityReplacement = (std::ops::Range<usize>, String);
type DeclarationReplacement = (Option<CompatibilityReplacement>, usize);

fn declaration_value_start(bytes: &[u8], property_end: usize) -> Option<usize> {
    let mut colon = property_end;
    while colon < bytes.len() && is_css_whitespace(bytes[colon]) {
        colon += 1;
    }
    (bytes.get(colon) == Some(&b':')).then(|| {
        let mut value_start = colon + 1;
        while value_start < bytes.len() && is_css_whitespace(bytes[value_start]) {
            value_start += 1;
        }
        value_start
    })
}

fn display_declaration_replacement(
    bytes: &[u8],
    cursor: usize,
) -> (Option<(std::ops::Range<usize>, String)>, usize) {
    let property_end = cursor + b"display".len();
    let Some(value_start) = declaration_value_start(bytes, property_end) else {
        return (None, property_end);
    };
    let declaration_end = bytes[value_start..]
        .iter()
        .position(|byte| matches!(byte, b';' | b'}'))
        .map_or(bytes.len(), |relative| value_start + relative);
    let important_start = bytes[value_start..declaration_end]
        .iter()
        .position(|byte| *byte == b'!')
        .map_or(declaration_end, |relative| value_start + relative);
    let mut value_end = important_start;
    while value_end > value_start && is_css_whitespace(bytes[value_end - 1]) {
        value_end -= 1;
    }
    let value = &bytes[value_start..value_end];
    let fallback = if value.eq_ignore_ascii_case(b"-webkit-box") {
        "flex"
    } else if value.eq_ignore_ascii_case(b"-webkit-inline-box") {
        "inline-flex"
    } else {
        std::str::from_utf8(value).unwrap_or_default()
    };
    let fallback_importance = if important_start < declaration_end {
        " !important"
    } else {
        ""
    };
    let authored_value = std::str::from_utf8(value).unwrap_or_default();
    (
        Some((
            cursor..value_end,
            format!(
                "display: {fallback}{fallback_importance}; {INTERNAL_DISPLAY_PROPERTY}: {authored_value}"
            ),
        )),
        value_end,
    )
}

fn line_clamp_declaration_end(bytes: &[u8], value_start: usize) -> usize {
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

fn parse_continue_compat_value(value: &str) -> Option<(String, bool)> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    parser
        .parse_entirely(
            |input| -> Result<(String, bool), cssparser::ParseError<'_, ()>> {
                let keyword = input.expect_ident_cloned()?.to_ascii_lowercase();
                if !matches!(
                    keyword.as_str(),
                    "auto"
                        | "collapse"
                        | "discard"
                        | "inherit"
                        | "initial"
                        | "revert"
                        | "revert-layer"
                        | "unset"
                ) {
                    return Err(input.new_custom_error(()));
                }
                let important = if input.is_exhausted() {
                    false
                } else {
                    input.expect_delim('!')?;
                    input.expect_ident_matching("important")?;
                    true
                };
                Ok((keyword, important))
            },
        )
        .ok()
}

fn continue_compat_replacement(
    bytes: &[u8],
    cursor: usize,
) -> (Option<(std::ops::Range<usize>, String)>, usize) {
    let property_end = cursor + AUTHORED_CONTINUE.len();
    let Some(value_start) = declaration_value_start(bytes, property_end) else {
        return (None, property_end);
    };
    let declaration_end = line_clamp_declaration_end(bytes, value_start);
    let value = std::str::from_utf8(&bytes[value_start..declaration_end]).unwrap_or_default();
    let Some((keyword, important)) = parse_continue_compat_value(value) else {
        return (None, declaration_end);
    };
    let lowered = if keyword == "collapse" {
        "discard"
    } else {
        keyword.as_str()
    };
    let important = if important { " !important" } else { "" };
    (
        Some((
            cursor..declaration_end,
            format!(
                "continue: {lowered}{important}; {INTERNAL_CONTINUE_PROPERTY}: {keyword}{important}"
            ),
        )),
        declaration_end,
    )
}

fn parse_line_clamp_compat_value(
    value: &str,
) -> Option<(String, String, bool, bool, &'static str)> {
    enum BlockEllipsis {
        Auto,
        None,
        String(String),
    }

    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let (max_lines, block_ellipsis, important, preserve_authored, continue_value) = parser
        .parse_entirely(
            |input| -> Result<
                (String, BlockEllipsis, bool, bool, &'static str),
                cssparser::ParseError<'_, ()>,
            > {
                let (max_lines, block_ellipsis, preserve_authored, continue_value) = if input
                    .try_parse(|input| input.expect_ident_matching("none"))
                    .is_ok()
                {
                    ("none".to_string(), BlockEllipsis::None, true, "auto")
                } else if input
                    .try_parse(|input| input.expect_ident_matching("auto"))
                    .is_ok()
                {
                    let block_ellipsis = if input
                        .try_parse(|input| input.expect_ident_matching("no-ellipsis"))
                        .is_ok()
                    {
                        BlockEllipsis::None
                    } else {
                        BlockEllipsis::Auto
                    };
                    ("none".to_string(), block_ellipsis, false, "collapse")
                } else if let Ok(lines) = input.try_parse(Parser::expect_integer) {
                    if lines < 1 {
                        return Err(input.new_custom_error(()));
                    }
                    let (block_ellipsis, preserve_authored) = if input
                        .try_parse(|input| input.expect_ident_matching("no-ellipsis"))
                        .is_ok()
                    {
                        (BlockEllipsis::None, false)
                    } else if input
                        .try_parse(|input| input.expect_ident_matching("none"))
                        .is_ok()
                    {
                        (BlockEllipsis::None, true)
                    } else if input
                        .try_parse(|input| input.expect_ident_matching("auto"))
                        .is_ok()
                    {
                        (BlockEllipsis::Auto, true)
                    } else if let Ok(marker) = input.try_parse(|input| {
                        input.expect_string().map(|value| value.as_ref().to_owned())
                    }) {
                        (BlockEllipsis::String(marker), true)
                    } else {
                        (BlockEllipsis::Auto, true)
                    };
                    (
                        lines.to_string(),
                        block_ellipsis,
                        preserve_authored,
                        "collapse",
                    )
                } else {
                    (
                        "none".to_string(),
                        BlockEllipsis::String(input.expect_string()?.as_ref().to_owned()),
                        false,
                        "collapse",
                    )
                };
                let important = if input.is_exhausted() {
                    false
                } else {
                    input.expect_delim('!')?;
                    input.expect_ident_matching("important")?;
                    true
                };
                Ok((
                    max_lines,
                    block_ellipsis,
                    important,
                    preserve_authored,
                    continue_value,
                ))
            },
        )
        .ok()?;
    let block_ellipsis = match block_ellipsis {
        BlockEllipsis::Auto => "auto".to_string(),
        BlockEllipsis::None => "none".to_string(),
        BlockEllipsis::String(value) => {
            let mut serialised = String::new();
            serialize_string(&value, &mut serialised).ok()?;
            serialised
        },
    };
    Some((
        max_lines,
        block_ellipsis,
        important,
        preserve_authored,
        continue_value,
    ))
}

fn line_clamp_compat_replacement(
    bytes: &[u8],
    cursor: usize,
) -> (Option<(std::ops::Range<usize>, String)>, usize) {
    let property_end = cursor + AUTOMATIC_LINE_CLAMP.len();
    let Some(value_start) = declaration_value_start(bytes, property_end) else {
        return (None, property_end);
    };
    let declaration_end = line_clamp_declaration_end(bytes, value_start);
    let value = std::str::from_utf8(&bytes[value_start..declaration_end]).unwrap_or_default();
    let Some((max_lines, block_ellipsis, important, preserve_authored, continue_value)) =
        parse_line_clamp_compat_value(value)
    else {
        return (None, declaration_end);
    };
    let important = if important { " !important" } else { "" };
    if preserve_authored {
        let authored = std::str::from_utf8(&bytes[cursor..declaration_end]).unwrap_or_default();
        return (
            Some((
                cursor..declaration_end,
                format!("{authored}; {INTERNAL_CONTINUE_PROPERTY}: {continue_value}{important}"),
            )),
            declaration_end,
        );
    }
    (
        Some((
            cursor..declaration_end,
            format!(
                "max-lines: {max_lines}{important}; continue: discard{important}; block-ellipsis: {block_ellipsis}{important}; {INTERNAL_CONTINUE_PROPERTY}: {continue_value}{important}"
            ),
        )),
        declaration_end,
    )
}

fn parse_legacy_line_clamp_compat_value(value: &str) -> Option<(String, bool)> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    parser
        .parse_entirely(
            |input| -> Result<(String, bool), cssparser::ParseError<'_, ()>> {
                let max_lines = if input
                    .try_parse(|input| input.expect_ident_matching("none"))
                    .is_ok()
                {
                    "none".to_string()
                } else {
                    let lines = input.expect_integer()?;
                    if lines < 1 {
                        return Err(input.new_custom_error(()));
                    }
                    lines.to_string()
                };
                let important = if input.is_exhausted() {
                    false
                } else {
                    input.expect_delim('!')?;
                    input.expect_ident_matching("important")?;
                    true
                };
                Ok((max_lines, important))
            },
        )
        .ok()
}

fn legacy_line_clamp_compat_replacement(
    bytes: &[u8],
    cursor: usize,
) -> (Option<(std::ops::Range<usize>, String)>, usize) {
    let property_end = cursor + LEGACY_LINE_CLAMP.len();
    let Some(value_start) = declaration_value_start(bytes, property_end) else {
        return (None, property_end);
    };
    let declaration_end = line_clamp_declaration_end(bytes, value_start);
    let value = std::str::from_utf8(&bytes[value_start..declaration_end]).unwrap_or_default();
    let Some((max_lines, important)) = parse_legacy_line_clamp_compat_value(value) else {
        return (None, declaration_end);
    };
    let authored = std::str::from_utf8(&bytes[cursor..declaration_end]).unwrap_or_default();
    let important = if important { " !important" } else { "" };
    (
        Some((
            cursor..declaration_end,
            format!(
                "{authored}; max-lines: {max_lines}{important}; continue: auto{important}; block-ellipsis: auto{important}; {INTERNAL_CONTINUE_PROPERTY}: auto{important}"
            ),
        )),
        declaration_end,
    )
}

fn text_align_compat_replacement(bytes: &[u8], cursor: usize) -> DeclarationReplacement {
    let property_end = cursor + TEXT_ALIGN.len();
    let Some(value_start) = declaration_value_start(bytes, property_end) else {
        return (None, property_end);
    };
    let declaration_end = line_clamp_declaration_end(bytes, value_start);
    let value = std::str::from_utf8(&bytes[value_start..declaration_end]).unwrap_or_default();
    let Some((keyword, important)) = parse_text_align_compat_value(value) else {
        return (None, declaration_end);
    };
    let authored = std::str::from_utf8(&bytes[cursor..declaration_end]).unwrap_or_default();
    let important = if important { " !important" } else { "" };
    (
        Some((
            cursor..declaration_end,
            format!("{authored}; {INTERNAL_LEGACY_TEXT_ALIGN_PROPERTY}: {keyword}{important}"),
        )),
        declaration_end,
    )
}

fn parse_text_align_compat_value(value: &str) -> Option<(String, bool)> {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    parser
        .parse_entirely(
            |input| -> Result<(String, bool), cssparser::ParseError<'_, ()>> {
                let keyword = input.expect_ident_cloned()?.to_ascii_lowercase();
                if !matches!(
                    keyword.as_ref(),
                    "start"
                        | "end"
                        | "left"
                        | "right"
                        | "center"
                        | "justify"
                        | "match-parent"
                        | "-moz-left"
                        | "-moz-center"
                        | "-moz-right"
                        | "-webkit-left"
                        | "-webkit-center"
                        | "-webkit-right"
                        | "inherit"
                        | "initial"
                        | "unset"
                        | "revert"
                        | "revert-layer"
                ) {
                    return Err(input.new_custom_error(()));
                }
                let important = if input.is_exhausted() {
                    false
                } else {
                    input.expect_delim('!')?;
                    input.expect_ident_matching("important")?;
                    true
                };
                Ok((keyword.clone(), important))
            },
        )
        .ok()
}

fn needs_compatibility_rewrite(css: &str) -> bool {
    let bytes = css.as_bytes();
    bytes
        .windows(b"display".len())
        .any(|window| window.eq_ignore_ascii_case(b"display"))
        || bytes
            .windows(AUTOMATIC_LINE_CLAMP.len())
            .any(|window| window.eq_ignore_ascii_case(AUTOMATIC_LINE_CLAMP))
        || bytes
            .windows(AUTHORED_CONTINUE.len())
            .any(|window| window.eq_ignore_ascii_case(AUTHORED_CONTINUE))
        || bytes
            .windows(TEXT_ALIGN.len())
            .any(|window| window.eq_ignore_ascii_case(TEXT_ALIGN))
}

fn starts_declaration_property(bytes: &[u8], cursor: usize, property: &[u8]) -> bool {
    cursor + property.len() <= bytes.len()
        && bytes[cursor..cursor + property.len()].eq_ignore_ascii_case(property)
        && (cursor == 0 || !is_ident_continue(bytes[cursor - 1]))
}

fn declaration_compatibility_replacement(
    bytes: &[u8],
    cursor: usize,
) -> Option<DeclarationReplacement> {
    if starts_declaration_property(bytes, cursor, LEGACY_LINE_CLAMP) {
        return Some(legacy_line_clamp_compat_replacement(bytes, cursor));
    }
    if starts_declaration_property(bytes, cursor, AUTHORED_CONTINUE) {
        return Some(continue_compat_replacement(bytes, cursor));
    }
    if starts_declaration_property(bytes, cursor, AUTOMATIC_LINE_CLAMP) {
        return Some(line_clamp_compat_replacement(bytes, cursor));
    }
    if starts_declaration_property(bytes, cursor, TEXT_ALIGN) {
        return Some(text_align_compat_replacement(bytes, cursor));
    }
    starts_declaration_property(bytes, cursor, b"display")
        .then(|| display_declaration_replacement(bytes, cursor))
}

/// Preserve legacy WebKit layout declarations that Stylo's Servo build
/// otherwise discards or normalises during parsing.
pub fn rewrite_webkit_box_orient(css: &str) -> Cow<'_, str> {
    if !needs_compatibility_rewrite(css) {
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
                let Some((replacement, next_cursor)) =
                    declaration_compatibility_replacement(bytes, cursor)
                else {
                    declaration_start = false;
                    cursor += 1;
                    continue;
                };
                replacements.extend(replacement);
                declaration_start = false;
                cursor = next_cursor;
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
    for (range, replacement) in replacements.into_iter().rev() {
        rewritten.replace_range(range, &replacement);
    }
    Cow::Owned(rewritten)
}
