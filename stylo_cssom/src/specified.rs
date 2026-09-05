use crate::declaration_parser::compatibility::{
    CanonicalProperty, shorthand_members as shorthand_member_properties,
};
use std::collections::HashSet;
use std::sync::LazyLock;
use style::stylesheets::UrlExtraData;

pub fn project_inline_style_declaration(
    declaration: &stylo_cssom_model::SpecifiedDeclaration,
    url_data: &UrlExtraData,
) -> Vec<stylo_cssom_model::RuleDeclaration> {
    if let Some(source) = declaration
        .shorthand_source
        .filter(|source| source.has_pending_substitution())
        && let Some(value) = declaration.shorthand_value.as_ref()
    {
        let stylo_cssom_model::SpecifiedPropertyName::Standard(property) = &declaration.property
        else {
            unreachable!("a pending shorthand member must be a standard longhand");
        };
        return vec![
            stylo_cssom_model::RuleDeclaration::from_pending_substitution(
                property.schema().name,
                source.property(),
                projected_specified_style_value_text(value),
                url_data.as_str(),
            )
            .expect("a pending inline longhand must belong to its source shorthand")
            .with_importance(declaration.importance == stylo_cssom_model::Importance::Important),
        ];
    }
    project_inline_compatibility_declaration(declaration)
        .into_iter()
        .map(|(property, value, importance)| {
            stylo_cssom_model::RuleDeclaration::new(property, value)
                .with_importance(importance == stylo_cssom_model::Importance::Important)
        })
        .collect()
}

fn project_inline_compatibility_declaration(
    declaration: &stylo_cssom_model::SpecifiedDeclaration,
) -> Vec<(String, String, stylo_cssom_model::Importance)> {
    use stylo_cssom_model::{
        InlineCompatibilityProperty as Compat, SpecifiedPropertyName as Property,
    };

    let importance = declaration.importance;
    let one = |property: &str, value: String| vec![(property.to_owned(), value, importance)];
    let value = projected_specified_style_value_text(&declaration.value);
    match &declaration.property {
        Property::Compatibility(Compat::FlowTolerance) => {
            if value.eq_ignore_ascii_case("auto") {
                Vec::new()
            } else if value.eq_ignore_ascii_case("normal") {
                one("masonry-slack", "infinite".to_owned())
            } else if value.eq_ignore_ascii_case("infinite") {
                one("masonry-slack", "auto".to_owned())
            } else {
                one("masonry-slack", value)
            }
        },
        Property::Compatibility(Compat::GridLanesPack) => {
            if value.eq_ignore_ascii_case("normal") {
                one("grid-auto-flow", "row".to_owned())
            } else if value.eq_ignore_ascii_case("dense") {
                one("grid-auto-flow", value)
            } else {
                Vec::new()
            }
        },
        Property::Compatibility(Compat::Continue) => {
            project_continue_compatibility(&declaration.value, importance)
        },
        Property::Compatibility(Compat::LineClamp) => {
            project_line_clamp_compatibility(&declaration.value, importance)
        },
        Property::Compatibility(Compat::WebkitLineClamp) => {
            project_webkit_line_clamp_compatibility(&declaration.value, importance)
        },
        Property::Compatibility(Compat::LegacyTextAlign) => vec![
            ("text-align".to_owned(), value.clone(), importance),
            (
                crate::webkit_box_orient_rewrite::INTERNAL_LEGACY_TEXT_ALIGN_PROPERTY.to_owned(),
                value,
                importance,
            ),
        ],
        Property::Compatibility(Compat::WebkitBoxDisplay) => {
            let fallback = if value.eq_ignore_ascii_case("-webkit-box") {
                "flex"
            } else {
                "inline-flex"
            };
            vec![
                ("display".to_owned(), fallback.to_owned(), importance),
                (
                    crate::webkit_box_orient_rewrite::INTERNAL_DISPLAY_PROPERTY.to_owned(),
                    value,
                    importance,
                ),
            ]
        },
        Property::Standard(property) if property.schema().name == "display" => {
            let fallback = if value.eq_ignore_ascii_case("-webkit-box") {
                Some("flex")
            } else if value.eq_ignore_ascii_case("-webkit-inline-box") {
                Some("inline-flex")
            } else {
                None
            };
            if let Some(fallback) = fallback {
                vec![
                    ("display".to_owned(), fallback.to_owned(), importance),
                    (
                        crate::webkit_box_orient_rewrite::INTERNAL_DISPLAY_PROPERTY.to_owned(),
                        value,
                        importance,
                    ),
                ]
            } else {
                vec![
                    ("display".to_owned(), value.clone(), importance),
                    (
                        crate::webkit_box_orient_rewrite::INTERNAL_DISPLAY_PROPERTY.to_owned(),
                        value,
                        importance,
                    ),
                ]
            }
        },
        Property::Standard(property)
            if matches!(property.schema().name, "transition" | "transition-property") =>
        {
            one(
                property.schema().name,
                replace_projected_ident(
                    &declaration.value,
                    "overlay",
                    crate::overlay_transition_rewrite::INTERNAL_OVERLAY_TRANSITION_PROPERTY,
                ),
            )
        },
        Property::Standard(property) if property.schema().name == "position-visibility" => one(
            property.schema().name,
            replace_projected_idents(
                &declaration.value,
                &[
                    ("anchor-visible", "anchors-visible"),
                    ("anchor-valid", "anchors-valid"),
                ],
            ),
        ),
        Property::Standard(property)
            if matches!(property.schema().name, "text-align" | "text-align-all") =>
        {
            vec![
                (property.schema().name.to_owned(), value.clone(), importance),
                (
                    crate::webkit_box_orient_rewrite::INTERNAL_LEGACY_TEXT_ALIGN_PROPERTY
                        .to_owned(),
                    value,
                    importance,
                ),
            ]
        },
        Property::Standard(property) => one(property.schema().name, value),
        Property::Custom(property) => one(property, value),
    }
}

