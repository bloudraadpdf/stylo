/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::{CascadePriority, DeclarationIterator};
use crate::properties::{
    declaration_block::parse_style_attribute, LonghandId, OwnedPropertyDeclarationId,
};
use crate::rule_tree::{CascadeLevel, RuleTree, StyleSource};
use crate::selector_parser::PseudoElement;
use crate::shared_lock::{SharedRwLock, StylesheetGuards};
use crate::stylesheets::{layer_rule::LayerOrder, CssRuleType};
use servo_arc::Arc;

fn assert_cascade_restriction(
    level: CascadeLevel,
    pseudo: Option<&PseudoElement>,
    restricted: bool,
) {
    let lock = SharedRwLock::new();
    let declarations = parse_style_attribute(
        "font-size:15px;color:red;--value:20px;display:none;position:absolute;float:right;list-style:inside none decimal",
        &url::Url::parse("https://example.test/style.css").unwrap().into(),
        None,
        selectors::matching::QuirksMode::NoQuirks,
        CssRuleType::Keyframe,
    );
    let declarations = Arc::new(lock.wrap(declarations));
    let guard = lock.read();
    let guards = StylesheetGuards::same(&guard);
    let tree = RuleTree::new();
    let rule = tree.insert_ordered_rules(std::iter::once((
        StyleSource::from_declarations(declarations),
        CascadePriority::new(level, LayerOrder::root()),
    )));
    let ids = DeclarationIterator::new(&rule, &guards, pseudo)
        .map(|(declaration, _, _)| declaration.id().to_owned())
        .collect::<Vec<_>>();
    for id in [LonghandId::FontSize, LonghandId::Color] {
        assert!(
            ids.contains(&OwnedPropertyDeclarationId::Longhand(id)),
            "{level:?}: {id:?}"
        );
    }
    assert!(ids
        .iter()
        .any(|id| matches!(id, OwnedPropertyDeclarationId::Custom(_))));
    for id in [
        LonghandId::Display,
        LonghandId::Position,
        LonghandId::Float,
        LonghandId::ListStyleImage,
        LonghandId::ListStylePosition,
        LonghandId::ListStyleType,
    ] {
        assert_eq!(
            ids.contains(&OwnedPropertyDeclarationId::Longhand(id)),
            !restricted,
            "{level:?}: {id:?}"
        );
    }
}

#[test]
fn marker_animation_and_transition_cascades_keep_property_restrictions() {
    for level in [CascadeLevel::Animations, CascadeLevel::Transitions] {
        assert_cascade_restriction(level, Some(&PseudoElement::Marker), true);
    }
}

#[test]
fn marker_ua_and_ordinary_element_effects_keep_all_properties() {
    assert_cascade_restriction(CascadeLevel::UANormal, Some(&PseudoElement::Marker), false);
    for level in [CascadeLevel::Animations, CascadeLevel::Transitions] {
        assert_cascade_restriction(level, None, false);
    }
}
