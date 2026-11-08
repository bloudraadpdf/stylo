/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::{
    declaration_block::parse_style_attribute, PropertyDeclaration, PropertyId, ShorthandId,
};
use crate::stylesheets::{CssRuleType, UrlExtraData};

fn block(css: &str) -> super::PropertyDeclarationBlock {
    let url: UrlExtraData = url::Url::parse("https://example.test/style.css")
        .unwrap()
        .into();
    parse_style_attribute(
        css,
        &url,
        None,
        selectors::matching::QuirksMode::NoQuirks,
        CssRuleType::Style,
    )
}

fn serialized_value(block: &super::PropertyDeclarationBlock, name: &str) -> String {
    let id = PropertyId::parse_enabled_for_all_content(name).unwrap();
    let mut serialized = String::new();
    block.property_value_to_css(&id, &mut serialized).unwrap();
    serialized
}

#[test]
fn text_autospace_size_accepts_signed_numbers_and_cascades_independently() {
    for (input, expected) in [
        ("0", "0"),
        ("0.5", "0.5"),
        ("-0.25", "-0.25"),
        ("calc(1 / 7)", "calc(0.142857)"),
    ] {
        assert_serialization("text-autospace-size", input, Some(expected));
    }
    for input in ["1px", "25%", "normal"] {
        assert_serialization("text-autospace-size", input, None);
    }
    let declarations = block("text-autospace-size:0.5;text-spacing:none");
    assert_eq!(
        serialized_value(&declarations, "text-autospace-size"),
        "0.5"
    );
    assert!(!ShorthandId::TextSpacing
        .longhands()
        .any(|id| id == super::LonghandId::TextAutospaceSize));
}

fn assert_serialization(name: &str, input: &str, expected: Option<&str>) {
    let block = block(&format!("{name}:{input}"));
    let serialized = serialized_value(&block, name);
    assert_eq!(
        (!block.is_empty()).then_some(serialized.as_str()),
        expected,
        "{name}:{input}"
    );
}

#[test]
fn text_stroke_preserves_fractional_widths_and_expands_the_shorthand() {
    assert_serialization("-webkit-text-stroke-width", "0.1px", Some("0.1px"));
    assert_serialization("-webkit-text-stroke-width", "-1px", None);
    let declaration = block("-webkit-text-stroke:0.5px lime");
    assert_eq!(
        serialized_value(&declaration, "-webkit-text-stroke-width"),
        "0.5px"
    );
    assert_eq!(
        serialized_value(&declaration, "-webkit-text-stroke-color"),
        "lime"
    );
}

#[test]
fn text_justify_applies_to_first_line_and_first_letter() {
    for restriction in [
        super::PropertyFlags::APPLIES_TO_FIRST_LINE,
        super::PropertyFlags::APPLIES_TO_FIRST_LETTER,
    ] {
        assert!(super::LonghandId::TextJustify.flags().contains(restriction));
        assert!(!super::LonghandId::TextIndent.flags().contains(restriction));
    }
    for value in ["auto", "none", "inter-word", "inter-character"] {
        assert_serialization("text-justify", value, Some(value));
    }
}

#[test]
fn animation_longhands_apply_to_first_line_and_first_letter() {
    for restriction in [
        super::PropertyFlags::APPLIES_TO_FIRST_LINE,
        super::PropertyFlags::APPLIES_TO_FIRST_LETTER,
    ] {
        for id in [
            super::LonghandId::AnimationName,
            super::LonghandId::AnimationDuration,
            super::LonghandId::AnimationTimingFunction,
            super::LonghandId::AnimationDelay,
            super::LonghandId::AnimationIterationCount,
            super::LonghandId::AnimationDirection,
            super::LonghandId::AnimationFillMode,
            super::LonghandId::AnimationPlayState,
            super::LonghandId::AnimationComposition,
        ] {
            assert!(id.flags().contains(restriction), "{id:?}");
        }
    }
}

#[test]
fn sizing_shorthands_expand_both_axes_and_css_wide_values() {
    for prefix in ["", "min-", "max-"] {
        let name = format!("{prefix}size");
        for (input, width, height) in [
            ("100px", "100px", "100px"),
            ("100px 200px", "100px", "200px"),
            ("20% min-content", "20%", "min-content"),
            (
                "fit-content(40px)",
                "fit-content(40px)",
                "fit-content(40px)",
            ),
            ("initial", "initial", "initial"),
            ("inherit", "inherit", "inherit"),
            ("unset", "unset", "unset"),
            ("revert", "revert", "revert"),
            ("revert-layer", "revert-layer", "revert-layer"),
        ] {
            let declarations = block(&format!("{name}:{input}"));
            for (axis, expected) in [("width", width), ("height", height)] {
                assert_eq!(
                    serialized_value(&declarations, &format!("{prefix}{axis}")),
                    expected,
                    "{name}:{input}"
                );
            }
        }
        for invalid in ["10px 20px 30px", "-1px", "initial auto", "portrait", "A4"] {
            assert!(
                block(&format!("{name}:{invalid}")).is_empty(),
                "{name}:{invalid}"
            );
        }
    }
}