fn project_continue_compatibility(
    value: &stylo_cssom_model::SpecifiedStyleValue,
    importance: stylo_cssom_model::Importance,
) -> Vec<(String, String, stylo_cssom_model::Importance)> {
    let Some(keyword) = projected_single_ident(value) else {
        return Vec::new();
    };
    if !matches!(
        keyword.to_ascii_lowercase().as_str(),
        "auto"
            | "collapse"
            | "discard"
            | "inherit"
            | "initial"
            | "revert"
            | "revert-layer"
            | "unset"
    ) {
        return Vec::new();
    }
    let lowered = if keyword.eq_ignore_ascii_case("collapse") {
        "discard"
    } else {
        keyword
    };
    vec![
        ("continue".to_owned(), lowered.to_owned(), importance),
        (
            crate::webkit_box_orient_rewrite::INTERNAL_CONTINUE_PROPERTY.to_owned(),
            keyword.to_owned(),
            importance,
        ),
    ]
}

fn project_webkit_line_clamp_compatibility(
    value: &stylo_cssom_model::SpecifiedStyleValue,
    importance: stylo_cssom_model::Importance,
) -> Vec<(String, String, stylo_cssom_model::Importance)> {
    let valid = projected_single_ident(value)
        .is_some_and(|value| value.eq_ignore_ascii_case("none"))
        || projected_single_positive_integer(value).is_some();
    if !valid {
        return Vec::new();
    }
    let value = projected_specified_style_value_text(value);
    vec![
        ("-webkit-line-clamp".to_owned(), value.clone(), importance),
        ("max-lines".to_owned(), value, importance),
        ("continue".to_owned(), "auto".to_owned(), importance),
        ("block-ellipsis".to_owned(), "auto".to_owned(), importance),
        (
            crate::webkit_box_orient_rewrite::INTERNAL_CONTINUE_PROPERTY.to_owned(),
            "auto".to_owned(),
            importance,
        ),
    ]
}

fn project_line_clamp_compatibility(
    value: &stylo_cssom_model::SpecifiedStyleValue,
    importance: stylo_cssom_model::Importance,
) -> Vec<(String, String, stylo_cssom_model::Importance)> {
    use stylo_cssom_model::SpecifiedComponentValue as Component;

    let Some(components) = projected_components(value) else {
        return Vec::new();
    };
    let authored = projected_specified_style_value_text(value);
    let output = |property: &str, value: &str| (property.to_owned(), value.to_owned(), importance);
    let marker = |value: &str| {
        (
            crate::webkit_box_orient_rewrite::INTERNAL_CONTINUE_PROPERTY.to_owned(),
            value.to_owned(),
            importance,
        )
    };
    match components {
        [Component::Ident(value)] if value.eq_ignore_ascii_case("none") => {
            vec![output("line-clamp", &authored), marker("auto")]
        },
        [Component::Ident(value)] if value.eq_ignore_ascii_case("auto") => vec![
            output("max-lines", "none"),
            output("continue", "discard"),
            output("block-ellipsis", "auto"),
            marker("collapse"),
        ],
        [Component::Ident(value), Component::Ident(ellipsis)]
            if value.eq_ignore_ascii_case("auto")
                && ellipsis.eq_ignore_ascii_case("no-ellipsis") =>
        {
            vec![
                output("max-lines", "none"),
                output("continue", "discard"),
                output("block-ellipsis", "none"),
                marker("collapse"),
            ]
        },
        [Component::String(marker_value)] => vec![
            output("max-lines", "none"),
            output("continue", "discard"),
            output("block-ellipsis", &serialize_css_string(marker_value)),
            marker("collapse"),
        ],
        [Component::Number { value: lines, .. }] if positive_integer(*lines) => {
            vec![output("line-clamp", &authored), marker("collapse")]
        },
        [
            Component::Number { value: lines, .. },
            Component::Ident(ellipsis),
        ] if positive_integer(*lines) && ellipsis.eq_ignore_ascii_case("no-ellipsis") => {
            vec![
                output("max-lines", &lines.to_string()),
                output("continue", "discard"),
                output("block-ellipsis", "none"),
                marker("collapse"),
            ]
        },
        [
            Component::Number { value: lines, .. },
            Component::Ident(ellipsis),
        ] if positive_integer(*lines)
            && matches!(ellipsis.to_ascii_lowercase().as_str(), "none" | "auto") =>
        {
            vec![output("line-clamp", &authored), marker("collapse")]
        },
        [Component::Number { value: lines, .. }, Component::String(_)]
            if positive_integer(*lines) =>
        {
            vec![output("line-clamp", &authored), marker("collapse")]
        },
        _ => Vec::new(),
    }
}

fn projected_components(
    value: &stylo_cssom_model::SpecifiedStyleValue,
) -> Option<&[stylo_cssom_model::SpecifiedComponentValue]> {
    let stylo_cssom_model::SpecifiedStyleValue::Components(components) = value else {
        return None;
    };
    Some(components)
}

