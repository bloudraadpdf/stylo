/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use super::*;
use crate::computed_values::writing_mode::T as WritingMode;
use crate::servo::media_queries::Device;
use crate::values::computed::RubyPosition;
use servo_arc::Arc;

fn ruby_style(
    device: &Device,
    parent: Option<&ComputedValues>,
    display: Display,
    writing_mode: WritingMode,
    position: RubyPosition,
) -> Arc<ComputedValues> {
    let mut style = StyleBuilder::for_inheritance(device, None, parent, None);
    style.mutate_box().set_display(display);
    style.mutate_inherited_box().set_writing_mode(writing_mode);
    style.mutate_inherited_text().set_ruby_position(position);
    style.writing_mode = crate::logical_geometry::WritingMode::new(style.get_inherited_box());
    StyleAdjuster::new(&mut style)
        .adjust_for_inter_character_ruby(parent.unwrap_or(device.default_computed_values()));
    style.build()
}

#[test]
fn inter_character_adjusts_annotations_but_not_annotation_containers() {
    let device = crate::test_support::test_device();
    for root_mode in [
        WritingMode::HorizontalTb,
        WritingMode::VerticalRl,
        WritingMode::VerticalLr,
    ] {
        let root = ruby_style(
            &device,
            None,
            Display::Ruby,
            root_mode,
            RubyPosition::InterCharacter,
        );
        let container = ruby_style(
            &device,
            Some(&root),
            Display::RubyTextContainer,
            WritingMode::VerticalLr,
            RubyPosition::InterCharacter,
        );
        assert_eq!(
            container.get_inherited_box().clone_writing_mode(),
            WritingMode::VerticalLr
        );
        for parent in [&*root, &*container] {
            let annotation = ruby_style(
                &device,
                Some(parent),
                Display::RubyText,
                WritingMode::HorizontalTb,
                RubyPosition::InterCharacter,
            );
            let expected = if root_mode == WritingMode::HorizontalTb {
                WritingMode::VerticalRl
            } else {
                WritingMode::HorizontalTb
            };
            assert_eq!(
                annotation.get_inherited_box().clone_writing_mode(),
                expected
            );
            assert_eq!(
                annotation.writing_mode.is_vertical(),
                expected != WritingMode::HorizontalTb
            );
            assert_eq!(
                annotation.ruby_container_writing_mode(),
                root.ruby_container_writing_mode()
            );
        }
    }
}

#[test]
fn inter_character_context_resets_at_nested_ruby_and_preserves_other_positions() {
    use crate::values::specified::box_::{DisplayInside, DisplayOutside};

    let device = crate::test_support::test_device();
    let ordinary = ruby_style(
        &device,
        None,
        Display::Block,
        WritingMode::HorizontalTb,
        RubyPosition::Over,
    );
    assert_eq!(ordinary.ruby_container_writing_mode(), None);
    let outer = ruby_style(
        &device,
        None,
        Display::Ruby,
        WritingMode::VerticalRl,
        RubyPosition::InterCharacter,
    );
    let nested = ruby_style(
        &device,
        Some(&outer),
        Display::new(DisplayOutside::Block, DisplayInside::Ruby),
        WritingMode::HorizontalTb,
        RubyPosition::InterCharacter,
    );
    for position in [
        RubyPosition::Over,
        RubyPosition::Under,
        RubyPosition::InterCharacter,
    ] {
        let container = ruby_style(
            &device,
            Some(&nested),
            Display::RubyTextContainer,
            WritingMode::VerticalLr,
            position,
        );
        let annotation = ruby_style(
            &device,
            Some(&container),
            Display::RubyText,
            WritingMode::VerticalLr,
            RubyPosition::Over,
        );
        assert_eq!(
            annotation.get_inherited_box().clone_writing_mode(),
            if position == RubyPosition::InterCharacter {
                WritingMode::VerticalRl
            } else {
                WritingMode::VerticalLr
            },
        );
    }
    let unrelated = ruby_style(
        &device,
        Some(&nested),
        Display::InlineBlock,
        WritingMode::HorizontalTb,
        RubyPosition::InterCharacter,
    );
    assert_eq!(
        unrelated.get_inherited_box().clone_writing_mode(),
        WritingMode::HorizontalTb
    );
}