#[test]
fn size_property_does_not_expand_in_page_rules() {
    let url: UrlExtraData = url::Url::parse("https://example.test/style.css")
        .unwrap()
        .into();
    let declarations = parse_style_attribute(
        "size:100px 200px",
        &url,
        None,
        selectors::matching::QuirksMode::NoQuirks,
        CssRuleType::Page,
    );
    assert_eq!(declarations.len(), 1);
    let mut size = String::new();
    declarations
        .property_value_to_css(
            &PropertyId::NonCustom(super::LonghandId::Size.into()),
            &mut size,
        )
        .unwrap();
    assert_eq!(size, "100px 200px");
    assert_eq!(serialized_value(&declarations, "width"), "");
    assert_eq!(serialized_value(&declarations, "height"), "");
}

#[test]
fn size_is_not_a_preferred_serialization_shorthand() {
    let declarations = block("width:100px;height:100px");
    let mut css = String::new();
    declarations.to_css(&mut css).unwrap();
    assert_eq!(css, "width: 100px; height: 100px;");
    assert_eq!(serialized_value(&declarations, "size"), "100px");
}

#[test]
fn anchor_center_applies_to_self_alignment_only() {
    let _lock = crate::test_support::pref_lock().lock().unwrap();
    let _pref =
        crate::test_support::BoolPrefGuard::set("layout.css.anchor-positioning.enabled", true);
    for name in ["align-items", "justify-items"] {
        assert_serialization(name, "anchor-center", None);
    }
    for name in ["align-self", "justify-self"] {
        assert_serialization(name, "anchor-center", Some("anchor-center"));
    }
}

#[test]
fn fit_tolerance_uses_the_current_grid_lanes_property_name() {
    let _lock = crate::test_support::pref_lock().lock().unwrap();
    let _pref = crate::test_support::BoolPrefGuard::set(
        "layout.css.grid-template-masonry-value.enabled",
        true,
    );
    assert_serialization("fit-tolerance", "0", Some("0px"));
    assert_serialization("fit-tolerance", "25%", Some("25%"));
    assert!(block("flow-tolerance:0").is_empty());
    assert!(block("fit-tolerance:-1px").is_empty());
    assert!(block("fit-tolerance:auto").is_empty());
}

#[test]
fn masonry_slack_alias_preserves_fit_tolerance_keyword_semantics() {
    let _lock = crate::test_support::pref_lock().lock().unwrap();
    let _pref = crate::test_support::BoolPrefGuard::set(
        "layout.css.grid-template-masonry-value.enabled",
        true,
    );
    for (input, backing) in [
        ("normal", "infinite"),
        ("infinite", "auto"),
        ("10px", "10px"),
    ] {
        let declarations = block(&format!("fit-tolerance:{input}"));
        assert_eq!(serialized_value(&declarations, "masonry-slack"), backing);
    }
    assert!(block("fit-tolerance:auto").is_empty());
    assert_eq!(
        serialized_value(&block("masonry-slack:infinite"), "masonry-slack"),
        "infinite"
    );
}

#[test]
fn scroll_initial_target_accepts_only_none_and_nearest() {
    for value in ["none", "nearest"] {
        assert_serialization("scroll-initial-target", value, Some(value));
    }
    for value in ["auto", "all", "nearest none"] {
        assert_serialization("scroll-initial-target", value, None);
    }
}

#[test]
fn break_inside_accepts_only_auto_and_avoid_values() {
    for value in [
        "auto",
        "avoid",
        "avoid-page",
        "avoid-column",
        "avoid-region",
    ] {
        assert_serialization("break-inside", value, Some(value));
    }
    for value in ["region", "page", "column", "always", "auto avoid"] {
        assert_serialization("break-inside", value, None);
    }
    for name in ["break-before", "break-after"] {
        assert_serialization(name, "region", Some("region"));
    }
}

#[test]
fn webkit_text_orientation_is_a_native_alias() {
    for name in ["text-orientation", "-webkit-text-orientation"] {
        for value in ["mixed", "upright", "sideways"] {
            assert_serialization(name, value, Some(value));
        }
    }
}