fn projected_single_ident(value: &stylo_cssom_model::SpecifiedStyleValue) -> Option<&str> {
    if let stylo_cssom_model::SpecifiedStyleValue::CssWide(keyword) = value {
        return Some(crate::declaration_parser::compatibility::css_wide_keyword_text(*keyword));
    }
    let [stylo_cssom_model::SpecifiedComponentValue::Ident(value)] = projected_components(value)?
    else {
        return None;
    };
    Some(value)
}

fn projected_single_positive_integer(
    value: &stylo_cssom_model::SpecifiedStyleValue,
) -> Option<f32> {
    let [stylo_cssom_model::SpecifiedComponentValue::Number { value, .. }] =
        projected_components(value)?
    else {
        return None;
    };
    positive_integer(*value).then_some(*value)
}

fn positive_integer(value: f32) -> bool {
    value >= 1.0 && value.fract() == 0.0
}

fn serialize_css_string(value: &str) -> String {
    let mut serialized = String::new();
    cssparser::serialize_string(value, &mut serialized)
        .expect("writing CSS to a string is infallible");
    serialized
}

#[cfg(test)]
mod inline_compatibility_projection_tests {
    use super::{
        project_inline_compatibility_declaration, project_inline_style_declaration,
        projected_specified_property_value, serialize_specified_declarations,
    };

    #[test]
    fn all_shorthand_serialisation_does_not_repeat_completeness_scans() {
        let declarations = crate::declaration_parser::parse_inline_style_property_declarations(
            "all",
            "initial",
            crate::declaration_parser::CssomDeclarationPriority::Normal,
            &"about:blank".into(),
        )
        .expect("the all shorthand must parse");
        let started = std::time::Instant::now();

        assert_eq!(
            serialize_specified_declarations(&declarations),
            "all: initial;"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "serialising one all shorthand took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn indexed_shorthand_members_preserve_value_and_priority_boundaries() {
        for (css, property, expected) in [
            ("margin:1px 2px", "margin", Some("1px 2px")),
            (
                "margin-top:1px;margin-right:2px;margin-bottom:1px;margin-left:2px",
                "margin",
                Some("1px 2px"),
            ),
            ("margin:initial;margin-top:inherit", "margin", None),
            ("margin:1px;margin-top:2px!important", "margin", None),
            ("margin:1px!important", "margin", Some("1px")),
            (
                "margin-top:1px;margin-right:1px;margin-bottom:1px",
                "margin",
                None,
            ),
            ("margin:var(--space)", "margin", Some("var(--space)")),
            ("margin:var(--space);margin-top:2px", "margin", None),
            (
                "container:initial;container-type:inline-size",
                "container",
                None,
            ),
            (
                "position-try:initial;position-try-order:normal",
                "position-try",
                None,
            ),
        ] {
            let declarations = crate::declaration_parser::parse_inline_style_declarations(
                css,
                "about:blank".into(),
            );
            assert_eq!(
                projected_specified_property_value(&declarations, property).as_deref(),
                expected,
                "{css}"
            );
        }
    }

    #[test]
    fn indexed_shorthand_members_preserve_canonical_schema_and_observation_order() {
        for schema in stylo_cssom_model::STANDARD_PROPERTIES {
            let names = super::shorthand_member_properties(schema)
                .iter()
                .map(|property| match property {
                    super::CanonicalProperty::Native(property) => property.schema().name,
                })
                .collect::<Vec<_>>();
            let expected = schema.shorthand_expansion.to_vec();
            assert_eq!(names, expected, "{}", schema.name);
        }

        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "margin-left:4px;margin-bottom:3px;margin-right:2px;margin-top:1px;--margin-top:9px",
            "about:blank".into(),
        );
        let schema = stylo_cssom_model::property_schema("margin").unwrap();
        let members = super::SpecifiedDeclarationIndex::new(&declarations)
            .members(schema)
            .expect("all canonical margin members are present");
        assert_eq!(members.indices, [3, 2, 1, 0]);
        assert_eq!(
            projected_specified_property_value(&declarations, "margin").as_deref(),
            Some("1px 2px 3px 4px")
        );
        assert_eq!(
            serialize_specified_declarations(&declarations),
            "margin: 1px 2px 3px 4px; --margin-top: 9px;"
        );
        assert_eq!(
            projected_specified_property_value(&declarations, "--margin-top").as_deref(),
            Some("9px")
        );
        assert!(matches!(
            declarations[0].property,
            stylo_cssom_model::SpecifiedPropertyName::Standard(property) if property.schema().name == "margin-left"
        ));
    }

    fn projected_declarations(
        declarations: &[stylo_cssom_model::SpecifiedDeclaration],
    ) -> Vec<stylo_cssom_model::RuleDeclaration> {
        let url_data = crate::context::ABOUT_BLANK.clone().into();
        declarations
            .iter()
            .flat_map(|declaration| project_inline_style_declaration(declaration, &url_data))
            .collect()
    }

