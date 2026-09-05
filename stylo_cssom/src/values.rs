use crate::style_fragment_parser::parse_style_fragment_with as parse_fragment_with;
use cssparser::{Parser, ParserInput, Token};
use style::{
    parser::Parse,
    properties::PropertyDeclaration,
    stylesheets::supports_rule::{
        Declaration as SupportsDeclaration, parse_condition_or_declaration,
    },
};

pub fn parse_value<T: Parse>(source: &str) -> Option<T> {
    parse_fragment_with(source, T::parse)
}

/// Return whether a CSS component value contains a container-relative length.
#[must_use]
pub fn value_uses_container_length_units(css: &str) -> bool {
    fn parser_uses_container_length_units(parser: &mut Parser<'_, '_>) -> bool {
        let mut found = false;
        while let Ok(token) = parser.next_including_whitespace_and_comments() {
            match token.clone() {
                Token::Dimension { unit, .. }
                    if matches!(
                        unit.as_ref().to_ascii_lowercase().as_str(),
                        "cqw" | "cqh" | "cqi" | "cqb" | "cqmin" | "cqmax"
                    ) =>
                {
                    found = true;
                },
                Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock => {
                    let nested = parser.parse_nested_block(
                        |inner| -> Result<bool, cssparser::ParseError<'_, ()>> {
                            Ok(parser_uses_container_length_units(inner))
                        },
                    );
                    found |= nested.unwrap_or(false);
                },
                _ => {},
            }
        }
        found
    }

    let mut input = ParserInput::new(css);
    parser_uses_container_length_units(&mut Parser::new(&mut input))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CssSupportsQuery<'a> {
    Declaration { property: &'a str, value: &'a str },
    Condition(&'a str),
}

#[derive(Clone, Copy, Debug)]
pub struct CssSupportsInput<'a>(CssSupportsQuery<'a>);

impl<'a> CssSupportsInput<'a> {
    #[must_use]
    pub const fn declaration(property: &'a str, value: &'a str) -> Self {
        Self(CssSupportsQuery::Declaration { property, value })
    }

    #[must_use]
    pub const fn condition(condition: &'a str) -> Self {
        Self(CssSupportsQuery::Condition(condition))
    }
}

#[must_use]
pub fn supports(input: CssSupportsInput<'_>) -> bool {
    css_supports(input.0)
}

pub fn css_supports(query: CssSupportsQuery<'_>) -> bool {
    crate::context::initialise_required_servo_style_prefs();
    match query {
        CssSupportsQuery::Declaration { property, value } => {
            if property.eq_ignore_ascii_case("position-try") {
                return crate::declaration_parser::position_try_shorthand_is_valid(value);
            }
            if crate::declaration_parser::parse_inline_compatibility_declaration(
                property,
                value,
                stylo_cssom_model::Importance::Normal,
                &std::sync::Arc::from("about:blank"),
            )
            .is_some()
            {
                return true;
            }
            let declaration = format!("{property}:{value}");
            parse_fragment_with(&declaration, |context, input| {
                SupportsDeclaration::parse(input).map(|declaration| declaration.eval(context))
            })
            .unwrap_or(false)
        },
        CssSupportsQuery::Condition(condition) => {
            parse_fragment_with(condition, |context, input| {
                parse_condition_or_declaration(input).map(|condition| condition.eval(context))
            })
            .unwrap_or(false)
        },
    }
}

pub fn filter_component_ranges(css: &str) -> Option<Vec<(std::ops::Range<usize>, bool)>> {
    use style::values::generics::effects::Filter;
    use style::values::specified::effects::Filter as SpecifiedFilter;

    parse_fragment_with(css, |context, input| {
        let mut components = Vec::new();
        while !input.is_exhausted() {
            let start = input.position().byte_index();
            let filter = SpecifiedFilter::parse(context, input)?;
            let end = input.position().byte_index();
            components.push((start..end, matches!(filter, Filter::Url(_))));
        }
        Ok(components)
    })
}

pub fn parse_font_shorthand(css: &str) -> Option<Vec<PropertyDeclaration>> {
    use style::properties::SourcePropertyDeclaration;
    parse_fragment_with(css, |context, input| {
        let mut decls = SourcePropertyDeclaration::default();
        style::properties::shorthands::font::parse_into(&mut decls, context, input)?;
        Ok(decls.declarations.iter().cloned().collect())
    })
}

pub fn svg_transform_attribute_value_is_valid(value: &str) -> bool {
    crate::svg_presentation::transform_attribute_operations(value).is_some()
}

pub fn number_list(source: &str) -> Option<Vec<f32>> {
    parse_fragment_with(source, |_context, input| {
        let mut values = vec![input.expect_number()?];
        while !input.is_exhausted() {
            let _ = input.try_parse(Parser::expect_comma);
            values.push(input.expect_number()?);
        }
        Ok(values)
    })
}

pub fn parse_css_string_literal(css: &str) -> Option<String> {
    parse_fragment_with(css, |_context, input| {
        let value = input.expect_string()?.as_ref().to_owned();
        Ok(value)
    })
}

pub fn parse_margin_break(css: &str) -> Option<[style::values::specified::MarginBreak; 2]> {
    parse_fragment_with(css, |_context, input| {
        let before = style::values::specified::MarginBreak::parse(input)?;
        let after = if input.is_exhausted() {
            before
        } else {
            style::values::specified::MarginBreak::parse(input)?
        };
        Ok([before, after])
    })
}

pub fn percentage_points(css: &str) -> Option<Vec<[f32; 2]>> {
    parse_fragment_with(css, |context, input| {
        input.parse_comma_separated(|input| {
            let inline = style::values::specified::Percentage::parse(context, input)?;
            let block = style::values::specified::Percentage::parse(context, input)?;
            Ok([inline.get(), block.get()])
        })
    })
}

pub use style::values::specified::TransformBox;

pub fn keyword_is(source: &str, expected: &str) -> bool {
    crate::registration::single_identifier(source)
        .is_some_and(|value| value.eq_ignore_ascii_case(expected))
}