#[test]
fn ruby_container_mode_is_distinct_even_when_annotation_container_properties_match() {
    let device = crate::test_support::test_device();
    let container = |root_mode| {
        let root = ruby_style(
            &device,
            None,
            Display::Ruby,
            root_mode,
            RubyPosition::InterCharacter,
        );
        ruby_style(
            &device,
            Some(&root),
            Display::RubyTextContainer,
            WritingMode::VerticalLr,
            RubyPosition::InterCharacter,
        )
    };
    let horizontal = container(WritingMode::HorizontalTb);
    let vertical = container(WritingMode::VerticalRl);
    assert_eq!(
        horizontal.get_inherited_box().clone_writing_mode(),
        vertical.get_inherited_box().clone_writing_mode()
    );
    assert_ne!(
        horizontal.ruby_container_writing_mode(),
        vertical.ruby_container_writing_mode()
    );
    for parent in [&*horizontal, &*vertical, &*horizontal] {
        let annotation = ruby_style(
            &device,
            Some(parent),
            Display::RubyText,
            WritingMode::HorizontalTb,
            RubyPosition::InterCharacter,
        );
        assert_eq!(
            annotation.writing_mode.is_vertical(),
            !parent.ruby_container_writing_mode().unwrap().is_vertical()
        );
    }
}

#[test]
fn ruby_writing_mode_precedes_logical_mapping_and_respects_blockification() {
    use crate::properties::LonghandId;

    let device = crate::test_support::test_device();
    let root = ruby_style(
        &device,
        None,
        Display::Ruby,
        WritingMode::HorizontalTb,
        RubyPosition::InterCharacter,
    );
    for case in [
        "annotation",
        "absolute",
        "float",
        "block",
        "top-layer",
        "flex-item",
    ] {
        let mut parent = StyleBuilder::for_inheritance(&device, None, Some(&root), None);
        if case == "flex-item" {
            parent.mutate_box().set_display(Display::Flex);
        } else {
            parent.mutate_box().set_display(Display::Ruby);
        }
        let parent = parent.build();
        let mut annotation = StyleBuilder::for_inheritance(&device, None, Some(&parent), None);
        annotation.mutate_box().set_display(if case == "block" {
            Display::Block
        } else {
            Display::RubyText
        });
        match case {
            "absolute" => annotation.mutate_box().set_position(Position::Absolute),
            "float" => annotation.mutate_box().set_float(Float::Left),
            "top-layer" => annotation.mutate_box().set__servo_top_layer(
                crate::properties::longhands::_servo_top_layer::computed_value::T::Top,
            ),
            _ => {},
        }
        StyleAdjuster::new(&mut annotation).adjust_ruby_writing_mode(&parent, false);
        let expected = if case == "annotation" {
            WritingMode::VerticalRl
        } else {
            WritingMode::HorizontalTb
        };
        assert_eq!(
            annotation.get_inherited_box().clone_writing_mode(),
            expected,
            "{case}"
        );
        assert_eq!(
            LonghandId::InlineSize.to_physical(annotation.writing_mode),
            if case == "annotation" {
                LonghandId::Height
            } else {
                LonghandId::Width
            },
            "{case}",
        );
        assert_eq!(
            LonghandId::BlockSize.to_physical(annotation.writing_mode),
            if case == "annotation" {
                LonghandId::Width
            } else {
                LonghandId::Height
            },
            "{case}",
        );
    }
}