#[test]
fn text_orientation_accepts_the_legacy_sideways_right_keyword() {
    assert_serialization("text-orientation", "sideways-right", Some("sideways"));
}

#[test]
fn grid_lanes_shorthand_expands_tracks_in_the_selected_axis() {
    let _preferences = crate::test_support::pref_lock().lock().unwrap();
    let _grid = crate::test_support::BoolPrefGuard::set("layout.grid.enabled", true);
    let block = block("grid-lanes: row fill-reverse 20% 40% \"b a\"");
    assert_eq!(block.len(), 4);
    for (name, expected) in [
        ("grid-template-rows", "20% 40%"),
        ("grid-template-columns", "none"),
        ("grid-template-areas", "\"b\" \"a\""),
        ("grid-lanes-direction", "row fill-reverse"),
        ("grid-lanes", "\"b a\" 20% 40% row fill-reverse"),
    ] {
        assert_eq!(serialized_value(&block, name), expected, "{name}");
    }
}

#[test]
fn grid_lanes_shorthand_validates_and_serializes_each_component() {
    let _preferences = crate::test_support::pref_lock().lock().unwrap();
    let _grid = crate::test_support::BoolPrefGuard::set("layout.grid.enabled", true);
    for (css, expected) in [
        ("none", "column"),
        ("normal", "normal"),
        ("10px 20px", "10px 20px column"),
        (
            "column fill-reverse \"a\" calc(10px)",
            "\"a\" calc(10px) column fill-reverse",
        ),
        ("\"b b a\" 1fr 2fr 3fr row", "\"b b a\" 1fr 2fr 3fr row"),
        (
            "row track-reverse fill-reverse repeat(2, auto)",
            "repeat(2, auto) row track-reverse fill-reverse",
        ),
        (
            "column \"a b\" [line1] 1fr [line2] 2fr",
            "\"a b\" [line1] 1fr [line2] 2fr column",
        ),
    ] {
        assert_serialization("grid-lanes", css, Some(expected));
    }
    for css in [
        "row normal \"a a\" 1fr",
        "\"a\" \"b\" row",
        "fit-content(-10px)",
        "[] normal",
        "[one] 10px [two] [three]",
        "[auto] 1px",
        "20% 40% column, reverse",
        "none auto",
        "row-reverse 10px",
        "normal fill-reverse",
        "row row",
        "\"a b a\" 1fr",
    ] {
        assert_serialization("grid-lanes", css, None);
    }
}

#[test]
fn grid_lanes_direction_retains_specified_reversal_order() {
    for value in [
        "row track-reverse fill-reverse",
        "column fill-reverse track-reverse",
    ] {
        assert_serialization("grid-lanes-direction", value, Some(value));
    }
}

#[test]
fn grid_spans_preserve_calculations_until_computed_value_clamping() {
    let _preferences = crate::test_support::pref_lock().lock().unwrap();
    let _grid = crate::test_support::BoolPrefGuard::set("layout.grid.enabled", true);
    for (value, expected) in [
        ("span calc(-2)", "span calc(-2)"),
        ("span min(-1, 6)", "span calc(-1)"),
        ("span calc(0)", "span calc(0)"),
        ("span calc(-2) i", "span calc(-2) i"),
        ("calc(-2) span", "span calc(-2)"),
        (
            "span calc(sibling-index() - 2)",
            "span calc(-2 + sibling-index())",
        ),
    ] {
        assert_serialization("grid-column-start", value, Some(expected));
    }
    for value in ["span -2", "span 0", "span calc(0) 2", "calc(-2) span 3"] {
        assert_serialization("grid-column-start", value, None);
    }
}

#[test]
fn text_group_alignment_has_a_closed_keyword_grammar() {
    for value in ["none", "start", "end", "left", "right", "center"] {
        assert_serialization("text-group-align", value, Some(value));
    }
    for value in [
        "auto",
        "match-parent",
        "justify",
        "left right",
        "none center",
        "5px",
    ] {
        assert_serialization("text-group-align", value, None);
    }
}

#[test]
fn text_fitting_retains_optional_line_policy_and_scale_limit() {
    for value in [
        "none",
        "grow",
        "shrink",
        "grow consistent",
        "grow per-line",
        "grow per-line-all",
        "grow per-line 132%",
        "grow consistent 300%",
        "shrink per-line 4%",
        "shrink consistent 50%",
        "none 120%",
        "grow -10%",
    ] {
        assert_serialization("text-fit", value, Some(value));
    }
    for value in [
        "larger",
        "stretch per-line",
        "grow 150% consistent",
        "consistent 100%",
        "grow consistent 1.0",
    ] {
        assert_serialization("text-fit", value, None);
    }
}

