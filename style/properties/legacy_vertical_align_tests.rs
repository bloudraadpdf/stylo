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
fn modern_vertical_align_grammar_and_resets_remain_available() {
    for (css, expected) in [
        ("vertical-align:first", "first"),
        ("vertical-align:baseline 0", "baseline"),
        ("vertical-align:super middle first", "first middle super"),
    ] {
        assert_eq!(value(&parse(css), "vertical-align"), expected, "{css}");
    }
    assert_eq!(
        value(
            &parse("baseline-source:first;vertical-align:baseline"),
            "baseline-source"
        ),
        "auto"
    );
}

#[test]
fn vertical_align_expands_modern_properties_and_retains_private_compatibility() {
    for (css, expected) in [
        (
            "vertical-align:middle;alignment-baseline:text-top",
            "text-top",
        ),
        (
            "alignment-baseline:text-top;vertical-align:middle",
            "middle",
        ),
        (
            "vertical-align:middle!important;alignment-baseline:text-top!important",
            "text-top",
        ),
    ] {
        let block = parse(css);
        assert_eq!(value(&block, "vertical-align"), expected, "{css}");
        assert_eq!(value(&block, "alignment-baseline"), expected, "{css}");
        assert!(block.declarations().iter().any(|declaration| matches!(
            declaration,
            PropertyDeclaration::BdLegacyVerticalAlign(
                crate::values::specified::VerticalAlign::Middle
            )
        )));
    }
    assert_eq!(
        value(&parse("alignment-baseline:middle"), "vertical-align"),
        ""
    );
    assert!(!parse("vertical-align:first middle").is_empty());
    assert!(parse("-bd-legacy-vertical-align:middle").is_empty());
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
fn vertical_align_removal_removes_all_components() {
    let mut block = parse("vertical-align:middle;alignment-baseline:text-top");
    let property = super::PropertyId::parse_enabled_for_all_content("vertical-align").unwrap();
    let first = block.first_declaration_to_remove(&property).unwrap();
    block.remove_property(&property, first);
    assert_eq!(value(&block, "vertical-align"), "");
    assert_eq!(value(&block, "alignment-baseline"), "");
    assert!(!block
        .declarations()
        .iter()
        .any(|declaration| matches!(declaration, PropertyDeclaration::BdLegacyVerticalAlign(_))));
    let update = parse("vertical-align:text-bottom");
    for declaration in update.declarations() {
        block.push(
            declaration.clone(),
            super::declaration_block::Importance::Normal,
        );
    }
    assert_eq!(value(&block, "vertical-align"), "text-bottom");
    assert_eq!(value(&block, "alignment-baseline"), "text-bottom");
}

#[test]
fn css_wide_resets_include_the_private_compatibility_component() {
    for property in ["all", "vertical-align"] {
        for keyword in ["initial", "inherit", "unset"] {
            let block = parse(&format!("vertical-align:middle;{property}:{keyword}"));
            assert!(
                block.declarations().iter().any(|declaration| matches!(
                    declaration,
                    PropertyDeclaration::CSSWideKeyword(value)
                        if value.id == super::LonghandId::BdLegacyVerticalAlign
                )),
                "{property}:{keyword}"
            );
        }
    }
}

#[test]
fn native_baseline_interpolates_as_zero_shift() {
    use crate::values::animated::{Animate, Procedure};
    use crate::values::generics::box_::GenericVerticalAlign;

    for progress in [-0.5, 0.0, 0.3, 0.6, 1.0, 1.5] {
        assert_eq!(
            GenericVerticalAlign::Baseline.animate(
                &GenericVerticalAlign::Length(100.0f32),
                Procedure::Interpolate { progress },
            ),
            Ok(GenericVerticalAlign::Length((100.0 * progress) as f32))
        );
    }
}
