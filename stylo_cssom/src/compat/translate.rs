#![allow(clippy::match_same_arms)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::elidable_lifetime_names)]
#![allow(clippy::needless_continue)]
#![allow(clippy::single_match_else)]

use std::borrow::Cow;

use cssparser::{ParseError, Parser, ParserInput, Token};

use super::CompatMode;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatWarning {
    pub kind: CompatWarningKind,

    pub property: String,

    pub line: u32,

    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatWarningKind {
    UnknownVendor { required: CompatMode },

    ValueDropped { value: String },

    PropertyDropped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatTranslation<'a> {
    pub rewritten: Cow<'a, str>,

    pub warnings: Vec<CompatWarning>,
}

impl CompatTranslation<'_> {
    pub fn is_borrowed(&self) -> bool {
        matches!(self.rewritten, Cow::Borrowed(_))
    }
}

pub fn translate_compat(css: &str, compat: CompatMode) -> CompatTranslation<'_> {
    translate_compat_with_registrations(css, compat, ColourRegistrationOutput::Append)
}

fn translate_compat_with_registrations<'css>(
    css: &'css str,
    compat: CompatMode,
    registrations: ColourRegistrationOutput<'_>,
) -> CompatTranslation<'css> {
    let grid_lanes_rewritten = rewrite_grid_lanes_properties(css);
    let grid_lanes_working: Cow<'_, str> = match grid_lanes_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => Cow::Borrowed(css),
    };

    let f15_rewritten = rewrite_bd_barcode_kebab_aliases(grid_lanes_working.as_ref());
    let f15_working: Cow<'_, str> = match f15_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => grid_lanes_working,
    };

    let f14_rewritten = rewrite_attr_function_tokens(f15_working.as_ref(), compat);
    let f14_working: Cow<'_, str> = match &f14_rewritten {
        Some(owned) => Cow::Owned(owned.clone()),
        None => f15_working,
    };

    let f14_ident_rewritten = rewrite_pdfreactor_attr_ident_type(f14_working.as_ref(), compat);
    let f14_working: Cow<'_, str> = match f14_ident_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => f14_working,
    };

    let from_font_rewritten =
        rewrite_pdfreactor_font_size_adjust_from_font(f14_working.as_ref(), compat);
    let f14_working: Cow<'_, str> = match from_font_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => f14_working,
    };

    let text_scale_down_rewritten =
        rewrite_pdfreactor_text_overflow_scale_down(f14_working.as_ref(), compat);
    let f14_working: Cow<'_, str> = match text_scale_down_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => f14_working,
    };

    let lh_working: Cow<'_, str> = if matches!(compat, CompatMode::PdfReactor) {
        match rewrite_percentage_line_height(f14_working.as_ref()) {
            Some(owned) => Cow::Owned(owned),
            None => f14_working,
        }
    } else {
        f14_working
    };

    let lh_working: Cow<'_, str> = if matches!(compat, CompatMode::PdfReactor) {
        match rewrite_trailing_content_line_break(lh_working.as_ref()) {
            Some(owned) => Cow::Owned(owned),
            None => lh_working,
        }
    } else {
        lh_working
    };

    let break_pseudo_rewritten = rewrite_break_pseudo_aliases(lh_working.as_ref(), compat);
    let f14_working: Cow<'_, str> = match break_pseudo_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => lh_working,
    };

    let no_content_rewritten = rewrite_no_content_pseudo_alias(f14_working.as_ref(), compat);
    let f14_working: Cow<'_, str> = match no_content_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => f14_working,
    };

    let dc_rewritten = rewrite_double_colon_structural_pseudo_classes(f14_working.as_ref(), compat);
    let f14_working: Cow<'_, str> = match dc_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => f14_working,
    };

    let (f2_at_rule_rewritten, mut f2_at_rule_warnings) =
        rewrite_spot_colour_at_rules(f14_working.as_ref(), compat);
    let f2_at_rule_working: Cow<'_, str> = match f2_at_rule_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => f14_working,
    };
    let f2_fn_rewritten =
        rewrite_spot_colour_function_tokens(f2_at_rule_working.as_ref(), compat, registrations);
    let f2_fn_working: Cow<'_, str> = match f2_fn_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => f2_at_rule_working,
    };

    let cmyk_rewritten = rewrite_pdfreactor_cmyk_function_tokens(f2_fn_working.as_ref(), compat);
    let cmyk_working: Cow<'_, str> = match cmyk_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => f2_fn_working,
    };

    let gray_rewritten = rewrite_pdfreactor_gray_function_tokens(cmyk_working.as_ref(), compat);
    let cmyk_working: Cow<'_, str> = match gray_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => cmyk_working,
    };

    let legacy_attr_rewritten = rewrite_pdfreactor_legacy_attr_types(cmyk_working.as_ref(), compat);
    let cmyk_working: Cow<'_, str> = match legacy_attr_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => cmyk_working,
    };

    let string_set_rewritten =
        rewrite_pdfreactor_string_set_self_keyword(cmyk_working.as_ref(), compat);
    let cmyk_working: Cow<'_, str> = match string_set_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => cmyk_working,
    };

    let page_float_rewritten =
        rewrite_pdfreactor_page_float_keywords(cmyk_working.as_ref(), compat);
    let cmyk_working: Cow<'_, str> = match page_float_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => cmyk_working,
    };

    let marks_bleed_rewritten =
        rewrite_pdfreactor_marks_bleed_keyword(cmyk_working.as_ref(), compat);
    let cmyk_working: Cow<'_, str> = match marks_bleed_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => cmyk_working,
    };

    let page_box_origin_rewritten =
        rewrite_pdfreactor_page_box_background_origin(cmyk_working.as_ref(), compat);
    let cmyk_working: Cow<'_, str> = match page_box_origin_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => cmyk_working,
    };

    let a28_rewritten = rewrite_pdfreactor_comment_colour_keywords(cmyk_working.as_ref(), compat);
    let a28_working: Cow<'_, str> = match a28_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => cmyk_working,
    };

    let a27_rewritten = rewrite_pdfreactor_counter_style_aliases(a28_working.as_ref(), compat);
    let a27_working: Cow<'_, str> = match a27_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => a28_working,
    };

    let a26_rewritten = rewrite_pdfreactor_page_relative_unit_aliases(a27_working.as_ref(), compat);
    let a26_working: Cow<'_, str> = match a26_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => a27_working,
    };

    let corner_rewritten =
        rewrite_pdfreactor_corner_margin_box_aliases(a26_working.as_ref(), compat);
    let corner_working: Cow<'_, str> = match corner_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => a26_working,
    };

    let nth_rewritten = rewrite_pdfreactor_nth_page_pseudo_alias(corner_working.as_ref(), compat);
    let nth_working: Cow<'_, str> = match nth_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => corner_working,
    };

    let last_rewritten = rewrite_pdfreactor_last_page_pseudo_alias(nth_working.as_ref(), compat);
    let last_working: Cow<'_, str> = match last_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => nth_working,
    };

    let sidenote_area_rewritten =
        rewrite_pdfreactor_outside_sidenote_area(last_working.as_ref(), compat);
    let last_working: Cow<'_, str> = match sidenote_area_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => last_working,
    };

    let supports_rewritten = rewrite_supports_property_probes(last_working.as_ref(), compat);
    let supports_working: Cow<'_, str> = match supports_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => last_working,
    };

    let custom_unset_rewritten =
        rewrite_pdfreactor_custom_property_unset(supports_working.as_ref(), compat);
    let pre_walk_working: Cow<'_, str> = match custom_unset_rewritten {
        Some(owned) => Cow::Owned(owned),
        None => supports_working,
    };

    let (mut warnings, body_rewrites_present, post_walk) = {
        let working: &str = pre_walk_working.as_ref();
        let mut input = ParserInput::new(working);
        let mut parser = Parser::new(&mut input);
        let mut state = TranslateState {
            compat,
            injections: Vec::new(),
            value_replacements: Vec::new(),
            tail_descriptors: Vec::new(),
            warnings: Vec::new(),
            at_page_depth: 0,
            at_font_face_depth: 0,
        };
        walk_top_level(&mut parser, &mut state, working);
        let body_rewrites = !state.injections.is_empty()
            || !state.value_replacements.is_empty()
            || !state.tail_descriptors.is_empty();
        let rewritten = if body_rewrites {
            Some(apply_rewrites(
                working,
                &state.injections,
                &state.value_replacements,
                &state.tail_descriptors,
            ))
        } else {
            None
        };
        (state.warnings, body_rewrites, rewritten)
    };

    warnings.append(&mut f2_at_rule_warnings);

    let rewritten = match (post_walk, pre_walk_working) {
        (Some(owned), _) => Cow::Owned(owned),

        (None, pre) => match pre {
            Cow::Owned(owned) => Cow::Owned(owned),
            Cow::Borrowed(_) => Cow::Borrowed(css),
        },
    };

    let _ = body_rewrites_present;

    CompatTranslation {
        rewritten,
        warnings,
    }
}

fn rewrite_grid_lanes_properties(css: &str) -> Option<String> {
    const FLOW_TOLERANCE: &[u8] = b"flow-tolerance";
    const GRID_LANES_PACK: &[u8] = b"grid-lanes-pack";

    if !css
        .as_bytes()
        .windows(FLOW_TOLERANCE.len())
        .any(|candidate| candidate.eq_ignore_ascii_case(FLOW_TOLERANCE))
        && !css
            .as_bytes()
            .windows(GRID_LANES_PACK.len())
            .any(|candidate| candidate.eq_ignore_ascii_case(GRID_LANES_PACK))
    {
        return None;
    }

    let bytes = css.as_bytes();
    let mut output = String::with_capacity(css.len());
    let mut index = 0;
    let mut changed = false;
    while index < bytes.len() {
        if matches!(bytes[index], b'"' | b'\'') {
            let end = skip_css_string(bytes, index).min(bytes.len());
            output.push_str(&css[index..end]);
            index = end;
            continue;
        }
        if bytes[index] == b'/' && bytes.get(index + 1) == Some(&b'*') {
            let comment_end = css[index + 2..]
                .find("*/")
                .map_or(bytes.len(), |offset| index + 2 + offset + 2);
            output.push_str(&css[index..comment_end]);
            index = comment_end;
            continue;
        }

        let projected_property = if bytes[index..]
            .get(..FLOW_TOLERANCE.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(FLOW_TOLERANCE))
        {
            Some((
                FLOW_TOLERANCE.len(),
                "masonry-slack",
                GridLanesProjection::FlowTolerance,
            ))
        } else if bytes[index..]
            .get(..GRID_LANES_PACK.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(GRID_LANES_PACK))
        {
            Some((
                GRID_LANES_PACK.len(),
                "grid-auto-flow",
                GridLanesProjection::Packing,
            ))
        } else {
            None
        };
        let property_end = projected_property.map_or(index, |(length, _, _)| index + length);
        let has_property = projected_property.is_some()
            && index
                .checked_sub(1)
                .and_then(|before| bytes.get(before))
                .is_none_or(|before| !is_name_continuation_byte(*before));
        if has_property {
            let mut colon = property_end;
            while bytes.get(colon).is_some_and(u8::is_ascii_whitespace) {
                colon += 1;
            }
            if bytes.get(colon) == Some(&b':') {
                let (_, internal_property, projection_kind) =
                    projected_property.expect("a matched property projects");
                if matches!(projection_kind, GridLanesProjection::FlowTolerance) {
                    let value_start = skip_css_trivia(bytes, colon + 1);
                    let projection = [
                        (b"normal".as_slice(), "infinite"),
                        (b"infinite".as_slice(), "auto"),
                    ]
                    .into_iter()
                    .find(|(source, _)| {
                        let value_end = value_start.saturating_add(source.len());
                        bytes
                            .get(value_start..value_end)
                            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(source))
                            && bytes
                                .get(value_end)
                                .is_none_or(|after| !is_name_continuation_byte(*after))
                            && bytes
                                .get(skip_css_trivia(bytes, value_end))
                                .is_none_or(|byte| matches!(byte, b';' | b'}' | b')' | b'!'))
                    });
                    if let Some((source_value, internal_value)) = projection {
                        output.push_str(internal_property);
                        output.push_str(&css[property_end..value_start]);
                        output.push_str(internal_value);
                        index = value_start + source_value.len();
                        changed = true;
                        continue;
                    }
                    let invalid_auto = b"auto";
                    let auto_end = value_start.saturating_add(invalid_auto.len());
                    if bytes
                        .get(value_start..auto_end)
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(invalid_auto))
                        && bytes
                            .get(auto_end)
                            .is_none_or(|after| !is_name_continuation_byte(*after))
                    {
                        index = copy_css_character(css, index, &mut output);
                        continue;
                    }
                } else {
                    let value_start = skip_css_trivia(bytes, colon + 1);
                    let source_value = b"normal";
                    let value_end = value_start.saturating_add(source_value.len());
                    let has_normal_value = bytes
                        .get(value_start..value_end)
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(source_value))
                        && bytes
                            .get(value_end)
                            .is_none_or(|after| !is_name_continuation_byte(*after));
                    if has_normal_value {
                        let tail = skip_css_trivia(bytes, value_end);
                        if bytes
                            .get(tail)
                            .is_some_and(|byte| !matches!(byte, b';' | b'}' | b')' | b'!'))
                        {
                            index = copy_css_character(css, index, &mut output);
                            continue;
                        }
                        output.push_str(internal_property);
                        output.push_str(&css[property_end..value_start]);
                        output.push_str("row");
                        index = value_end;
                        changed = true;
                        continue;
                    }
                    let dense_value = b"dense";
                    let dense_end = value_start.saturating_add(dense_value.len());
                    let has_dense_value = bytes
                        .get(value_start..dense_end)
                        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(dense_value))
                        && bytes
                            .get(dense_end)
                            .is_none_or(|after| !is_name_continuation_byte(*after));
                    let tail = skip_css_trivia(bytes, dense_end);
                    if !has_dense_value
                        || bytes
                            .get(tail)
                            .is_some_and(|byte| !matches!(byte, b';' | b'}' | b')' | b'!'))
                    {
                        index = copy_css_character(css, index, &mut output);
                        continue;
                    }
                }
                output.push_str(internal_property);
                index = property_end;
                changed = true;
                continue;
            }
        }

        index = copy_css_character(css, index, &mut output);
    }
    changed.then_some(output)
}

#[derive(Clone, Copy)]
enum GridLanesProjection {
    FlowTolerance,
    Packing,
}

fn copy_css_character(css: &str, index: usize, output: &mut String) -> usize {
    let character = css[index..]
        .chars()
        .next()
        .expect("the lexical Grid 3 property projection retains a character boundary");
    output.push(character);
    index + character.len_utf8()
}

fn skip_css_trivia(bytes: &[u8], mut index: usize) -> usize {
    loop {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes.get(index..index.saturating_add(2)) != Some(b"/*") {
            return index;
        }
        index += 2;
        while bytes
            .get(index..index.saturating_add(2))
            .is_some_and(|pair| pair != b"*/")
        {
            index += 1;
        }
        if bytes.get(index..index.saturating_add(2)) != Some(b"*/") {
            return bytes.len();
        }
        index += 2;
    }
}

fn rewrite_percentage_line_height(css: &str) -> Option<String> {
    use std::fmt::Write as _;

    const MARKER: &[u8] = b"line-height";
    let bytes = css.as_bytes();
    if !bytes
        .windows(MARKER.len())
        .any(|w| w.eq_ignore_ascii_case(MARKER))
    {
        return None;
    }
    let mut out = String::with_capacity(css.len());
    let mut cursor = 0usize;
    let mut changed = false;
    while let Some(found) = bytes[cursor..]
        .windows(MARKER.len())
        .position(|w| w.eq_ignore_ascii_case(MARKER))
    {
        let name_start = cursor + found;
        let name_end = name_start + MARKER.len();
        out.push_str(&css[cursor..name_end]);
        cursor = name_end;

        let preceded_by_ident = name_start > 0
            && matches!(bytes[name_start - 1], b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_');
        let after = css[name_end..].trim_start();
        if preceded_by_ident || !after.starts_with(':') {
            continue;
        }
        let colon_offset = name_end + (css.len() - name_end - after.len());
        let value_start = colon_offset + 1;
        let value_end = css[value_start..]
            .find([';', '}'])
            .map_or(css.len(), |i| value_start + i);
        let value = css[value_start..value_end].trim();
        let Some(number) = value.strip_suffix('%') else {
            out.push_str(&css[cursor..value_end]);
            cursor = value_end;
            continue;
        };
        let Ok(percent) = number.trim().parse::<f32>() else {
            out.push_str(&css[cursor..value_end]);
            cursor = value_end;
            continue;
        };
        out.push_str(&css[cursor..value_start]);
        let _ = write!(out, " {}", percent / 100.0);
        cursor = value_end;
        changed = true;
    }
    out.push_str(&css[cursor..]);
    changed.then_some(out)
}

fn skip_css_string(bytes: &[u8], start: usize) -> usize {
    let quote = bytes[start];
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            92 => i += 2,
            b'\n' => return i,
            b if b == quote => return i + 1,
            _ => i += 1,
        }
    }
    i
}

fn ends_with_newline_escape(inner: &str) -> bool {
    let inner = inner.strip_suffix(' ').unwrap_or(inner);
    let Some(rest) = inner.strip_suffix(['A', 'a']) else {
        return false;
    };
    let backslashes = rest
        .chars()
        .rev()
        .take_while(|&c| c == char::from(92))
        .count();
    backslashes % 2 == 1
}

fn rewrite_trailing_content_line_break(css: &str) -> Option<String> {
    let bytes = css.as_bytes();
    let has = |needle: &[u8]| {
        bytes
            .windows(needle.len())
            .any(|w| w.eq_ignore_ascii_case(needle))
    };
    if !has(b"content") || !has(b"white-space") {
        return None;
    }

    let mut opens: Vec<(usize, usize)> = Vec::new();
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    let mut pops = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'"' | b'\'' => i = skip_css_string(bytes, i),
            b'{' => {
                opens.push((i, pops));
                i += 1;
            },
            b'}' => {
                if let Some((start, pops_at_open)) = opens.pop() {
                    if pops == pops_at_open {
                        blocks.push((start + 1, i));
                    }
                    pops += 1;
                }
                i += 1;
            },
            _ => i += 1,
        }
    }

    let mut insertions: Vec<usize> = Vec::new();
    for &(body_start, body_end) in &blocks {
        if let Some(close_quote) = trailing_break_insertion_for_block(css, body_start, body_end) {
            insertions.push(close_quote);
        }
    }
    if insertions.is_empty() {
        return None;
    }
    insertions.sort_unstable();
    let mut out = String::with_capacity(css.len() + insertions.len() * 2);
    let mut cursor = 0usize;
    for pos in insertions {
        out.push_str(&css[cursor..pos]);
        out.push(char::from(92));
        out.push('A');
        cursor = pos;
    }
    out.push_str(&css[cursor..]);
    Some(out)
}

fn trailing_break_insertion_for_block(
    css: &str,
    body_start: usize,
    body_end: usize,
) -> Option<usize> {
    let bytes = css.as_bytes();
    let mut preserving_white_space = false;
    let mut content_close_quote: Option<usize> = None;

    let mut decl_start = body_start;
    let mut i = body_start;
    loop {
        let at_end = i >= body_end;
        if !at_end && (bytes[i] == b'"' || bytes[i] == b'\'') {
            i = skip_css_string(bytes, i);
            continue;
        }
        if at_end || bytes[i] == b';' {
            let decl = &css[decl_start..i.min(body_end)];
            if let Some(colon) = decl.find(':') {
                let name = decl[..colon].trim();
                let value_start = decl_start + colon + 1;
                let value_end = i.min(body_end);
                if name.eq_ignore_ascii_case("content") {
                    content_close_quote =
                        final_string_newline_escape_close(css, value_start, value_end);
                } else if name.eq_ignore_ascii_case("white-space") {
                    let first = css[value_start..value_end]
                        .split_whitespace()
                        .next()
                        .unwrap_or("");
                    preserving_white_space |= ["pre", "pre-wrap", "pre-line", "break-spaces"]
                        .iter()
                        .any(|kw| first.eq_ignore_ascii_case(kw));
                }
            }
            if at_end {
                break;
            }
            decl_start = i + 1;
        }
        i += 1;
    }
    if preserving_white_space {
        content_close_quote
    } else {
        None
    }
}

fn final_string_newline_escape_close(
    css: &str,
    value_start: usize,
    value_end: usize,
) -> Option<usize> {
    let bytes = css.as_bytes();
    let mut last_string: Option<(usize, usize)> = None;
    let mut i = value_start;
    while i < value_end {
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let after = skip_css_string(bytes, i);
            let close = after
                .checked_sub(1)
                .filter(|&c| bytes.get(c) == Some(&bytes[i]))?;
            last_string = Some((i, close));
            i = after;
        } else {
            i += 1;
        }
    }
    let (open, close) = last_string?;
    let tail = css[close + 1..value_end].trim();
    if !(tail.is_empty() || tail.eq_ignore_ascii_case("!important")) {
        return None;
    }
    let inner = &css[open + 1..close];
    if !ends_with_newline_escape(inner) {
        return None;
    }

    let without_escape_terminator = inner.strip_suffix(' ').unwrap_or(inner);
    let before_escape = &without_escape_terminator[..without_escape_terminator.len() - 2];
    let preceding_value = &css[value_start..open];
    (!before_escape.is_empty() || !preceding_value.trim().is_empty()).then_some(close)
}