    #[test]
    fn projected_shorthands_resolve_aliases_and_incomplete_source_shorthands() {
        let declarations = crate::declaration_parser::parse_inline_style_property_declarations(
            "-webkit-border-radius",
            "1px",
            crate::declaration_parser::CssomDeclarationPriority::Normal,
            &"about:blank".into(),
        )
        .expect("the supported alias must parse");
        assert_eq!(
            projected_specified_property_value(&declarations, "-webkit-border-radius").as_deref(),
            Some("1px")
        );

        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "grid: 30px 40px / 50px 60px; grid-auto-flow: column",
            "about:blank".into(),
        );
        assert_eq!(
            projected_specified_property_value(&declarations, "grid-template").as_deref(),
            Some("30px 40px / 50px 60px")
        );

        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "grid: auto-flow / 10px; grid-template-rows: 20px",
            "about:blank".into(),
        );
        assert_eq!(
            projected_specified_property_value(&declarations, "grid-template").as_deref(),
            Some("20px / 10px")
        );
        assert_eq!(
            serialize_specified_declarations(&declarations),
            "grid: 20px / 10px;"
        );

        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            concat!(
                "grid-auto-rows:auto;grid-auto-columns:auto;grid-auto-flow:row;",
                "grid-template:20px / 10px",
            ),
            "about:blank".into(),
        );
        assert_eq!(
            serialize_specified_declarations(&declarations),
            "grid: 20px / 10px;"
        );

        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "grid: 1px / 2px; grid-auto-flow: row",
            "about:blank".into(),
        );
        assert_eq!(
            projected_specified_property_value(&declarations, "grid").as_deref(),
            Some("1px / 2px")
        );
    }

    #[test]
    fn standard_declaration_projection_preserves_canonical_numbers() {
        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "font: normal normal xx-large/1.2 cursive",
            "about:blank".into(),
        );

        assert_eq!(
            serialize_specified_declarations(&declarations),
            "font: xx-large / 1.2 cursive;"
        );
    }

    #[test]
    fn standard_declaration_projection_uses_property_aware_colour_serialisation() {
        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "color: rgb(from rgb(20% 40% 60%) r g b)",
            "about:blank".into(),
        );

        assert_eq!(
            serialize_specified_declarations(&declarations),
            "color: rgb(from rgb(51, 102, 153) r g b);"
        );

        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "color: rgb(from var(--color) calc(r * .3 + g * .59 + b * .11) r g)",
            "about:blank".into(),
        );
        assert_eq!(
            serialize_specified_declarations(&declarations),
            "color: rgb(from var(--color) calc(r * .3 + g * .59 + b * .11) r g);"
        );
    }

    #[test]
    fn unresolved_variable_shorthands_survive_typed_projection() {
        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "background: var(--colour, lightgreen); --colour: initial",
            "about:blank".into(),
        );

        assert_eq!(
            serialize_specified_declarations(&declarations),
            "background: var(--colour, lightgreen); --colour: initial;"
        );
        let projected = projected_declarations(&declarations);
        assert!(projected.iter().any(|declaration| {
            declaration.name() == "background-color"
                && declaration
                    .pending_substitution()
                    .is_some_and(|pending| pending.tokens() == "var(--colour, lightgreen)")
        }));

        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "--size: 8px; margin: var(--size); margin-top: 10px",
            "about:blank".into(),
        );
        assert_eq!(
            serialize_specified_declarations(&declarations),
            "--size: 8px; margin-right: ; margin-bottom: ; margin-left: ; margin-top: 10px;"
        );
        let projected = projected_declarations(&declarations);
        assert!(projected.iter().any(|declaration| {
            declaration.name() == "margin-right"
                && declaration
                    .pending_substitution()
                    .is_some_and(|pending| pending.tokens() == "var(--size)")
        }));
        assert!(
            projected
                .iter()
                .any(|declaration| declaration.name() == "margin-top"
                    && declaration.value() == "10px")
        );
    }

    #[test]
    fn partial_pending_shorthands_use_empty_longhand_cssom_serialization() {
        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "margin:var(--size); margin-top:10px",
            "about:blank".into(),
        );
        let css = serialize_specified_declarations(&declarations);
        assert!(!css.contains("margin:"), "{css}");
        assert!(!css.contains("var(--size)"), "{css}");
        for property in ["margin-right", "margin-bottom", "margin-left"] {
            assert!(css.contains(&format!("{property}: ;")), "{css}");
        }
        assert!(css.contains("margin-top: 10px;"), "{css}");
    }

    #[test]
    fn custom_property_projection_preserves_token_boundaries() {
        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "--comment: a/* comment */b; --url: foo url(bar)",
            "https://example.test/styles/".into(),
        );

        assert_eq!(
            serialize_specified_declarations(&declarations),
            "--comment: a/* comment */b; --url: foo url(bar);"
        );
    }

    #[test]
    fn custom_property_css_text_serializes_decoded_names_as_identifiers() {
        for (name, source) in [
            ("--a;b", r"--a\;b: value;"),
            (r"--\", r"--\\: value;"),
            ("--A", "--A: value;"),
            ("--a", "--a: value;"),
            ("--0", "--0: value;"),
        ] {
            let declarations = crate::declaration_parser::parse_inline_style_declarations(
                source,
                "about:blank".into(),
            );
            assert_eq!(declarations.len(), 1, "{source}");
            assert_eq!(
                declarations[0].property,
                stylo_cssom_model::SpecifiedPropertyName::Custom(name.into())
            );
            let serialized = serialize_specified_declarations(&declarations);
            assert_eq!(serialized, source, "{name}");
            let reparsed = crate::declaration_parser::parse_inline_style_declarations(
                &serialized,
                "about:blank".into(),
            );
            assert_eq!(reparsed.len(), 1, "{name}");
            assert_eq!(reparsed[0].property, declarations[0].property);
            assert_eq!(
                projected_specified_property_value(&reparsed, name),
                Some("value".to_owned())
            );
        }
    }

    #[test]
    fn legacy_page_break_shorthands_project_as_canonical_longhands() {
        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            "page-break-before:always;page-break-after:right;page-break-inside:avoid",
            "about:blank".into(),
        );

        assert_eq!(
            serialize_specified_declarations(&declarations),
            "break-before: page; break-after: right; break-inside: avoid;"
        );
    }

    #[test]
    fn independent_gap_rule_longhands_serialise_as_available_shorthands() {
        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            concat!(
                "column-rule-style:solid;column-rule-width:10px;column-rule-color:pink;",
                "row-rule-style:solid;row-rule-width:10px;row-rule-color:green",
            ),
            "about:blank".into(),
        );

        assert_eq!(
            serialize_specified_declarations(&declarations),
            "column-rule: 10px solid pink; row-rule: 10px solid green;"
        );
    }

    #[test]
    fn typed_webkit_declarations_lower_without_rewriting_an_inline_css_string() {
        let declarations = crate::declaration_parser::parse_inline_style_declarations(
            concat!(
                "line-clamp:auto no-ellipsis!important;",
                "-webkit-line-clamp:4;continue:collapse;",
                "text-align:-webkit-right;display:-webkit-box",
            ),
            "about:blank".into(),
        );
        let projected = declarations
            .iter()
            .flat_map(project_inline_compatibility_declaration)
            .collect::<Vec<_>>();

        for expected in [
            ("max-lines", "none"),
            ("block-ellipsis", "none"),
            ("-webkit-line-clamp", "4"),
            ("continue", "discard"),
            ("--moegoe-continue", "collapse"),
            ("--moegoe-legacy-text-align", "-moz-right"),
            ("--moegoe-webkit-box-display", "-webkit-box"),
        ] {
            assert!(
                projected
                    .iter()
                    .any(|(property, value, _)| property == expected.0 && value == expected.1),
                "missing typed projection {expected:?}: {projected:?}",
            );
        }
        assert!(projected.iter().any(|(property, _, importance)| {
            property == "max-lines" && *importance == stylo_cssom_model::Importance::Important
        }));
    }
}

