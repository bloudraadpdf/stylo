use std::collections::BTreeSet;

use cssparser::{Parser, ParserInput, Token};
use style::properties::{LonghandId, PropertyDeclaration, PropertyDeclarationId};

/// Inputs which can prevent resolution before layout or external state is available.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpecifiedValueDependencies {
    pub font_metrics: bool,
    pub geometry: bool,
    pub custom_properties: BTreeSet<String>,
    pub unsupported_functions: BTreeSet<String>,
    pub invalid: bool,
}

/// Inspect a native declaration without resolving its specified value.
#[must_use]
pub fn property_declaration_dependencies(
    declaration: &PropertyDeclaration,
) -> SpecifiedValueDependencies {
    let mut value = String::new();
    if declaration.to_css(&mut value).is_err() {
        return SpecifiedValueDependencies {
            invalid: true,
            ..Default::default()
        };
    }
    let mut result = specified_value_dependencies(&declaration.id().name(), &value);
    if declaration.id() == PropertyDeclarationId::Longhand(LonghandId::PositionTryFallbacks)
        && !matches!(declaration, PropertyDeclaration::PositionTryFallbacks(value) if value.0.is_empty())
    {
        result.geometry = true;
    }
    result
}

fn specified_value_dependencies(property: &str, value: &str) -> SpecifiedValueDependencies {
    let mut result = SpecifiedValueDependencies::default();
    if property.eq_ignore_ascii_case("font-size-adjust") && !value.eq_ignore_ascii_case("none") {
        result.font_metrics = true;
    }
    if property.eq_ignore_ascii_case("math-depth") && value != "0" {
        result.font_metrics = true;
    }
    let mut input = ParserInput::new(value);
    visit(
        &mut Parser::new(&mut input),
        &mut result,
        property.eq_ignore_ascii_case("font-size")
            || property.eq_ignore_ascii_case("font")
            || property.starts_with("--"),
    );
    result
}

fn visit(
    parser: &mut Parser<'_, '_>,
    result: &mut SpecifiedValueDependencies,
    math_font_keyword: bool,
) {
    while !parser.is_exhausted() {
        let Ok(token) = parser.next_including_whitespace_and_comments().cloned() else {
            result.invalid = true;
            break;
        };
        match token {
            Token::Dimension { unit, .. } => match unit.to_ascii_lowercase().as_str() {
                "ex" | "rex" | "ch" | "rch" | "cap" | "rcap" | "ic" | "ric" | "lh" | "rlh" => {
                    result.font_metrics = true
                },
                "px" | "in" | "cm" | "mm" | "q" | "pt" | "pc" | "em" | "rem" | "deg" | "grad"
                | "rad" | "turn" | "s" | "ms" | "dpi" | "dpcm" | "dppx" | "x" | "fr" => {},
                _ => result.geometry = true,
            },
            Token::Ident(name) if math_font_keyword && name.eq_ignore_ascii_case("math") => {
                result.font_metrics = true;
            },
            Token::Function(name) => {
                let name = name.to_ascii_lowercase();
                match name.as_str() {
                    "anchor" | "anchor-size" | "calc-size" => result.geometry = true,
                    "var" | "calc" | "min" | "max" | "clamp" | "round" | "mod" | "rem" | "abs"
                    | "sign" | "sqrt" | "pow" | "hypot" | "log" | "exp" | "sin" | "cos" | "tan"
                    | "asin" | "acos" | "atan" | "atan2" | "rgb" | "rgba" | "hsl" | "hsla"
                    | "hwb" | "lab" | "lch" | "oklab" | "oklch" | "color" | "color-mix"
                    | "light-dark" | "url" | "format" | "local" | "counter" | "counters"
                    | "reversed" | "rotate" | "rotatex" | "rotatey" | "rotatez" | "rotate3d"
                    | "translate" | "translatex" | "translatey" | "translatez" | "translate3d"
                    | "scale" | "scalex" | "scaley" | "scalez" | "scale3d" | "matrix"
                    | "matrix3d" | "skew" | "skewx" | "skewy" | "perspective" => {},
                    _ => {
                        result.unsupported_functions.insert(name.clone());
                    },
                }
                let nested = parser.parse_nested_block(
                    |inner| -> Result<(), cssparser::ParseError<'_, ()>> {
                        if name == "var" {
                            match inner.expect_ident_cloned() {
                                Ok(name) if name.starts_with("--") => {
                                    result.custom_properties.insert(name.to_string());
                                },
                                _ => result.invalid = true,
                            }
                        }
                        visit(inner, result, math_font_keyword);
                        Ok(())
                    },
                );
                result.invalid |= nested.is_err();
            },
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                let nested = parser.parse_nested_block(
                    |inner| -> Result<(), cssparser::ParseError<'_, ()>> {
                        visit(inner, result, math_font_keyword);
                        Ok(())
                    },
                );
                result.invalid |= nested.is_err();
            },
            Token::BadUrl(_)
            | Token::BadString(_)
            | Token::CloseParenthesis
            | Token::CloseSquareBracket
            | Token::CloseCurlyBracket => result.invalid = true,
            _ => {},
        }
    }
}

