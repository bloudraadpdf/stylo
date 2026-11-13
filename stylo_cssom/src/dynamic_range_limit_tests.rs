use crate::values::parse_value;
use style::values::specified::DynamicRangeLimit;
use style_traits::ToCss;

#[test]
fn dynamic_range_limit_preserves_specified_mixtures_and_rejects_invalid_arguments() {
    for source in [
        "standard",
        "constrained",
        "no-limit",
        "dynamic-range-limit-mix(no-limit 80%, standard 20%)",
        "dynamic-range-limit-mix(no-limit 8%, standard 2%)",
        "dynamic-range-limit-mix(dynamic-range-limit-mix(constrained 90%, no-limit 10%) 1%, no-limit 80%)",
    ] {
        assert_eq!(parse_value::<DynamicRangeLimit>(source).unwrap().to_css_string(), source);
    }
    assert_eq!(
        parse_value::<DynamicRangeLimit>("dynamic-range-limit-mix(80% no-limit, 20% standard)").unwrap().to_css_string(),
        "dynamic-range-limit-mix(no-limit 80%, standard 20%)"
    );
    for source in [
        "high",
        "constrained-high",
        "dynamic-range-limit-mix(no-limit 1%)",
        "dynamic-range-limit-mix(no-limit 0%, standard 0%)",
        "dynamic-range-limit-mix(no-limit 101%, standard 1%)",
        "dynamic-range-limit-mix(no-limit -1%, standard 1%)",
        "dynamic-range-limit-mix(no-limit 80%, standard 20%,)",
        "dynamic-range-limit-mix(no-limit, standard 20%)",
    ] {
        assert!(parse_value::<DynamicRangeLimit>(source).is_none(), "{source}");
    }
}
