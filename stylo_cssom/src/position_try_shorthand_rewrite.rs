use std::borrow::Cow;

use crate::{
    css_scan::{rewrite_style_rules, split_top_level},
    declaration_parser::{position_try_shorthand_longhands, split_inline_declaration_importance},
};

fn rewrite_declaration_block(block: &str) -> String {
    let mut output = String::with_capacity(block.len());
    for range in split_top_level(block.as_bytes(), b';') {
        let declaration = &block[range.clone()];
        let leading = declaration.len() - declaration.trim_start().len();
        let Some((property, raw_value)) = declaration[leading..].split_once(':') else {
            output.push_str(declaration);
            if range.end < block.len() {
                output.push(';');
            }
            continue;
        };
        if !property.trim().eq_ignore_ascii_case("position-try") {
            output.push_str(declaration);
            if range.end < block.len() {
                output.push(';');
            }
            continue;
        }

        let (value, importance) = split_inline_declaration_importance(raw_value.trim());
        let Some(longhands) = position_try_shorthand_longhands(value.trim()) else {
            output.push_str(declaration);
            if range.end < block.len() {
                output.push(';');
            }
            continue;
        };
        let priority = if importance == stylo_cssom_model::Importance::Important {
            " !important"
        } else {
            ""
        };
        output.push_str(&declaration[..leading]);
        output.push_str("position-try-order:");
        output.push_str(longhands.order());
        output.push_str(priority);
        output.push_str(";position-try-fallbacks:");
        output.push_str(longhands.fallbacks());
        output.push_str(priority);
        if range.end < block.len() {
            output.push(';');
        }
    }
    output
}

/// Expand the Gecko-only `position-try` shorthand into the two typed
/// longhands accepted by Stylo's Servo cascade.
pub fn rewrite_position_try_shorthand(css: &str) -> Cow<'_, str> {
    if !css
        .as_bytes()
        .windows(b"position-try".len())
        .any(|window| window.eq_ignore_ascii_case(b"position-try"))
    {
        return Cow::Borrowed(css);
    }
    let rewritten = rewrite_style_rules(css, &|prelude, body| {
        format!("{prelude}{{{}}}", rewrite_declaration_block(body))
    });
    if rewritten == css {
        Cow::Borrowed(css)
    } else {
        Cow::Owned(rewritten)
    }
}

#[cfg(test)]
mod tests {
    use super::rewrite_position_try_shorthand;

    #[test]
    fn expands_only_valid_style_rule_shorthands_with_their_priority() {
        let css = concat!(
            ".a{POSITION-TRY:most-width --a, flip-inline!important;color:red}",
            "@media print{.b{position-try:--b}}",
            ".c{position-try:normal --a, most-width --b;content:'position-try: --x'}",
        );

        assert_eq!(
            rewrite_position_try_shorthand(css),
            concat!(
                ".a{position-try-order:most-width !important;",
                "position-try-fallbacks:--a, flip-inline !important;color:red}",
                "@media print{.b{position-try-order:normal;position-try-fallbacks:--b}}",
                ".c{position-try:normal --a, most-width --b;content:'position-try: --x'}",
            )
        );
    }
}
