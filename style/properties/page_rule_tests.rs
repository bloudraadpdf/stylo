/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::{PropertyId, declaration_block::parse_style_attribute};
use crate::stylesheets::{CssRuleType, UrlExtraData};

#[test]
fn writing_mode_is_allowed_in_page_rules() {
    let _lock = crate::test_support::pref_lock().lock().unwrap();
    let _pref = crate::test_support::BoolPrefGuard::set("layout.writing-mode.enabled", true);
    let url: UrlExtraData = url::Url::parse("https://example.test/style.css").unwrap().into();
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
            declarations.property_value_to_css(&id, &mut serialized).unwrap();
            assert_eq!(serialized, value);
        }
    }
}