fn rewrite_bd_barcode_kebab_aliases(css: &str) -> Option<String> {
    let bytes = css.as_bytes();
    const BD_MARKER: &[u8] = b"-bd-barcode-type";
    const RO_MARKER: &[u8] = b"-ro-barcode-type";
    let has_marker = bytes
        .windows(BD_MARKER.len())
        .any(|w| w.eq_ignore_ascii_case(BD_MARKER))
        || bytes
            .windows(RO_MARKER.len())
            .any(|w| w.eq_ignore_ascii_case(RO_MARKER));
    if !has_marker {
        return None;
    }

    const REWRITES: &[(&[u8], &str)] = &[
        (b"usps-onecode", "usps-intelligent-mail"),
        (b"databar stacked", "data-bar-stacked"),
        (b"databar-limited", "data-bar-limited"),
        (b"dp-leitcode", "deutsche-post-leitcode"),
        (b"codablockf", "codablock-f"),
        (b"kixcode", "kix"),
        (b"auspost", "australia-post"),
        (b"microqr", "micro-qr"),
        (b"code2of5 interleaved", "itf"),
        (b"aztec-code", "aztec"),
        (b"code-128", "code128"),
        (b"logmars", "logmars"),
        (b"code-39", "code39"),
        (b"code-93", "code93"),
        (b"ean-13", "ean13"),
        (b"itf14", "itf14"),
        (b"ean-8", "ean8"),
        (b"upc-a", "upca"),
        (b"upc-e", "upce"),
        (b"qrcode", "qr-code"),
        (b"maxicode mode-4", "maxi-code"),
        (b"maxicode", "maxi-code"),
    ];

    let mut out = String::with_capacity(css.len());
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        let matched_marker = [BD_MARKER, RO_MARKER].into_iter().find_map(|m| {
            (bytes.len() >= i + m.len()
                && bytes[i..i + m.len()].eq_ignore_ascii_case(m)
                && bytes
                    .get(i + m.len())
                    .is_none_or(|b| !is_name_continuation_byte(*b)))
            .then_some(m.len())
        });
        let Some(marker_len) = matched_marker else {
            let Some(ch) = css[i..].chars().next() else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
            continue;
        };

        out.push_str(&css[i..i + marker_len]);
        i += marker_len;

        let value_start = {
            let mut j = i;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == b':' {
                Some(j + 1)
            } else {
                None
            }
        };
        let Some(value_start) = value_start else {
            continue;
        };
        out.push_str(&css[i..value_start]);

        let mut k = value_start;
        while k < bytes.len() && bytes[k] != b';' && bytes[k] != b'}' {
            let mut matched = None;
            for (needle, replacement) in REWRITES {
                if bytes.len() >= k + needle.len()
                    && bytes[k..k + needle.len()].eq_ignore_ascii_case(needle)
                    && bytes
                        .get(k + needle.len())
                        .is_none_or(|b| !is_name_continuation_byte(*b))
                {
                    matched = Some((needle.len(), *replacement));
                    break;
                }
            }
            if let Some((consumed, replacement)) = matched {
                out.push_str(replacement);
                k += consumed;
                changed = true;
            } else {
                let Some(ch) = css[k..].chars().next() else {
                    break;
                };
                out.push(ch);
                k += ch.len_utf8();
            }
        }
        i = k;
    }

    changed.then_some(out)
}

fn rewrite_attr_function_tokens(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    let has_attr = css
        .as_bytes()
        .windows("-ro-attr".len())
        .any(|w| w.eq_ignore_ascii_case(b"-ro-attr"));
    let has_counter_offset = css
        .as_bytes()
        .windows("-ro-counter-offset".len())
        .any(|w| w.eq_ignore_ascii_case(b"-ro-counter-offset"));
    if !has_attr && !has_counter_offset {
        return None;
    }
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        let rest = &css[i..];
        let lower_n = rest
            .as_bytes()
            .iter()
            .take("-ro-counter-offset(".len().max("-ro-attr-ancestor(".len()))
            .map(u8::to_ascii_lowercase)
            .collect::<Vec<u8>>();
        if lower_n.starts_with(b"-ro-attr-ancestor(") {
            out.push_str("-bd-attr-ancestor(");
            i += "-ro-attr-ancestor(".len();
            changed = true;
            continue;
        }
        if lower_n.starts_with(b"-ro-attr(") {
            out.push_str("-bd-attr(");
            i += "-ro-attr(".len();
            changed = true;
            continue;
        }
        if lower_n.starts_with(b"-ro-counter-offset(") {
            out.push_str("-bd-counter-offset(");
            i += "-ro-counter-offset(".len();
            changed = true;
            continue;
        }

        let Some(ch) = rest.chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn rewrite_pdfreactor_attr_ident_type(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    const ALIASES: &[(&[u8], &str)] = &[(b"-ro-ident", "-bd-ident")];
    rewrite_case_insensitive_aliases(css, ALIASES, ident_alias_boundary)
}

fn rewrite_break_pseudo_aliases(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    if !css
        .as_bytes()
        .windows("-ro-".len())
        .any(|w| w.eq_ignore_ascii_case(b"-ro-"))
    {
        return None;
    }

    const ALIASES: &[(&[u8], &str)] = &[
        (b"-ro-footnote-area", "-bd-footnote-area"),
        (b"-ro-before-break", "-bd-before-break"),
        (b"-ro-after-break", "-bd-after-break"),
    ];
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut i = 0;
    let mut changed = false;
    'outer: while i < bytes.len() {
        let rest = &css[i..];
        for (alias, replacement) in ALIASES {
            if rest.len() < alias.len() {
                continue;
            }
            if rest.as_bytes()[..alias.len()].eq_ignore_ascii_case(alias)
                && !rest
                    .as_bytes()
                    .get(alias.len())
                    .copied()
                    .is_some_and(is_name_continuation_byte)
            {
                out.push_str(replacement);
                i += alias.len();
                changed = true;
                continue 'outer;
            }
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn rewrite_supports_property_probes(css: &str, compat: CompatMode) -> Option<String> {
    if matches!(compat, CompatMode::None) {
        return None;
    }
    const AT_SUPPORTS: &[u8] = b"@supports";
    let bytes = css.as_bytes();
    if !bytes
        .windows(AT_SUPPORTS.len())
        .any(|w| w.eq_ignore_ascii_case(AT_SUPPORTS))
    {
        return None;
    }

    let mut out = String::with_capacity(css.len());
    let mut i = 0usize;
    let mut changed = false;
    while i < bytes.len() {
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let mut k = i + 1;
            while k < bytes.len() {
                if bytes[k] == 92 {
                    k = (k + 2).min(bytes.len());
                    continue;
                }
                if bytes[k] == quote {
                    k += 1;
                    break;
                }
                k += 1;
            }
            out.push_str(&css[i..k]);
            i = k;
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let close = css[i + 2..]
                .find("*/")
                .map_or(bytes.len(), |p| i + 2 + p + 2);
            out.push_str(&css[i..close]);
            i = close;
            continue;
        }
        let rest = &bytes[i..];
        let is_at_supports = rest.len() >= AT_SUPPORTS.len()
            && rest[..AT_SUPPORTS.len()].eq_ignore_ascii_case(AT_SUPPORTS)
            && !rest
                .get(AT_SUPPORTS.len())
                .copied()
                .is_some_and(is_name_continuation_byte);
        if !is_at_supports {
            let Some(ch) = css[i..].chars().next() else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }

        let prelude_start = i + AT_SUPPORTS.len();
        let prelude_end = bytes[prelude_start..]
            .iter()
            .position(|&b| b == b'{')
            .map_or(bytes.len(), |p| prelude_start + p);
        out.push_str(&css[i..prelude_start]);
        out.push_str(&rewrite_probes_in_supports_prelude(
            &css[prelude_start..prelude_end],
            compat,
            &mut changed,
        ));
        i = prelude_end;
    }
    changed.then_some(out)
}

fn rewrite_pdfreactor_custom_property_unset(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut i = 0usize;
    let mut changed = false;
    while i < bytes.len() {
        if bytes[i] == b'"' || bytes[i] == b'\'' {
            let quote = bytes[i];
            let mut k = i + 1;
            while k < bytes.len() {
                if bytes[k] == 92 {
                    k = (k + 2).min(bytes.len());
                    continue;
                }
                if bytes[k] == quote {
                    k += 1;
                    break;
                }
                k += 1;
            }
            out.push_str(&css[i..k]);
            i = k;
            continue;
        }
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let close = css[i + 2..]
                .find("*/")
                .map_or(bytes.len(), |p| i + 2 + p + 2);
            out.push_str(&css[i..close]);
            i = close;
            continue;
        }
        if bytes[i] == b'-' && bytes.get(i + 1) == Some(&b'-') {
            let mut j = i + 2;
            while j < bytes.len() && is_name_continuation_byte(bytes[j]) {
                j += 1;
            }
            let name_end = j;
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if name_end > i + 2 && j < bytes.len() && bytes[j] == b':' {
                j += 1;
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                const UNSET: &[u8] = b"unset";
                let value_end = j + UNSET.len();
                if value_end <= bytes.len() && bytes[j..value_end].eq_ignore_ascii_case(UNSET) {
                    let mut t = value_end;
                    while t < bytes.len() && bytes[t].is_ascii_whitespace() {
                        t += 1;
                    }
                    let terminated = t >= bytes.len() || matches!(bytes[t], b';' | b'}' | b'!');
                    if terminated {
                        out.push_str(&css[i..j]);
                        out.push_str("initial");
                        i = value_end;
                        changed = true;
                        continue;
                    }
                }
            }
        }
        let Some(ch) = css[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn rewrite_probes_in_supports_prelude(
    prelude: &str,
    compat: CompatMode,
    changed: &mut bool,
) -> String {
    let bytes = prelude.as_bytes();
    let mut out = String::with_capacity(prelude.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'(' {
            let Some(ch) = prelude[i..].chars().next() else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }

        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        let name_start = j;
        while j < bytes.len() && is_name_continuation_byte(bytes[j]) {
            j += 1;
        }
        let name_end = j;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if name_end == name_start || j >= bytes.len() || bytes[j] != b':' {
            out.push('(');
            i += 1;
            continue;
        }
        let value_start = j + 1;
        let mut depth = 0usize;
        let mut k = value_start;
        while k < bytes.len() {
            match bytes[k] {
                b'(' => depth += 1,
                b')' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                },
                _ => {},
            }
            k += 1;
        }
        if k >= bytes.len() {
            out.push('(');
            i += 1;
            continue;
        }
        let lower_name = prelude[name_start..name_end].to_ascii_lowercase();
        let raw_value = &prelude[value_start..k];
        if required_compat_for(&lower_name) != Some(compat) {
            out.push_str(&prelude[i..=k]);
            i = k + 1;
            continue;
        }
        match translate_property(&lower_name, raw_value) {
            Translated::Native {
                native_property,
                native_value,
            } => {
                out.push('(');
                out.push_str(native_property);
                out.push_str(": ");
                out.push_str(&native_value);
                out.push(')');
                *changed = true;
            },
            Translated::Natives(pairs) if !pairs.is_empty() => {
                out.push('(');
                for (idx, (native_property, native_value)) in pairs.iter().enumerate() {
                    if idx > 0 {
                        out.push_str(" and ");
                    }
                    out.push('(');
                    out.push_str(native_property);
                    out.push_str(": ");
                    out.push_str(native_value);
                    out.push(')');
                }
                out.push(')');
                *changed = true;
            },
            Translated::Satisfied => {
                out.push_str("(--bd-compat-satisfied: 1)");
                *changed = true;
            },
            Translated::Natives(_) | Translated::ValueDropped | Translated::PropertyDropped => {
                out.push_str(&prelude[i..=k]);
            },
        }
        i = k + 1;
    }
    out
}

fn rewrite_no_content_pseudo_alias(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    const ALIAS: &[u8] = b":-ro-no-content";
    const REPLACEMENT: &str = ":-bd-no-content";
    if !css
        .as_bytes()
        .windows(ALIAS.len())
        .any(|w| w.eq_ignore_ascii_case(ALIAS))
    {
        return None;
    }
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        let rest = &css[i..];
        if rest.len() >= ALIAS.len()
            && rest.as_bytes()[..ALIAS.len()].eq_ignore_ascii_case(ALIAS)
            && !rest
                .as_bytes()
                .get(ALIAS.len())
                .copied()
                .is_some_and(is_name_continuation_byte)
        {
            let preceding_is_colon = i > 0 && bytes[i - 1] == b':';
            if !preceding_is_colon {
                out.push_str(REPLACEMENT);
                i += ALIAS.len();
                changed = true;
                continue;
            }
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn rewrite_double_colon_structural_pseudo_classes(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    if !css.contains("::") {
        return None;
    }
    const NAMES: &[&str] = &[
        "first-child",
        "last-child",
        "only-child",
        "nth-child",
        "nth-last-child",
        "first-of-type",
        "last-of-type",
        "only-of-type",
        "nth-of-type",
        "nth-last-of-type",
        "empty",
        "root",
    ];
    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        if bytes[i] == b':' && bytes.get(i + 1) == Some(&b':') {
            let after = &css[i + 2..];
            let matched = NAMES.iter().find(|name| {
                after.len() >= name.len()
                    && after.as_bytes()[..name.len()].eq_ignore_ascii_case(name.as_bytes())
                    && !after
                        .as_bytes()
                        .get(name.len())
                        .copied()
                        .is_some_and(is_name_continuation_byte)
            });
            if let Some(name) = matched {
                out.push(':');
                out.push_str(&css[i + 2..i + 2 + name.len()]);
                i += 2 + name.len();
                changed = true;
                continue;
            }
        }
        let Some(ch) = css[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn rewrite_case_insensitive_aliases(
    css: &str,
    aliases: &[(&[u8], &str)],
    boundary_matches: impl Fn(&[u8], usize, usize) -> bool,
) -> Option<String> {
    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    let mut changed = false;
    'outer: while i < bytes.len() {
        let rest = &css[i..];
        for (alias, replacement) in aliases {
            if rest.len() < alias.len()
                || !rest.as_bytes()[..alias.len()].eq_ignore_ascii_case(alias)
                || !boundary_matches(bytes, i, alias.len())
            {
                continue;
            }
            out.push_str(replacement);
            i += alias.len();
            changed = true;
            continue 'outer;
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn ident_alias_boundary(bytes: &[u8], at: usize, alias_len: usize) -> bool {
    !bytes
        .get(at + alias_len)
        .copied()
        .is_some_and(is_name_continuation_byte)
        && (at == 0 || !is_name_continuation_byte(bytes[at - 1]))
}

fn rewrite_pdfreactor_comment_colour_keywords(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }

    const ALIASES: &[(&[u8], &str)] = &[
        (b"-ro-comment-highlight", "#FFFF0B"),
        (b"-ro-comment-underline", "#23FF06"),
        (b"-ro-comment-strikeout", "#FB0007"),
    ];
    let bytes = css.as_bytes();

    let has_marker = bytes
        .windows("-ro-comment-".len())
        .any(|w| w.eq_ignore_ascii_case(b"-ro-comment-"));
    if !has_marker {
        return None;
    }
    rewrite_case_insensitive_aliases(css, ALIASES, ident_alias_boundary)
}

fn rewrite_pdfreactor_counter_style_aliases(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    const ALIASES: &[(&[u8], &str)] = &[
        (b"-ro-spelled-out-en-ordinal", "bd-spelled-out-en-ordinal"),
        (b"-ro-spelled-out-en", "bd-spelled-out-en"),
        (b"-ro-spelled-out-de", "bd-spelled-out-de"),
        (b"-ro-spelled-out-fr", "bd-spelled-out-fr"),
    ];
    let bytes = css.as_bytes();

    let has_spelled = bytes
        .windows("-ro-spelled-out-".len())
        .any(|w| w.eq_ignore_ascii_case(b"-ro-spelled-out-"));
    if !has_spelled {
        return None;
    }
    rewrite_case_insensitive_aliases(css, ALIASES, ident_alias_boundary)
}

fn rewrite_pdfreactor_page_relative_unit_aliases(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }

    const ALIASES: &[(&[u8], &str)] = &[
        (b"-ro-pmin", "-bd-pmin"),
        (b"-ro-pmax", "-bd-pmax"),
        (b"-ro-bmin", "-bd-bmin"),
        (b"-ro-bmax", "-bd-bmax"),
        (b"-ro-pw", "-bd-pw"),
        (b"-ro-ph", "-bd-ph"),
        (b"-ro-pi", "-bd-pi"),
        (b"-ro-pb", "-bd-pb"),
        (b"-ro-bw", "-bd-bw"),
        (b"-ro-bh", "-bd-bh"),
        (b"-ro-bi", "-bd-bi"),
        (b"-ro-bb", "-bd-bb"),
    ];
    let bytes = css.as_bytes();

    let has_marker = bytes
        .windows(b"-ro-p".len())
        .any(|w| w.eq_ignore_ascii_case(b"-ro-p"))
        || bytes
            .windows(b"-ro-b".len())
            .any(|w| w.eq_ignore_ascii_case(b"-ro-b"));
    if !has_marker {
        return None;
    }
    rewrite_case_insensitive_aliases(css, ALIASES, |bytes, at, alias_len| {
        if bytes
            .get(at + alias_len)
            .copied()
            .is_some_and(is_name_continuation_byte)
        {
            return false;
        }
        at == 0 || {
            let prev = bytes[at - 1];
            prev.is_ascii_digit() || prev == b'.' || !is_name_continuation_byte(prev)
        }
    })
}

fn rewrite_pdfreactor_outside_sidenote_area(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }

    const FOREIGN_AT_RULE: &[u8] = b"@-ro-sidenote";
    let bytes = css.as_bytes();
    if !bytes
        .windows(FOREIGN_AT_RULE.len())
        .any(|window| window.eq_ignore_ascii_case(FOREIGN_AT_RULE))
    {
        return None;
    }

    let mut out = String::with_capacity(css.len() + 96);
    let mut cursor = 0usize;
    let mut at = 0usize;
    let mut changed = false;

    while at < bytes.len() {
        match bytes[at] {
            b'"' | b'\'' => {
                at = skip_css_string(bytes, at);
                continue;
            },
            b'/' if bytes.get(at + 1) == Some(&b'*') => {
                at = skip_css_comment(css, at);
                continue;
            },
            _ => {},
        }

        let matches_at_rule = bytes.len() >= at + FOREIGN_AT_RULE.len()
            && bytes[at..at + FOREIGN_AT_RULE.len()].eq_ignore_ascii_case(FOREIGN_AT_RULE)
            && is_at_keyword_boundary(bytes.get(at + FOREIGN_AT_RULE.len()).copied());
        if !matches_at_rule {
            let Some(ch) = css[at..].chars().next() else {
                break;
            };
            at += ch.len_utf8();
            continue;
        }

        let mut prelude = skip_css_whitespace_and_comments(css, at + FOREIGN_AT_RULE.len());
        if bytes.get(prelude) != Some(&b':') {
            at += FOREIGN_AT_RULE.len();
            continue;
        }
        prelude = skip_css_whitespace_and_comments(css, prelude + 1);

        const OUTSIDE: &[u8] = b"outside";
        let outside_end = prelude + OUTSIDE.len();
        let matches_outside = outside_end <= bytes.len()
            && bytes[prelude..outside_end].eq_ignore_ascii_case(OUTSIDE)
            && bytes
                .get(outside_end)
                .is_none_or(|&next| !is_name_continuation_byte(next));
        if !matches_outside {
            at += FOREIGN_AT_RULE.len();
            continue;
        }

        let open = skip_css_whitespace_and_comments(css, outside_end);
        if bytes.get(open) != Some(&b'{') {
            at += FOREIGN_AT_RULE.len();
            continue;
        }
        let Some(close) = matching_css_curly_close(css, open) else {
            at += FOREIGN_AT_RULE.len();
            continue;
        };

        let body = &css[open + 1..close];
        let inner_gaps = sidenote_inner_gap_values(body);

        out.push_str(&css[cursor..at]);
        out.push_str("@-bd-sidenote { -bd-sidenote-side: outside;");
        out.push_str(body);
        for gap in inner_gaps {
            if out.trim_end().ends_with(|c: char| c != ';' && c != '{') {
                out.push(';');
            }
            out.push_str(" -bd-sidenote-offset: ");
            out.push_str(&gap);
            out.push(';');
        }
        out.push('}');

        cursor = close + 1;
        at = close + 1;
        changed = true;
    }

    if !changed {
        return None;
    }
    out.push_str(&css[cursor..]);
    Some(out)
}

fn skip_css_comment(css: &str, at: usize) -> usize {
    css[at + 2..]
        .find("*/")
        .map_or(css.len(), |end| at + 2 + end + 2)
}

fn skip_css_whitespace_and_comments(css: &str, mut at: usize) -> usize {
    let bytes = css.as_bytes();
    loop {
        while bytes.get(at).is_some_and(u8::is_ascii_whitespace) {
            at += 1;
        }
        if bytes.get(at) == Some(&b'/') && bytes.get(at + 1) == Some(&b'*') {
            at = skip_css_comment(css, at);
            continue;
        }
        return at;
    }
}

fn matching_css_curly_close(css: &str, open: usize) -> Option<usize> {
    let bytes = css.as_bytes();
    let mut depth = 1usize;
    let mut at = open + 1;
    while at < bytes.len() {
        match bytes[at] {
            b'"' | b'\'' => at = skip_css_string(bytes, at),
            b'/' if bytes.get(at + 1) == Some(&b'*') => at = skip_css_comment(css, at),
            b'{' => {
                depth += 1;
                at += 1;
            },
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(at);
                }
                at += 1;
            },
            _ => at += 1,
        }
    }
    None
}

fn sidenote_inner_gap_values(body: &str) -> Vec<String> {
    let mut input = ParserInput::new(body);
    let mut parser = Parser::new(&mut input);
    let mut values = Vec::new();

    loop {
        parser.skip_whitespace();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Ident(ident) => {
                    let name = ident.as_ref().to_string();
                    if !matches!(parser.next(), Ok(Token::Colon)) {
                        super::metadata::skip_to_decl_end(&mut parser);
                        continue;
                    }
                    let value_start = parser.position().byte_index();
                    let value_end = skip_to_decl_end_byte(&mut parser);
                    if name.eq_ignore_ascii_case("margin-inline-end") {
                        let value = body[value_start..value_end].trim();
                        if !value.is_empty() {
                            values.push(value.to_string());
                        }
                    }
                },
                Token::Semicolon | Token::WhiteSpace(_) | Token::Comment(_) => {},
                _ => super::metadata::skip_to_decl_end(&mut parser),
            },
            Err(_) => return values,
        }
    }
}

fn rewrite_pdfreactor_corner_margin_box_aliases(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    const ALIASES: &[(&[u8], &str)] = &[
        (b"@right-top-corner", "@top-right-corner"),
        (b"@right-bottom-corner", "@bottom-right-corner"),
        (b"@left-top-corner", "@top-left-corner"),
        (b"@left-bottom-corner", "@bottom-left-corner"),
    ];
    let bytes = css.as_bytes();

    let has_marker = bytes
        .windows("-corner".len())
        .any(|w| w.eq_ignore_ascii_case(b"-corner"));
    if !has_marker {
        return None;
    }
    rewrite_case_insensitive_aliases(css, ALIASES, |bytes, at, alias_len| {
        is_at_keyword_boundary(bytes.get(at + alias_len).copied())
    })
}

fn rewrite_pdfreactor_nth_page_pseudo_alias(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }

    const ALIASES: [&[u8]; 2] = [b":-ro-nth(", b":ro-nth("];
    const REPLACEMENT: &str = ":nth(";
    let bytes = css.as_bytes();
    let matches_alias_at = |at: usize| {
        ALIASES
            .iter()
            .find(|alias| {
                bytes.len() >= at + alias.len()
                    && bytes[at..at + alias.len()].eq_ignore_ascii_case(alias)
            })
            .map(|alias| alias.len())
    };
    if !(0..bytes.len()).any(|at| matches_alias_at(at).is_some()) {
        return None;
    }
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        let preceding_is_colon = i > 0 && bytes[i - 1] == b':';
        if !preceding_is_colon && let Some(alias_len) = matches_alias_at(i) {
            let arg_start = i + alias_len;
            if let Some(close) = css[arg_start..].find(')') {
                let arg = css[arg_start..arg_start + close].trim();
                let mut parts = arg.split_whitespace();
                if let (Some("1"), Some(of), Some(name), None) =
                    (parts.next(), parts.next(), parts.next(), parts.next())
                    && of.eq_ignore_ascii_case("of")
                    && !name.is_empty()
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
                {
                    out.push(' ');
                    out.push_str(name);
                    out.push_str(":first-of-group");
                    i = arg_start + close + 1;
                    changed = true;
                    continue;
                }
            }
            out.push_str(REPLACEMENT);
            i += alias_len;
            changed = true;
            continue;
        }
        let Some(ch) = css[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn rewrite_pdfreactor_last_page_pseudo_alias(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    const ALIAS: &[u8] = b":-ro-last";
    const REPLACEMENT: &str = ":last";
    let bytes = css.as_bytes();
    if !bytes
        .windows(ALIAS.len())
        .any(|w| w.eq_ignore_ascii_case(ALIAS))
    {
        return None;
    }
    fn is_ident_continuation(b: u8) -> bool {
        b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
    }
    let mut out = String::with_capacity(css.len());
    let mut i = 0;
    let mut changed = false;
    while i < bytes.len() {
        let rest = &css[i..];
        if rest.len() >= ALIAS.len() && rest.as_bytes()[..ALIAS.len()].eq_ignore_ascii_case(ALIAS) {
            let preceding_is_colon = i > 0 && bytes[i - 1] == b':';

            let next_byte = bytes.get(i + ALIAS.len()).copied();
            let suffix_continues_ident = next_byte.is_some_and(is_ident_continuation);
            if !preceding_is_colon && !suffix_continues_ident {
                out.push_str(REPLACEMENT);
                i += ALIAS.len();
                changed = true;
                continue;
            }
        }
        let Some(ch) = rest.chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    changed.then_some(out)
}

fn rewrite_spot_colour_at_rules(
    css: &str,
    compat: CompatMode,
) -> (Option<String>, Vec<CompatWarning>) {
    const PRINCE: &[u8] = b"@prince-color";
    const RO_SPOT: &[u8] = b"@-ro-spot-color";
    const BD: &str = "@-bd-colour";

    let has_prince = css
        .as_bytes()
        .windows(PRINCE.len())
        .any(|w| w.eq_ignore_ascii_case(PRINCE));
    let has_ro = css
        .as_bytes()
        .windows(RO_SPOT.len())
        .any(|w| w.eq_ignore_ascii_case(RO_SPOT));
    if !has_prince && !has_ro {
        return (None, Vec::new());
    }

    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut warnings: Vec<CompatWarning> = Vec::new();
    let mut i = 0usize;
    let mut line: u32 = 1;
    let mut column: u32 = 1;
    let mut changed = false;

    while i < bytes.len() {
        let remaining = &bytes[i..];

        let matched: Option<(&[u8], CompatMode, &str)> = if remaining.len() >= RO_SPOT.len()
            && remaining[..RO_SPOT.len()].eq_ignore_ascii_case(RO_SPOT)
            && is_at_keyword_boundary(remaining.get(RO_SPOT.len()).copied())
        {
            Some((RO_SPOT, CompatMode::PdfReactor, "@-ro-spot-color"))
        } else if remaining.len() >= PRINCE.len()
            && remaining[..PRINCE.len()].eq_ignore_ascii_case(PRINCE)
            && is_at_keyword_boundary(remaining.get(PRINCE.len()).copied())
        {
            Some((PRINCE, CompatMode::Prince, "@prince-color"))
        } else {
            None
        };

        if let Some((alias_bytes, required, authored)) = matched {
            if compat == required {
                out.push_str(BD);
                changed = true;
            } else {
                out.push_str(authored);
                warnings.push(CompatWarning {
                    kind: CompatWarningKind::UnknownVendor { required },
                    property: authored.to_string(),
                    line,
                    column,
                });
            }

            i += alias_bytes.len();

            #[allow(clippy::cast_possible_truncation)]
            let alias_len = alias_bytes.len() as u32;
            column = column.saturating_add(alias_len);
            continue;
        }

        let Some(ch) = css[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
        if ch == '\n' {
            line = line.saturating_add(1);
            column = 1;
        } else {
            column = column.saturating_add(1);
        }
    }

    let rewritten = changed.then_some(out);
    (rewritten, warnings)
}

struct SpotColourRegistration {
    name: String,
    colour_values: String,
}

impl SpotColourRegistration {
    fn serialization(&self) -> String {
        format!(
            "@-bd-colour {} {{ colour-values: {}; }}",
            self.name, self.colour_values
        )
    }
}

enum ColourRegistrationOutput<'a> {
    Append,
    Collect(&'a mut Vec<SpotColourRegistration>),
    /// The enclosing root has already collected these generated siblings.
    SourceOnly,
}

fn rewrite_spot_colour_function_tokens(
    css: &str,
    compat: CompatMode,
    registrations: ColourRegistrationOutput<'_>,
) -> Option<String> {
    let aliases: &[(&[u8], &str)] = match compat {
        CompatMode::Prince => &[
            (b"prince-separation(", "-bd-separation("),
            (b"prince-spot(", "-bd-spot("),
        ],
        CompatMode::PdfReactor => &[
            (b"-ro-separation(", "-bd-separation("),
            (b"-ro-spot(", "-bd-spot("),
        ],
        CompatMode::None => return None,
    };

    if !aliases.iter().any(|(needle, _)| {
        css.as_bytes()
            .windows(needle.len())
            .any(|w| w.eq_ignore_ascii_case(needle))
    }) {
        return None;
    }

    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut i = 0usize;
    let mut changed = false;

    let mut hoisted: Vec<SpotColourRegistration> = Vec::new();
    while i < bytes.len() {
        let remaining = &bytes[i..];
        let mut hit = false;
        for (needle, replacement) in aliases {
            if remaining.len() >= needle.len()
                && remaining[..needle.len()].eq_ignore_ascii_case(needle)
            {
                out.push_str(replacement);
                i += needle.len();
                if let Some(inline) = rewrite_inline_spot_arguments(&css[i..]) {
                    out.push_str(&inline.arguments);
                    i += inline.consumed;
                    if let Some(entry) = inline.registry_entry
                        && !hoisted.iter().any(|existing| existing.name == entry.name)
                    {
                        hoisted.push(entry);
                    }
                }
                changed = true;
                hit = true;
                break;
            }
        }
        if hit {
            continue;
        }
        let Some(ch) = css[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }
    if changed && !hoisted.is_empty() {
        match registrations {
            ColourRegistrationOutput::Append => {
                for registration in hoisted {
                    out.push('\n');
                    out.push_str(&registration.serialization());
                }
                out.push('\n');
            },
            ColourRegistrationOutput::Collect(output) => output.extend(hoisted),
            ColourRegistrationOutput::SourceOnly => {},
        }
    }
    changed.then_some(out)
}

struct InlineSpotArguments {
    arguments: String,

    consumed: usize,

    registry_entry: Option<SpotColourRegistration>,
}

fn rewrite_inline_spot_arguments(rest: &str) -> Option<InlineSpotArguments> {
    let bytes = rest.as_bytes();
    let mut depth = 0usize;
    let mut quote: Option<u8> = None;
    let mut splits: Vec<usize> = Vec::new();
    let mut close: Option<usize> = None;
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if b == 92 {
                i += 2;
                continue;
            }
            if b == q {
                quote = None;
            }
        } else {
            match b {
                b'"' | b'\'' => quote = Some(b),
                b'(' => depth += 1,
                b')' => {
                    if depth == 0 {
                        close = Some(i);
                        break;
                    }
                    depth -= 1;
                },
                b',' if depth == 0 => splits.push(i),
                _ => {},
            }
        }
        i += 1;
    }
    let close = close?;
    if splits.len() > 2 {
        return None;
    }
    let name_end = splits.first().copied().unwrap_or(close);
    let name_raw = rest[..name_end].trim();
    let quoted_name = unquote_css_string(name_raw);
    if splits.len() < 2 {
        let name = quoted_name?;
        let mut arguments = escape_css_ident(&name);
        arguments.push_str(&rest[name_end..=close]);
        return Some(InlineSpotArguments {
            arguments,
            consumed: close + 1,
            registry_entry: None,
        });
    }

    let tint_raw = rest[splits[0] + 1..splits[1]].trim();
    let alternate_raw = rest[splits[1] + 1..close].trim();
    let tint_is_number_or_percentage = tint_raw
        .strip_suffix('%')
        .unwrap_or(tint_raw)
        .parse::<f32>()
        .is_ok();
    if !tint_is_number_or_percentage || alternate_raw.is_empty() {
        return None;
    }
    let name = quoted_name.unwrap_or_else(|| name_raw.to_string());
    if name.is_empty() {
        return None;
    }
    let escaped = escape_css_ident(&name);
    Some(InlineSpotArguments {
        arguments: format!("{escaped}, {tint_raw})"),
        consumed: close + 1,
        registry_entry: Some(SpotColourRegistration {
            name: escaped,
            colour_values: alternate_raw.to_string(),
        }),
    })
}

fn unquote_css_string(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let q = bytes[0];
    if (q != b'"' && q != b'\'') || bytes[bytes.len() - 1] != q {
        return None;
    }
    let inner = &s[1..s.len() - 1];
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == char::from(92) {
            if let Some(escaped) = chars.next() {
                out.push(escaped);
            }
        } else {
            out.push(c);
        }
    }
    Some(out)
}

fn escape_css_ident(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (idx, ch) in name.chars().enumerate() {
        if idx == 0 && ch.is_ascii_digit() {
            out.push(char::from(92));
            out.push('3');
            out.push(ch);
            out.push(' ');
        } else if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' || (ch as u32) >= 0x80 {
            out.push(ch);
        } else {
            out.push(char::from(92));
            out.push(ch);
        }
    }
    out
}

fn rewrite_pdfreactor_cmyk_function_tokens(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }

    const NEEDLE: &[u8] = b"cmyk(";
    if !css
        .as_bytes()
        .windows(NEEDLE.len())
        .any(|w| w.eq_ignore_ascii_case(NEEDLE))
    {
        return None;
    }

    const DEVICE_PREFIX: &[u8] = b"device-";

    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len() + 32);
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        if bytes.len() >= i + NEEDLE.len()
            && bytes[i..i + NEEDLE.len()].eq_ignore_ascii_case(NEEDLE)
        {
            let already_prefixed = i >= DEVICE_PREFIX.len()
                && bytes[i - DEVICE_PREFIX.len()..i].eq_ignore_ascii_case(DEVICE_PREFIX);

            if already_prefixed {
                let Some(ch) = css[i..].chars().next() else {
                    break;
                };
                out.push(ch);
                i += ch.len_utf8();
            } else {
                out.push_str("device-cmyk(");
                i += NEEDLE.len();
                changed = true;
            }
            continue;
        }

        let Some(ch) = css[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }

    changed.then_some(out)
}