fn replace_projected_ident(
    value: &stylo_cssom_model::SpecifiedStyleValue,
    source: &str,
    replacement: &str,
) -> String {
    replace_projected_idents(value, &[(source, replacement)])
}

fn replace_projected_idents(
    value: &stylo_cssom_model::SpecifiedStyleValue,
    replacements: &[(&str, &str)],
) -> String {
    let mut value = value.clone();
    rewrite_projected_component_idents(&mut value, replacements);
    projected_specified_style_value_text(&value)
}

fn rewrite_projected_component_idents(
    value: &mut stylo_cssom_model::SpecifiedStyleValue,
    replacements: &[(&str, &str)],
) {
    use stylo_cssom_model::{SpecifiedComponentValue as Component, SpecifiedStyleValue as Value};

    fn rewrite_components(values: &mut [Component], replacements: &[(&str, &str)]) {
        for component in values {
            match component {
                Component::Ident(value) => {
                    if let Some((_, replacement)) = replacements
                        .iter()
                        .find(|(source, _)| value.eq_ignore_ascii_case(source))
                    {
                        *value = (*replacement).into();
                    }
                },
                Component::Function { arguments, .. } => {
                    rewrite_components(arguments, replacements)
                },
                Component::Block { values, .. } => rewrite_components(values, replacements),
                _ => {},
            }
        }
    }

    match value {
        Value::Components(values) => rewrite_components(values, replacements),
        Value::List { values, .. } => {
            for value in values {
                rewrite_projected_component_idents(value, replacements);
            }
        },
        Value::CssWide(_) | Value::Opacity(_) | Value::TokenStream(_) => {},
    }
}

