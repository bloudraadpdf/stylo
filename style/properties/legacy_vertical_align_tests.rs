use super::{
    declaration_block::parse_style_attribute, PropertyDeclaration, PropertyDeclarationBlock,
};
use crate::stylesheets::{CssRuleType, UrlExtraData};

fn parse(css: &str) -> PropertyDeclarationBlock {
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

fn value(block: &PropertyDeclarationBlock, name: &str) -> String {
    let mut output = String::new();
    block
        .property_value_to_css(
            &super::PropertyId::parse_enabled_for_all_content(name).unwrap(),
            &mut output,
        )
        .unwrap();
    output
}

#[test]
fn native_vertical_align_is_independent_from_modern_baseline_properties() {
    for css in [
        "vertical-align:middle;alignment-baseline:text-top",
        "alignment-baseline:text-top;vertical-align:middle",
        "vertical-align:middle!important;alignment-baseline:text-top!important",
    ] {
        let block = parse(css);
        assert_eq!(value(&block, "vertical-align"), "middle", "{css}");
        assert_eq!(value(&block, "alignment-baseline"), "text-top", "{css}");
        assert!(block
            .declarations()
            .iter()
            .any(|declaration| matches!(declaration, PropertyDeclaration::VerticalAlign(_))));
    }
    assert_eq!(
        value(&parse("alignment-baseline:middle"), "vertical-align"),
        ""
    );
    assert!(parse("vertical-align:first middle").is_empty());
    assert!(!parse("baseline-source:first;alignment-baseline:middle").is_empty());
}

#[test]
fn native_vertical_align_css_text_round_trips() {
    for css in [
        "vertical-align:middle;alignment-baseline:text-top",
        "alignment-baseline:text-top;vertical-align:middle",
        "vertical-align:middle!important;alignment-baseline:text-top",
        "vertical-align:middle;alignment-baseline:text-top!important",
        "vertical-align:middle;vertical-align:text-bottom",
        "vertical-align:middle;all:initial",
        "vertical-align:middle;all:unset",
    ] {
        let block = parse(css);
        let mut serialised = String::new();
        block.to_css(&mut serialised).unwrap();
        assert!(!serialised.contains("-bd-legacy"), "{css}: {serialised}");
        let restored = parse(&serialised);
        for property in [
            "vertical-align",
            "alignment-baseline",
            "baseline-shift",
            "baseline-source",
        ] {
            assert_eq!(
                value(&restored, property),
                value(&block, property),
                "{css}: {serialised}"
            );
            let id = super::PropertyId::parse_enabled_for_all_content(property).unwrap();
            assert_eq!(
                restored.property_priority(&id),
                block.property_priority(&id),
                "{css}: {serialised}"
            );
        }
    }
}

#[test]
fn native_vertical_align_removal_preserves_modern_properties() {
    let mut block = parse("vertical-align:middle;alignment-baseline:text-top");
    let property = super::PropertyId::parse_enabled_for_all_content("vertical-align").unwrap();
    let first = block.first_declaration_to_remove(&property).unwrap();
    block.remove_property(&property, first);
    assert_eq!(value(&block, "vertical-align"), "");
    assert_eq!(value(&block, "alignment-baseline"), "text-top");
    let update = parse("vertical-align:text-bottom");
    for declaration in update.declarations() {
        block.push(
            declaration.clone(),
            super::declaration_block::Importance::Normal,
        );
    }
    assert_eq!(value(&block, "vertical-align"), "text-bottom");
    assert_eq!(value(&block, "alignment-baseline"), "text-top");
}
