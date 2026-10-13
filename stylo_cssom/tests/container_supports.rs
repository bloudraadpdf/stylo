use stylo_cssom::values::{CssSupportsQuery, css_supports};

#[test]
fn container_supports_uses_the_existing_shorthand_parser() {
    for value in [
        "initial",
        "inherit",
        "unset",
        "none",
        "none / inline-size",
        "none / size",
        "inline-size / inline-size",
        "block-size / size",
        "foo / inline-size",
        "foo/inline-size",
        "FoO / size",
        "foo bar / size",
        "foo / normal",
    ] {
        assert!(
            css_supports(CssSupportsQuery::Declaration {
                property: "container",
                value,
            }),
            "{value}"
        );
        for condition in [
            format!("container: {value}"),
            format!("(container: {value})"),
            format!("(display: block) and (container: {value})"),
            format!("not (container: {value} / invalid)"),
        ] {
            assert!(
                css_supports(CssSupportsQuery::Condition(&condition)),
                "{condition}"
            );
        }
    }
    for value in [
        "",
        "/ size",
        "foo /",
        "foo / block-size",
        "foo / size normal",
    ] {
        assert!(
            !css_supports(CssSupportsQuery::Declaration {
                property: "container",
                value,
            }),
            "{value}"
        );
        for condition in [
            format!("container: {value}"),
            format!("(container: {value})"),
            format!("(display: block) and (container: {value})"),
        ] {
            assert!(
                !css_supports(CssSupportsQuery::Condition(&condition)),
                "{condition}"
            );
        }
    }
}

#[test]
fn adapter_support_conditions_keep_native_boolean_and_token_rules() {
    for condition in [
        "(container: foo / size !important)",
        "(container: foo / size) or (display: invalid)",
        "not (container: foo / block-size)",
        "(position-try: most-width --fallback)",
    ] {
        assert!(
            css_supports(CssSupportsQuery::Condition(condition)),
            "{condition}"
        );
    }
    for condition in [
        "(container: foo / size !important trailing)",
        "(container: foo / size;)",
        "(container: foo / size) and (display: invalid)",
        "(container: foo / size) and",
        "not (container: foo / size)",
        "(position-try: invalid --fallback)",
    ] {
        assert!(
            !css_supports(CssSupportsQuery::Condition(condition)),
            "{condition}"
        );
    }
}