#[test]
fn improper_ruby_children_use_their_anonymous_containers_layout_parent_mode() {
    let device = crate::test_support::test_device();
    let outside = ruby_style(
        &device,
        None,
        Display::Block,
        WritingMode::HorizontalTb,
        RubyPosition::Over,
    );
    let orphan_container = ruby_style(
        &device,
        Some(&outside),
        Display::RubyTextContainer,
        WritingMode::VerticalLr,
        RubyPosition::InterCharacter,
    );
    let orphan = ruby_style(
        &device,
        Some(&orphan_container),
        Display::RubyText,
        WritingMode::HorizontalTb,
        RubyPosition::InterCharacter,
    );
    assert_eq!(
        orphan_container.get_inherited_box().clone_writing_mode(),
        WritingMode::VerticalLr
    );
    assert_eq!(
        orphan.get_inherited_box().clone_writing_mode(),
        WritingMode::VerticalRl
    );
    let direct_orphan = ruby_style(
        &device,
        Some(&outside),
        Display::RubyText,
        WritingMode::HorizontalTb,
        RubyPosition::InterCharacter,
    );
    assert_eq!(
        direct_orphan.get_inherited_box().clone_writing_mode(),
        WritingMode::HorizontalTb
    );
    let anonymous_parent = ruby_style(
        &device,
        None,
        Display::Block,
        WritingMode::HorizontalTb,
        RubyPosition::InterCharacter,
    );
    let direct_orphan = ruby_style(
        &device,
        Some(&anonymous_parent),
        Display::RubyText,
        WritingMode::HorizontalTb,
        RubyPosition::Over,
    );
    assert_eq!(
        direct_orphan.get_inherited_box().clone_writing_mode(),
        WritingMode::VerticalRl
    );
    let outer = ruby_style(
        &device,
        None,
        Display::Ruby,
        WritingMode::HorizontalTb,
        RubyPosition::InterCharacter,
    );
    for display in [Display::Inline, Display::InlineBlock] {
        let intermediate = ruby_style(
            &device,
            Some(&outer),
            display,
            WritingMode::VerticalLr,
            RubyPosition::InterCharacter,
        );
        let inner = ruby_style(
            &device,
            Some(&intermediate),
            Display::RubyText,
            WritingMode::HorizontalTb,
            RubyPosition::InterCharacter,
        );
        assert_eq!(
            inner.get_inherited_box().clone_writing_mode(),
            WritingMode::HorizontalTb
        );
    }
    let contents = ruby_style(
        &device,
        Some(&outer),
        Display::Contents,
        WritingMode::VerticalLr,
        RubyPosition::InterCharacter,
    );
    assert_eq!(
        contents.ruby_container_writing_mode(),
        outer.ruby_container_writing_mode()
    );
    let mut annotation = StyleBuilder::for_inheritance(&device, None, Some(&contents), None);
    annotation.mutate_box().set_display(Display::RubyText);
    StyleAdjuster::new(&mut annotation).adjust_ruby_writing_mode(&outer, false);
    assert_eq!(
        annotation.get_inherited_box().clone_writing_mode(),
        WritingMode::VerticalRl
    );
}

#[test]
fn inter_character_uses_annotation_container_position_instead_of_annotation_property() {
    let device = crate::test_support::test_device();
    let root = ruby_style(
        &device,
        None,
        Display::Ruby,
        WritingMode::HorizontalTb,
        RubyPosition::Over,
    );
    for (container_position, annotation_position, expected) in [
        (
            RubyPosition::InterCharacter,
            RubyPosition::Over,
            WritingMode::VerticalRl,
        ),
        (
            RubyPosition::Over,
            RubyPosition::InterCharacter,
            WritingMode::HorizontalTb,
        ),
    ] {
        let container = ruby_style(
            &device,
            Some(&root),
            Display::RubyTextContainer,
            WritingMode::VerticalLr,
            container_position,
        );
        let annotation = ruby_style(
            &device,
            Some(&container),
            Display::RubyText,
            WritingMode::HorizontalTb,
            annotation_position,
        );
        assert_eq!(
            annotation.get_inherited_box().clone_writing_mode(),
            expected
        );
    }
}