#[test]
fn native_common_properties_validate_and_serialize_their_grammars() {
    for (name, input, expected) in [
        ("align-content", "flow-start", Some("flow-start")),
        ("justify-content", "safe flow-end", Some("safe flow-end")),
        (
            "align-items",
            "unsafe flow-start",
            Some("unsafe flow-start"),
        ),
        ("justify-items", "flow-end", Some("flow-end")),
        ("align-self", "safe flow-start", Some("safe flow-start")),
        ("justify-self", "unsafe flow-end", Some("unsafe flow-end")),
        (
            "place-content",
            "flow-start flow-end",
            Some("flow-start flow-end"),
        ),
        ("place-items", "flow-end", Some("flow-end")),
        ("place-self", "safe flow-start", Some("safe flow-start")),
        ("align-content", "flow-start flow-end", None),
        ("justify-items", "legacy flow-start", None),
        ("align-self", "flow-end safe", None),
        ("corner-top-left-shape", "squircle", Some("squircle")),
        ("corner-start-start-shape", "squircle", Some("squircle")),
        (
            "corner-top-left-shape",
            "superellipse(calc(0.5 * 4))",
            Some("superellipse(calc(2))"),
        ),
        ("corner-top-shape", "scoop scoop", Some("scoop")),
        (
            "corner-inline-end-shape",
            "bevel notch",
            Some("bevel notch"),
        ),
        ("corner-block-start-shape", "round scoop bevel", None),
        (
            "border",
            "hairline dotted green",
            Some("hairline dotted green"),
        ),
        ("border-width", "5px hairline", Some("5px hairline")),
        ("border-left-width", "hairline", Some("hairline")),
        ("flex-wrap", "balance", Some("balance")),
        ("flex-wrap", "wrap balance", Some("balance")),
        ("flex-wrap", "balance wrap", Some("balance")),
        (
            "flex-wrap",
            "wrap-reverse balance",
            Some("wrap-reverse balance"),
        ),
        (
            "flex-wrap",
            "balance wrap-reverse",
            Some("wrap-reverse balance"),
        ),
        ("flex-wrap", "nowrap balance", None),
        ("flex-wrap", "balance balance", None),
        ("flex-wrap", "wrap wrap-reverse", None),
        ("flex-line-count", "1", Some("1")),
        ("flex-line-count", "2", Some("2")),
        ("flex-line-count", "0", None),
        ("flex-line-count", "-1", None),
        ("flex-line-count", "1.5", None),
        ("view-transition-scope", "ALL", Some("all")),
        ("view-transition-scope", "none", Some("none")),
        ("view-transition-scope", "nearest", None),
        ("view-transition-group", "normal", Some("normal")),
        ("view-transition-group", "contain", Some("contain")),
        ("view-transition-group", "nearest", Some("nearest")),
        ("view-transition-group", "none", Some("none")),
        (
            "view-transition-group",
            r"group\ name",
            Some(r"group\ name"),
        ),
        ("view-transition-group", "default", None),
        ("view-transition-group", "contain nearest", None),
        ("grid-lanes-direction", "normal", Some("normal")),
        ("grid-lanes-direction", "row", Some("row")),
        (
            "grid-lanes-direction",
            "column track-reverse fill-reverse",
            Some("column track-reverse fill-reverse"),
        ),
        ("grid-lanes-direction", "normal fill-reverse", None),
        (
            "grid-lanes-direction",
            "row fill-reverse fill-reverse",
            None,
        ),
        ("grid-lanes-direction", "row column", None),
        ("-webkit-box-orient", "horizontal", Some("horizontal")),
        ("-webkit-box-orient", "vertical", Some("vertical")),
        ("-webkit-box-orient", "inline-axis", Some("inline-axis")),
        ("-webkit-box-orient", "block-axis", Some("block-axis")),
        ("-webkit-box-orient", "sideways", None),
    ] {
        assert_serialization(name, input, expected);
    }
}