pub fn serialize_specified_declarations(
    declarations: &[stylo_cssom_model::SpecifiedDeclaration],
) -> String {
    let mut css = String::new();
    let mut emitted_shorthands = Vec::new();
    let index = SpecifiedDeclarationIndex::new(declarations);
    let shorthand_completeness = shorthand_completeness(&index);
    let synthesised_shorthands =
        synthesised_independent_shorthands(&index, &shorthand_completeness);
    let synthesised_members = synthesised_shorthands
        .iter()
        .flat_map(|shorthand| shorthand.members.iter().copied())
        .collect::<HashSet<_>>();
    for (index, declaration) in declarations.iter().enumerate() {
        if let Some(shorthand) = synthesised_shorthands
            .iter()
            .find(|shorthand| shorthand.first == index)
        {
            if !css.is_empty() {
                css.push(' ');
            }
            css.push_str(shorthand.schema.name);
            css.push_str(": ");
            css.push_str(&shorthand.value);
            if declaration.importance == stylo_cssom_model::Importance::Important {
                css.push_str(" !important");
            }
            css.push(';');
            continue;
        }
        if synthesised_members.contains(&index) {
            continue;
        }
        let shorthand = declaration
            .shorthand_source
            .map(stylo_cssom_model::SpecifiedShorthandSource::property)
            .filter(|shorthand| !emitted_shorthands.contains(shorthand))
            .filter(|_| shorthand_completeness[index]);
        if declaration.shorthand_source.is_some() && shorthand.is_none() {
            if declaration
                .shorthand_source
                .is_some_and(|source| emitted_shorthands.contains(&source.property()))
            {
                continue;
            }
        }
        if !css.is_empty() {
            css.push(' ');
        }
        if let Some(shorthand) = shorthand {
            css.push_str(shorthand.schema().name);
            emitted_shorthands.push(shorthand);
        } else {
            match &declaration.property {
                stylo_cssom_model::SpecifiedPropertyName::Standard(property) => {
                    css.push_str(property.schema().name)
                },
                stylo_cssom_model::SpecifiedPropertyName::Custom(property) => {
                    cssparser::serialize_identifier(property, &mut css)
                        .expect("writing CSS to a string is infallible");
                },
                stylo_cssom_model::SpecifiedPropertyName::Compatibility(property) => {
                    css.push_str(property.css_name())
                },
            }
        }
        css.push_str(": ");
        if shorthand.is_some()
            || !declaration
                .shorthand_source
                .is_some_and(stylo_cssom_model::SpecifiedShorthandSource::has_pending_substitution)
        {
            write_specified_style_value(
                shorthand
                    .and_then(|_| declaration.shorthand_value.as_ref())
                    .unwrap_or(&declaration.value),
                &mut css,
            );
        }
        if declaration.importance == stylo_cssom_model::Importance::Important {
            css.push_str(" !important");
        }
        css.push(';');
    }
    css
}

struct SynthesisedShorthand {
    schema: &'static stylo_cssom_model::PropertySchemaRow,
    first: usize,
    members: Vec<usize>,
    value: String,
}

struct SpecifiedDeclarationIndex<'a> {
    declarations: &'a [stylo_cssom_model::SpecifiedDeclaration],
    standard: Box<[Option<usize>]>,
}

impl<'a> SpecifiedDeclarationIndex<'a> {
    fn new(declarations: &'a [stylo_cssom_model::SpecifiedDeclaration]) -> Self {
        let mut standard =
            vec![None; stylo_cssom_model::STANDARD_PROPERTIES.len()].into_boxed_slice();
        for (index, declaration) in declarations.iter().enumerate() {
            match CanonicalProperty::from_specified(&declaration.property) {
                Some(CanonicalProperty::Native(property)) => {
                    standard[property.index()].get_or_insert(index);
                },
                None => {},
            }
        }
        Self {
            declarations,
            standard,
        }
    }

    fn members(
        &self,
        schema: &'static stylo_cssom_model::PropertySchemaRow,
    ) -> Option<ShorthandMembers<'a>> {
        let indices = shorthand_member_properties(schema)
            .iter()
            .map(|property| match property {
                CanonicalProperty::Native(property) => self.standard[property.index()],
            })
            .collect::<Option<Vec<_>>>()?;
        Some(ShorthandMembers {
            schema,
            declarations: self.declarations,
            indices,
        })
    }
}

struct ShorthandMembers<'a> {
    schema: &'static stylo_cssom_model::PropertySchemaRow,
    declarations: &'a [stylo_cssom_model::SpecifiedDeclaration],
    indices: Vec<usize>,
}

impl<'a> ShorthandMembers<'a> {
    fn serialize(&self, fallback: impl FnOnce() -> Option<String>) -> Option<String> {
        self.importance()?;
        if let Some(value) = self.source_value() {
            return Some(projected_specified_style_value_text(value));
        }
        let wide = self
            .declarations()
            .find_map(|declaration| match declaration.value {
                stylo_cssom_model::SpecifiedStyleValue::CssWide(keyword) => Some(keyword),
                _ => None,
            });
        if let Some(keyword) = wide {
            let value = stylo_cssom_model::SpecifiedStyleValue::CssWide(keyword);
            return self
                .declarations()
                .all(|declaration| declaration.value == value)
                .then(|| projected_specified_style_value_text(&value));
        }
        if crate::declaration_parser::compatibility::is_all(self.schema) {
            return None;
        }
        fallback()
    }

    fn declarations(
        &self,
    ) -> impl Iterator<Item = &'a stylo_cssom_model::SpecifiedDeclaration> + '_ {
        let declarations = self.declarations;
        self.indices.iter().map(move |&index| &declarations[index])
    }

    fn importance(&self) -> Option<stylo_cssom_model::Importance> {
        let importance = self.declarations[*self.indices.first()?].importance;
        self.declarations()
            .all(|declaration| declaration.importance == importance)
            .then_some(importance)
    }

    fn source_value(&self) -> Option<&'a stylo_cssom_model::SpecifiedStyleValue> {
        let first = &self.declarations[*self.indices.first()?];
        let value = first.shorthand_value.as_ref()?;
        self.declarations()
            .all(|declaration| {
                declaration
                    .shorthand_source
                    .is_some_and(|source| source.property() == self.schema.id)
                    && declaration.shorthand_value.as_ref() == Some(value)
                    && declaration.importance == first.importance
            })
            .then_some(value)
    }
}

fn standard_inline_style_block(
    declarations: &[stylo_cssom_model::SpecifiedDeclaration],
) -> crate::declaration_parser::InlineStyleBlock {
    let url_data: UrlExtraData = crate::context::ABOUT_BLANK.clone().into();
    let projected = declarations
        .iter()
        .flat_map(|declaration| project_inline_style_declaration(declaration, &url_data));
    crate::declaration_parser::stylo_inline_style_block(projected, &url_data)
}