fn rewrite_pdfreactor_gray_function_tokens(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    const GRAY_NEEDLE: &[u8] = b"gray(";
    const GREY_NEEDLE: &[u8] = b"grey(";

    let bytes = css.as_bytes();

    let has_any = bytes
        .windows(GRAY_NEEDLE.len())
        .any(|w| w.eq_ignore_ascii_case(GRAY_NEEDLE) || w.eq_ignore_ascii_case(GREY_NEEDLE));
    if !has_any {
        return None;
    }

    let mut out = String::with_capacity(css.len() + 32);
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        let matches_keyword = bytes.len() >= i + GRAY_NEEDLE.len()
            && (bytes[i..i + GRAY_NEEDLE.len()].eq_ignore_ascii_case(GRAY_NEEDLE)
                || bytes[i..i + GREY_NEEDLE.len()].eq_ignore_ascii_case(GREY_NEEDLE));

        let should_rewrite =
            matches_keyword && { !(i > 0 && is_name_continuation_byte(bytes[i - 1])) };

        if should_rewrite {
            let body_start = i + GRAY_NEEDLE.len();
            let Some(close_offset) = bytes[body_start..].iter().position(|&b| b == b')') else {
                out.push_str(&css[i..body_start]);
                i = body_start;
                continue;
            };
            let body_end = body_start + close_offset;
            let body = &css[body_start..body_end];

            #[allow(clippy::redundant_closure_for_method_calls)]
            let parts: Vec<&str> = body.split(',').map(|s| s.trim()).collect();
            let level = parts.first().copied().unwrap_or("0");
            let alpha = parts.get(1).copied();

            match alpha {
                Some(alpha_token) => {
                    out.push_str("rgb(");
                    out.push_str(level);
                    out.push(' ');
                    out.push_str(level);
                    out.push(' ');
                    out.push_str(level);
                    out.push_str(" / ");
                    out.push_str(alpha_token);
                    out.push(')');
                },
                None => {
                    out.push_str("rgb(");
                    out.push_str(level);
                    out.push(' ');
                    out.push_str(level);
                    out.push(' ');
                    out.push_str(level);
                    out.push(')');
                },
            }
            i = body_end + 1;
            changed = true;
            continue;
        }

        let Some(ch) = css[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }

    changed.then_some(out)
}

fn legacy_attr_type_replacement(keyword: &str) -> Option<&'static str> {
    Some(match keyword.to_ascii_lowercase().as_str() {
        "string" => "raw-string",
        "integer" => "type(<integer>)",
        "length" => "type(<length>)",
        "angle" => "type(<angle>)",
        "time" => "type(<time>)",
        "frequency" => "type(<frequency>)",
        "color" => "type(<color>)",
        "url" => "type(<url>)",
        _ => return None,
    })
}

fn attr_body_end(bytes: &[u8], body_start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = body_start;
    while i < bytes.len() {
        match bytes[i] {
            92 => i += 1,
            quote @ (b'"' | b'\'') => {
                i += 1;
                while i < bytes.len() && bytes[i] != quote {
                    i += if bytes[i] == 92 { 2 } else { 1 };
                }
            },
            b'(' => depth += 1,
            b')' if depth == 0 => return Some(i),
            b')' => depth -= 1,
            _ => {},
        }
        i += 1;
    }
    None
}

fn rewrite_legacy_attr_body(body: &str) -> Option<String> {
    let (head, tail) = match body.find(',') {
        Some(at) => (&body[..at], Some(&body[at..])),
        None => (body, None),
    };
    let mut tokens = head.split_whitespace();
    let name = tokens.next()?;
    let keyword = tokens.next()?;
    if tokens.next().is_some() {
        return None;
    }
    let replacement = legacy_attr_type_replacement(keyword)?;
    let mut out = String::with_capacity(body.len() + 12);
    out.push_str(name);
    out.push(' ');
    out.push_str(replacement);
    if let Some(tail) = tail {
        out.push_str(tail);
    }
    Some(out)
}

fn rewrite_pdfreactor_legacy_attr_types(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    const NEEDLE: &[u8] = b"attr(";

    let bytes = css.as_bytes();

    if !bytes
        .windows(NEEDLE.len())
        .any(|w| w.eq_ignore_ascii_case(NEEDLE))
    {
        return None;
    }

    let mut out = String::with_capacity(css.len() + 32);
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        let is_attr = i + NEEDLE.len() <= bytes.len()
            && bytes[i..i + NEEDLE.len()].eq_ignore_ascii_case(NEEDLE)
            && !(i > 0 && is_name_continuation_byte(bytes[i - 1]));

        if is_attr {
            let body_start = i + NEEDLE.len();
            if let Some(body_end) = attr_body_end(bytes, body_start)
                && let Some(rewritten) = rewrite_legacy_attr_body(&css[body_start..body_end])
            {
                out.push_str("attr(");
                out.push_str(&rewritten);
                out.push(')');
                i = body_end + 1;
                changed = true;
                continue;
            }

            out.push_str(&css[i..body_start]);
            i = body_start;
            continue;
        }

        let Some(ch) = css[i..].chars().next() else {
            break;
        };
        out.push(ch);
        i += ch.len_utf8();
    }

    changed.then_some(out)
}

pub fn pdfreactor_builtin_counter_stylesheet_root(
    compat: CompatMode,
) -> Option<stylo_cssom_model::InternalStylesheetRoot> {
    matches!(compat, CompatMode::PdfReactor).then(|| {
        stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::Author,
            [
                stylo_cssom_model::RuleNode::counter_style(
                    "asterisks",
                    [
                        stylo_cssom_model::RuleDeclaration::new("system", "symbolic"),
                        stylo_cssom_model::RuleDeclaration::new("symbols", r#""*""#),
                    ],
                ),
                stylo_cssom_model::RuleNode::counter_style(
                    "-ro-footnote",
                    [
                        stylo_cssom_model::RuleDeclaration::new("system", "symbolic"),
                        stylo_cssom_model::RuleDeclaration::new(
                            "symbols",
                            r#""*" "\2051 " "\2020 " "\2021 ""#,
                        ),
                        stylo_cssom_model::RuleDeclaration::new("suffix", r#"" ""#),
                    ],
                ),
            ],
        )
    })
}

#[must_use]
pub fn project_compat_root(
    root: &stylo_cssom_model::InternalStylesheetRoot,
    compat: CompatMode,
) -> (
    stylo_cssom_model::InternalStylesheetRoot,
    Vec<CompatWarning>,
) {
    let serialization = root.projection_serialization();
    let mut registrations = Vec::new();
    let translation = translate_compat_with_registrations(
        &serialization,
        compat,
        ColourRegistrationOutput::Collect(&mut registrations),
    );
    let mut rules = if translation.rewritten == serialization {
        root.rules().to_vec()
    } else {
        crate::author_rule_projection::project_rule_sources(root.rules(), &mut |css| {
            translate_compat_with_registrations(css, compat, ColourRegistrationOutput::SourceOnly)
                .rewritten
                .into_owned()
        })
    };
    for registration in registrations {
        let css = registration.serialization();
        let generated = translate_compat(&css, compat);
        rules.extend_from_slice(
            crate::authored_rules::ParsedStylesheet::parse(&generated.rewritten)
                .expect("a generated colour registration must remain a valid stylesheet")
                .rule_nodes(),
        );
    }
    (
        stylo_cssom_model::InternalStylesheetRoot::new(root.origin(), rules),
        translation.warnings,
    )
}

#[cfg(test)]
mod internal_stylesheet_root_tests {
    use stylo_cssom_model::{
        InternalStylesheetRoot, RuleDeclaration, RuleGrammar, RuleNode, StyleOrigin,
    };

    use super::{CompatMode, pdfreactor_builtin_counter_stylesheet_root, project_compat_root};

    #[test]
    fn vendor_and_compatibility_projections_retain_typed_rule_grammars() {
        let counters = pdfreactor_builtin_counter_stylesheet_root(CompatMode::PdfReactor)
            .expect("PDFReactor compatibility must supply its counter styles");
        assert_eq!(
            counters.projection_serialization(),
            concat!(
                "@counter-style asterisks { system: symbolic; symbols: \"*\"; }\n",
                "@counter-style -ro-footnote { system: symbolic; symbols: \"*\" \"\\2051 \" \"\\2020 \" \"\\2021 \"; suffix: \" \"; }",
            )
        );
        assert!(
            counters
                .rules()
                .iter()
                .all(|rule| rule.grammar() == RuleGrammar::CounterStyle)
        );

        let authored = InternalStylesheetRoot::new(
            StyleOrigin::Author,
            [RuleNode::internal_style(
                "p",
                [RuleDeclaration::new("float", "-ro-top")],
            )],
        );
        let (projection, warnings) = project_compat_root(&authored, CompatMode::PdfReactor);
        assert!(warnings.is_empty());
        assert_eq!(projection.rules()[0].grammar(), RuleGrammar::Style);
        assert!(
            projection
                .projection_serialization()
                .contains("float: -bd-top")
        );
    }

    #[test]
    fn compatibility_projection_retains_namespace_context_across_rules() {
        let parsed = crate::authored_rules::ParsedStylesheet::parse(
            "@namespace Foo 'y'; @namespace foo 'x'; Foo|test { background: lime }",
        )
        .expect("the namespaced fixture must parse");
        let authored =
            InternalStylesheetRoot::new(StyleOrigin::Author, parsed.rule_nodes().to_vec());

        let (projection, warnings) = project_compat_root(&authored, CompatMode::None);

        assert!(warnings.is_empty());
        assert_eq!(
            projection
                .rules()
                .iter()
                .map(RuleNode::grammar)
                .collect::<Vec<_>>(),
            [
                RuleGrammar::Namespace,
                RuleGrammar::Namespace,
                RuleGrammar::Style
            ]
        );
        assert!(projection.projection_serialization().contains("Foo|test"));
    }

    #[test]
    fn compatibility_projection_preserves_named_rule_source_identity() {
        let parsed = crate::authored_rules::ParsedStylesheet::parse(
            "@media all { @position-try --side { left:1px } .other { float:-ro-top } }",
        )
        .unwrap();
        let source = InternalStylesheetRoot::new(StyleOrigin::Author, parsed.rule_nodes());
        let original = source.rules()[0].payload().nested()[0]
            .payload()
            .source_stamp()
            .unwrap();
        for compat in [CompatMode::None, CompatMode::PdfReactor] {
            let (projection, _) = project_compat_root(&source, compat);
            assert_eq!(
                projection.rules()[0].payload().nested()[0]
                    .payload()
                    .source_stamp(),
                Some(original),
                "{compat:?}"
            );
            if compat == CompatMode::PdfReactor {
                let projected = crate::rule_parser::ParsedCssRule::parse_stylesheet(
                    &projection.projection_serialization(),
                );
                let nested = projected[0].nested_rules().expect("projected media rules");
                assert_eq!(
                    nested[1].declaration_value("float").as_deref(),
                    Some("-bd-top")
                );
            }
        }
    }

    #[test]
    fn compatibility_projection_separates_generated_rules_from_contextual_sources() {
        let parsed = crate::authored_rules::ParsedStylesheet::parse(concat!(
            "@namespace ink 'urn:ink'; @media all {",
            "@position-try --side { left:1px }",
            "ink|p { margin:var(--space); color:-ro-spot('Ink',50%,rgb(1,2,3)); -prince-pdf-profile:PDF/A-1b }",
            "ink|div { color:-ro-spot('Ink',75%,rgb(4,5,6)) } }",
        ))
        .expect("the contextual compatibility fixture must parse");
        let source = InternalStylesheetRoot::new(StyleOrigin::Author, parsed.rule_nodes());
        let original = source.rules()[1].payload().nested();
        let original_block = original[1]
            .payload()
            .declaration_block()
            .expect("pending style block");
        assert!(
            original_block
                .declarations()
                .iter()
                .any(|declaration| declaration.pending_substitution().is_some())
        );
        let expected_warnings =
            super::translate_compat(&source.projection_serialization(), CompatMode::PdfReactor)
                .warnings;
        assert!(
            !expected_warnings.is_empty(),
            "warning-location control must exercise a warning"
        );

        let (projection, warnings) = project_compat_root(&source, CompatMode::PdfReactor);
        assert_eq!(warnings, expected_warnings);
        assert_eq!(
            projection
                .rules()
                .iter()
                .map(RuleNode::grammar)
                .collect::<Vec<_>>(),
            [
                RuleGrammar::Namespace,
                RuleGrammar::Media,
                RuleGrammar::BdColour
            ],
            "the colour definition is emitted once at the root, not inside an authored source",
        );
        let projected = projection.rules()[1].payload().nested();
        assert_eq!(projected.len(), original.len());
        assert_eq!(
            projected[0].payload().source_stamp(),
            original[0].payload().source_stamp()
        );
        assert_eq!(
            projected[1].payload().declaration_block(),
            Some(original_block)
        );
        for source in std::iter::once(&projection.rules()[1]).chain(projected.iter()) {
            assert!(!source.projection_serialization().contains("@-bd-colour"));
        }
        let generated = projection.rules()[2].projection_serialization();
        assert!(generated.contains("Ink"));
        assert!(
            generated.contains("rgb(1,2,3)"),
            "the first definition wins: {generated}"
        );
        assert!(
            projection.rules()[1]
                .projection_serialization()
                .contains("-bd-spot(Ink, 50%)")
        );
    }
}

fn rewrite_pdfreactor_page_float_keywords(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    if !mentions_ident(css, "float") {
        return None;
    }
    rewrite_declaration_value_idents(
        css,
        b"float",
        &[(b"-ro-bottom", "-bd-bottom"), (b"-ro-top", "-bd-top")],
    )
}

fn rewrite_pdfreactor_font_size_adjust_from_font(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor)
        || !mentions_ident(css, "font-size-adjust")
        || !mentions_ident(css, "-ro-from-font")
    {
        return None;
    }

    rewrite_declaration_value_idents(css, b"font-size-adjust", &[(b"-ro-from-font", "from-font")])
}

