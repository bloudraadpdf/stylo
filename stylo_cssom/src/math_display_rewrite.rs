use std::{borrow::Cow, ops::Range};

use crate::css_scan::{is_css_whitespace, is_ident_continue};

/// MathML Core requires `math` to compute to `flow` on pseudo-elements.
/// Stylo's Servo configuration does not expose the `math` grammar, so this
/// closed projection preserves the specified outer display at the parser
/// boundary without introducing a fake MathML layout mode into the IR.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NonMathMlPseudoDisplay {
    InlineFlow,
    BlockFlow,
}

impl NonMathMlPseudoDisplay {
    fn parse_math_value(value: &[u8]) -> Option<Self> {
        let words = value
            .split(|byte| is_css_whitespace(*byte))
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>();
        match words.as_slice() {
            [math] if math.eq_ignore_ascii_case(b"math") => Some(Self::InlineFlow),
            [block, math]
                if block.eq_ignore_ascii_case(b"block") && math.eq_ignore_ascii_case(b"math") =>
            {
                Some(Self::BlockFlow)
            },
            [inline, math]
                if inline.eq_ignore_ascii_case(b"inline") && math.eq_ignore_ascii_case(b"math") =>
            {
                Some(Self::InlineFlow)
            },
            _ => None,
        }
    }

    const fn css_keyword(self) -> &'static str {
        match self {
            Self::InlineFlow => "inline",
            Self::BlockFlow => "block",
        }
    }
}

struct DisplayRewrite {
    source: Range<usize>,
    replacement: NonMathMlPseudoDisplay,
}

pub fn rewrite_non_mathml_pseudo_display(css: &str) -> Cow<'_, str> {
    if !css
        .as_bytes()
        .windows(4)
        .any(|window| window.eq_ignore_ascii_case(b"math"))
    {
        return Cow::Borrowed(css);
    }

    let bytes = css.as_bytes();
    let mut rewrites = Vec::new();
    let mut cursor = 0;
    while cursor + b"display".len() <= bytes.len() {
        let Some(relative) = bytes[cursor..]
            .windows(b"display".len())
            .position(|window| window.eq_ignore_ascii_case(b"display"))
        else {
            break;
        };
        let property_start = cursor + relative;
        cursor = property_start + b"display".len();
        if property_start > 0 && is_ident_continue(bytes[property_start - 1])
            || cursor < bytes.len() && is_ident_continue(bytes[cursor])
        {
            continue;
        }

        let mut colon = cursor;
        while colon < bytes.len() && is_css_whitespace(bytes[colon]) {
            colon += 1;
        }
        if bytes.get(colon) != Some(&b':') {
            continue;
        }

        let Some(open_brace) = bytes[..property_start]
            .iter()
            .rposition(|byte| *byte == b'{')
        else {
            continue;
        };
        let prelude_start = bytes[..open_brace]
            .iter()
            .rposition(|byte| matches!(byte, b'{' | b'}' | b';'))
            .map_or(0, |index| index + 1);
        if !bytes[prelude_start..open_brace]
            .windows(2)
            .any(|window| window == b"::")
        {
            continue;
        }

        let mut value_start = colon + 1;
        while value_start < bytes.len() && is_css_whitespace(bytes[value_start]) {
            value_start += 1;
        }
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
        let Some(replacement) =
            NonMathMlPseudoDisplay::parse_math_value(&bytes[value_start..value_end])
        else {
            continue;
        };
        rewrites.push(DisplayRewrite {
            source: value_start..value_end,
            replacement,
        });
        cursor = declaration_end;
    }

    if rewrites.is_empty() {
        return Cow::Borrowed(css);
    }
    let mut rewritten = css.to_owned();
    for rewrite in rewrites.into_iter().rev() {
        rewritten.replace_range(rewrite.source, rewrite.replacement.css_keyword());
    }
    Cow::Owned(rewritten)
}

#[cfg(test)]
mod tests {
    use super::rewrite_non_mathml_pseudo_display;

    #[test]
    fn pseudo_math_values_retain_their_outer_display_and_compute_inside_to_flow() {
        let css = ".a::before { display: math } .b::after { display: block math !important }";
        assert_eq!(
            rewrite_non_mathml_pseudo_display(css),
            ".a::before { display: inline } .b::after { display: block !important }"
        );
    }

    #[test]
    fn ordinary_mathml_rules_are_not_rewritten_as_pseudo_elements() {
        let css = "math { display: block math } span { display: math }";
        assert!(matches!(
            rewrite_non_mathml_pseudo_display(css),
            std::borrow::Cow::Borrowed(_)
        ));
    }
}