fn synthesised_independent_shorthands(
    index: &SpecifiedDeclarationIndex<'_>,
    shorthand_completeness: &[bool],
) -> Vec<SynthesisedShorthand> {
    let declarations = index.declarations;
    let block = LazyLock::new(|| standard_inline_style_block(declarations));
    let mut shorthands = Vec::new();

    for schema in (0..).map_while(stylo_cssom_model::property_schema_at) {
        if schema.kind != stylo_cssom_model::PropertyKind::Shorthand
            || schema.shorthand_expansion.len() < 2
        {
            continue;
        }
        let Some(members) = index.members(schema) else {
            continue;
        };
        if members.importance().is_none()
            || members.indices.iter().any(|&member| {
                !declaration_is_available_to_synthesised_shorthand(
                    &declarations[member],
                    shorthand_completeness[member],
                    schema,
                )
            })
        {
            continue;
        }
        let mut ordered_members = members.indices.clone();
        ordered_members.sort_unstable();
        if ordered_members
            .windows(2)
            .any(|pair| pair[1] != pair[0] + 1)
        {
            continue;
        }
        let Some(value) = members.serialize(|| {
            crate::declaration_parser::inline_style_get_property_value(&block, schema.name)
        }) else {
            continue;
        };
        let first = ordered_members[0];
        shorthands.push(SynthesisedShorthand {
            schema,
            first,
            members: ordered_members,
            value,
        });
    }
    shorthands.sort_by(|left, right| right.members.len().cmp(&left.members.len()));
    let mut reserved = HashSet::new();
    shorthands.retain(|shorthand| {
        if shorthand
            .members
            .iter()
            .any(|member| reserved.contains(member))
        {
            return false;
        }
        reserved.extend(shorthand.members.iter().copied());
        true
    });
    shorthands.sort_by_key(|shorthand| shorthand.first);
    shorthands
}

fn declaration_is_available_to_synthesised_shorthand(
    declaration: &stylo_cssom_model::SpecifiedDeclaration,
    source_is_complete: bool,
    candidate: &'static stylo_cssom_model::PropertySchemaRow,
) -> bool {
    let Some(source) = declaration.shorthand_source else {
        return true;
    };
    let source = source.property().schema();
    if !source_is_complete {
        return true;
    }
    let source = shorthand_member_properties(source);
    let candidate = shorthand_member_properties(candidate);
    source.len() < candidate.len() && source.iter().all(|member| candidate.contains(member))
}

fn shorthand_completeness(index: &SpecifiedDeclarationIndex<'_>) -> Vec<bool> {
    struct Group<'a> {
        property: stylo_cssom_model::StandardPropertyId,
        value: &'a stylo_cssom_model::SpecifiedStyleValue,
        importance: stylo_cssom_model::Importance,
        complete: bool,
    }

    let mut groups = Vec::<Group<'_>>::new();
    index
        .declarations
        .iter()
        .map(|declaration| {
            let Some((property, value)) = declaration
                .shorthand_source
                .map(stylo_cssom_model::SpecifiedShorthandSource::property)
                .zip(declaration.shorthand_value.as_ref())
            else {
                return false;
            };
            if let Some(group) = groups.iter().find(|group| {
                group.property == property
                    && group.value == value
                    && group.importance == declaration.importance
            }) {
                return group.complete;
            }
            let complete = index.members(property.schema()).is_some_and(|members| {
                members.source_value() == Some(value)
                    && members.importance() == Some(declaration.importance)
            });
            groups.push(Group {
                property,
                value,
                importance: declaration.importance,
                complete,
            });
            complete
        })
        .collect()
}

pub fn projected_inline_style_property_value(
    projection: &stylo_cssom_model::InlineStyleProjection,
    property_name: &str,
) -> Option<crate::value_serialization::ResolvedValueSerialization> {
    projected_specified_property_value(projection.declarations(), property_name)
        .map(crate::value_serialization::ResolvedValueSerialization::new)
}

pub fn projected_specified_property_value(
    declarations: &[stylo_cssom_model::SpecifiedDeclaration],
    property_name: &str,
) -> Option<String> {
    if property_name.starts_with("--") {
        let property = stylo_cssom_model::SpecifiedPropertyName::Custom(property_name.into());
        let declaration = declarations
            .iter()
            .rev()
            .find(|declaration| declaration.property == property)?;
        return Some(projected_specified_style_value_text(&declaration.value));
    }
    let schema = crate::declaration_parser::inline_style_cssom_property_schema(
        &property_name.to_ascii_lowercase(),
    )?;
    if schema.kind == stylo_cssom_model::PropertyKind::Shorthand {
        let members = SpecifiedDeclarationIndex::new(declarations).members(schema)?;
        return members.serialize(|| {
            let block = standard_inline_style_block(declarations);
            crate::declaration_parser::inline_style_get_property_value(&block, schema.name)
        });
    }
    let property = CanonicalProperty::Native(schema.id);
    let declaration = declarations.iter().rev().find(|declaration| {
        CanonicalProperty::from_specified(&declaration.property) == Some(property)
    })?;
    if declaration
        .shorthand_source
        .is_some_and(stylo_cssom_model::SpecifiedShorthandSource::has_pending_substitution)
    {
        return None;
    }
    if let stylo_cssom_model::SpecifiedPropertyName::Compatibility(authored) = declaration.property
        && !authored.css_name().eq_ignore_ascii_case(property_name)
    {
        let block = standard_inline_style_block(declarations);
        return crate::declaration_parser::inline_style_get_property_value(&block, schema.name);
    }
    Some(projected_specified_style_value_text(&declaration.value))
}