fn rewrite_pdfreactor_text_overflow_scale_down(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor)
        || !mentions_ident(css, "text-overflow")
        || !mentions_ident(css, "-ro-scale-down")
    {
        return None;
    }

    rewrite_declaration_value_idents(
        css,
        b"text-overflow",
        &[(b"-ro-scale-down", "-bd-scale-down")],
    )
}

fn rewrite_pdfreactor_marks_bleed_keyword(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor)
        || !mentions_ident(css, "marks")
        || !mentions_ident(css, "-ro-bleed")
    {
        return None;
    }

    rewrite_declaration_value_idents(css, b"marks", &[(b"-ro-bleed", "bleed")])
}

fn rewrite_pdfreactor_page_box_background_origin(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }
    const PROPERTY: &[u8] = b"background-origin";
    const VALUE: &[u8] = b"-ro-page-box";
    if !mentions_ident(css, "-ro-page-box") {
        return None;
    }
    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut i = 0usize;
    let mut changed = false;
    while i < bytes.len() {
        if let Some(next) = copy_opaque_token(css, i, &mut out) {
            i = next;
            continue;
        }
        if !ident_at(bytes, i, PROPERTY) {
            let Some(ch) = css[i..].chars().next() else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        let mut cursor = i + PROPERTY.len();
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b':') {
            out.push_str(&css[i..i + PROPERTY.len()]);
            i += PROPERTY.len();
            continue;
        }
        cursor += 1;
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if !ident_at(bytes, cursor, VALUE) {
            out.push_str(&css[i..cursor]);
            i = cursor;
            continue;
        }
        let mut after = cursor + VALUE.len();
        while bytes.get(after).is_some_and(u8::is_ascii_whitespace) {
            after += 1;
        }
        let terminated = after >= bytes.len() || matches!(bytes[after], b';' | b'}' | b'!');
        if !terminated {
            out.push_str(&css[i..cursor]);
            i = cursor;
            continue;
        }
        out.push_str("background-attachment: fixed");
        i = cursor + VALUE.len();
        changed = true;
    }
    changed.then_some(out)
}

fn copy_opaque_token(css: &str, at: usize, out: &mut String) -> Option<usize> {
    let bytes = css.as_bytes();
    let end = match bytes[at] {
        quote @ (b'"' | b'\'') => {
            let mut j = at + 1;
            let closed = loop {
                match bytes.get(j) {
                    None => break j,

                    Some(b'\n') => break j,
                    Some(92) => j += 2,
                    Some(&b) if b == quote => break j + 1,
                    Some(_) => j += 1,
                }
            };

            closed.min(bytes.len())
        },
        b'/' if bytes.get(at + 1) == Some(&b'*') => css[at + 2..]
            .find("*/")
            .map_or(bytes.len(), |k| at + 2 + k + 2),
        _ => return None,
    };
    out.push_str(&css[at..end]);
    Some(end)
}

fn ident_at(bytes: &[u8], at: usize, needle: &[u8]) -> bool {
    bytes.len() >= at + needle.len()
        && bytes[at..at + needle.len()].eq_ignore_ascii_case(needle)
        && !(at > 0 && is_name_continuation_byte(bytes[at - 1]))
        && bytes
            .get(at + needle.len())
            .is_none_or(|&b| !is_name_continuation_byte(b))
}

fn rewrite_declaration_value_idents(
    css: &str,
    property: &[u8],
    replacements: &[(&[u8], &str)],
) -> Option<String> {
    let bytes = css.as_bytes();
    let mut out = String::with_capacity(css.len());
    let mut i = 0usize;
    let mut changed = false;

    while i < bytes.len() {
        if let Some(next) = copy_opaque_token(css, i, &mut out) {
            i = next;
            continue;
        }
        if !ident_at(bytes, i, property) {
            let Some(ch) = css[i..].chars().next() else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        out.push_str(&css[i..i + property.len()]);
        i += property.len();

        let mut colon = i;
        while bytes.get(colon).is_some_and(u8::is_ascii_whitespace) {
            colon += 1;
        }
        if bytes.get(colon) != Some(&b':') {
            continue;
        }
        out.push_str(&css[i..=colon]);
        i = colon + 1;

        while i < bytes.len() && bytes[i] != b';' && bytes[i] != b'}' {
            if let Some(next) = copy_opaque_token(css, i, &mut out) {
                i = next;
                continue;
            }

            let is_function_argument = out.trim_end().ends_with('(');
            if !is_function_argument
                && let Some((needle, replacement)) = replacements
                    .iter()
                    .find(|(needle, _)| ident_at(bytes, i, needle))
            {
                out.push_str(replacement);
                i += needle.len();
                changed = true;
                continue;
            }
            let Some(ch) = css[i..].chars().next() else {
                break;
            };
            out.push(ch);
            i += ch.len_utf8();
        }
    }

    changed.then_some(out)
}

fn rewrite_css_identifier_tokens(
    value: &str,
    replacement_for: impl Fn(&str) -> Option<&'static str>,
    quoted_identifiers: bool,
) -> Option<String> {
    struct TokenReplacement {
        start_byte: usize,
        end_byte: usize,
        replacement: String,
    }

    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    let mut replacements = Vec::<TokenReplacement>::new();

    loop {
        let start = parser.position().byte_index();
        let token = match parser.next_including_whitespace_and_comments() {
            Ok(token) => token.clone(),
            Err(_) => break,
        };
        let end = parser.position().byte_index();

        match token {
            Token::Ident(name) => {
                if let Some(replacement) = replacement_for(name.as_ref()) {
                    replacements.push(TokenReplacement {
                        start_byte: start,
                        end_byte: end,
                        replacement: replacement.to_string(),
                    });
                }
            },
            Token::QuotedString(name) if quoted_identifiers => {
                if let Some(replacement) = replacement_for(name.as_ref()) {
                    let quote = value.as_bytes().get(start).copied().unwrap_or(b'"');
                    let quote = if matches!(quote, b'\'' | b'"') {
                        quote
                    } else {
                        b'"'
                    };
                    replacements.push(TokenReplacement {
                        start_byte: start,
                        end_byte: end,
                        replacement: format!(
                            "{}{replacement}{}",
                            char::from(quote),
                            char::from(quote)
                        ),
                    });
                }
            },
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                let _ = parser.parse_nested_block(|inner| -> Result<(), ParseError<'_, ()>> {
                    while inner.next_including_whitespace_and_comments().is_ok() {}
                    Ok(())
                });
            },
            _ => {},
        }
    }

    if replacements.is_empty() {
        return None;
    }

    let mut out = String::with_capacity(value.len());
    let mut cursor = 0usize;
    for replacement in replacements {
        out.push_str(&value[cursor..replacement.start_byte]);
        out.push_str(&replacement.replacement);
        cursor = replacement.end_byte;
    }
    out.push_str(&value[cursor..]);
    Some(out)
}

fn rewrite_pdfreactor_font_family_value(value: &str) -> Option<String> {
    fn native_alias(name: &str) -> Option<&'static str> {
        if name.eq_ignore_ascii_case("-ro-color-emoji") {
            Some("-bd-color-emoji")
        } else if name.eq_ignore_ascii_case("-ro-emoji") {
            Some("-bd-emoji")
        } else {
            None
        }
    }

    rewrite_css_identifier_tokens(value, native_alias, true)
}

fn rewrite_position_visibility_value(value: &str) -> Option<String> {
    fn stylo_backing_keyword(name: &str) -> Option<&'static str> {
        if name.eq_ignore_ascii_case("anchor-valid") {
            Some("anchors-valid")
        } else if name.eq_ignore_ascii_case("anchor-visible") {
            Some("anchors-visible")
        } else {
            None
        }
    }

    rewrite_css_identifier_tokens(value, stylo_backing_keyword, false)
}

fn rewrite_pdfreactor_string_set_self_keyword(css: &str, compat: CompatMode) -> Option<String> {
    if !matches!(compat, CompatMode::PdfReactor) {
        return None;
    }

    if !mentions_ident(css, "string-set")
        || !(mentions_ident(css, "self")
            || mentions_ident(css, "before")
            || mentions_ident(css, "after")
            || mentions_ident(css, "first-letter"))
    {
        return None;
    }

    rewrite_declaration_value_idents(
        css,
        b"string-set",
        &[
            (b"self", "content(text)"),
            (b"first-letter", "content(first-letter)"),
            (b"before", "content(before)"),
            (b"after", "content(after)"),
        ],
    )
}

fn mentions_ident(css: &str, name: &str) -> bool {
    let bytes = css.as_bytes();
    let needle = name.as_bytes();
    (0..bytes.len()).any(|at| ident_at(bytes, at, needle))
}

#[inline]
fn is_at_keyword_boundary(next: Option<u8>) -> bool {
    match next {
        None => true,
        Some(b) => !is_name_continuation_byte(b),
    }
}

#[inline]
fn is_name_continuation_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b >= 0x80
}

#[derive(Debug, Clone)]
struct Injection {
    before_close_byte: usize,
    native_declaration: String,
}

#[derive(Debug, Clone)]
struct ValueReplacement {
    start_byte: usize,
    end_byte: usize,
    replacement: String,
}

#[derive(Debug, Clone)]
struct TailDescriptor {
    native_declaration: String,
}

struct TranslateState {
    compat: CompatMode,
    injections: Vec<Injection>,
    value_replacements: Vec<ValueReplacement>,
    tail_descriptors: Vec<TailDescriptor>,
    warnings: Vec<CompatWarning>,

    at_page_depth: u32,

    at_font_face_depth: u32,
}

fn walk_top_level<'i>(parser: &mut Parser<'i, '_>, state: &mut TranslateState, full_css: &str) {
    loop {
        parser.skip_whitespace();
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::AtKeyword(ref name) => {
                    consume_at_rule(parser, state, full_css, name.as_ref());
                },
                Token::WhiteSpace(_) | Token::Comment(_) => {},
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return;
                },
                _ => {
                    parser.reset(&snapshot);
                    if walk_qualified_rule(parser, state, full_css).is_err() {
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
    state: &mut TranslateState,
    full_css: &str,
    at_keyword: &str,
) {
    let is_prince_pdf = at_keyword.eq_ignore_ascii_case("prince-pdf");

    let is_ro_preferences = at_keyword.eq_ignore_ascii_case("-ro-preferences");

    let is_page_at_rule = at_keyword.eq_ignore_ascii_case("page");
    let is_font_face_at_rule = at_keyword.eq_ignore_ascii_case("font-face");

    let body_is_declaration_block = matches!(
        at_keyword.to_ascii_lowercase().as_str(),
        "page"
            | "font-face"
            | "top-left-corner"
            | "top-left"
            | "top-center"
            | "top-right"
            | "top-right-corner"
            | "left-top"
            | "left-middle"
            | "left-bottom"
            | "right-top"
            | "right-middle"
            | "right-bottom"
            | "bottom-left-corner"
            | "bottom-left"
            | "bottom-center"
            | "bottom-right"
            | "bottom-right-corner"
            | "footnote"
            | "sidenote"
            | "-bd-sidenote"
            | "-bd-colour"
    );

    let at_rule_location = parser.current_source_location();
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Semicolon => return,
                Token::CurlyBracketBlock => {
                    if is_prince_pdf && state.compat == CompatMode::Prince {
                        let _ =
                            parser.parse_nested_block(|inner| -> Result<(), ParseError<'i, ()>> {
                                scan_prince_pdf_descriptors(inner, state, full_css);
                                Ok(())
                            });
                    } else if is_ro_preferences && state.compat == CompatMode::PdfReactor {
                        let _ =
                            parser.parse_nested_block(|inner| -> Result<(), ParseError<'i, ()>> {
                                scan_ro_preferences_descriptors(inner, state, full_css);
                                Ok(())
                            });
                    } else if is_prince_pdf || is_ro_preferences {
                        let _ =
                            parser.parse_nested_block(|inner| -> Result<(), ParseError<'i, ()>> {
                                while inner.next_including_whitespace_and_comments().is_ok() {}
                                Ok(())
                            });
                        let required = if is_prince_pdf {
                            CompatMode::Prince
                        } else {
                            CompatMode::PdfReactor
                        };
                        state.warnings.push(CompatWarning {
                            kind: CompatWarningKind::UnknownVendor { required },
                            property: format!("@{at_keyword}"),
                            line: at_rule_location.line + 1,
                            column: at_rule_location.column,
                        });
                    } else if body_is_declaration_block {
                        if is_page_at_rule {
                            state.at_page_depth += 1;
                        }
                        if is_font_face_at_rule {
                            state.at_font_face_depth += 1;
                        }
                        let _ =
                            parser.parse_nested_block(|inner| -> Result<(), ParseError<'i, ()>> {
                                scan_block(inner, state, full_css);
                                Ok(())
                            });
                        if is_font_face_at_rule {
                            state.at_font_face_depth -= 1;
                        }
                        if is_page_at_rule {
                            state.at_page_depth -= 1;
                        }
                    } else {
                        let _ =
                            parser.parse_nested_block(|inner| -> Result<(), ParseError<'i, ()>> {
                                walk_top_level(inner, state, full_css);
                                Ok(())
                            });
                    }
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
    state: &mut TranslateState,
    full_css: &str,
) -> Result<(), ()> {
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::CurlyBracketBlock => {
                    let body_start = parser.position().byte_index();
                    let _ = parser.parse_nested_block(|inner| -> Result<(), ParseError<'i, ()>> {
                        scan_block(inner, state, full_css);
                        Ok(())
                    });
                    let after_close = parser.position().byte_index();
                    let _ = (body_start, after_close);
                    return Ok(());
                },
                Token::Semicolon => return Ok(()),
                _ => continue,
            },
            Err(_) => return Err(()),
        }
    }
}

fn scan_block<'i>(parser: &mut Parser<'i, '_>, state: &mut TranslateState, full_css: &str) {
    loop {
        parser.skip_whitespace();
        let decl_start = parser.current_source_location();
        let decl_start_byte = parser.position().byte_index();
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Ident(ident) => {
                    let name = ident.as_ref().to_string();
                    let name_end_byte = parser.position().byte_index();
                    match parser.next() {
                        Ok(Token::Colon) => {},
                        _ => {
                            super::metadata::skip_to_decl_end(parser);
                            continue;
                        },
                    }
                    let value_start = parser.position().byte_index();
                    let value_end = skip_to_decl_end_byte(parser);
                    let value = &full_css[value_start..value_end];
                    let raw_value = value.trim();

                    if state.compat == CompatMode::Prince
                        && state.at_page_depth > 0
                        && (name.eq_ignore_ascii_case("bleed")
                            || name.eq_ignore_ascii_case("prince-bleed")
                            || name.eq_ignore_ascii_case("-prince-bleed"))
                    {
                        state.value_replacements.push(ValueReplacement {
                            start_byte: decl_start_byte,
                            end_byte: name_end_byte,
                            replacement: "-bd-prince-bleed".to_string(),
                        });
                    }

                    if state.compat == CompatMode::PdfReactor
                        && state.at_font_face_depth == 0
                        && name.eq_ignore_ascii_case("font-family")
                        && let Some(replacement) = rewrite_pdfreactor_font_family_value(value)
                    {
                        state.value_replacements.push(ValueReplacement {
                            start_byte: value_start,
                            end_byte: value_end,
                            replacement,
                        });
                    }

                    if name.eq_ignore_ascii_case("position-visibility")
                        && let Some(replacement) = rewrite_position_visibility_value(value)
                    {
                        state.value_replacements.push(ValueReplacement {
                            start_byte: value_start,
                            end_byte: value_end,
                            replacement,
                        });
                    }

                    handle_property(
                        state,
                        full_css,
                        &name,
                        raw_value,
                        decl_start.line + 1,
                        decl_start.column,
                        BlockKind::StyleRule,
                    );
                },
                Token::AtKeyword(ref at_name) => {
                    consume_at_rule(parser, state, full_css, at_name.as_ref());
                },
                Token::CurlyBracketBlock => {
                    let _ = parser.parse_nested_block(|inner| -> Result<(), ParseError<'i, ()>> {
                        scan_block(inner, state, full_css);
                        Ok(())
                    });
                },
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return;
                },
                Token::Semicolon | Token::WhiteSpace(_) | Token::Comment(_) => {},
                _ => {
                    super::metadata::skip_to_decl_end(parser);
                },
            },
            Err(_) => return,
        }
    }
}

fn scan_prince_pdf_descriptors<'i>(
    parser: &mut Parser<'i, '_>,
    state: &mut TranslateState,
    full_css: &str,
) {
    loop {
        parser.skip_whitespace();
        let decl_start = parser.current_source_location();
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Ident(ident) => {
                    let name = ident.as_ref().to_string();
                    match parser.next() {
                        Ok(Token::Colon) => {},
                        _ => {
                            super::metadata::skip_to_decl_end(parser);
                            continue;
                        },
                    }
                    let value_start = parser.position().byte_index();
                    let value_end = skip_to_decl_end_byte(parser);
                    let raw_value = full_css[value_start..value_end].trim();

                    handle_property(
                        state,
                        full_css,
                        &name,
                        raw_value,
                        decl_start.line + 1,
                        decl_start.column,
                        BlockKind::PrincePdfAtRule,
                    );
                },
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return;
                },
                Token::Semicolon | Token::WhiteSpace(_) | Token::Comment(_) => {},
                _ => {
                    super::metadata::skip_to_decl_end(parser);
                },
            },
            Err(_) => return,
        }
    }
}

fn scan_ro_preferences_descriptors<'i>(
    parser: &mut Parser<'i, '_>,
    state: &mut TranslateState,
    full_css: &str,
) {
    loop {
        parser.skip_whitespace();
        let decl_start = parser.current_source_location();
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Ident(ident) => {
                    let name = ident.as_ref().to_string();
                    match parser.next() {
                        Ok(Token::Colon) => {},
                        _ => {
                            super::metadata::skip_to_decl_end(parser);
                            continue;
                        },
                    }
                    let value_start = parser.position().byte_index();
                    let value_end = skip_to_decl_end_byte(parser);
                    let raw_value = full_css[value_start..value_end].trim();

                    let lower = ascii_lower(&name);
                    let outcome = translate_ro_preferences_descriptor(&lower, raw_value);
                    let line = decl_start.line + 1;
                    let column = decl_start.column;
                    match outcome {
                        Translated::Native {
                            native_property,
                            native_value,
                        } => {
                            let native_decl = format!("{native_property}: {native_value};");
                            state.tail_descriptors.push(TailDescriptor {
                                native_declaration: native_decl,
                            });
                        },
                        Translated::Natives(pairs) => {
                            for (native_property, native_value) in pairs {
                                let native_decl = format!("{native_property}: {native_value};");
                                state.tail_descriptors.push(TailDescriptor {
                                    native_declaration: native_decl,
                                });
                            }
                        },
                        Translated::Satisfied => {},
                        Translated::ValueDropped => {
                            state.warnings.push(CompatWarning {
                                kind: CompatWarningKind::ValueDropped {
                                    value: raw_value.to_string(),
                                },
                                property: name.clone(),
                                line,
                                column,
                            });
                        },
                        Translated::PropertyDropped => {
                            state.warnings.push(CompatWarning {
                                kind: CompatWarningKind::PropertyDropped,
                                property: name.clone(),
                                line,
                                column,
                            });
                        },
                    }
                },
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return;
                },
                Token::Semicolon | Token::WhiteSpace(_) | Token::Comment(_) => {},
                _ => {
                    super::metadata::skip_to_decl_end(parser);
                },
            },
            Err(_) => return,
        }
    }
}