#[cfg(test)]
mod tests {
    use super::specified_value_dependencies as dependencies;

    #[test]
    fn typed_declarations_keep_metric_and_absolute_lengths_distinct() {
        use style::{properties::PropertyDeclaration, values::specified::FontSize};

        for (value, metrics) in [("12pt", false), ("1ex", true), ("math", true)] {
            let value =
                crate::values::parse_value::<FontSize>(value).expect("the font size must parse");
            let result =
                super::property_declaration_dependencies(&PropertyDeclaration::FontSize(value));
            assert_eq!(result.font_metrics, metrics);
            assert!(!result.invalid);
        }
    }

    #[test]
    fn typed_position_options_need_layout_even_without_functions() {
        use style::{properties::PropertyDeclaration, values::specified::PositionTryFallbacks};

        for (source, geometry) in [("none", false), ("flip-block", true), ("--alternate", true)] {
            let value = crate::values::parse_value::<PositionTryFallbacks>(source)
                .expect("the position options must parse");
            let result = super::property_declaration_dependencies(
                &PropertyDeclaration::PositionTryFallbacks(value),
            );
            assert_eq!(result.geometry, geometry);
        }
    }

    #[test]
    fn nested_escaped_units_and_variables_keep_all_dependencies() {
        let result = dependencies(
            "width",
            r"calc(1cqw + var(--Size, max(2e\78, var(--other))))",
        );
        assert!(result.geometry);
        assert!(result.font_metrics);
        assert_eq!(
            result.custom_properties.into_iter().collect::<Vec<_>>(),
            ["--Size", "--other"]
        );
        assert!(!result.invalid);
    }

    #[test]
    fn resource_strings_do_not_become_dependencies() {
        let result = dependencies("background", r#"url("var(--x) 2ch anchor(--x)")"#);
        assert_eq!(result, Default::default());
    }

    #[test]
    fn keyword_metric_dependencies_and_unknown_functions_are_conservative() {
        assert!(dependencies("font-size", r"m\61th").font_metrics);
        assert!(dependencies("font-size-adjust", "0.5").font_metrics);
        assert!(dependencies("math-depth", "auto-add").font_metrics);
        assert!(!dependencies("font-size-adjust", "none").font_metrics);
        assert!(!dependencies("math-depth", "0").font_metrics);
        let result = dependencies("width", "future(12pt) attr(data-width type(<length>))");
        assert_eq!(
            result.unsupported_functions.into_iter().collect::<Vec<_>>(),
            ["attr", "future", "type"]
        );
    }

    #[test]
    fn ordinary_size_units_need_no_late_inputs() {
        assert_eq!(
            dependencies("height", "calc(12pt + 1em + 1rem)"),
            Default::default()
        );
        assert!(dependencies("height", "1unknown").geometry);
        assert!(dependencies("width", "var(not-custom)").invalid);
    }
}
