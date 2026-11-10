use crate::style_fragment_parser::parse_style_fragment_with as parse_fragment_with;
use cssparser::{Parser, ParserInput, Token};
use style::{
    parser::Parse,
    properties::PropertyDeclaration,
    stylesheets::supports_rule::{
        parse_condition_or_declaration, Declaration as SupportsDeclaration, SupportsCondition,
    },
};

pub fn parse_value<T: Parse>(source: &str) -> Option<T> {
    parse_fragment_with(source, T::parse)
}

#[derive(Clone, Copy)]
pub enum ColorWellColorSpace {
    LimitedSrgb,
    DisplayP3,
}

/// HTML's colour-well value, independent of the element's CSS foreground.
pub fn color_well_value(
    source: &str,
    alpha: bool,
    space: ColorWellColorSpace,
) -> style::color::AbsoluteColor {
    use style::color::{AbsoluteColor, ColorSpace};
    let mut color = parse_value::<style::values::specified::Color>(source)
        .and_then(|color| color.to_computed_color(None))
        .map(|color| color.resolve_to_absolute(&AbsoluteColor::BLACK))
        .unwrap_or(AbsoluteColor::BLACK);
    if !alpha {
        color.alpha = 1.0;
    }
    match space {
        ColorWellColorSpace::LimitedSrgb => {
            color = color.to_color_space(ColorSpace::Srgb);
            let channels = color
                .components
                .map(|channel| (channel.clamp(0.0, 1.0) * 255.0).round() / 255.0);
            if alpha {
                AbsoluteColor::new(
                    ColorSpace::Srgb,
                    channels.0,
                    channels.1,
                    channels.2,
                    color.alpha,
                )
            } else {
                AbsoluteColor::new(ColorSpace::Srgb, channels.0, channels.1, channels.2, 1.0)
                    .into_srgb_legacy()
            }
        },
        ColorWellColorSpace::DisplayP3 => color.to_color_space(ColorSpace::DisplayP3),
    }
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
            if property.starts_with("--") {
                if !crate::is_valid_custom_property_name(property) {
                    return false;
                }
            } else if style::properties::PropertyId::parse_enabled_for_all_content(property)
                .is_err()
                && crate::declaration_parser::inline_compatibility_property(property).is_none()
            {
                return false;
            }
            if crate::declaration_parser::parse_inline_compatibility_declaration(
                property,
                value,
                stylo_cssom_model::Importance::Normal,
                &std::sync::Arc::from("about:blank"),
                selectors::matching::QuirksMode::NoQuirks,
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
            let projected = crate::compat::translate::rewrite_grid_lanes_properties(condition);
            let condition = projected.as_deref().unwrap_or(condition);
            parse_fragment_with(condition, |context, input| {
                let mut condition = parse_condition_or_declaration(input)?;
                project_active_view_transition_selectors(&mut condition);
                Ok(condition.eval(context))
            })
            .unwrap_or(false)
        },
    }
}

