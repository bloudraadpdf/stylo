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

#[test]
fn native_common_properties_validate_and_serialize_their_grammars() {
    for (name, input, expected) in [
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
