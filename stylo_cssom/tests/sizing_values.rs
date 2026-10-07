use stylo_cssom::declaration_parser::{
    CssomDeclarationContext, CssomDeclarationPriority, cssom_declaration_remove_property,
    cssom_declaration_set_property, inline_style_get_property_value, parse_cssom_declaration_block,
    parse_inline_style_block,
};
use stylo_cssom::declaration_serialization::serialise_cssom_declaration_block;

#[test]
fn sizing_shorthands_have_native_cssom_expansions() {
    for prefix in ["", "min-", "max-"] {
        let property = format!("{prefix}size");
        for value in ["100px", "inherit", "initial", "unset", "revert-layer"] {
            let block = parse_inline_style_block(&format!("{property}:{value}"));
            for axis in ["width", "height"] {
                assert_eq!(
                    inline_style_get_property_value(&block, &format!("{prefix}{axis}")).as_deref(),
                    Some(value),
                    "{property}:{value}"
                );
            }
            assert_eq!(
                inline_style_get_property_value(&block, &property).as_deref(),
                Some(value)
            );
        }
    }
}

#[test]
fn frame_sizing_keywords_have_native_cssom_values() {
    for value in [
        "normal",
        "content",
        "10px",
        "content-width content-height",
        "content-width, content-height",
    ] {
        let block = parse_inline_style_block(&format!("frame-sizing:{value}"));
        assert_eq!(
            inline_style_get_property_value(&block, "frame-sizing"),
            None,
            "{value}"
        );
    }
    for value in [
        "auto",
        "content-width",
        "content-height",
        "content-inline-size",
        "content-block-size",
    ] {
        let block = parse_inline_style_block(&format!("frame-sizing:{value}"));
        assert_eq!(
            inline_style_get_property_value(&block, "frame-sizing").as_deref(),
            Some(value),
            "{value}"
        );
    }
}

#[test]
fn page_size_cssom_mutations_keep_descriptor_identity() {
    let mut block = parse_cssom_declaration_block("size:A4", CssomDeclarationContext::Page);
    assert!(cssom_declaration_set_property(
        &mut block,
        "size",
        "100px 200px",
        CssomDeclarationPriority::Normal,
        CssomDeclarationContext::Page,
    ));
    assert_eq!(
        serialise_cssom_declaration_block(&block).into_css_text(),
        "size: 100px 200px;"
    );
    assert_eq!(
        cssom_declaration_remove_property(&mut block, "size").as_deref(),
        Some("100px 200px")
    );
    assert_eq!(
        serialise_cssom_declaration_block(&block).into_css_text(),
        ""
    );
}

/// css-page-3 §7.1: `size` in a page rule is the page size descriptor; it does not also parse as the css-sizing-4
/// `size` shorthand of the ordinary properties that a page rule accepts.
#[test]
fn a_page_rule_parses_size_only_as_the_page_size_descriptor() {
    let rule =
        stylo_cssom::rule_parser::ParsedCssRule::parse("@page { margin: 20pt; size: 200pt 300pt }")
            .expect("the page rule must parse");
    let node = rule.to_rule_node();
    let names = node
        .payload()
        .declaration_block()
        .expect("a page rule has a declaration block")
        .declarations()
        .iter()
        .map(|declaration| declaration.name())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "margin-top",
            "margin-right",
            "margin-bottom",
            "margin-left",
            "size"
        ]
    );
}
