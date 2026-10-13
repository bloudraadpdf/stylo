/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::parser::Parse;
use crate::properties::longhands::list_style_position::computed_value::T as ListStylePosition;
use crate::selector_parser::PseudoElement;

fn adjusted_display(
    origin: &str,
    position: ListStylePosition,
    pseudo: Option<&PseudoElement>,
) -> Display {
    let device = crate::test_support::test_device();
    let url = crate::test_support::test_url_data();
    let context = crate::test_support::author_parser_context(&url);
    let mut input = cssparser::ParserInput::new(origin);
    let display = Display::parse(&context, &mut cssparser::Parser::new(&mut input))
        .expect("the originating display must parse");
    let mut parent = StyleBuilder::for_inheritance(&device, None, None, None);
    parent.mutate_box().set_display(display);
    parent.mutate_list().set_list_style_position(position);
    let parent = parent.build();
    let mut marker = StyleBuilder::for_inheritance(&device, None, Some(&parent), pseudo);
    marker.mutate_box().set_display(Display::Inline);
    StyleAdjuster::new(&mut marker).blockify_if_necessary(&parent, true);
    marker.get_box().clone_display()
}

#[test]
fn outside_block_container_markers_are_blockified() {
    for origin in [
        "list-item",
        "flow-root list-item",
        "inline flow-root list-item",
    ] {
        assert_eq!(
            adjusted_display(
                origin,
                ListStylePosition::Outside,
                Some(&PseudoElement::Marker)
            ),
            Display::Block,
            "{origin}",
        );
    }
}

#[test]
fn inside_and_inline_flow_markers_stay_inline() {
    for origin in [
        "list-item",
        "inline list-item",
        "flow-root list-item",
        "inline flow-root list-item",
    ] {
        assert_eq!(
            adjusted_display(
                origin,
                ListStylePosition::Inside,
                Some(&PseudoElement::Marker)
            ),
            Display::Inline,
            "{origin}",
        );
    }
    assert_eq!(
        adjusted_display(
            "inline list-item",
            ListStylePosition::Outside,
            Some(&PseudoElement::Marker)
        ),
        Display::Inline,
    );
}

#[test]
fn non_marker_pseudos_and_non_list_origins_are_not_blockified() {
    for pseudo in [
        None,
        Some(&PseudoElement::Before),
        Some(&PseudoElement::After),
    ] {
        assert_eq!(
            adjusted_display("list-item", ListStylePosition::Outside, pseudo),
            Display::Inline
        );
    }
    for origin in ["block", "inline-block", "none", "contents"] {
        assert_eq!(
            adjusted_display(
                origin,
                ListStylePosition::Outside,
                Some(&PseudoElement::Marker)
            ),
            Display::Inline,
            "{origin}",
        );
    }
}
