#[test]
fn parser_boundaries_require_grammar_specific_inputs() {
    let declarations = include_str!("declaration_parser.rs");
    let rules = include_str!("rule_parser.rs");
    let typed_om = include_str!("typed_om.rs");

    for input in ["DeclarationInput", "DeclarationPropertyInput"] {
        assert!(declarations.contains(&format!("pub struct {input}")));
    }
    assert!(rules.contains("pub struct RuleInput"));
    for input in [
        "TypedOmComputedNumericInput",
        "TypedOmUnparsedInput",
        "TypedOmListIterationsInput",
        "TypedOmBackgroundSizeInput",
        "TypedOmTextDecorationSkipInput",
        "TypedOmColorInput",
        "TypedOmImageInput",
        "TypedOmTransformInput",
        "TypedOmFontStretchInput",
    ] {
        assert!(typed_om.contains(input));
    }
}

#[test]
fn observable_outputs_have_no_generic_string_access() {
    for source in [
        include_str!("declaration_serialization.rs"),
        include_str!("rule_parser.rs"),
        include_str!("value_serialization.rs"),
    ] {
        assert!(!source.contains("impl AsRef<str>"));
        assert!(!source.contains("impl std::ops::Deref"));
    }

    for (source, output) in [
        (
            include_str!("declaration_serialization.rs"),
            "DeclarationSerialization",
        ),
        (include_str!("rule_parser.rs"), "RuleSerialization"),
        (
            include_str!("value_serialization.rs"),
            "ResolvedValueSerialization",
        ),
    ] {
        let implementation = source
            .split_once(&format!("impl {output} {{"))
            .expect("the observable output implementation must exist")
            .1
            .split_once("}\n")
            .expect("the observable output implementation must terminate")
            .0;
        assert!(!implementation.contains("as_str"));
    }
}

#[test]
fn observable_block_text_is_not_used_to_recover_parser_state() {
    let rules = include_str!("rule_parser.rs");
    assert!(!rules.contains("parse_cssom_declaration_block(&block.serialization"));
    assert!(!rules.contains("parse_cssom_declaration_block(&declarations.serialization"));
    assert!(!rules.contains("parse_rule_node(&node.serialization"));
}