#[test]
fn corner_shorthands_parse_independent_shape_and_radius_components() {
    for (name, input, expected) in [
        ("corner", "normal", None),
        ("corner", "round 30%", Some("30%")),
        ("corner", "0px notch", Some("notch")),
        ("corner", "0% bevel", Some("bevel 0%")),
        (
            "corner",
            "10px 20px / 30px 40px bevel scoop",
            Some("bevel scoop 10px 20px / 30px 40px"),
        ),
        ("corner-top-left", "10px 20px round", Some("10px 20px")),
        ("corner-top-left", "squircle 5px", Some("squircle 5px")),
        ("corner-start-start", "round", Some("0px")),
        (
            "corner-top",
            "bevel scoop 10px 20px / 30px 40px",
            Some("bevel scoop 10px 20px / 30px 40px"),
        ),
        ("corner-block-start", "20% round", Some("20%")),
        (
            "corner-inline-end",
            "5px / 8px squircle notch",
            Some("squircle notch 5px / 8px"),
        ),
        ("corner", "0px superellipse(1)", Some("0px")),
        ("corner", "30%", Some("30%")),
        ("corner", "bevel", Some("bevel")),
        ("corner", "round", Some("0px")),
        (
            "corner",
            "bevel scoop 1px 2px 3px",
            Some("bevel scoop 1px 2px 3px"),
        ),
        ("corner-top-left", "10px", Some("10px")),
        ("corner-top-left", "scoop", Some("scoop")),
        ("corner-top", "normal / normal / normal", None),
        ("corner", "normal / normal / normal / normal / normal", None),
        ("corner", "3% normal", None),
        ("corner-top-left", "10px / 20px bevel", None),
        ("corner-top", "10px 20px 30px", None),
        ("corner", "10px round / 20px bevel", None),
        ("corner", "bevel 10px scoop", None),
        ("corner", "10px bevel 20px", None),
        ("corner", "10px / bevel", None),
        ("corner", "-1px bevel", None),
    ] {
        assert_serialization(name, input, expected);
    }

    for (side, first, second) in [
        ("top", "top-left", "top-right"),
        ("right", "top-right", "bottom-right"),
        ("bottom", "bottom-left", "bottom-right"),
        ("left", "top-left", "bottom-left"),
        ("block-start", "start-start", "start-end"),
        ("block-end", "end-start", "end-end"),
        ("inline-start", "start-start", "end-start"),
        ("inline-end", "start-end", "end-end"),
    ] {
        let declarations = block(&format!("corner-{side}: bevel scoop 10px 30px / 20px 40px"));
        for (name, expected) in [
            (format!("border-{first}-radius"), "10px 20px"),
            (format!("border-{second}-radius"), "30px 40px"),
            (format!("corner-{first}-shape"), "bevel"),
            (format!("corner-{second}-shape"), "scoop"),
        ] {
            assert_eq!(
                serialized_value(&declarations, &name),
                expected,
                "{side}: {name}"
            );
        }
    }
    let declarations = block("corner-shape: bevel; corner: 10px");
    assert_eq!(
        serialized_value(&declarations, "corner-top-left-shape"),
        "round"
    );
    let declarations = block("border-radius: 10px; corner: scoop");
    assert_eq!(
        serialized_value(&declarations, "border-top-left-radius"),
        "0px"
    );
}

#[test]
fn side_radius_shorthands_preserve_both_corner_axes() {
    for (side, first, second) in [
        ("top", "top-left", "top-right"),
        ("right", "top-right", "bottom-right"),
        ("bottom", "bottom-left", "bottom-right"),
        ("left", "top-left", "bottom-left"),
        ("block-start", "start-start", "start-end"),
        ("block-end", "end-start", "end-end"),
        ("inline-start", "start-start", "end-start"),
        ("inline-end", "start-end", "end-end"),
    ] {
        let name = format!("border-{side}-radius");
        let first = format!("border-{first}-radius");
        let second = format!("border-{second}-radius");
        for (input, expected, first_value, second_value) in [
            ("10px 10px", "10px", "10px", "10px"),
            ("10px 20%", "10px 20%", "10px", "20%"),
            ("10px / 20px", "10px / 20px", "10px 20px", "10px 20px"),
            (
                "10px 20px / 30% 40%",
                "10px 20px / 30% 40%",
                "10px 30%",
                "20px 40%",
            ),
            ("inherit", "inherit", "inherit", "inherit"),
        ] {
            let declarations = block(&format!("{name}:{input}"));
            assert_eq!(
                serialized_value(&declarations, &name),
                expected,
                "{name}: {input}"
            );
            assert_eq!(
                serialized_value(&declarations, &first),
                first_value,
                "{name}: {first}"
            );
            assert_eq!(
                serialized_value(&declarations, &second),
                second_value,
                "{name}: {second}"
            );
        }
        for invalid in [
            "-1px",
            "1px -2%",
            "1px / -2px",
            "1px 2px 3px",
            "1px / 2px / 3px",
        ] {
            assert_serialization(&name, invalid, None);
        }
    }
}