pub fn projected_specified_property_importance(
    declarations: &[stylo_cssom_model::SpecifiedDeclaration],
    property: &str,
) -> Option<stylo_cssom_model::Importance> {
    if property.starts_with("--") {
        return declarations.iter().rev().find_map(|declaration| {
            matches!(&declaration.property, stylo_cssom_model::SpecifiedPropertyName::Custom(name) if name.as_ref() == property)
                .then_some(declaration.importance)
        });
    }
    let property = CanonicalProperty::from_name(property)?;
    if let CanonicalProperty::Native(property) = property
        && property.schema().kind == stylo_cssom_model::PropertyKind::Shorthand
    {
        return SpecifiedDeclarationIndex::new(declarations)
            .members(property.schema())?
            .importance();
    }
    declarations.iter().rev().find_map(|declaration| {
        (CanonicalProperty::from_specified(&declaration.property) == Some(property))
            .then_some(declaration.importance)
    })
}

#[must_use]
pub fn serialize_projected_specified_style_value(
    value: &stylo_cssom_model::SpecifiedStyleValue,
) -> crate::value_serialization::ResolvedValueSerialization {
    crate::value_serialization::ResolvedValueSerialization::new(
        projected_specified_style_value_text(value),
    )
}

fn projected_specified_style_value_text(value: &stylo_cssom_model::SpecifiedStyleValue) -> String {
    let mut css = String::new();
    write_specified_style_value(value, &mut css);
    css
}

fn write_specified_style_value(value: &stylo_cssom_model::SpecifiedStyleValue, css: &mut String) {
    match value {
        stylo_cssom_model::SpecifiedStyleValue::CssWide(keyword) => {
            css.push_str(crate::declaration_parser::compatibility::css_wide_keyword_text(*keyword));
        },
        stylo_cssom_model::SpecifiedStyleValue::Opacity(opacity) => {
            css.push_str(&opacity.value().to_string())
        },
        stylo_cssom_model::SpecifiedStyleValue::TokenStream(value) => css.push_str(value),
        stylo_cssom_model::SpecifiedStyleValue::Components(values) => {
            serialize_specified_components(values, css)
        },
        stylo_cssom_model::SpecifiedStyleValue::List { separator, values } => {
            let separator = match separator {
                stylo_cssom_model::ListSeparator::Space => " ",
                stylo_cssom_model::ListSeparator::Comma => ", ",
                stylo_cssom_model::ListSeparator::Slash => " / ",
            };
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    css.push_str(separator);
                }
                write_specified_style_value(value, css);
            }
        },
    }
}

fn serialize_specified_components(
    values: &[stylo_cssom_model::SpecifiedComponentValue],
    css: &mut String,
) {
    for (index, value) in values.iter().enumerate() {
        if index != 0
            && !matches!(
                value,
                stylo_cssom_model::SpecifiedComponentValue::Delimiter(',')
            )
        {
            css.push(' ');
        }
        match value {
            stylo_cssom_model::SpecifiedComponentValue::Ident(value) => {
                cssparser::serialize_identifier(value, css)
                    .expect("writing CSS to a string is infallible");
            },
            stylo_cssom_model::SpecifiedComponentValue::AtKeyword(value) => {
                css.push('@');
                cssparser::serialize_identifier(value, css)
                    .expect("writing CSS to a string is infallible");
            },
            stylo_cssom_model::SpecifiedComponentValue::Hash { value, .. } => {
                css.push('#');
                cssparser::serialize_identifier(value, css)
                    .expect("writing CSS to a string is infallible");
            },
            stylo_cssom_model::SpecifiedComponentValue::Number { serialization, .. }
            | stylo_cssom_model::SpecifiedComponentValue::Percentage { serialization, .. }
            | stylo_cssom_model::SpecifiedComponentValue::Dimension { serialization, .. } => {
                css.push_str(serialization);
            },
            stylo_cssom_model::SpecifiedComponentValue::String(value) => {
                cssparser::serialize_string(value, css)
                    .expect("writing CSS to a string is infallible");
            },
            stylo_cssom_model::SpecifiedComponentValue::Url { value, .. } => {
                css.push_str("url(");
                cssparser::serialize_string(value, css)
                    .expect("writing CSS to a string is infallible");
                css.push(')');
            },
            stylo_cssom_model::SpecifiedComponentValue::Function { name, arguments } => {
                cssparser::serialize_identifier(name, css)
                    .expect("writing CSS to a string is infallible");
                css.push('(');
                serialize_specified_components(arguments, css);
                css.push(')');
            },
            stylo_cssom_model::SpecifiedComponentValue::Block { opening, values } => {
                css.push(*opening);
                serialize_specified_components(values, css);
                css.push(match opening {
                    '(' => ')',
                    '[' => ']',
                    '{' => '}',
                    _ => unreachable!("CSS blocks have a known opening delimiter"),
                });
            },
            stylo_cssom_model::SpecifiedComponentValue::Delimiter(value) => css.push(*value),
            stylo_cssom_model::SpecifiedComponentValue::Operator(value) => css.push_str(value),
        }
    }
}
