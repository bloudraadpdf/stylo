use stylo_cssom::values::{CssSupportsQuery, css_supports};

#[test]
fn named_features_report_the_renderer_capabilities() {
    for name in [
        "anchor-position-follows-transforms",
        "single-axis-scroll-container",
    ] {
        assert!(css_supports(CssSupportsQuery::Condition(&format!(
            "named-feature({name})"
        ))));
    }
    for value in [
        "unknown-feature",
        "anchor-position-follows-transforms extra",
        "",
    ] {
        assert!(!css_supports(CssSupportsQuery::Condition(&format!(
            "named-feature({value})"
        ))));
    }
}