#[test]
fn border_shape_specified_serialization_preserves_authored_reference_boxes() {
    for value in [
        "circle()",
        "circle() half-border-box",
        "circle() border-box",
        "circle() circle()",
        "circle() border-box circle() padding-box",
        "circle() border-box circle() content-box",
        "circle() content-box circle() border-box",
        "circle() half-border-box circle() half-border-box",
    ] {
        assert_serialization("border-shape", value, Some(value));
    }
}

#[test]
fn logical_position_keywords_preserve_their_axis_and_offsets() {
    for (name, input, expected) in [
        ("background-position-x", "x-start", Some("x-start")),
        ("background-position-x", "x-end 10px", Some("x-end 10px")),
        (
            "background-position-y",
            "y-start -20%",
            Some("y-start -20%"),
        ),
        ("background-position-y", "y-end", Some("y-end")),
        (
            "background-position-x",
            "0.5em, x-start, x-end",
            Some("0.5em, x-start, x-end"),
        ),
        (
            "background-position-y",
            "0.5em, y-start, y-end",
            Some("0.5em, y-start, y-end"),
        ),
        (
            "background-position",
            "y-start x-end",
            Some("x-end y-start"),
        ),
        (
            "background-position",
            "x-start 2px y-end 3px",
            Some("x-start 2px y-end 3px"),
        ),
        ("object-position", "x-end y-start", Some("x-end y-start")),
        ("background-position-x", "y-start", None),
        ("background-position-y", "x-end", None),
        ("background-position-x", "x-start center", None),
        (
            "background-image",
            "linear-gradient(to x-start, red, blue)",
            None,
        ),
        ("transform-origin", "x-start y-end", None),
    ] {
        assert_serialization(name, input, expected);
    }
}

#[test]
fn font_width_alias_and_percentage_calculations_serialize_canonically() {
    let PropertyId::NonCustom(id) =
        PropertyId::parse_enabled_for_all_content("font-stretch").unwrap()
    else {
        panic!("font-stretch must be a standard property");
    };
    assert_eq!(id.unaliased().name(), "font-width");
    assert_serialization(
        "font-stretch",
        "calc(100% + (sign(20cqw - 10px) * 5%))",
        Some("calc(100% + (5% * sign(20cqw - 10px)))"),
    );
    for value in ["condensed", "234.5%", "calc(-100%)"] {
        assert_serialization("font-width", value, Some(value));
    }
    let block = block("font-width:75%;font-stretch:expanded");
    for name in ["font-width", "font-stretch"] {
        let id = PropertyId::parse_enabled_for_all_content(name).unwrap();
        let mut serialized = String::new();
        block.property_value_to_css(&id, &mut serialized).unwrap();
        assert_eq!(serialized, "expanded");
    }
}

#[test]
fn object_fit_retains_the_scale_down_constraint() {
    for (input, expected) in [
        ("contain scale-down", Some("scale-down")),
        ("scale-down contain", Some("scale-down")),
        ("cover scale-down", Some("cover scale-down")),
        ("scale-down cover", Some("cover scale-down")),
        ("fill scale-down", None),
        ("none scale-down", None),
        ("cover contain", None),
        ("scale-down scale-down", None),
    ] {
        assert_serialization("object-fit", input, expected);
    }
}

#[test]
fn overflow_clip_margin_preserves_signed_offsets_and_one_shared_edge() {
    for property in [
        "overflow-clip-margin",
        "overflow-clip-margin-block",
        "overflow-clip-margin-inline",
        "overflow-clip-margin-top",
        "overflow-clip-margin-inline-start",
    ] {
        for (input, expected) in [
            ("-10px", Some("-10px")),
            ("-10px content-box", Some("content-box -10px")),
            (
                "border-box calc(10px - 20px)",
                Some("border-box calc(-10px)"),
            ),
            ("0px content-box", Some("content-box")),
            ("50px 50px", None),
            ("calc(100% - 10px)", None),
        ] {
            assert_serialization(property, input, expected);
        }
    }
}

#[test]
fn scroll_control_properties_accept_their_native_keyword_grammar() {
    for property in ["scroll-axis-lock", "scroll-target-group"] {
        for (input, expected) in [
            ("auto", Some("auto")),
            ("none", Some("none")),
            ("inherit", Some("inherit")),
            ("revert", Some("revert")),
            ("auto none", None),
            ("both", None),
        ] {
            assert_serialization(property, input, expected);
        }
    }
}

