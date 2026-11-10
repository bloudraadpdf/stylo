use stylo_cssom::{declaration_parser, specified};

#[test]
fn scale_calculations_retain_percentage_units() {
    for (source, expected) in [
        ("200%", "2"),
        ("100% 200% 300%", "1 2 3"),
        ("calc(200%)", "calc(200%)"),
        ("calc(200%) calc(3)", "calc(200%) calc(3)"),
        ("calc(2 * 1) 3 calc(4 * 100%)", "calc(2) 3 calc(400%)"),
        ("calc(-100%)", "calc(-100%)"),
        (
            "calc(200% * sign(1em - 1px))",
            "calc(200% * sign(1em - 1px))",
        ),
    ] {
        let declarations = declaration_parser::parse_inline_style_declarations(
            &format!("scale:{source}"),
            "about:blank".into(),
            selectors::matching::QuirksMode::NoQuirks,
        );
        assert_eq!(
            specified::projected_specified_property_value(&declarations, "scale").as_deref(),
            Some(expected),
            "{source}",
        );
    }
}

#[test]
fn scale_and_skew_axis_function_names_are_lowercase() {
    for (source, expected) in [
        ("scaleX(720%)", "scalex(7.2)"),
        ("scaleY(-85%)", "scaley(-0.85)"),
        ("scaleZ(25%)", "scalez(0.25)"),
        ("skewX(90deg)", "skewx(90deg)"),
        ("skewY(-90deg)", "skewy(-90deg)"),
    ] {
        let declarations = declaration_parser::parse_inline_style_declarations(
            &format!("transform:{source}"),
            "about:blank".into(),
            selectors::matching::QuirksMode::NoQuirks,
        );
        assert_eq!(
            specified::projected_specified_property_value(&declarations, "transform").as_deref(),
            Some(expected),
            "{source}",
        );
    }
}
