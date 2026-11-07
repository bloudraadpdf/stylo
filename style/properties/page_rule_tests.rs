/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::{declaration_block::parse_style_attribute, PropertyId};
use crate::stylesheets::{CssRuleType, UrlExtraData};

#[test]
fn font_variant_is_allowed_in_page_rules() {
    let url: UrlExtraData = url::Url::parse("https://example.test/style.css")
        .unwrap()
        .into();
    let id = PropertyId::parse_enabled_for_all_content("font-variant-caps").unwrap();
    for value in ["normal", "small-caps"] {
        let declarations = parse_style_attribute(
            &format!("font-variant:{value}"),
            &url,
            None,
            selectors::matching::QuirksMode::NoQuirks,
            CssRuleType::Page,
        );
        let mut serialized = String::new();
        declarations
            .property_value_to_css(&id, &mut serialized)
            .unwrap();
        assert_eq!(serialized, value);
    }
}

#[test]
fn writing_mode_is_allowed_in_page_rules() {
    let _lock = crate::test_support::pref_lock().lock().unwrap();
    let _pref = crate::test_support::BoolPrefGuard::set("layout.writing-mode.enabled", true);
    let url: UrlExtraData = url::Url::parse("https://example.test/style.css")
        .unwrap()
        .into();
    let id = PropertyId::parse_enabled_for_all_content("writing-mode").unwrap();
    for rule_type in [CssRuleType::Style, CssRuleType::Page] {
        for value in [
            "horizontal-tb",
            "vertical-rl",
            "vertical-lr",
            "sideways-rl",
            "sideways-lr",
        ] {
            let declarations = parse_style_attribute(
                &format!("writing-mode:{value}"),
                &url,
                None,
                selectors::matching::QuirksMode::NoQuirks,
                rule_type,
            );
            assert!(
                !declarations.is_empty(),
                "writing-mode:{value} must survive in {rule_type:?}"
            );
            let mut serialized = String::new();
            declarations
                .property_value_to_css(&id, &mut serialized)
                .unwrap();
            assert_eq!(serialized, value);
        }
    }
}

#[test]
fn scoped_declarations_use_the_innermost_rule_grammar() {
    use crate::parser::{NestingContext, ParserContext};
    use crate::stylesheets::Origin;
    use style_traits::ParsingMode;

    let url: UrlExtraData = url::Url::parse("https://example.test/style.css")
        .unwrap()
        .into();
    let mut context = ParserContext::new(
        Origin::Author,
        &url,
        Some(CssRuleType::Scope),
        ParsingMode::DEFAULT,
        selectors::matching::QuirksMode::NoQuirks,
        Default::default(),
        None,
        None,
    );
    for name in ["display", "place-self", "border-collapse", "--border-model"] {
        assert!(PropertyId::parse(name, &context).is_ok(), "{name}");
    }
    for (kind, allowed, forbidden) in [
        (CssRuleType::PositionTry, "left", "all"),
        (CssRuleType::Keyframe, "opacity", "animation-name"),
        (CssRuleType::Page, "size", "display"),
    ] {
        context.nest_for_rule(kind, |nested| {
            assert!(
                PropertyId::parse(allowed, nested).is_ok(),
                "{kind:?}: {allowed}"
            );
            assert!(
                PropertyId::parse(forbidden, nested).is_err(),
                "{kind:?}: {forbidden}"
            );
            if kind == CssRuleType::PositionTry {
                assert!(PropertyId::parse("--border-model", nested).is_err());
            }
        });
        assert!(PropertyId::parse("display", &context).is_ok());
    }
    context.nesting_context = NestingContext::new(
        crate::stylesheets::CssRuleTypes::from_bits(
            CssRuleType::Style.bit() | CssRuleType::Scope.bit() | CssRuleType::Media.bit(),
        ),
        Some(CssRuleType::Scope),
    );
    assert!(PropertyId::parse("place-self", &context).is_ok());
    assert!(PropertyId::parse("--border-model", &context).is_ok());
}

#[test]
fn page_child_boxes_accept_shared_properties_without_page_descriptors() {
    use crate::parser::ParserContext;
    use crate::stylesheets::Origin;
    use style_traits::ParsingMode;

    let url: UrlExtraData = url::Url::parse("https://example.test/style.css")
        .unwrap()
        .into();
    let mut context = ParserContext::new(
        Origin::Author,
        &url,
        Some(CssRuleType::Scope),
        ParsingMode::DEFAULT,
        selectors::matching::QuirksMode::NoQuirks,
        Default::default(),
        None,
        None,
    );
    context.nest_for_rule(CssRuleType::Page, |page| {
        for kind in [
            CssRuleType::Margin,
            CssRuleType::Footnote,
            CssRuleType::Sidenote,
        ] {
            page.nest_for_rule(kind, |nested| {
                for name in [
                    "content",
                    "width",
                    "height",
                    "max-height",
                    "border-top-width",
                    "transform",
                ] {
                    assert!(PropertyId::parse(name, nested).is_ok(), "{kind:?}: {name}");
                }
                for name in ["size", "display", "animation-name"] {
                    assert!(PropertyId::parse(name, nested).is_err(), "{kind:?}: {name}");
                }
            });
        }
    });
}