#[test]
fn text_box_shorthand_preserves_its_trim_and_metric_defaults() {
    for (input, expected) in [
        ("none", Some("normal")),
        ("auto none", Some("normal")),
        ("auto", Some("trim-both")),
        ("trim-start auto", Some("trim-start")),
        ("text text trim-both", Some("text")),
        ("none cap text", Some("none cap")),
        ("text alphabetic trim-end", Some("trim-end alphabetic")),
        ("trim-end alphabetic", Some("trim-end alphabetic")),
        ("normal text", None),
        ("cap trim-both alphabetic", None),
    ] {
        assert_serialization("text-box", input, expected);
    }
}

#[test]
fn native_all_expands_pending_values_into_the_new_longhands() {
    let _preferences = crate::test_support::pref_lock().lock().unwrap();
    let block = block("all:var(--reset)!important");
    for name in [
        "view-transition-scope",
        "view-transition-group",
        "grid-lanes-direction",
        "-webkit-box-orient",
    ] {
        let id = PropertyId::parse_enabled_for_all_content(name).unwrap();
        let (declaration, importance) = block
            .declaration_importance_iter()
            .find(|(declaration, _)| declaration.id().name() == name)
            .unwrap_or_else(|| panic!("missing All member {name}"));
        let PropertyDeclaration::WithVariables(value) = declaration else {
            panic!("{name} lost pending substitution")
        };
        assert_eq!(value.value.from_shorthand(), Some(ShorthandId::All));
        assert_eq!(value.value.variable_value().css, "var(--reset)");
        assert_eq!(
            value.value.variable_value().url_data.as_str(),
            "https://example.test/style.css"
        );
        assert!(importance.important());
        let mut serialized = String::new();
        block.property_value_to_css(&id, &mut serialized).unwrap();
        assert!(serialized.is_empty(), "{name}");
    }
    let mut shorthand = String::new();
    block
        .property_value_to_css(
            &PropertyId::parse_enabled_for_all_content("all").unwrap(),
            &mut shorthand,
        )
        .unwrap();
    assert_eq!(shorthand, "var(--reset)");
}

#[test]
fn mask_does_not_serialize_when_a_reset_longhand_is_missing() {
    let mut block = block("mask:initial");
    let removed = PropertyId::parse_enabled_for_all_content("mask-border-mode").unwrap();
    let first = block.first_declaration_to_remove(&removed).unwrap();
    block.remove_property(&removed, first);

    assert_eq!(serialized_value(&block, "mask"), "");
}

#[test]
fn view_timeline_properties_retain_typed_names_axes_and_insets() {
    for (name, input, expected) in [
        ("view-timeline-name", "--visible, none", "--visible, none"),
        ("view-timeline-axis", "inline, y", "inline, y"),
        ("view-timeline-inset", "10px 20%, auto", "10px 20%, auto"),
        ("view-timeline-inset", "inherit", "inherit"),
    ] {
        assert_serialization(name, input, Some(expected));
    }
    for (name, input) in [
        ("view-timeline-name", "visible"),
        ("view-timeline-axis", "diagonal"),
        ("view-timeline-inset", "1px 2px 3px"),
    ] {
        assert!(block(&format!("{name}:{input}")).is_empty());
    }
}

#[test]
fn view_timeline_insets_participate_in_zoom_inheritance() {
    let properties = super::LonghandIdSet::zoom_dependent();
    assert!(properties.contains(super::LonghandId::ViewTimelineInset));
    assert!(!properties.contains(super::LonghandId::ViewTimelineAxis));
    assert!(!properties.contains(super::LonghandId::ViewTimelineName));
}

#[test]
fn scroll_padding_edges_participate_in_zoom_inheritance() {
    use super::LonghandId;
    let properties = super::LonghandIdSet::zoom_dependent();
    for property in [
        LonghandId::ScrollPaddingTop,
        LonghandId::ScrollPaddingRight,
        LonghandId::ScrollPaddingBottom,
        LonghandId::ScrollPaddingLeft,
        LonghandId::ScrollPaddingBlockStart,
        LonghandId::ScrollPaddingBlockEnd,
        LonghandId::ScrollPaddingInlineStart,
        LonghandId::ScrollPaddingInlineEnd,
    ] {
        assert!(properties.contains(property), "{property:?}");
    }
}