fn translate_ro_preferences_descriptor(lower_name: &str, raw_value: &str) -> Translated {
    match lower_name {
        "page-layout" => {
            let v = collapse_whitespace_ascii_lower(raw_value);
            let mapped = match v.as_str() {
                "auto" => Some("auto"),
                "1 column" | "one column" | "1 columns" | "one columns" => Some("one-column"),

                "2 column" | "two column" | "2 columns" | "two columns" => Some("two-column-left"),
                "1 page" | "one page" | "1 pages" | "one pages" => Some("single-page"),
                "2 page" | "two page" | "2 pages" | "two pages" => Some("two-page-right"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-viewer-page-layout",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "first-page-side" | "first-page-side-view" => {
            let v = ascii_lower(raw_value.trim());
            let valid = matches!(v.as_str(), "auto" | "left" | "right" | "verso" | "recto");
            if !valid {
                return Translated::ValueDropped;
            }
            let native_property = if lower_name == "first-page-side" {
                "-bd-first-page-side"
            } else {
                "-bd-first-page-side-view"
            };
            Translated::Native {
                native_property,
                native_value: v,
            }
        },

        "initial-page" | "pages-counter-offset" => {
            let v = raw_value.trim();

            if v.is_empty() {
                return Translated::ValueDropped;
            }
            let body = v
                .strip_prefix('+')
                .or_else(|| v.strip_prefix('-'))
                .unwrap_or(v);
            if !body.bytes().all(|b| b.is_ascii_digit()) {
                return Translated::ValueDropped;
            }
            let native_property = if lower_name == "initial-page" {
                "-bd-initial-page"
            } else {
                "-bd-pages-counter-offset"
            };
            Translated::Native {
                native_property,
                native_value: v.to_string(),
            }
        },

        "initial-zoom" => {
            let v = ascii_lower(raw_value.trim());
            let keyword_ok = matches!(
                v.as_str(),
                "auto"
                    | "fit-page"
                    | "fit-page-height"
                    | "fit-page-width"
                    | "fit-content"
                    | "fit-content-height"
                    | "fit-content-width"
            );

            let percentage_ok = if let Some(num) = v.strip_suffix('%') {
                !num.is_empty() && num.parse::<f32>().is_ok()
            } else {
                false
            };
            if !keyword_ok && !percentage_ok {
                return Translated::ValueDropped;
            }
            Translated::Native {
                native_property: "-bd-initial-zoom",
                native_value: v,
            }
        },

        "pdf-shape-optimization" => {
            let v = ascii_lower(raw_value.trim());
            let mapped = match v.as_str() {
                "visual" => Some("auto"),
                "none" => Some("none"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-shape-optimization",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "number-of-copies" => {
            let v = raw_value.trim();
            if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
                return Translated::ValueDropped;
            }
            Translated::Native {
                native_property: "-bd-pdf-viewer-num-copies",
                native_value: v.to_string(),
            }
        },

        "print-page-range" => {
            let v = collapse_whitespace_ascii_lower(raw_value);
            if v.is_empty() {
                return Translated::ValueDropped;
            }

            let ok = v
                .split(' ')
                .all(|tok| !tok.is_empty() && tok.bytes().all(|b| b.is_ascii_digit()));
            if !ok {
                return Translated::ValueDropped;
            }
            Translated::Native {
                native_property: "-bd-pdf-viewer-print-page-range",
                native_value: v,
            }
        },

        "view-area" | "view-clip" | "print-area" | "print-clip" => {
            let v = ascii_lower(raw_value.trim());
            let native_value = match v.as_str() {
                "auto" => Some("auto"),
                "mediabox" | "media-box" => Some("media-box"),
                "cropbox" | "crop-box" => Some("crop-box"),
                "bleedbox" | "bleed-box" => Some("bleed-box"),
                "trimbox" | "trim-box" => Some("trim-box"),
                "artbox" | "art-box" => Some("art-box"),
                _ => None,
            };
            match native_value {
                Some(native) => {
                    let native_property = match lower_name {
                        "view-area" => "-bd-pdf-viewer-view-area",
                        "view-clip" => "-bd-pdf-viewer-view-clip",
                        "print-area" => "-bd-pdf-viewer-print-area",
                        "print-clip" => "-bd-pdf-viewer-print-clip",
                        _ => unreachable!("matched on these four arms above"),
                    };
                    Translated::Native {
                        native_property,
                        native_value: native.to_string(),
                    }
                },
                None => Translated::ValueDropped,
            }
        },

        "pdf-script-action" => {
            let v = raw_value.trim();
            if v.is_empty() {
                return Translated::Native {
                    native_property: "-bd-pdf-open-action-script",
                    native_value: "none".to_string(),
                };
            }

            Translated::Native {
                native_property: "-bd-pdf-open-action-script",
                native_value: v.to_string(),
            }
        },
        _ => Translated::PropertyDropped,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockKind {
    StyleRule,
    PrincePdfAtRule,
}

fn pdfreactor_sidenote_float_native_value(raw_value: &str) -> Option<&'static str> {
    let mut input = ParserInput::new(raw_value);
    let mut parser = Parser::new(&mut input);
    parser.skip_whitespace();

    let Token::Function(function_name) = parser.next().ok()?.clone() else {
        return None;
    };
    if !function_name.eq_ignore_ascii_case("-ro-sidenote") {
        return None;
    }

    let native = parser
        .parse_nested_block(|inner| -> Result<&'static str, ParseError<'_, ()>> {
            inner.skip_whitespace();
            let side = inner.expect_ident_cloned()?;
            let native = if side.eq_ignore_ascii_case("left") {
                "leftnote"
            } else if side.eq_ignore_ascii_case("right") {
                "rightnote"
            } else if side.eq_ignore_ascii_case("inside") {
                "insidenote"
            } else if side.eq_ignore_ascii_case("outside") {
                "outsidenote"
            } else {
                return Err(inner.new_custom_error(()));
            };

            inner.skip_whitespace();
            if !inner.is_exhausted() {
                return Err(inner.new_custom_error(()));
            }
            Ok(native)
        })
        .ok()?;

    parser.skip_whitespace();
    parser.is_exhausted().then_some(native)
}

fn handle_property(
    state: &mut TranslateState,
    full_css: &str,
    property: &str,
    raw_value: &str,
    line: u32,
    column: u32,
    block: BlockKind,
) {
    let lower = ascii_lower(property);

    if matches!(state.compat, CompatMode::Prince)
        && lower == "float"
        && matches!(block, BlockKind::StyleRule)
        && let Some(shorthand) = PrinceFloatShorthand::parse(raw_value)
        && shorthand.bare_spelling_needs_compat()
    {
        let outcome = shorthand.expand().map_or(
            Translated::ValueDropped,
            PrinceFloatLonghands::into_translated,
        );
        apply_translation_outcome(
            state, full_css, property, raw_value, line, column, block, outcome,
        );
        return;
    }

    if matches!(state.compat, CompatMode::PdfReactor)
        && lower == "float"
        && matches!(block, BlockKind::StyleRule)
        && let Some(native_value) = pdfreactor_sidenote_float_native_value(raw_value)
    {
        inject_native_declaration(
            state,
            block,
            full_css,
            line,
            column,
            format!("-bd-float-reference-sidenote: {native_value};"),
        );
        return;
    }

    if matches!(state.compat, CompatMode::PdfReactor)
        && lower == "-ro-marks"
        && state.at_page_depth > 0
        && matches!(block, BlockKind::StyleRule)
    {
        let trimmed = raw_value.trim();
        let tokens: Vec<&str> = trimmed.split_ascii_whitespace().collect();
        let has_bleed = tokens.iter().any(|t| t.eq_ignore_ascii_case("bleed"));
        if has_bleed {
            let kept: Vec<String> = tokens
                .iter()
                .filter(|t| !t.eq_ignore_ascii_case("bleed"))
                .map(|t| t.to_ascii_lowercase())
                .collect();

            let marks_value = if kept.is_empty() {
                "none".to_string()
            } else {
                kept.join(" ")
            };
            if let Some(close_byte) = find_containing_close_brace(full_css, line, column) {
                state.injections.push(Injection {
                    before_close_byte: close_byte,
                    native_declaration: format!("marks: {marks_value};"),
                });
                state.injections.push(Injection {
                    before_close_byte: close_byte,
                    native_declaration: "bleed: 6pt;".to_string(),
                });
            }
            return;
        }
    }

    if matches!(state.compat, CompatMode::Prince)
        && lower == "text-justify"
        && raw_value.trim().eq_ignore_ascii_case("prince-cjk")
    {
        if let BlockKind::StyleRule = block
            && let Some(close_byte) = find_containing_close_brace(full_css, line, column)
        {
            state.injections.push(Injection {
                before_close_byte: close_byte,
                native_declaration: "text-justify: -bd-cjk;".to_string(),
            });
        }
        return;
    }

    if matches!(state.compat, CompatMode::PdfReactor) && lower == "shape-outside" {
        if raw_value.trim().eq_ignore_ascii_case("-ro-self") {
            if let BlockKind::StyleRule = block
                && let Some(close_byte) = find_containing_close_brace(full_css, line, column)
            {
                state.injections.push(Injection {
                    before_close_byte: close_byte,
                    native_declaration: "shape-outside: -bd-self;".to_string(),
                });
            }
            return;
        }

        let collapsed: String = raw_value.chars().filter(|c| !c.is_whitespace()).collect();
        if collapsed.eq_ignore_ascii_case("attr(srcurl)")
            || collapsed.eq_ignore_ascii_case("attr(srctype(<url>))")
        {
            if let BlockKind::StyleRule = block
                && let Some(close_byte) = find_containing_close_brace(full_css, line, column)
            {
                state.injections.push(Injection {
                    before_close_byte: close_byte,
                    native_declaration: "shape-outside: var(--bd-self-src-image);".to_string(),
                });
            }
            return;
        }
    }

    if let Some(required) = required_compat_for(&lower) {
        if state.compat != required {
            if is_known_foreign(&lower) {
                state.warnings.push(CompatWarning {
                    kind: CompatWarningKind::UnknownVendor { required },
                    property: property.to_string(),
                    line,
                    column,
                });
            }
            return;
        }

        apply_translation_outcome(
            state,
            full_css,
            property,
            raw_value,
            line,
            column,
            block,
            translate_property(&lower, raw_value),
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_translation_outcome(
    state: &mut TranslateState,
    full_css: &str,
    property: &str,
    raw_value: &str,
    line: u32,
    column: u32,
    block: BlockKind,
    outcome: Translated,
) {
    match outcome {
        Translated::Native {
            native_property,
            native_value,
        } => {
            let native_decl = format!("{native_property}: {native_value};");
            inject_native_declaration(state, block, full_css, line, column, native_decl);
        },
        Translated::Natives(pairs) => {
            for (native_property, native_value) in pairs {
                let native_decl = format!("{native_property}: {native_value};");
                inject_native_declaration(state, block, full_css, line, column, native_decl);
            }
        },
        Translated::Satisfied => {},
        Translated::ValueDropped => {
            state.warnings.push(CompatWarning {
                kind: CompatWarningKind::ValueDropped {
                    value: raw_value.to_string(),
                },
                property: property.to_string(),
                line,
                column,
            });
        },
        Translated::PropertyDropped => {
            state.warnings.push(CompatWarning {
                kind: CompatWarningKind::PropertyDropped,
                property: property.to_string(),
                line,
                column,
            });
        },
    }
}

fn translate_tab_size_extension(raw_value: &str) -> Translated {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Translated::ValueDropped;
    }

    if trimmed.eq_ignore_ascii_case("nearest") {
        return Translated::Native {
            native_property: "-bd-tab-snap",
            native_value: "nearest".to_string(),
        };
    }

    let tokens: Vec<&str> = trimmed.split_ascii_whitespace().collect();
    match tokens.as_slice() {
        [single] => Translated::Native {
            native_property: "tab-size",
            native_value: (*single).to_string(),
        },

        many if many.len() >= 2 => {
            let stops = many
                .iter()
                .map(|len| format!("{len} left"))
                .collect::<Vec<_>>()
                .join(", ");
            Translated::Natives(vec![
                ("tab-size", many[0].to_string()),
                ("-bd-tab-stops", stops),
            ])
        },
        _ => Translated::ValueDropped,
    }
}

fn inject_native_declaration(
    state: &mut TranslateState,
    block: BlockKind,
    full_css: &str,
    line: u32,
    column: u32,
    native_decl: String,
) {
    match block {
        BlockKind::StyleRule => {
            let containing = find_containing_close_brace(full_css, line, column);
            if let Some(close_byte) = containing {
                state.injections.push(Injection {
                    before_close_byte: close_byte,
                    native_declaration: native_decl,
                });
            }
        },
        BlockKind::PrincePdfAtRule => {
            state.tail_descriptors.push(TailDescriptor {
                native_declaration: native_decl,
            });
        },
    }
}

enum Translated {
    Native {
        native_property: &'static str,
        native_value: String,
    },

    Natives(Vec<(&'static str, String)>),

    Satisfied,

    ValueDropped,

    PropertyDropped,
}

fn translate_prince_background_image_resolution(raw_value: &str) -> Translated {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return Translated::ValueDropped;
    }

    let mut parts = trimmed.splitn(2, ',');
    let primary = parts.next().unwrap_or("").trim();
    let fallback = parts.next().map(str::trim);
    let Some(primary_native) = map_prince_resolution_token(primary) else {
        return Translated::ValueDropped;
    };
    match fallback {
        None => Translated::Native {
            native_property: "-bd-image-resolution",
            native_value: primary_native,
        },
        Some(fb) => {
            if !primary.eq_ignore_ascii_case("auto") {
                return Translated::ValueDropped;
            }

            let fb_lower = fb.to_ascii_lowercase();
            let fb_resolution = if fb_lower == "auto" {
                None
            } else {
                match map_prince_resolution_token(fb) {
                    Some(value) => Some(value),
                    None => return Translated::ValueDropped,
                }
            };
            let native_value = match fb_resolution {
                None => primary_native,
                Some(fallback_native) => {
                    format!("{primary_native} {fallback_native}")
                },
            };
            Translated::Native {
                native_property: "-bd-image-resolution",
                native_value,
            }
        },
    }
}

fn map_prince_resolution_token(token: &str) -> Option<String> {
    let lower = token.to_ascii_lowercase();
    match lower.as_str() {
        "auto" => Some("from-image".to_string()),
        "normal" => Some("96dpi".to_string()),
        _ => {
            let collapsed: String = lower.split_whitespace().collect();
            let has_resolution_unit = collapsed.ends_with("dpi")
                || collapsed.ends_with("dpcm")
                || collapsed.ends_with("dppx")
                || (collapsed.ends_with('x') && !collapsed.ends_with("dppx"));

            let has_digit = collapsed.chars().any(|c| c.is_ascii_digit());
            if has_resolution_unit && has_digit {
                Some(token.to_string())
            } else {
                None
            }
        },
    }
}

fn translate_prince_event_scripts(raw_value: &str) -> Option<String> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.eq_ignore_ascii_case("none") {
        return Some("none".to_string());
    }

    let mut input = ParserInput::new(trimmed);
    let mut parser = Parser::new(&mut input);

    let mut out_specs: Vec<String> = Vec::new();
    loop {
        parser.skip_whitespace();
        let event_ident = match parser.next() {
            Ok(Token::Ident(ident)) => ident.as_ref().to_ascii_lowercase(),

            _ => return None,
        };
        let event_short = match event_ident.as_str() {
            "will-close" => "wc",
            "will-save" => "ws",
            "did-save" => "ds",
            "will-print" => "wp",
            "did-print" => "dp",
            _ => return None,
        };

        let script = match parser.next() {
            Ok(Token::QuotedString(s)) => s.as_ref().to_string(),
            _ => return None,
        };

        let mut escaped = String::with_capacity(script.len() + 2);
        escaped.push('"');
        for ch in script.chars() {
            if ch == char::from(92) {
                escaped.push(char::from(92));
                escaped.push(char::from(92));
            } else if ch == char::from(34) {
                escaped.push(char::from(92));
                escaped.push(char::from(34));
            } else if ch == char::from(10) {
                escaped.push(char::from(92));
                escaped.push_str("A ");
            } else if ch == char::from(13) {
                escaped.push(char::from(92));
                escaped.push_str("D ");
            } else if ch == char::from(12) {
                escaped.push(char::from(92));
                escaped.push_str("C ");
            } else {
                escaped.push(ch);
            }
        }
        escaped.push('"');

        out_specs.push(format!("{event_short}({escaped})"));

        parser.skip_whitespace();
        match parser.next() {
            Ok(Token::Comma) => continue,
            Err(_) => break,
            _ => return None,
        }
    }

    if out_specs.is_empty() {
        return None;
    }
    Some(out_specs.join(", "))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrinceFloatReferenceComponent {
    Column,
    Page,
    Unsupported,
}

impl PrinceFloatReferenceComponent {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "column" => Some(Self::Column),
            "page" => Some(Self::Page),
            "sidenote" | "leftnote" | "rightnote" | "insidenote" | "outsidenote" | "wide"
            | "wide-left" | "wide-right" | "wide-inside" | "wide-outside" => {
                Some(Self::Unsupported)
            },
            _ => None,
        }
    }

    const fn native(self) -> Option<PrinceNativeFloatReference> {
        match self {
            Self::Column => Some(PrinceNativeFloatReference::Column),
            Self::Page => Some(PrinceNativeFloatReference::Page),
            Self::Unsupported => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrinceFloatPlacementComponent {
    None,
    Left,
    Right,
    Inside,
    Outside,
    Top,
    Bottom,
    TopBottom,
    Footnote,
    Unsupported,
}

impl PrinceFloatPlacementComponent {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "none" => Some(Self::None),
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "inside" => Some(Self::Inside),
            "outside" => Some(Self::Outside),
            "top" => Some(Self::Top),
            "bottom" => Some(Self::Bottom),
            "top-bottom" => Some(Self::TopBottom),
            "footnote" => Some(Self::Footnote),
            "snap" | "align-top" | "align-bottom" | "inline-footnote" => Some(Self::Unsupported),
            _ => None,
        }
    }

    const fn native(self) -> Option<PrinceNativeFloatPlacement> {
        match self {
            Self::None => Some(PrinceNativeFloatPlacement::None),
            Self::Left => Some(PrinceNativeFloatPlacement::Left),
            Self::Right => Some(PrinceNativeFloatPlacement::Right),
            Self::Inside => Some(PrinceNativeFloatPlacement::Inside),
            Self::Outside => Some(PrinceNativeFloatPlacement::Outside),
            Self::Top => Some(PrinceNativeFloatPlacement::Top),
            Self::Bottom => Some(PrinceNativeFloatPlacement::Bottom),
            Self::TopBottom => Some(PrinceNativeFloatPlacement::TopUnlessRoom),
            Self::Footnote => Some(PrinceNativeFloatPlacement::Footnote),
            Self::Unsupported => None,
        }
    }

    const fn needs_prince_compat(self) -> bool {
        matches!(self, Self::TopBottom | Self::Unsupported)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrinceFloatModifierComponent {
    Normal,
    UnlessFit,
}

impl PrinceFloatModifierComponent {
    fn parse(token: &str) -> Option<Self> {
        match token {
            "normal" => Some(Self::Normal),
            "unless-fit" => Some(Self::UnlessFit),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrinceFloatShorthand {
    reference: Option<PrinceFloatReferenceComponent>,
    placement: Option<PrinceFloatPlacementComponent>,
    modifier: Option<PrinceFloatModifierComponent>,
}

impl PrinceFloatShorthand {
    fn parse(raw_value: &str) -> Option<Self> {
        let trimmed = raw_value.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut parsed = Self {
            reference: None,
            placement: None,
            modifier: None,
        };
        for token in trimmed.split_ascii_whitespace() {
            let lower = token.to_ascii_lowercase();
            if let Some(reference) = PrinceFloatReferenceComponent::parse(&lower) {
                if parsed.reference.replace(reference).is_some() {
                    return None;
                }
            } else if let Some(placement) = PrinceFloatPlacementComponent::parse(&lower) {
                if parsed.placement.replace(placement).is_some() {
                    return None;
                }
            } else if let Some(modifier) = PrinceFloatModifierComponent::parse(&lower) {
                if parsed.modifier.replace(modifier).is_some() {
                    return None;
                }
            } else {
                return None;
            }
        }
        Some(parsed)
    }

    const fn bare_spelling_needs_compat(self) -> bool {
        self.reference.is_some()
            || self.modifier.is_some()
            || match self.placement {
                Some(placement) => placement.needs_prince_compat(),
                None => false,
            }
    }

    fn expand(self) -> Option<PrinceFloatLonghands> {
        if self.modifier.is_some() {
            return None;
        }
        let reference = match self.reference {
            Some(reference) => Some(reference.native()?),
            None => None,
        };
        let mut placement = match self.placement {
            Some(placement) => Some(placement.native()?),
            None => None,
        };
        if reference.is_some() && placement.is_none() {
            placement = Some(PrinceNativeFloatPlacement::TopUnlessRoom);
        }
        (reference.is_some() || placement.is_some()).then_some(PrinceFloatLonghands {
            reference,
            placement,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrinceNativeFloatReference {
    Column,
    Page,
}

impl PrinceNativeFloatReference {
    const fn css(self) -> &'static str {
        match self {
            Self::Column => "column",
            Self::Page => "page",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrinceNativeFloatPlacement {
    None,
    Left,
    Right,
    Inside,
    Outside,
    Top,
    Bottom,
    TopUnlessRoom,
    Footnote,
}

impl PrinceNativeFloatPlacement {
    const fn css(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Left => "left",
            Self::Right => "right",
            Self::Inside => "inside",
            Self::Outside => "outside",
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::TopUnlessRoom => "top-unless-room",
            Self::Footnote => "footnote",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PrinceFloatLonghands {
    reference: Option<PrinceNativeFloatReference>,
    placement: Option<PrinceNativeFloatPlacement>,
}

impl PrinceFloatLonghands {
    fn into_translated(self) -> Translated {
        match (self.reference, self.placement) {
            (None, None) => Translated::ValueDropped,
            (Some(reference), None) => Translated::Native {
                native_property: "float-reference",
                native_value: reference.css().to_string(),
            },
            (None, Some(placement)) => Translated::Native {
                native_property: "float",
                native_value: placement.css().to_string(),
            },
            (Some(reference), Some(placement)) => Translated::Natives(vec![
                ("float-reference", reference.css().to_string()),
                ("float", placement.css().to_string()),
            ]),
        }
    }
}

fn translate_prince_float_shorthand(raw_value: &str) -> Translated {
    PrinceFloatShorthand::parse(raw_value)
        .and_then(PrinceFloatShorthand::expand)
        .map_or(
            Translated::ValueDropped,
            PrinceFloatLonghands::into_translated,
        )
}

const PDFREACTOR_OVERSIZE_PAPER_SIZES_MM: [(&str, u32, u32); 26] = [
    ("a0", 841, 1189),
    ("a1", 594, 841),
    ("a2", 420, 594),
    ("a6", 105, 148),
    ("a7", 74, 105),
    ("a8", 52, 74),
    ("a9", 37, 52),
    ("a10", 26, 37),
    ("ra0", 860, 1220),
    ("ra1", 610, 860),
    ("ra2", 430, 610),
    ("ra3", 305, 430),
    ("ra4", 215, 305),
    ("ra5", 152, 215),
    ("ra6", 107, 152),
    ("ra7", 76, 107),
    ("ra8", 53, 76),
    ("sra0", 900, 1280),
    ("sra1", 640, 900),
    ("sra2", 450, 640),
    ("sra3", 320, 450),
    ("sra4", 225, 320),
    ("sra5", 160, 225),
    ("sra6", 112, 160),
    ("sra7", 80, 112),
    ("sra8", 56, 80),
];

fn resolve_pdfreactor_oversize_paper_name(value: &str) -> String {
    let mut paper: Option<(u32, u32)> = None;
    let mut landscape = false;
    for token in value.split_whitespace() {
        if token.eq_ignore_ascii_case("landscape") {
            landscape = true;
        } else if token.eq_ignore_ascii_case("portrait") {
        } else if let Some(&(_, width, height)) = PDFREACTOR_OVERSIZE_PAPER_SIZES_MM
            .iter()
            .find(|(name, _, _)| token.eq_ignore_ascii_case(name))
        {
            if paper.is_some() {
                return value.to_string();
            }
            paper = Some((width, height));
        } else {
            return value.to_string();
        }
    }
    let Some((width, height)) = paper else {
        return value.to_string();
    };
    let (width, height) = if landscape {
        (height, width)
    } else {
        (width, height)
    };
    format!("{width}mm {height}mm")
}

fn translate_property(lower_property: &str, raw_value: &str) -> Translated {
    match lower_property {
        "-ro-height" => Translated::Native {
            native_property: "height",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-block-size" => Translated::Native {
            native_property: "block-size",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-source" => Translated::Native {
            native_property: "-bd-source",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-source-page" => Translated::Native {
            native_property: "-bd-source-page",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-source-area" => Translated::Native {
            native_property: "-bd-source-area",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-conformance" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped: Option<&str> = match v.as_str() {
                "none" => Some("none"),
                "pdf/a-1a" => Some("a1a"),
                "pdf/a-1b" => Some("a1b"),
                "pdf/a-2a" => Some("a2a"),
                "pdf/a-2b" => Some("a2b"),
                "pdf/a-2u" => Some("a2u"),
                "pdf/a-3a" => Some("a3a"),
                "pdf/a-3b" => Some("a3b"),
                "pdf/a-3u" => Some("a3u"),
                "pdf/a-4" => Some("a4"),
                "pdf/a-4e" => Some("a4e"),
                "pdf/a-4f" => Some("a4f"),
                "pdf/ua-1" => Some("ua1"),
                "pdf/ua-2" => Some("ua2"),
                "pdf/x-1a:2001" | "pdf/x-1a-2001" => Some("pdf-x-1a-2001"),
                "pdf/x-1a:2003" | "pdf/x-1a-2003" => Some("pdf-x-1a-2003"),
                "pdf/x-3:2002" | "pdf/x-3-2002" => Some("pdf-x-3-2002"),
                "pdf/x-3:2003" | "pdf/x-3-2003" => Some("pdf-x-3-2003"),
                "pdf/x-4" => Some("pdf-x-4"),
                "pdf/x-4p" => Some("pdf-x-4p"),
                "pdf/x-6" => Some("pdf-x-6"),
                "pdf/x-6p" => Some("pdf-x-6p"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-conformance",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-ro-pdf-format" => {
            let v = raw_value.trim().to_ascii_lowercase();
            match v.as_str() {
                "none" | "pdf" => Translated::Native {
                    native_property: "-bd-pdf-format",
                    native_value: v,
                },
                _ => Translated::ValueDropped,
            }
        },

        "-prince-pdf-page-layout" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "auto" => Some("auto"),
                "single-page" => Some("single-page"),
                "one-column" => Some("one-column"),
                "two-column-left" => Some("two-column-left"),
                "two-column-right" => Some("two-column-right"),
                "two-page-left" => Some("two-page-left"),
                "two-page-right" => Some("two-page-right"),

                "two-page" | "two-column" => None,
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-viewer-page-layout",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },
        "-prince-pdf-page-mode" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "auto" => Some("auto"),
                "show-attachments" => Some("attachments"),
                "show-bookmarks" => Some("outlines"),
                "fullscreen" => Some("full-screen"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-viewer-page-mode",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-prince-pdf-display-doc-title" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "true" => Some("yes"),
                "false" => Some("no"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-viewer-display-doc-title",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-prince-pdf-duplex" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "auto" => Some("auto"),
                "simplex" => Some("simplex"),
                "duplex-flip-short-edge" => Some("duplex-flip-short-edge"),
                "duplex-flip-long-edge" => Some("duplex-flip-long-edge"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-viewer-duplex",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-prince-pdf-paper-tray" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "auto" => Some("auto"),
                "pick-tray-by-pdf-size" => Some("yes"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-viewer-pick-tray-by-pdf-size",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-prince-pdf-print-scaling" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "auto" => Some("auto"),
                "none" => Some("none"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-viewer-print-scaling",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-prince-pdf-annotation-type" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "none" => Some("none"),
                "text" => Some("note"),
                "highlight" => Some("highlight"),
                "underline" => Some("underline"),
                "wavy" => Some("squiggly"),
                "line-through" => Some("strikeout"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-comment",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },
        "-prince-pdf-annotation-color" => Translated::Native {
            native_property: "-bd-pdf-comment-colour",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-annotation-contents" => Translated::Native {
            native_property: "-bd-pdf-comment-contents",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-annotation-title" => Translated::Native {
            native_property: "-bd-pdf-comment-title",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-annotation-icon" => Translated::Native {
            native_property: "-bd-pdf-comment-icon",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-pdf-annotation-author" => Translated::Native {
            native_property: "-bd-pdf-comment-author",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-annotation-createdate" => Translated::Native {
            native_property: "-bd-pdf-comment-createdate",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-annotation-modifydate" => Translated::Native {
            native_property: "-bd-pdf-comment-modifydate",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-annotation-position" => Translated::Native {
            native_property: "-bd-pdf-comment-position",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-comment-style" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "note" => Some("note"),
                "highlight" => Some("highlight"),
                "underline" => Some("underline"),
                "strikeout" => Some("strikeout"),
                "squiggly" => Some("squiggly"),

                "invisible" => None,
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-comment",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-ro-comment-color" => Translated::Native {
            native_property: "-bd-pdf-comment-colour",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-comment-content" => {
            let v = raw_value.trim();
            if v.eq_ignore_ascii_case("none") {
                Translated::Native {
                    native_property: "-bd-pdf-comment-contents",
                    native_value: "auto".to_string(),
                }
            } else if v.contains("content(") {
                Translated::ValueDropped
            } else {
                Translated::Native {
                    native_property: "-bd-pdf-comment-contents",
                    native_value: v.to_string(),
                }
            }
        },

        "-ro-comment-title" => {
            let v = raw_value.trim();
            if v.eq_ignore_ascii_case("none") {
                Translated::Native {
                    native_property: "-bd-pdf-comment-title",
                    native_value: "auto".to_string(),
                }
            } else {
                Translated::Native {
                    native_property: "-bd-pdf-comment-title",
                    native_value: v.to_string(),
                }
            }
        },

        "-ro-comment-date" => Translated::Native {
            native_property: "-bd-pdf-comment-date",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-comment-dateformat" => Translated::Native {
            native_property: "-bd-pdf-comment-date-format",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-comment-author" => Translated::Native {
            native_property: "-bd-pdf-comment-author",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-comment-subject" => Translated::Native {
            native_property: "-bd-pdf-comment-subject",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-comment-statemodel" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "marked" => Some("marked"),
                "review" => Some("review"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-comment-state-model",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-ro-comment-position" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "auto" => Some("auto"),
                "page-left" | "page-right" => Some("margin"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-comment-position",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-ro-comment-state" => {
            let v = raw_value.trim().to_ascii_lowercase();
            match v.as_str() {
                "open" => Translated::Native {
                    native_property: "-bd-pdf-comment-open",
                    native_value: "open".to_string(),
                },
                "closed" => Translated::Native {
                    native_property: "-bd-pdf-comment-open",
                    native_value: "closed".to_string(),
                },
                "marked" | "unmarked" | "accepted" | "rejected" | "cancelled" | "completed"
                | "none" => Translated::Native {
                    native_property: "-bd-pdf-comment-state",
                    native_value: v,
                },
                _ => Translated::ValueDropped,
            }
        },

        "-ro-comment-start" | "-ro-comment-end" => Translated::PropertyDropped,

        "-prince-tooltip" => {
            let v = raw_value.trim();
            let lower = v.to_ascii_lowercase();
            if lower == "none" {
                Translated::Native {
                    native_property: "-bd-pdf-tooltip",
                    native_value: "none".to_string(),
                }
            } else if lower == "normal" {
                Translated::Native {
                    native_property: "-bd-pdf-tooltip",
                    native_value: "auto".to_string(),
                }
            } else if lower == "transparent" || v.contains('(') {
                Translated::ValueDropped
            } else {
                Translated::Native {
                    native_property: "-bd-pdf-tooltip",
                    native_value: v.to_string(),
                }
            }
        },

        "-ro-pdf-tag-type" => match map_ro_pdf_tag_type(raw_value.trim()) {
            Some(native) => Translated::Native {
                native_property: "-bd-pdf-tag",
                native_value: native,
            },
            None => Translated::ValueDropped,
        },
        "-prince-pdf-tag-type" => match map_prince_pdf_tag_type(raw_value.trim()) {
            Some(native) => Translated::Native {
                native_property: "-bd-pdf-tag",
                native_value: native,
            },
            None => Translated::ValueDropped,
        },

        "-prince-pdf-profile" => match map_prince_pdf_profile(raw_value.trim()) {
            Some(native) => Translated::Native {
                native_property: "-bd-pdf-conformance",
                native_value: native,
            },
            None => Translated::ValueDropped,
        },
        "-prince-pdf-output-intent" => Translated::Native {
            native_property: "-bd-pdf-output-intent",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-color-conversion" => Translated::Native {
            native_property: "-bd-pdf-colour-conversion",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-color-options" => Translated::Native {
            native_property: "-bd-pdf-colour-options",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-fallback-cmyk-profile" => Translated::Native {
            native_property: "-bd-pdf-fallback-cmyk-profile",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-filter-resolution" => Translated::Native {
            native_property: "-bd-filter-resolution",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-tagged" => Translated::Native {
            native_property: "-bd-pdf-tagged",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-role-map" => Translated::Native {
            native_property: "-bd-pdf-role-map",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-xmp" => Translated::Native {
            native_property: "-bd-pdf-xmp",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-pdf-script" => Translated::Native {
            native_property: "-bd-pdf-script",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-pdf-event-scripts" => match translate_prince_event_scripts(raw_value) {
            Some(translated) => Translated::Native {
                native_property: "-bd-pdf-event-scripts",
                native_value: translated,
            },
            None => Translated::ValueDropped,
        },

        "-prince-pdf-page-colorspace" => Translated::Native {
            native_property: "-bd-pdf-page-colourspace",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-overprint" => Translated::Native {
            native_property: "-bd-pdf-overprint",
            native_value: match raw_value.trim() {
                "mode1" | "mode0" | "preserve" => "preserve",
                "none" => "none",

                _ => "auto",
            }
            .to_string(),
        },

        "-ro-pdf-overprint-content" => Translated::Native {
            native_property: "-bd-pdf-overprint-content",
            native_value: match raw_value.trim() {
                "mode1" | "mode0" | "preserve" => "preserve",
                "none" => "none",

                _ => "auto",
            }
            .to_string(),
        },

        "-ro-media-size" => Translated::Native {
            native_property: "-bd-pdf-media-size",
            native_value: resolve_pdfreactor_oversize_paper_name(raw_value.trim()),
        },
        "-ro-crop-size" => {
            let trimmed = raw_value.trim();
            let lower = trimmed.to_ascii_lowercase();
            match lower.as_str() {
                "trim" | "bleed" | "art" | "media" | "crop" => Translated::Native {
                    native_property: "-bd-pdf-page-clip",
                    native_value: lower,
                },
                _ => Translated::Native {
                    native_property: "-bd-pdf-crop-size",
                    native_value: trimmed.to_string(),
                },
            }
        },
        "-ro-art-size" => Translated::Native {
            native_property: "-bd-pdf-art-size",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-page-clip" => Translated::Native {
            native_property: "-bd-pdf-page-clip",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-crop-mark-length" => Translated::Native {
            native_property: "-bd-page-crop-mark-length",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-crop-mark-offset" => Translated::Native {
            native_property: "-bd-page-crop-mark-offset",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-bleed-mark-length" => Translated::Native {
            native_property: "-bd-page-bleed-mark-length",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-bleed-mark-offset" => Translated::Native {
            native_property: "-bd-page-bleed-mark-offset",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-registration-mark-offset" => Translated::Native {
            native_property: "-bd-page-registration-mark-offset",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-registration-mark-size" => Translated::Native {
            native_property: "-bd-page-registration-mark-size",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-marks-color" => Translated::Natives(vec![
            ("-bd-pdf-mark-crop-color", raw_value.trim().to_string()),
            ("-bd-pdf-mark-bleed-color", raw_value.trim().to_string()),
        ]),
        "-ro-marks-offset" | "-prince-mark-offset" => Translated::Native {
            native_property: "-bd-page-marks-offset",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-marks-width" | "-prince-mark-width" => Translated::Native {
            native_property: "-bd-page-marks-width",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-mark-length" => Translated::Native {
            native_property: "-bd-page-crop-mark-length",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-image-recompression" => {
            let v = raw_value.trim();
            let lower = v.to_ascii_lowercase();
            let mapped = if lower == "lossy" {
                "75".to_string()
            } else {
                v.to_string()
            };
            Translated::Native {
                native_property: "-bd-image-recompression",
                native_value: mapped,
            }
        },

        "-ro-image-interactivity" => Translated::Native {
            native_property: "-bd-image-interactivity",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-image-orientation" => Translated::Native {
            native_property: "-bd-image-orientation",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-image-resampling" => {
            let v = raw_value.trim().to_ascii_lowercase();
            match v.as_str() {
                "low" => Translated::Native {
                    native_property: "-bd-image-resampling",
                    native_value: "nearest".to_string(),
                },
                "medium" => Translated::Native {
                    native_property: "-bd-image-resampling",
                    native_value: "linear".to_string(),
                },
                "high" => Translated::Native {
                    native_property: "-bd-image-resampling",
                    native_value: "cubic".to_string(),
                },
                "auto" | "none" | "nearest" | "linear" | "cubic" => Translated::Native {
                    native_property: "-bd-image-resampling",
                    native_value: v,
                },
                _ => Translated::ValueDropped,
            }
        },
        "-ro-image-resolution" | "-prince-image-resolution" => Translated::Native {
            native_property: "-bd-image-resolution",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-background-image-resolution" | "background-image-resolution" => {
            translate_prince_background_image_resolution(raw_value)
        },
        "-prince-image-magic" => Translated::Native {
            native_property: "-bd-image-magic",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-image-clip-path" => Translated::Native {
            native_property: "-bd-image-clip-path",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-page-rotation" => Translated::Native {
            native_property: "-bd-pdf-page-rotation",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-rotate-body" => Translated::Native {
            native_property: "-bd-rotate-body",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-text-rendering" => Translated::Native {
            native_property: "-bd-pdf-text-rendering",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-paint-reordering" => Translated::Native {
            native_property: "-bd-paint-reordering",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-font-embedding-type" => Translated::Native {
            native_property: "-bd-font-embedding-type",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-glyph-layout-mode" => {
            let v = raw_value.trim();
            let lower = v.to_ascii_lowercase();
            let mapped = match lower.as_str() {
                "latin" => "metric",
                "cjk" => "optical",
                "balanced" => "auto",

                _ => v,
            };
            Translated::Native {
                native_property: "-bd-glyph-layout-mode",
                native_value: mapped.to_string(),
            }
        },
        "-ro-rasterization" => Translated::Native {
            native_property: "-bd-rasterization",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-rasterization-max-size" => Translated::Native {
            native_property: "-bd-rasterization-max-size",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-rasterization-supersampling" => Translated::Native {
            native_property: "-bd-rasterization-supersampling",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-pdf-shape-optimization" => Translated::Native {
            native_property: "-bd-pdf-shape-optimization",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-passdown-styles" => Translated::Native {
            native_property: "-bd-pdf-passdown-styles",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-bookmarks-enabled" => {
            let native_value = match raw_value.trim().to_ascii_lowercase().as_str() {
                "false" => "none".to_string(),
                "true" => "auto".to_string(),
                other => other.to_string(),
            };
            Translated::Native {
                native_property: "-bd-pdf-bookmarks-enabled",
                native_value,
            }
        },

        "text-overline" | "-prince-text-overline" => Translated::Native {
            native_property: "-bd-text-overline",
            native_value: raw_value.trim().to_string(),
        },
        "text-overline-color" | "-prince-text-overline-color" => Translated::Native {
            native_property: "-bd-text-overline-color",
            native_value: raw_value.trim().to_string(),
        },
        "text-overline-style" | "-prince-text-overline-style" => Translated::Native {
            native_property: "-bd-text-overline-style",
            native_value: raw_value.trim().to_string(),
        },
        "text-underline" | "-prince-text-underline" => Translated::Native {
            native_property: "-bd-text-underline",
            native_value: raw_value.trim().to_string(),
        },
        "text-underline-color" | "-prince-text-underline-color" => Translated::Native {
            native_property: "-bd-text-underline-color",
            native_value: raw_value.trim().to_string(),
        },
        "text-underline-style" | "-prince-text-underline-style" => Translated::Native {
            native_property: "-bd-text-underline-style",
            native_value: raw_value.trim().to_string(),
        },
        "text-line-through" | "-prince-text-line-through" => Translated::Native {
            native_property: "-bd-text-linethrough",
            native_value: raw_value.trim().to_string(),
        },
        "text-line-through-color" | "-prince-text-line-through-color" => Translated::Native {
            native_property: "-bd-text-linethrough-color",
            native_value: raw_value.trim().to_string(),
        },
        "text-line-through-style" | "-prince-text-line-through-style" => Translated::Native {
            native_property: "-bd-text-linethrough-style",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-tab-size" | "-ro-tab-size" => translate_tab_size_extension(raw_value),

        "text-justify-ext" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let native_value = match v.as_str() {
                "auto" => "auto",
                "none" => "none",
                "inter-word" => "inter-word",
                "inter-character" | "distribute" => "inter-character",
                "prince-cjk" => "-bd-cjk",
                _ => return Translated::ValueDropped,
            };
            Translated::Native {
                native_property: "text-justify",
                native_value: native_value.to_string(),
            }
        },

        "-prince-lang" => Translated::Native {
            native_property: "-bd-lang",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-shrink-to-fit" => Translated::Native {
            native_property: "-bd-shrink-to-fit",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-table-column-span" => Translated::Native {
            native_property: "-bd-table-column-span",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-table-row-span" | "-ro-rowspan" => Translated::Native {
            native_property: "-bd-table-row-span",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-caption-page" => Translated::Native {
            native_property: "-bd-caption-page",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-border-clip" => {
            let v = raw_value.trim().to_ascii_lowercase();
            match v.as_str() {
                "square" | "round" | "bevel" => Translated::Native {
                    native_property: "-bd-border-clip",
                    native_value: v,
                },
                _ => Translated::ValueDropped,
            }
        },
        "-ro-target-candidate" => Translated::Native {
            native_property: "-bd-target-candidate",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-truncate-margin-after-break" => Translated::Native {
            native_property: "-bd-truncate-margin-after-break",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-listitem-value" => Translated::Native {
            native_property: "-bd-listitem-value",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-counter-set" => Translated::Native {
            native_property: "counter-set",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-column-clip" => Translated::Native {
            native_property: "-bd-column-clip",
            native_value: match raw_value.trim().to_ascii_lowercase().as_str() {
                "none" => "normal".to_string(),
                "auto" => "clip".to_string(),
                other => other.to_string(),
            },
        },

        "-ro-overflow-clip-margin" => Translated::Native {
            native_property: "overflow-clip-margin",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-replacedelement" => match raw_value.trim().to_ascii_lowercase().as_str() {
            "barcode" => Translated::Native {
                native_property: "-bd-replacedelement",
                native_value: "image".to_string(),
            },
            "qrcode" => Translated::Natives(vec![
                ("-bd-replacedelement", "image".to_string()),
                ("-bd-barcode-type", "qr-code".to_string()),
            ]),
            other => Translated::Native {
                native_property: "-bd-replacedelement",
                native_value: other.to_string(),
            },
        },
        "-ro-scale-content" => Translated::Native {
            native_property: "-bd-scale-content",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-position-origin" => Translated::Native {
            native_property: "-bd-position-origin",
            native_value: match raw_value.trim().to_ascii_lowercase().as_str() {
                "-ro-page-box" => "-bd-page-box".to_string(),
                "-ro-bleed-box" => "-bd-bleed-box".to_string(),
                other => other.to_string(),
            },
        },
        "-ro-line-break-opportunity" => Translated::Native {
            native_property: "-bd-line-break-opportunity",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-marker-side" => Translated::Native {
            native_property: "marker-side",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-object-slice" => Translated::Native {
            native_property: "-bd-object-slice",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-flow" => Translated::Native {
            native_property: "-bd-flow",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-border-length" => Translated::Native {
            native_property: "-bd-footnote-rule-length",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-float-reference" => Translated::Native {
            native_property: "float-reference",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-float-offset" => Translated::Native {
            native_property: "float-offset",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-footnote-fragmentation" => Translated::Native {
            native_property: "-bd-footnote-fragmentation",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-sidenote-align" => Translated::Native {
            native_property: "-bd-sidenote-align",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-sidenote-avoid" => Translated::Native {
            native_property: "-bd-sidenote-avoid",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-sidenote-offset" => Translated::Native {
            native_property: "-bd-sidenote-offset",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-line-grid" => {
            let v = raw_value.trim().to_ascii_lowercase();
            if v == "create" {
                Translated::Natives(vec![
                    ("-bd-line-grid", v),
                    ("-bd-baseline-grid", "auto".to_string()),
                ])
            } else {
                Translated::Native {
                    native_property: "-bd-line-grid",
                    native_value: v,
                }
            }
        },
        "-ro-line-snap" => Translated::Native {
            native_property: "-bd-line-snap",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-baseline-grid" => Translated::Native {
            native_property: "-bd-baseline-grid",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-line-stacking-strategy" => Translated::Native {
            native_property: "-bd-line-stacking-strategy",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-pdf-attachment-description" => Translated::Native {
            native_property: "-bd-pdf-attachment-description",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-attachment-location" => {
            let trimmed = raw_value.trim().to_ascii_lowercase();
            match trimmed.as_str() {
                "before" | "after" => Translated::Native {
                    native_property: "-bd-pdf-attachment-order",
                    native_value: trimmed,
                },
                _ => Translated::Native {
                    native_property: "-bd-pdf-attachment-location",
                    native_value: trimmed,
                },
            }
        },
        "-ro-pdf-attachment-mime-type" => Translated::Native {
            native_property: "-bd-pdf-attachment-mime-type",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-pdf-attachment-name" => Translated::Native {
            native_property: "-bd-pdf-attachment-name",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-pdf-attachment-url" => Translated::Native {
            native_property: "-bd-pdf-attachment-url",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-destination" => Translated::Native {
            native_property: "-bd-pdf-destination",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-pdf-page-label" | "-bfo-page-label" | "-ro-pdf-page-label" => Translated::Native {
            native_property: "-bd-pdf-page-label",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-anchor" => Translated::Native {
            native_property: "-bd-anchor",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-destination-area" => Translated::Native {
            native_property: "-bd-destination-area",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-prince-bookmark-target" | "-bfo-bookmark-target" => Translated::Native {
            native_property: "bookmark-target",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-bookmark-label" => Translated::Native {
            native_property: "bookmark-label",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-bookmark-level" => Translated::Native {
            native_property: "bookmark-level",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-bookmark-state" => Translated::Native {
            native_property: "bookmark-state",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-prince-hyphenate-character" => Translated::Native {
            native_property: "hyphenate-character",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-hyphenate-before" => {
            let trimmed = raw_value.trim();
            if trimmed.is_empty() || trimmed.parse::<i32>().is_err() {
                return Translated::ValueDropped;
            }
            Translated::Native {
                native_property: "hyphenate-limit-chars",
                native_value: format!("auto {trimmed} auto"),
            }
        },
        "-prince-hyphenate-after" => {
            let trimmed = raw_value.trim();
            if trimmed.is_empty() || trimmed.parse::<i32>().is_err() {
                return Translated::ValueDropped;
            }
            Translated::Native {
                native_property: "hyphenate-limit-chars",
                native_value: format!("auto auto {trimmed}"),
            }
        },

        "-prince-text-justify" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let native_value = match v.as_str() {
                "auto" => "auto",
                "prince-cjk" => "-bd-cjk",
                _ => return Translated::ValueDropped,
            };
            Translated::Native {
                native_property: "text-justify",
                native_value: native_value.to_string(),
            }
        },

        "-prince-clear" => {
            let v = raw_value.trim().to_ascii_lowercase();
            match v.as_str() {
                "none" | "left" | "right" | "both" => Translated::Native {
                    native_property: "clear",
                    native_value: v,
                },
                "inside" | "outside" | "column" | "page" | "end" => Translated::ValueDropped,
                _ => Translated::ValueDropped,
            }
        },

        "-prince-trim" => {
            let trimmed = raw_value.trim();
            if trimmed.eq_ignore_ascii_case("auto") {
                return Translated::Native {
                    native_property: "-bd-pdf-crop-size",
                    native_value: "auto".to_string(),
                };
            }

            let token_count = trimmed.split_ascii_whitespace().count();
            if token_count == 0 || token_count > 2 {
                return Translated::ValueDropped;
            }
            Translated::Native {
                native_property: "-bd-pdf-crop-size",
                native_value: trimmed.to_string(),
            }
        },

        "-prince-footnote-policy" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "auto" => Some("auto"),
                "keep-with-line" => Some("line"),
                "keep-with-block" => Some("block"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "footnote-policy",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-prince-pdf-form" => {
            let v = raw_value.trim().to_ascii_lowercase();
            let mapped = match v.as_str() {
                "enable" => Some("pdf"),
                "disable" | "auto" => Some("none"),
                _ => None,
            };
            match mapped {
                Some(native) => Translated::Native {
                    native_property: "-bd-pdf-format",
                    native_value: native.to_string(),
                },
                None => Translated::ValueDropped,
            }
        },

        "-prince-pdf-open-action" => {
            let trimmed = raw_value.trim();
            if trimmed.eq_ignore_ascii_case("none") {
                return Translated::Native {
                    native_property: "-bd-initial-zoom",
                    native_value: "auto".to_string(),
                };
            }

            let mut tokens: Vec<&str> = Vec::new();
            let bytes = trimmed.as_bytes();
            let mut depth: i32 = 0;
            let mut start = 0usize;
            let mut i = 0usize;
            while i < bytes.len() {
                let b = bytes[i];
                if b == b'(' {
                    depth += 1;
                } else if b == b')' {
                    depth -= 1;
                } else if depth == 0 && b.is_ascii_whitespace() {
                    if start < i {
                        tokens.push(&trimmed[start..i]);
                    }
                    start = i + 1;
                }
                i += 1;
            }
            if start < bytes.len() {
                tokens.push(&trimmed[start..bytes.len()]);
            }
            if tokens.len() != 1 {
                return Translated::ValueDropped;
            }
            let token = tokens[0];

            let lower = token.to_ascii_lowercase();
            let Some(rest) = lower.strip_prefix("zoom(") else {
                return Translated::ValueDropped;
            };
            let Some(arg) = rest.strip_suffix(')') else {
                return Translated::ValueDropped;
            };
            let arg = arg.trim();
            let native_value = if let Some(num) = arg.strip_suffix('%') {
                if num.is_empty() || num.parse::<f32>().is_err() {
                    return Translated::ValueDropped;
                }
                arg.to_string()
            } else {
                match arg {
                    "fit-page" => "fit-page".to_string(),
                    "fit-width" => "fit-page-width".to_string(),
                    "fit-height" => "fit-page-height".to_string(),
                    _ => return Translated::ValueDropped,
                }
            };
            Translated::Native {
                native_property: "-bd-initial-zoom",
                native_value,
            }
        },
        "-prince-pdf-link-type" => Translated::Native {
            native_property: "-bd-pdf-link-type",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-prince-pdf-tag-title" => {
            let trimmed = raw_value.trim();

            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("attr(") {
                return Translated::ValueDropped;
            }
            Translated::Native {
                native_property: "-bd-pdf-tag-title",
                native_value: trimmed.to_string(),
            }
        },

        "-prince-expansion-text" => {
            let trimmed = raw_value.trim();
            if trimmed.is_empty() {
                return Translated::ValueDropped;
            }
            let lower = trimmed.to_ascii_lowercase();

            if lower == "auto" || lower.starts_with("attr(") {
                return Translated::ValueDropped;
            }
            Translated::Native {
                native_property: "-bd-pdf-tag-expanded",
                native_value: trimmed.to_string(),
            }
        },

        "-ro-link" | "-prince-link" => {
            let trimmed = raw_value.trim();
            let native_value = if trimmed.starts_with('"') || trimmed.starts_with('\'') {
                format!("url({trimmed})")
            } else {
                trimmed.to_string()
            };
            Translated::Native {
                native_property: "-bd-link",
                native_value,
            }
        },
        "-ro-link-area" => {
            let mapped = match raw_value.trim().to_ascii_lowercase().as_str() {
                "block" | "all" | "all-block" => "border-box".to_string(),
                "content" | "content-block" => "content-box".to_string(),
                other => other.to_string(),
            };
            Translated::Native {
                native_property: "-bd-pdf-link-area",
                native_value: mapped,
            }
        },

        "-ro-link-color"
        | "-ro-link-colour"
        | "-ro-link-border-color"
        | "-ro-link-border-colour" => Translated::Native {
            native_property: "-bd-pdf-link-border-color",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-link-border-width" => Translated::Native {
            native_property: "-bd-pdf-link-border-width",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-link-border-style" => Translated::Native {
            native_property: "-bd-pdf-link-border-style",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-text-replace" | "-prince-text-replace" => Translated::Native {
            native_property: "-bd-text-replace",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-alt-text" | "-ro-alt-text" => Translated::Native {
            native_property: "-bd-pdf-alt-text",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-tag-alt" => Translated::Native {
            native_property: "-bd-pdf-tag-alt",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-tag-actual-text" => Translated::Native {
            native_property: "-bd-pdf-tag-actual-text",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-tag-expanded" => Translated::Native {
            native_property: "-bd-pdf-tag-expanded",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-tag-header-cell-scope" => Translated::Native {
            native_property: "-bd-pdf-tag-header-cell-scope",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-pdf-tag-table-summary" => Translated::Native {
            native_property: "-bd-pdf-tag-table-summary",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-tag-form" => Translated::Native {
            native_property: "-bd-pdf-tag-form",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-pdf-tag-form-checked" => Translated::Native {
            native_property: "-bd-pdf-tag-form-checked",
            native_value: match raw_value.trim().to_ascii_lowercase().as_str() {
                "neutral" => "mixed".to_string(),
                other => other.to_string(),
            },
        },

        "-ro-pdf-tag-form-name" => {
            let trimmed = raw_value.trim();
            if trimmed.eq_ignore_ascii_case("none") {
                Translated::Native {
                    native_property: "-bd-pdf-tag-form-name",
                    native_value: "none".to_string(),
                }
            } else {
                Translated::Native {
                    native_property: "-bd-pdf-tag-form-name",
                    native_value: trimmed.to_string(),
                }
            }
        },

        "-prince-page-fill" => Translated::Native {
            native_property: "-bd-page-fill",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-change-line-breaks-for-pagination" => Translated::Native {
            native_property: "-bd-change-line-breaks-for-pagination",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-line-break-choices" => Translated::Native {
            native_property: "-bd-line-break-choices",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-forced-breaks" => Translated::Native {
            native_property: "-bd-forced-breaks",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-n-lines" => Translated::Native {
            native_property: "-bd-n-lines",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-resize-adjust" => Translated::Native {
            native_property: "-bd-resize-adjust",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-resize-options" => Translated::Native {
            native_property: "-bd-resize-options",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-spread-length-options" => Translated::Native {
            native_property: "-bd-spread-length-options",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-text-wrap" => Translated::Native {
            native_property: "-bd-text-wrap",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-wrap-inside" => Translated::Native {
            native_property: "-bd-wrap-inside",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-prince-pdf-signature" | "-prince-signature" => Translated::Native {
            native_property: "-bd-pdf-signature",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-blank-page-content" => Translated::Native {
            native_property: "-bd-blank-page-content",

            native_value: raw_value.trim().to_string(),
        },
        "-prince-keep-with-previous" => Translated::Native {
            native_property: "-bd-keep-with-previous",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-orphans-fragments" => Translated::Native {
            native_property: "-bd-orphans-fragments",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-pdf-signature" => Translated::Native {
            native_property: "-bd-pdf-signature",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-blank-page-content" => Translated::Native {
            native_property: "-bd-blank-page-content",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-keep-with-previous" => Translated::Native {
            native_property: "-bd-keep-with-previous",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-orphans-fragments" => Translated::Native {
            native_property: "-bd-orphans-fragments",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-prince-margin-inside" => Translated::Native {
            native_property: "-bd-margin-inside",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-margin-outside" => Translated::Native {
            native_property: "-bd-margin-outside",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-margin-alt" => Translated::Native {
            native_property: "-bd-margin-alt",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-page-group" => Translated::Native {
            native_property: "-bd-page-group",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-hyphenate-limit-lines" | "-prince-hyphenate-limit-lines" => Translated::Native {
            native_property: "-bd-hyphenate-limit-lines",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-hyphenate-patterns" => Translated::Native {
            native_property: "-bd-hyphenate-patterns",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-hyphenate-lines" => Translated::Native {
            native_property: "-bd-hyphenate-lines",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-hyphenate-word-length" => Translated::Native {
            native_property: "-bd-hyphenate-word-length",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-linebreak-magic" => Translated::Native {
            native_property: "-bd-linebreak-magic",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-barcode" => Translated::Native {
            native_property: "-bd-barcode",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-checkdigit-mode" => Translated::Native {
            native_property: "-bd-barcode-checkdigit-mode",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-barcode-color" | "-ro-barcode-colour" => Translated::Native {
            native_property: "-bd-barcode-colour",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-composite-content" => Translated::Native {
            native_property: "-bd-barcode-composite-content",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-composite-type" => Translated::Native {
            native_property: "-bd-barcode-composite-type",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-barcode-content" => Translated::Native {
            native_property: "-bd-barcode-content",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-ecc-level" => Translated::Native {
            native_property: "-bd-barcode-ecc-level",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-encoding" => Translated::Native {
            native_property: "-bd-barcode-encoding",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-font-family" => Translated::Native {
            native_property: "-bd-barcode-font-family",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-font-size" => Translated::Native {
            native_property: "-bd-barcode-font-size",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-human-readable-affix" => Translated::Native {
            native_property: "-bd-barcode-human-readable-affix",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-human-readable-position" => Translated::Native {
            native_property: "-bd-barcode-human-readable-position",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-barcode-letter-spacing" => Translated::Native {
            native_property: "-bd-barcode-letter-spacing",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-reader-initialization" => Translated::Native {
            native_property: "-bd-barcode-reader-initialization",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-size" => Translated::Native {
            native_property: "-bd-barcode-size",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-structured-append" => Translated::Native {
            native_property: "-bd-barcode-structured-append",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-structured-append-position" => Translated::Native {
            native_property: "-bd-barcode-structured-append-position",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-symbol-width" => Translated::Native {
            native_property: "-bd-barcode-symbol-width",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-barcode-type" => Translated::Native {
            native_property: "-bd-barcode-type",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-ro-qrcode-content" => Translated::Native {
            native_property: "-bd-barcode-content",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-qrcode-ecc-level" => Translated::Native {
            native_property: "-bd-barcode-ecc-level",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-qrcode-encoding" => Translated::Native {
            native_property: "-bd-barcode-encoding",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-qrcode-size" => Translated::Native {
            native_property: "-bd-barcode-size",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-qrcode-errorcorrectionlevel" => Translated::Native {
            native_property: "-bd-barcode-ecc-level",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-qrcode-forcedcolors" => match raw_value.trim().to_ascii_lowercase().as_str() {
            "normal" => Translated::Native {
                native_property: "-bd-barcode-colour",
                native_value: "black".to_string(),
            },
            "none" => Translated::Native {
                native_property: "-bd-barcode-colour",
                native_value: "currentColor".to_string(),
            },
            _ => Translated::ValueDropped,
        },

        "-ro-qrcode-quality" => match raw_value.trim().to_ascii_lowercase().as_str() {
            "normal" | "high" => Translated::Satisfied,
            _ => Translated::ValueDropped,
        },

        "-ro-change-bar" => Translated::Native {
            native_property: "-bd-change-bar",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-change-bar-align" => Translated::Native {
            native_property: "-bd-change-bar-align",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-change-bar-color" | "-ro-change-bar-colour" => Translated::Native {
            native_property: "-bd-change-bar-colour",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-change-bar-exclusion" => Translated::Native {
            native_property: "-bd-change-bar-exclusion",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-change-bar-name" => Translated::Native {
            native_property: "-bd-change-bar-name",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-change-bar-offset" => Translated::Native {
            native_property: "-bd-change-bar-offset",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-change-bar-width" => Translated::Native {
            native_property: "-bd-change-bar-width",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-flow-from" => Translated::Native {
            native_property: "-bd-flow-from",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-flow-into" => {
            let trimmed = raw_value.trim();
            let lower = trimmed.to_ascii_lowercase();
            let native_value =
                if lower.ends_with(" element") || lower.ends_with(" content") || lower == "none" {
                    trimmed.to_string()
                } else {
                    format!("{trimmed} element")
                };
            Translated::Native {
                native_property: "-bd-flow-into",
                native_value,
            }
        },

        "-prince-float-policy" | "float-policy" => Translated::Native {
            native_property: "-bd-float-policy",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-float-tail" | "float-tail" => Translated::Native {
            native_property: "-bd-float-tail",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-float-modifier" | "float-modifier" => Translated::Native {
            native_property: "-bd-float-modifier",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-prince-float-defer-column" | "float-defer-column" => Translated::Native {
            native_property: "-bd-float-defer-column",
            native_value: raw_value.trim().to_string(),
        },
        "-prince-float-defer-page" | "float-defer-page" => Translated::Native {
            native_property: "float-defer-page",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-float" => translate_prince_float_shorthand(raw_value),

        "-prince-float-reference" => {
            let v = raw_value.trim().to_ascii_lowercase();
            match v.as_str() {
                "inline" | "column" | "page" | "region" => Translated::Native {
                    native_property: "float-reference",
                    native_value: v,
                },
                _ => Translated::ValueDropped,
            }
        },
        "-prince-float-placement" => {
            let v = raw_value.trim().to_ascii_lowercase();
            match v.as_str() {
                "none" | "left" | "right" | "inside" | "outside" | "top" | "bottom"
                | "footnote" => Translated::Native {
                    native_property: "float",
                    native_value: v,
                },

                "top-bottom" => Translated::Native {
                    native_property: "float",
                    native_value: "top-unless-room".to_string(),
                },
                _ => Translated::ValueDropped,
            }
        },

        "-ro-colorbar-top-left" => Translated::Native {
            native_property: "-bd-page-colorbar-top-left",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-colorbar-top-right" => Translated::Native {
            native_property: "-bd-page-colorbar-top-right",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-colorbar-bottom-left" => Translated::Native {
            native_property: "-bd-page-colorbar-bottom-left",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-colorbar-bottom-right" => Translated::Native {
            native_property: "-bd-page-colorbar-bottom-right",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-colorbar-left-top" => Translated::Native {
            native_property: "-bd-page-colorbar-left-top",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-colorbar-left-bottom" => Translated::Native {
            native_property: "-bd-page-colorbar-left-bottom",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-colorbar-right-top" => Translated::Native {
            native_property: "-bd-page-colorbar-right-top",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-colorbar-right-bottom" => Translated::Native {
            native_property: "-bd-page-colorbar-right-bottom",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-colorbar-offset" => Translated::Native {
            native_property: "-bd-page-colorbar-offset",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-marks" => Translated::Native {
            native_property: "marks",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },

        "-bfo-index" => Translated::Native {
            native_property: "-bd-index",
            native_value: raw_value.trim().to_string(),
        },
        "-bfo-pdf-tag" => Translated::Native {
            native_property: "-bd-pdf-tag",
            native_value: raw_value.trim().to_string(),
        },
        "-bfo-raster-resolution" => Translated::Native {
            native_property: "-bd-image-resolution",
            native_value: raw_value.trim().to_string(),
        },
        "-bfo-text-decoration-skip-ink-clearance" => Translated::PropertyDropped,
        "-bfo-trim" => Translated::Native {
            native_property: "-bd-pdf-crop-size",
            native_value: raw_value.trim().to_string(),
        },
        "-bfo-trim-top" | "-bfo-trim-right" | "-bfo-trim-bottom" | "-bfo-trim-left"
        | "-bfo-trim-size" => Translated::Native {
            native_property: "-bd-pdf-crop-size",
            native_value: raw_value.trim().to_string(),
        },

        "-ro-pdf-form-field-flags" => Translated::Native {
            native_property: "-bd-pdf-form-field-flags",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-pdf-form-field-maxlength" => Translated::Native {
            native_property: "-bd-pdf-form-field-maxlength",
            native_value: raw_value.trim().to_string(),
        },
        "-ro-pdf-signature-field-lock" => Translated::Native {
            native_property: "-bd-pdf-signature-field-lock",
            native_value: raw_value.trim().to_ascii_lowercase(),
        },
        "-ro-pdf-signature-field-name" => Translated::Native {
            native_property: "-bd-pdf-signature-field-name",
            native_value: raw_value.trim().to_string(),
        },

        "-prince-pdf-form-field-font-size" => Translated::Native {
            native_property: "font-size",
            native_value: raw_value.trim().to_string(),
        },

        _ => Translated::PropertyDropped,
    }
}

fn map_prince_pdf_profile(value: &str) -> Option<String> {
    let v = strip_quotes(value).to_ascii_lowercase();
    let native = match v.as_str() {
        "pdf/a-1a" | "pdfa-1a" => "a1a",
        "pdf/a-1b" | "pdfa-1b" => "a1b",
        "pdf/a-2a" | "pdfa-2a" => "a2a",
        "pdf/a-2b" | "pdfa-2b" => "a2b",
        "pdf/a-2u" | "pdfa-2u" => "a2u",
        "pdf/a-3a" | "pdfa-3a" => "a3a",
        "pdf/a-3b" | "pdfa-3b" => "a3b",
        "pdf/a-3u" | "pdfa-3u" => "a3u",
        "pdf/a-4" | "pdfa-4" => "a4",
        "pdf/a-4e" | "pdfa-4e" => "a4e",
        "pdf/a-4f" | "pdfa-4f" => "a4f",

        "pdf/ua-1" | "pdfua-1" => "ua1",

        "pdf/x-1a:2001" | "pdfx-1a-2001" => "pdf-x-1a-2001",
        "pdf/x-1a:2003" | "pdfx-1a-2003" => "pdf-x-1a-2003",
        "pdf/x-3:2002" | "pdfx-3-2002" => "pdf-x-3-2002",
        "pdf/x-3:2003" | "pdfx-3-2003" => "pdf-x-3-2003",
        "pdf/x-4" | "pdfx-4" => "pdf-x-4",
        "pdf/x-4p" | "pdfx-4p" => "pdf-x-4p",
        _ => return None,
    };
    Some(native.to_string())
}

fn strip_quotes(value: &str) -> String {
    let v = value.trim();
    if (v.starts_with('"') && v.ends_with('"') && v.len() >= 2)
        || (v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2)
    {
        v[1..v.len() - 1].to_string()
    } else {
        v.to_string()
    }
}

fn map_ro_pdf_tag_type(value: &str) -> Option<String> {
    let v = value.to_ascii_lowercase();
    let native = match v.as_str() {
        "auto" => "auto",
        "none" => "none",
        "artifact" => "artifact",
        "part" => "part",
        "art" => "article",
        "sect" => "section",
        "div" => "div",
        "blockquote" => "block-quote",
        "caption" => "caption",
        "toc" => "toc",
        "toci" => "toci",
        "index" => "index",
        "nonstruct" => "non-struct",
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "h4" => "h4",
        "h5" => "h5",
        "h6" => "h6",
        "p" => "p",
        "l" => "l",
        "li" => "li",
        "lbl" => "lbl",
        "lbody" => "l-body",
        "table" => "table",
        "tr" => "tr",
        "th" => "th",
        "td" => "td",
        "thead" => "thead",
        "tbody" => "tbody",
        "tfoot" => "tfoot",
        "span" => "span",
        "quote" => "inline-quote",
        "note" => "note",
        "reference" => "reference",
        "bibentry" => "bib-entry",
        "code" => "code",
        "link" => "link",
        "annot" => "annot",
        "figure" => "figure",
        "formula" => "formula",
        "form" => "form",

        "h" | "private" | "ruby" | "rb" | "rt" | "rp" | "warichu" | "wt" | "wp" => return None,
        _ => return None,
    };
    Some(native.to_string())
}

fn map_prince_pdf_tag_type(value: &str) -> Option<String> {
    let v = value.to_ascii_lowercase();
    let native = match v.as_str() {
        "auto" => "auto",
        "none" => "none",
        "artifact" => "artifact",
        "part" => "part",
        "art" => "article",
        "sect" => "section",
        "div" => "div",
        "index" => "index",
        "blockquote" => "block-quote",
        "caption" => "caption",
        "toc" => "toc",
        "toci" => "toci",
        "p" => "p",
        "h1" => "h1",
        "h2" => "h2",
        "h3" => "h3",
        "h4" => "h4",
        "h5" => "h5",
        "h6" => "h6",
        "l" => "l",
        "li" => "li",
        "lbl" => "lbl",
        "lbody" => "l-body",
        "span" => "span",
        "quote" => "inline-quote",
        "table" => "table",
        "bibentry" => "bib-entry",
        "code" => "code",
        "figure" => "figure",
        "formula" => "formula",
        "note" => "note",
        "reference" => "reference",

        "dl" | "dl-div" | "dt" | "dd" => return None,
        _ => return None,
    };
    Some(native.to_string())
}

fn is_known_foreign(lower_property: &str) -> bool {
    required_compat_for(lower_property).is_some()
}

fn required_compat_for(lower_property: &str) -> Option<CompatMode> {
    match lower_property {
        "-ro-pdf-conformance"
        | "-ro-pdf-format"
        | "-ro-pdf-tag-type"
        | "-ro-height"
        | "-ro-block-size"
        | "-ro-source"
        | "-ro-source-page"
        | "-ro-source-area"
        | "-ro-pdf-overprint"
        | "-ro-pdf-overprint-content"
        | "-ro-media-size"
        | "-ro-crop-size"
        | "-ro-art-size"
        | "-ro-page-clip"
        | "-ro-crop-mark-length"
        | "-ro-crop-mark-offset"
        | "-ro-bleed-mark-length"
        | "-ro-bleed-mark-offset"
        | "-ro-registration-mark-offset"
        | "-ro-registration-mark-size"
        | "-ro-marks-color"
        | "-ro-marks-offset"
        | "-ro-marks-width"
        | "-ro-image-recompression"
        | "-ro-image-resampling"
        | "-ro-image-resolution"
        | "-ro-image-clip-path"
        | "-ro-image-interactivity"
        | "-ro-image-orientation"
        | "-ro-pdf-page-rotation"
        | "-ro-pdf-text-rendering"
        | "-ro-paint-reordering"
        | "-ro-font-embedding-type"
        | "-ro-glyph-layout-mode"
        | "-ro-rasterization"
        | "-ro-rasterization-max-size"
        | "-ro-rasterization-supersampling"
        | "-ro-pdf-shape-optimization"
        | "-ro-passdown-styles"
        | "-ro-bookmarks-enabled"
        | "-ro-tab-size"
        | "text-justify-ext"
        | "-ro-target-candidate"
        | "-ro-truncate-margin-after-break"
        | "-ro-column-clip"
        | "-ro-listitem-value"
        | "-ro-counter-set"
        | "-ro-overflow-clip-margin"
        | "-ro-replacedelement"
        | "-ro-rowspan"
        | "-ro-scale-content"
        | "-ro-position-origin"
        | "-ro-line-break-opportunity"
        | "-ro-object-slice"
        | "-ro-marker-side"
        | "-ro-border-length"
        | "-ro-footnote-fragmentation"
        | "-ro-float-offset"
        | "-ro-float-reference"
        | "-ro-sidenote-align"
        | "-ro-sidenote-avoid"
        | "-ro-sidenote-offset"
        | "-ro-line-grid"
        | "-ro-line-snap"
        | "-ro-pdf-attachment-description"
        | "-ro-pdf-attachment-location"
        | "-ro-pdf-attachment-mime-type"
        | "-ro-pdf-attachment-name"
        | "-ro-pdf-attachment-url"
        | "-ro-pdf-page-label"
        | "-ro-anchor"
        | "-ro-destination-area"
        | "-ro-link"
        | "-ro-link-area"
        | "-ro-link-color"
        | "-ro-link-colour"
        | "-ro-link-border-color"
        | "-ro-link-border-colour"
        | "-ro-link-border-width"
        | "-ro-link-border-style"
        | "-ro-text-replace"
        | "-ro-alt-text"
        | "-ro-pdf-tag-alt"
        | "-ro-pdf-tag-actual-text"
        | "-ro-pdf-tag-expanded"
        | "-ro-pdf-tag-header-cell-scope"
        | "-ro-pdf-tag-table-summary"
        | "-ro-pdf-tag-form"
        | "-ro-pdf-tag-form-checked"
        | "-ro-pdf-tag-form-name"
        | "-ro-hyphenate-limit-lines"
        | "-ro-hyphenate-word-length"
        | "-ro-barcode"
        | "-ro-barcode-checkdigit-mode"
        | "-ro-barcode-color"
        | "-ro-barcode-colour"
        | "-ro-barcode-composite-content"
        | "-ro-barcode-composite-type"
        | "-ro-barcode-content"
        | "-ro-barcode-ecc-level"
        | "-ro-barcode-encoding"
        | "-ro-barcode-font-family"
        | "-ro-barcode-font-size"
        | "-ro-barcode-human-readable-affix"
        | "-ro-barcode-human-readable-position"
        | "-ro-barcode-letter-spacing"
        | "-ro-barcode-reader-initialization"
        | "-ro-barcode-size"
        | "-ro-barcode-structured-append"
        | "-ro-barcode-structured-append-position"
        | "-ro-barcode-symbol-width"
        | "-ro-barcode-type"
        | "-ro-qrcode-content"
        | "-ro-qrcode-ecc-level"
        | "-ro-qrcode-encoding"
        | "-ro-qrcode-size"
        | "-ro-qrcode-errorcorrectionlevel"
        | "-ro-qrcode-forcedcolors"
        | "-ro-qrcode-quality"
        | "-ro-change-bar"
        | "-ro-change-bar-align"
        | "-ro-change-bar-color"
        | "-ro-change-bar-colour"
        | "-ro-change-bar-exclusion"
        | "-ro-change-bar-name"
        | "-ro-change-bar-offset"
        | "-ro-change-bar-width"
        | "-ro-flow-from"
        | "-ro-flow-into"
        | "-ro-colorbar-top-left"
        | "-ro-colorbar-top-right"
        | "-ro-colorbar-bottom-left"
        | "-ro-colorbar-bottom-right"
        | "-ro-colorbar-left-top"
        | "-ro-colorbar-left-bottom"
        | "-ro-colorbar-right-top"
        | "-ro-colorbar-right-bottom"
        | "-ro-colorbar-offset"
        | "-ro-marks"
        | "-ro-pdf-form-field-flags"
        | "-ro-pdf-form-field-maxlength"
        | "-ro-pdf-signature-field-lock"
        | "-ro-pdf-signature-field-name"
        | "-ro-comment-style"
        | "-ro-comment-color"
        | "-ro-comment-content"
        | "-ro-comment-title"
        | "-ro-comment-author"
        | "-ro-comment-date"
        | "-ro-comment-dateformat"
        | "-ro-comment-position"
        | "-ro-comment-state"
        | "-ro-comment-statemodel"
        | "-ro-comment-subject"
        | "-ro-comment-start"
        | "-ro-comment-end"
        | "-ro-pdf-signature"
        | "-ro-blank-page-content"
        | "-ro-keep-with-previous"
        | "-ro-orphans-fragments" => Some(CompatMode::PdfReactor),

        "-prince-pdf-page-layout"
        | "-prince-pdf-page-mode"
        | "-prince-pdf-display-doc-title"
        | "-prince-pdf-duplex"
        | "-prince-pdf-paper-tray"
        | "-prince-pdf-print-scaling"
        | "-prince-pdf-annotation-type"
        | "-prince-pdf-annotation-color"
        | "-prince-pdf-annotation-contents"
        | "-prince-pdf-annotation-title"
        | "-prince-pdf-annotation-icon"
        | "-prince-pdf-annotation-author"
        | "-prince-pdf-annotation-createdate"
        | "-prince-pdf-annotation-modifydate"
        | "-prince-pdf-annotation-position"
        | "-prince-pdf-tag-type"
        | "-prince-pdf-profile"
        | "-prince-pdf-output-intent"
        | "-prince-pdf-color-conversion"
        | "-prince-pdf-color-options"
        | "-prince-fallback-cmyk-profile"
        | "-prince-filter-resolution"
        | "-prince-pdf-role-map"
        | "-prince-pdf-tagged"
        | "-prince-pdf-xmp"
        | "-prince-pdf-script"
        | "-prince-pdf-event-scripts"
        | "-prince-pdf-page-colorspace"
        | "-prince-mark-length"
        | "-prince-mark-offset"
        | "-prince-mark-width"
        | "-prince-image-magic"
        | "-prince-image-resolution"
        | "-prince-background-image-resolution"
        | "background-image-resolution"
        | "-prince-rotate-body"
        | "text-overline"
        | "text-overline-color"
        | "text-overline-style"
        | "text-underline"
        | "text-underline-color"
        | "text-underline-style"
        | "text-line-through"
        | "text-line-through-color"
        | "text-line-through-style"
        | "-prince-text-overline"
        | "-prince-text-overline-color"
        | "-prince-text-overline-style"
        | "-prince-text-underline"
        | "-prince-text-underline-color"
        | "-prince-text-underline-style"
        | "-prince-text-line-through"
        | "-prince-text-line-through-color"
        | "-prince-text-line-through-style"
        | "-prince-lang"
        | "-prince-shrink-to-fit"
        | "-prince-table-column-span"
        | "-prince-table-row-span"
        | "-prince-caption-page"
        | "-prince-flow"
        | "-prince-tab-size"
        | "-prince-border-clip"
        | "-prince-baseline-grid"
        | "-prince-line-stacking-strategy"
        | "-prince-pdf-destination"
        | "-prince-pdf-page-label"
        | "-prince-bookmark-target"
        | "-prince-bookmark-label"
        | "-prince-bookmark-level"
        | "-prince-bookmark-state"
        | "-prince-clear"
        | "-prince-hyphenate-character"
        | "-prince-text-justify"
        | "-prince-trim"
        | "-prince-hyphenate-before"
        | "-prince-hyphenate-after"
        | "-prince-footnote-policy"
        | "-prince-pdf-form"
        | "-prince-pdf-open-action"
        | "-prince-pdf-link-type"
        | "-prince-pdf-tag-title"
        | "-prince-expansion-text"
        | "-prince-link"
        | "-prince-text-replace"
        | "-prince-tooltip"
        | "-prince-alt-text"
        | "-prince-page-fill"
        | "-prince-change-line-breaks-for-pagination"
        | "-prince-line-break-choices"
        | "-prince-forced-breaks"
        | "-prince-n-lines"
        | "-prince-resize-adjust"
        | "-prince-resize-options"
        | "-prince-spread-length-options"
        | "-prince-text-wrap"
        | "-prince-wrap-inside"
        | "-prince-pdf-signature"
        | "-prince-signature"
        | "-prince-blank-page-content"
        | "-prince-keep-with-previous"
        | "-prince-orphans-fragments"
        | "-prince-margin-inside"
        | "-prince-margin-outside"
        | "-prince-margin-alt"
        | "-prince-page-group"
        | "-prince-hyphenate-limit-lines"
        | "-prince-hyphenate-patterns"
        | "-prince-hyphenate-lines"
        | "-prince-linebreak-magic"
        | "-prince-float-policy"
        | "float-policy"
        | "-prince-float-tail"
        | "float-tail"
        | "-prince-float-modifier"
        | "float-modifier"
        | "-prince-float-defer-column"
        | "float-defer-column"
        | "-prince-float-defer-page"
        | "float-defer-page"
        | "-prince-float-reference"
        | "-prince-float-placement"
        | "-prince-float"
        | "-prince-pdf-form-field-font-size" => Some(CompatMode::Prince),

        "-bfo-bookmark-target"
        | "-bfo-page-label"
        | "-bfo-index"
        | "-bfo-pdf-tag"
        | "-bfo-raster-resolution"
        | "-bfo-text-decoration-skip-ink-clearance"
        | "-bfo-trim"
        | "-bfo-trim-top"
        | "-bfo-trim-right"
        | "-bfo-trim-bottom"
        | "-bfo-trim-left"
        | "-bfo-trim-size" => Some(CompatMode::Prince),

        _ => None,
    }
}

fn apply_rewrites(
    css: &str,
    injections: &[Injection],
    value_replacements: &[ValueReplacement],
    tail_descriptors: &[TailDescriptor],
) -> String {
    let mut sorted_injections = injections.to_vec();
    sorted_injections.sort_by_key(|i| i.before_close_byte);
    let mut sorted_replacements = value_replacements.to_vec();
    sorted_replacements.sort_by_key(|replacement| replacement.start_byte);

    let extra_tail = tail_descriptors_to_css(tail_descriptors);
    let mut out = String::with_capacity(
        css.len()
            + sorted_injections
                .iter()
                .map(|injection| injection.native_declaration.len() + 1)
                .sum::<usize>()
            + sorted_replacements
                .iter()
                .map(|replacement| replacement.replacement.len())
                .sum::<usize>()
            + extra_tail.len(),
    );

    let mut cursor = 0usize;
    let mut injection_index = 0usize;
    let mut replacement_index = 0usize;
    while injection_index < sorted_injections.len() || replacement_index < sorted_replacements.len()
    {
        let next_injection = sorted_injections.get(injection_index);
        let next_replacement = sorted_replacements.get(replacement_index);
        let apply_replacement = match (next_injection, next_replacement) {
            (Some(injection), Some(replacement)) => {
                replacement.start_byte <= injection.before_close_byte
            },
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (None, None) => break,
        };

        if apply_replacement {
            let replacement = &sorted_replacements[replacement_index];
            replacement_index += 1;
            if replacement.start_byte < cursor
                || replacement.end_byte < replacement.start_byte
                || replacement.end_byte > css.len()
            {
                continue;
            }
            out.push_str(&css[cursor..replacement.start_byte]);
            out.push_str(&replacement.replacement);
            cursor = replacement.end_byte;
            continue;
        }

        let injection = &sorted_injections[injection_index];
        injection_index += 1;
        let close = injection.before_close_byte;
        if close < cursor || close > css.len() {
            continue;
        }
        out.push_str(&css[cursor..close]);

        let needs_separator = out.trim_end().ends_with(|c: char| c != ';' && c != '{');
        if needs_separator {
            out.push(';');
        }
        out.push(' ');
        out.push_str(&injection.native_declaration);
        cursor = close;
    }
    out.push_str(&css[cursor..]);
    out.push_str(&extra_tail);
    out
}

fn tail_descriptors_to_css(descriptors: &[TailDescriptor]) -> String {
    if descriptors.is_empty() {
        return String::new();
    }
    let mut out = String::from("\n:root {");
    for d in descriptors {
        out.push(' ');
        out.push_str(&d.native_declaration);
    }
    out.push_str(" }\n");
    out
}

fn skip_to_decl_end_byte<'i>(parser: &mut Parser<'i, '_>) -> usize {
    loop {
        let snapshot = parser.state();
        match parser.next_including_whitespace_and_comments() {
            Ok(token) => match token.clone() {
                Token::Semicolon => {
                    return snapshot.position().byte_index();
                },
                Token::CloseCurlyBracket => {
                    parser.reset(&snapshot);
                    return snapshot.position().byte_index();
                },
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    let _ = parser.parse_nested_block(|inner| -> Result<(), ParseError<'i, ()>> {
                        while inner.next_including_whitespace_and_comments().is_ok() {}
                        Ok(())
                    });
                },
                _ => {},
            },
            Err(_) => {
                return snapshot.position().byte_index();
            },
        }
    }
}

fn ascii_lower(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        out.push(c.to_ascii_lowercase());
    }
    out
}

fn collapse_whitespace_ascii_lower(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = true;
    for c in s.chars() {
        if c.is_ascii_whitespace() {
            if !in_ws {
                out.push(' ');
                in_ws = true;
            }
        } else {
            out.push(c.to_ascii_lowercase());
            in_ws = false;
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

fn find_containing_close_brace(full_css: &str, line: u32, column: u32) -> Option<usize> {
    let line_start = line_column_to_byte(full_css, line, column)?;
    let bytes = full_css.as_bytes();
    let mut depth: i32 = 0;
    let mut i = line_start;
    let mut in_string: Option<u8> = None;
    let mut in_comment = false;

    while i < bytes.len() {
        let b = bytes[i];
        if in_comment {
            if b == b'*' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
                in_comment = false;
                i += 2;
                continue;
            }
            i += 1;
            continue;
        }
        if let Some(quote) = in_string {
            if b == 92 {
                i += 2;
                continue;
            }
            if b == quote {
                in_string = None;
            }
            i += 1;
            continue;
        }
        match b {
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                in_comment = true;
                i += 2;
                continue;
            },
            b'"' | b'\'' => {
                in_string = Some(b);
                i += 1;
                continue;
            },
            b'{' => {
                depth += 1;
                i += 1;
            },
            b'}' => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
                i += 1;
            },
            _ => i += 1,
        }
    }
    None
}

fn line_column_to_byte(css: &str, line: u32, column: u32) -> Option<usize> {
    let mut current_line: u32 = 1;
    let mut current_col: u32 = 1;
    for (idx, ch) in css.char_indices() {
        if current_line == line && current_col == column {
            return Some(idx);
        }
        if ch == '\n' {
            current_line += 1;
            current_col = 1;
        } else {
            current_col += 1;
        }
    }
    None
}

#[cfg(test)]
mod grid_lanes_property_projection_tests {
    use super::{CompatMode, translate_compat};

    #[test]
    fn flow_tolerance_projects_to_the_typed_internal_property() {
        let stylesheet = "@supports (flow-tolerance: 5px) { .lanes { FLOW-TOLERANCE : 20%; } .normal { flow-tolerance: normal } .unbounded { flow-tolerance: infinite } }";
        assert_eq!(
            translate_compat(stylesheet, CompatMode::None).rewritten,
            "@supports (masonry-slack: 5px) { .lanes { masonry-slack : 20%; } .normal { masonry-slack: infinite } .unbounded { masonry-slack: auto } }"
        );
    }

    #[test]
    fn flow_tolerance_text_outside_property_positions_is_unchanged() {
        let stylesheet = r#"/* flow-tolerance: 1px */ .x { --flow-tolerance: 2px; content: "flow-tolerance: 3px"; }"#;
        assert!(translate_compat(stylesheet, CompatMode::None).is_borrowed());
    }

    #[test]
    fn grid_lanes_pack_projects_to_the_typed_grid_flow_carrier() {
        let stylesheet = "@supports (grid-lanes-pack:/* leading */dense) { .lanes { GRID-LANES-PACK : normal/* trailing */; } }";
        assert_eq!(
            translate_compat(stylesheet, CompatMode::None).rewritten,
            "@supports (grid-auto-flow:/* leading */dense) { .lanes { grid-auto-flow : row/* trailing */; } }"
        );
    }

    #[test]
    fn invalid_grid_lanes_pack_values_remain_unsupported() {
        for stylesheet in [
            ".x { grid-lanes-pack: auto; }",
            ".x { grid-lanes-pack: normal dense; }",
            ".x { grid-lanes-pack: dense reverse; }",
        ] {
            assert!(
                translate_compat(stylesheet, CompatMode::None).is_borrowed(),
                "{stylesheet}"
            );
        }
    }

    #[test]
    fn grid_lanes_direction_remains_native() {
        let stylesheet = ".columns { grid-lanes-direction: column track-reverse; } .rows { grid-lanes-direction: row fill-reverse track-reverse !important; }";
        assert_eq!(
            translate_compat(stylesheet, CompatMode::None).rewritten,
            stylesheet
        );
    }
}

#[cfg(test)]
mod position_visibility_syntax_adapter_tests {
    use super::{CompatMode, translate_compat};

    #[test]
    fn normative_singular_keywords_project_to_the_stylo_backing_keywords() {
        let stylesheet = ".valid { position-visibility: anchor-valid no-overflow } .visible { POSITION-VISIBILITY: anchor-visible }";

        assert_eq!(
            translate_compat(stylesheet, CompatMode::None).rewritten,
            ".valid { position-visibility: anchors-valid no-overflow } .visible { POSITION-VISIBILITY: anchors-visible }"
        );
    }

    #[test]
    fn position_visibility_text_outside_its_declaration_value_is_unchanged() {
        let stylesheet = r#".x { --condition: anchor-visible; content: "position-visibility: anchor-valid"; position-visibility: var(--condition) }"#;

        assert!(translate_compat(stylesheet, CompatMode::None).is_borrowed());
    }
}