fn project_active_view_transition_selectors(condition: &mut SupportsCondition) {
    match condition {
        SupportsCondition::Selector(selector) => {
            if let std::borrow::Cow::Owned(projected) =
                crate::active_view_transition_selector::rewrite_selector(&selector.0)
            {
                selector.0 = projected;
            }
        },
        SupportsCondition::Not(inner) | SupportsCondition::Parenthesized(inner) => {
            project_active_view_transition_selectors(inner);
        },
        SupportsCondition::And(conditions) | SupportsCondition::Or(conditions) => {
            conditions.iter_mut().for_each(project_active_view_transition_selectors);
        },
        _ => {},
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

#[cfg(test)]
mod tests {
    #[test]
    fn supports_conditions_project_active_view_transition_selectors() {
        use super::{css_supports, CssSupportsQuery};

        for (condition, expected) in [
            ("selector(::part(example):active-view-transition)", true),
            (
                "selector(::part(example):active-view-transition-type(example))",
                true,
            ),
            (
                "selector(::part(example):active-view-transition-type(none))",
                false,
            ),
            (
                "not (selector(::part(example):active-view-transition) and (display: block))",
                false,
            ),
            (
                "(display: unknown) or selector(::part(example):active-view-transition)",
                true,
            ),
            ("selector(::part(example):active-view-transition, div)", false),
        ] {
            assert_eq!(
                css_supports(CssSupportsQuery::Condition(condition)),
                expected,
                "{condition}"
            );
        }
    }

    #[test]
    fn offset_paths_accept_and_serialize_basic_shape_fill_rules() {
        use style::values::specified::motion::OffsetPath;
        use style_traits::ToCss;

        crate::preferences::initialise_required_servo_style_prefs();
        for (source, expected) in [
            (
                "polygon(nonzero round 1px, 1px 2px, 3em 4em)",
                "polygon(round 1px, 1px 2px, 3em 4em)",
            ),
            (
                "polygon(evenodd round 1px, 1px 2px, 3em 4em, 5pt 6%)",
                "polygon(evenodd round 1px, 1px 2px, 3em 4em, 5pt 6%)",
            ),
            (
                "polygon(evenodd round 6pt, 1px 2px, 3em 4em, 6pt 6%)",
                "polygon(evenodd round 6pt, 1px 2px, 3em 4em, 6pt 6%)",
            ),
            (
                "shape(evenodd from 0px 0px, line to 10px 10px)",
                "shape(evenodd from 0px 0px, line to 10px 10px)",
            ),
            (
                "shape(nonzero from 0px 0px, line to 10px 10px)",
                "shape(from 0px 0px, line to 10px 10px)",
            ),
            (
                "shape(evenodd from 0px 0px, close)",
                "shape(evenodd from 0px 0px, close)",
            ),
            ("polygon(round 1px, 1% 2%)", "polygon(round 1px, 1% 2%)"),
            (
                "shape(from 0px 0px, line to 10px 10px)",
                "shape(from 0px 0px, line to 10px 10px)",
            ),
            ("path('M0 0L10 10')", "path(\"M 0 0 L 10 10\")"),
        ] {
            let value =
                super::parse_value::<OffsetPath>(source).unwrap_or_else(|| panic!("offset-path must accept {source}"));
            assert_eq!(value.to_css_string(), expected, "{source}");
        }
        for source in [
            "path(nonzero, 'M0 0L10 10')",
            "path(evenodd, 'M0 0L10 10')",
            "polygon(evenodd nonzero, 1px 2px)",
            "polygon(evenodd round -1px, 1px 2px)",
            "polygon(evenodd round 1px 1px 2px)",
            "shape(evenodd nonzero from 0px 0px, close)",
            "shape(evenodd, from 0px 0px, close)",
            "shape(evenodd from 0px 0px)",
        ] {
            assert!(super::parse_value::<OffsetPath>(source).is_none(), "{source}");
        }
    }

    #[test]
    fn color_wells_sanitise_before_their_value_enters_the_cascade() {
        use super::{color_well_value, ColorWellColorSpace};
        use style::color::{AbsoluteColor, ColorSpace};
        for source in ["invalid", "red; --injected: blue", "var(--color)"] {
            assert_eq!(
                color_well_value(source, false, ColorWellColorSpace::LimitedSrgb),
                AbsoluteColor::BLACK
            );
        }
        let opaque = color_well_value(
            "rgb(100% 0% 0% / 0)",
            false,
            ColorWellColorSpace::LimitedSrgb,
        );
        assert_eq!(opaque.alpha, 1.0);
        assert_eq!(opaque.components.0, 1.0);
        let transparent = color_well_value(
            "rgb(100% 0% 0% / .25)",
            true,
            ColorWellColorSpace::LimitedSrgb,
        );
        assert_eq!(transparent.alpha, 0.25);
        let rounded = color_well_value(
            "color(srgb .5 .2 .1)",
            false,
            ColorWellColorSpace::LimitedSrgb,
        );
        assert_eq!(rounded.components.0, 128.0 / 255.0);
        let p3 = color_well_value(
            "color(display-p3 .3 .4 .5 / .25)",
            true,
            ColorWellColorSpace::DisplayP3,
        );
        assert_eq!(p3.color_space, ColorSpace::DisplayP3);
        assert_eq!(p3.alpha, 0.25);
    }
    use super::{parse_font_shorthand, supports, CssSupportsInput};
    use style::properties::PropertyDeclaration;
    use style::values::computed::font::{ScriptSpecificGenericFontFamily, SingleFontFamily};
    use style::values::specified::FontFamily;

    #[test]
    fn supports_compatibility_properties_uses_their_declaration_parser() {
        for (property, value, expected) in [
            ("grid-lanes-pack", "normal", true),
            ("GRID-LANES-PACK", "dense", true),
            ("grid-lanes-pack", "auto", false),
            ("grid-lanes-pack", "normal dense", false),
            (" grid-lanes-pack", "dense", false),
            ("grid-lanes-pack ", "dense", false),
            ("fit-tolerance", "normal", true),
        ] {
            assert_eq!(
                supports(CssSupportsInput::declaration(property, value)),
                expected,
                "{property}: {value}"
            );
        }
        use crate::declaration_parser::{
            inline_style_cssom_authored_value, inline_style_cssom_backing_property,
            inline_style_cssom_backing_value,
        };
        assert_eq!(inline_style_cssom_backing_property("GRID-LANES-PACK"), "grid-auto-flow");
        assert_eq!(inline_style_cssom_backing_value("grid-lanes-pack", "normal"), Some("row"));
        assert_eq!(inline_style_cssom_backing_value("grid-lanes-pack", "dense"), Some("dense"));
        assert_eq!(inline_style_cssom_backing_value("grid-lanes-pack", "auto"), None);
        for (carrier, expected) in [("row", "normal"), ("column", "normal"), ("dense", "dense"), ("row dense", "dense"), ("column dense", "dense")] {
            assert_eq!(
                inline_style_cssom_authored_value("grid-lanes-pack", Some(carrier.to_owned())),
                Some(expected.to_owned())
            );
        }
    }

    #[test]
    fn supports_conditions_share_grid_lanes_declaration_projection() {
        for (condition, expected) in [
            ("(grid-lanes-pack: normal)", true),
            ("(grid-lanes-pack: dense)", true),
            ("not (grid-lanes-pack: dense)", false),
            ("(grid-lanes-pack: normal) and (display: grid)", true),
            ("(grid-lanes-pack: auto)", false),
            ("not (grid-lanes-pack: auto)", true),
            ("(font-family: \"grid-lanes-pack: dense\")", true),
            ("(my-grid-lanes-pack: dense)", false),
        ] {
            assert_eq!(supports(CssSupportsInput::condition(condition)), expected, "{condition}");
        }
    }

    #[test]
    fn supports_declaration_requires_a_property_name_not_declaration_syntax() {
        for property in [
            " color",
            "color ",
            "\tcolor",
            "color\n",
            "co/**/lor",
            "c\\6flor",
            "--name ",
            "--",
        ] {
            assert!(
                !supports(CssSupportsInput::declaration(property, "red")),
                "{property:?}"
            );
        }
        for property in ["color", "COLOR", "--name"] {
            assert!(
                supports(CssSupportsInput::declaration(property, "  red  ")),
                "{property}"
            );
        }
        assert!(supports(CssSupportsInput::condition("  color : red  ")));
    }

    #[test]
    fn supports_at_rule_import_conditions_use_supported_at_rule_names() {
        assert!(supports(CssSupportsInput::condition("at-rule(@import)")));
        assert!(supports(CssSupportsInput::condition(
            "at-rule(@media) or at-rule(@doesnotexist)"
        )));
        assert!(supports(CssSupportsInput::condition("at-rule(@swash)")));
        assert!(supports(CssSupportsInput::condition(
            "not at-rule(@unknown)"
        )));
        assert!(!supports(CssSupportsInput::condition(
            "at-rule(@doesnotexist)"
        )));
        assert!(!supports(CssSupportsInput::condition("at-rule(@charset)")));
        assert!(!supports(CssSupportsInput::condition(
            "at-rule(@counter-style; system: fixed)"
        )));
    }

    #[test]
    fn font_shorthand_accepts_script_specific_generics() {
        for (source, expected) in [
            (
                "generic(fangsong)",
                ScriptSpecificGenericFontFamily::Fangsong,
            ),
            ("generic(kai)", ScriptSpecificGenericFontFamily::Kai),
            (
                "generic(khmer-mul)",
                ScriptSpecificGenericFontFamily::KhmerMul,
            ),
            (
                "generic(nastaliq)",
                ScriptSpecificGenericFontFamily::Nastaliq,
            ),
        ] {
            let declarations = parse_font_shorthand(&format!("25px {source}"))
                .expect("the script-specific generic is valid in the font shorthand");
            let family = declarations
                .iter()
                .find_map(|declaration| match declaration {
                    PropertyDeclaration::FontFamily(FontFamily::Values(families)) => {
                        families.iter().next()
                    },
                    _ => None,
                });
            assert_eq!(
                family,
                Some(&SingleFontFamily::ScriptSpecificGeneric(expected))
            );
        }
    }
}
