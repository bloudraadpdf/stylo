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

fn assert_serialization(name: &str, input: &str, expected: Option<&str>) {
    let block = block(&format!("{name}:{input}"));
    let id = PropertyId::parse_enabled_for_all_content(name).unwrap();
    let mut serialized = String::new();
    block.property_value_to_css(&id, &mut serialized).unwrap();
    assert_eq!(
        (!block.is_empty()).then_some(serialized.as_str()),
        expected,
        "{name}:{input}"
    );
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
            Some("column fill-reverse track-reverse"),
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
fn corner_shorthands_preserve_complete_radius_shape_pairs() {
    for (name, input, expected) in [
        ("corner", "normal", Some("normal")),
        ("corner", "round 30%", Some("30% round")),
        ("corner", "0px notch", Some("0px notch")),
        (
            "corner",
            "10px 20px bevel / 30px 40px scoop",
            Some("10px 20px bevel / 30px 40px scoop"),
        ),
        (
            "corner-top-left",
            "10px 20px round",
            Some("10px 20px round"),
        ),
        ("corner-top-left", "squircle 5px", Some("5px squircle")),
        ("corner-start-start", "normal", Some("normal")),
        (
            "corner-top",
            "bevel 10px 20px / scoop 30px 40px",
            Some("10px 20px bevel / 30px 40px scoop"),
        ),
        ("corner-block-start", "20% round", Some("20% round")),
        (
            "corner-inline-end",
            "5px squircle / 8px notch",
            Some("5px squircle / 8px notch"),
        ),
        ("corner", "0px superellipse(1)", Some("normal")),
        (
            "corner",
            "normal / 2px round / normal / 2px round",
            Some("normal / 2px round"),
        ),
        ("corner-top-left", "normal", Some("normal")),
        ("corner-top", "normal", Some("normal")),
        ("corner-block-start", "normal", Some("normal")),
        ("corner", "30%", None),
        ("corner", "bevel", None),
        ("corner-top-left", "10px", None),
        ("corner-top-left", "scoop", None),
        ("corner-top", "10px 20px / 30px 40px bevel scoop", None),
        ("corner-top", "normal / normal / normal", None),
        ("corner", "normal / normal / normal / normal / normal", None),
        ("corner", "3% normal", None),
        ("corner-top-left", "10px / 20px bevel", None),
        ("corner-top", "10px 20px 30px", None),
    ] {
        assert_serialization(name, input, expected);
    }

    let block = block("corner-bottom: 10px 20px bevel / 30px 40px scoop");
    for (name, expected) in [
        ("border-bottom-left-radius", "10px 20px"),
        ("border-bottom-right-radius", "30px 40px"),
        ("corner-bottom-left-shape", "bevel"),
        ("corner-bottom-right-shape", "scoop"),
    ] {
        let id = PropertyId::parse_enabled_for_all_content(name).unwrap();
        let mut serialized = String::new();
        block.property_value_to_css(&id, &mut serialized).unwrap();
        assert_eq!(serialized, expected, "{name}");
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