#[test]
fn image_resolution_keeps_the_source_and_resolution_group_together() {
    for value in [
        "3dpi snap from-image",
        "from-image snap 4dppx",
        "snap",
        "snap from-image snap",
        "from-image from-image",
        "2dpi 3dpi",
    ] {
        assert_serialization("image-resolution", value, None);
    }
    for (input, expected) in [
        ("3dpi from-image snap", "from-image 3dpi snap"),
        ("snap from-image 4dppx", "from-image 4dppx snap"),
        ("snap 2dpi from-image", "from-image 2dpi snap"),
        ("from-image snap", "from-image snap"),
    ] {
        assert_serialization("image-resolution", input, Some(expected));
    }
}

#[test]
fn border_clip_uses_current_grammar_and_copies_the_list_to_each_side() {
    for value in ["none", "10px 20% 1fr", "0px"] {
        assert_serialization("border-clip", value, Some(value));
        let declaration = block(&format!("border-clip:{value}"));
        for side in ["top", "right", "bottom", "left"] {
            assert_eq!(
                serialized_value(&declaration, &format!("border-{side}-clip")),
                value
            );
        }
    }
    for value in ["normal", "-1px", "-1fr", "none 2px", "1px, 2px"] {
        assert_serialization("border-clip", value, None);
    }
}

#[test]
fn box_shadow_longhands_expand_and_reset_independently() {
    assert_serialization(
        "box-shadow-offset",
        "10px 10px, none, 2px 3px",
        Some("10px, none, 2px 3px"),
    );
    assert_serialization("box-shadow-blur", "-1px", None);
    assert_serialization("box-shadow-spread", "-1px, 2px", Some("-1px, 2px"));
    let declarations =
        block("box-shadow: red 1px 2px 3px 4px inset; box-shadow-color: blue; box-shadow: none");
    for (name, value) in [
        ("offset", "none"),
        ("blur", "0px"),
        ("spread", "0px"),
        ("position", "outset"),
        ("color", "transparent"),
    ] {
        assert_eq!(
            serialized_value(&declarations, &format!("box-shadow-{name}")),
            value
        );
    }
}

#[test]
fn absent_shadow_offset_interpolates_as_two_zero_offsets() {
    use crate::values::{
        animated::{Animate, Procedure},
        computed::{BoxShadowOffset, Length},
    };
    let offset = BoxShadowOffset::Offset(Length::new(10.0), Length::new(20.0));
    assert_eq!(
        BoxShadowOffset::None
            .animate(&offset, Procedure::Interpolate { progress: 0.5 })
            .unwrap(),
        BoxShadowOffset::Offset(Length::new(5.0), Length::new(10.0))
    );
    assert_eq!(
        offset
            .animate(
                &BoxShadowOffset::None,
                Procedure::Interpolate { progress: 0.5 }
            )
            .unwrap(),
        BoxShadowOffset::Offset(Length::new(5.0), Length::new(10.0))
    );
}

#[test]
fn box_shadow_shorthand_omits_only_default_components() {
    assert_serialization("box-shadow", "1px 2px", Some("1px 2px"));
    assert_serialization(
        "box-shadow",
        "currentcolor 1px 2px 0px 0px outset",
        Some("1px 2px"),
    );
    assert_serialization(
        "box-shadow",
        "1px 2px 0px 3px inset",
        Some("1px 2px 0px 3px inset"),
    );
    assert_serialization("box-shadow", "none", Some("none"));
    assert_serialization("box-shadow", "currentcolor none", Some("currentcolor none"));
    assert_serialization("box-shadow", "1px", None);
}

#[test]
fn all_shorthand_schema_members_include_pref_controlled_properties() {
    let potential = ShorthandId::All.all_longhands();
    assert!(potential.contains(&super::LonghandId::AlignTracks));
    assert!(potential.contains(&super::LonghandId::BoxShadowOffset));
    assert!(!potential.contains(&super::LonghandId::Direction));
    assert!(!potential.contains(&super::LonghandId::UnicodeBidi));
    assert!(ShorthandId::All
        .longhands()
        .all(|id| potential.contains(&id)));
}

#[test]
fn margin_trim_accepts_only_the_block_container_grammar() {
    for (input, expected) in [
        ("none", Some("none")),
        ("block", Some("block")),
        ("block-start", Some("block-start")),
        ("block-end", Some("block-end")),
        ("block-start block-end", Some("block")),
        ("block-end block-start", Some("block")),
        ("BLOCK-END BLOCK-START", Some("block")),
        ("inline", None),
        ("inline-start", None),
        ("inline-end", None),
        ("block inline", None),
        ("inline-start inline-end", None),
        ("block-start inline-start", None),
        ("block-start block-start", None),
        ("none block", None),
        ("block block-start", None),
    ] {
        assert_serialization("margin-trim", input, expected);
    }
}
