use stylo_cssom_model::RuleDeclarationBlock;

const COMMON_PROPERTIES: [&str; 4] = [
    "view-transition-scope",
    "view-transition-group",
    "grid-lanes-direction",
    "-webkit-box-orient",
];

fn assert_reset_members(block: &RuleDeclarationBlock, keyword: &str) {
    for name in COMMON_PROPERTIES {
        let member = block
            .declarations()
            .iter()
            .find(|declaration| declaration.matches_name(name))
            .unwrap_or_else(|| panic!("missing native All member: {name}"));
        assert_eq!(member.value(), keyword, "{name}");
        assert!(!member.important(), "{name}");
    }
}

#[test]
fn original_all_tokens_preserve_native_cascade_and_importance() {
    let rule = super::ParsedCssRule::parse(
        r"#target { view-transition-scope:all!important; a\6cl: /* reset */ i\6e itial; view-transition-scope:none; all: nonsense; --custom: untouched }",
    ).unwrap().to_rule_node();
    let block = rule.payload().declaration_block().unwrap();
    for name in COMMON_PROPERTIES {
        let member = block
            .declarations()
            .iter()
            .find(|declaration| declaration.matches_name(name))
            .unwrap();
        assert_eq!(
            member.value(),
            if name == "view-transition-scope" {
                "all"
            } else {
                "initial"
            },
            "{name}"
        );
        assert_eq!(
            member.important(),
            name == "view-transition-scope",
            "{name}"
        );
    }
    assert_eq!(
        block
            .declarations()
            .iter()
            .filter(|member| member.matches_name("view-transition-scope"))
            .count(),
        1
    );
    assert_eq!(block.declarations().last().unwrap().name(), "--custom");
    assert_eq!(block.declarations().last().unwrap().value(), "untouched");
}

#[test]
fn original_all_members_stay_with_their_native_nested_declaration_block() {
    let rule = super::ParsedCssRule::parse(
        ".parent { all: initial; & .child { all: inherit; } all: unset; }",
    )
    .unwrap()
    .to_rule_node();
    assert_reset_members(rule.payload().declaration_block().unwrap(), "initial");
    let nested = rule.payload().nested();
    assert_eq!(nested.len(), 2);
    for (rule, keyword) in nested.iter().zip(["inherit", "unset"]) {
        assert_reset_members(rule.payload().declaration_block().unwrap(), keyword);
    }
}

#[test]
fn unrelated_cssom_mutation_keeps_authored_all_member_sources() {
    use crate::declaration_parser::{
        CssomDeclarationPriority, DeclarationPropertyInput, mutate_style_rule_declaration,
    };
    let rule = super::ParsedCssRule::parse(
        "#target { all: initial; view-transition-scope: all !important; }",
    )
    .unwrap()
    .to_rule_node();
    let changed = mutate_style_rule_declaration(
        &rule,
        DeclarationPropertyInput::new("color", "red"),
        CssomDeclarationPriority::Normal,
    )
    .unwrap();
    for name in COMMON_PROPERTIES {
        let original = rule
            .payload()
            .declaration_block()
            .unwrap()
            .declarations()
            .iter()
            .find(|declaration| declaration.matches_name(name))
            .unwrap();
        let changed = changed
            .payload()
            .declaration_block()
            .unwrap()
            .declarations()
            .iter()
            .find(|declaration| declaration.matches_name(name))
            .unwrap();
        assert_eq!(original, changed, "{name}");
    }
}

#[test]
fn native_rule_setters_publish_public_values_priority_and_member_order() {
    use crate::declaration_parser::{
        CssomDeclarationPriority, DeclarationPropertyInput, mutate_style_rule_declaration,
    };
    let rule = super::ParsedCssRule::parse("#target { color: red; }")
        .unwrap()
        .to_rule_node();
    let changed = mutate_style_rule_declaration(
        &rule,
        DeclarationPropertyInput::new("view-transition-scope", "all"),
        CssomDeclarationPriority::Important,
    )
    .expect("a supported native property must pass rule setter admission");
    let block = changed.payload().declaration_block().unwrap();
    assert_eq!(
        block
            .declarations()
            .iter()
            .map(|declaration| declaration.name())
            .collect::<Vec<_>>(),
        ["color", "view-transition-scope"]
    );
    let scope = &block.declarations()[1];
    assert_eq!(scope.value(), "all");
    assert!(scope.important());
    assert!(!block.serialization().contains("--moegoe-"));

    let authored =
        super::ParsedCssRule::parse("#target { view-transition-scope: all; color: red; }")
            .unwrap()
            .to_rule_node();
    assert_eq!(
        authored
            .payload()
            .declaration_block()
            .unwrap()
            .declarations()
            .iter()
            .map(|declaration| declaration.name())
            .collect::<Vec<_>>(),
        ["view-transition-scope", "color"]
    );

    let reset = mutate_style_rule_declaration(
        &changed,
        DeclarationPropertyInput::new("all", "initial"),
        CssomDeclarationPriority::Normal,
    )
    .unwrap();
    let overridden = mutate_style_rule_declaration(
        &reset,
        DeclarationPropertyInput::new("view-transition-scope", "all"),
        CssomDeclarationPriority::Normal,
    )
    .unwrap();
    assert!(
        !overridden
            .payload()
            .declaration_block()
            .unwrap()
            .shorthand_values()
            .iter()
            .any(|declaration| declaration.name() == "all"),
        "all cannot serialise after one native member differs"
    );
}

#[test]
fn keyframe_all_sources_and_mutations_obey_the_native_declaration_domain() {
    let keyframes = super::ParsedCssRule::parse(
        "@keyframes example { from { all: initial; all: inherit !important; view-transition-scope: all !important; } }",
    )
    .unwrap()
    .to_rule_node();
    let frame = &keyframes.payload().nested()[0];
    for (frame, keyword) in [
        (frame.clone(), "initial"),
        (
            super::replace_keyframe_declarations(
                frame,
                "all: unset; view-transition-scope: all !important",
            )
            .unwrap(),
            "unset",
        ),
    ] {
        let block = frame.payload().declaration_block().unwrap();
        assert_reset_members(block, keyword);
    }
}

#[test]
fn an_explicit_variable_reference_after_all_keeps_its_source_order() {
    let rule = super::ParsedCssRule::parse(
        "#target { all: initial; view-transition-scope: var(--scope); --scope: all }",
    )
    .unwrap()
    .to_rule_node();
    let declarations = rule.payload().declaration_block().unwrap().declarations();
    let scope = declarations
        .iter()
        .position(|declaration| declaration.matches_name("view-transition-scope"))
        .unwrap();
    let group = declarations
        .iter()
        .position(|declaration| declaration.matches_name("view-transition-group"))
        .unwrap();
    assert!(scope > group);
    assert_eq!(declarations[scope].value(), "var(--scope)");
    assert!(!declarations[scope].important());
}

#[test]
fn pending_all_retains_public_member_sources_until_individually_overwritten() {
    use crate::declaration_parser::{
        CssomDeclarationPriority, DeclarationPropertyInput, mutate_style_rule_declaration,
    };

    let rule = super::ParsedCssRule::parse("#target { all:var(--reset)!important }")
        .unwrap()
        .to_rule_node();
    let block = rule.payload().declaration_block().unwrap();
    for property in COMMON_PROPERTIES {
        let name = property;
        let declaration = block
            .declarations()
            .iter()
            .find(|declaration| declaration.matches_name(name))
            .unwrap_or_else(|| panic!("pending All member missing: {name}"));
        assert_eq!(declaration.value(), "", "{name}");
        assert!(declaration.important(), "{name}");
        assert_eq!(
            declaration
                .pending_substitution()
                .map(|source| (source.shorthand().schema().name, source.tokens())),
            Some(("all", "var(--reset)")),
            "{name}"
        );
    }
    assert_eq!(
        block
            .shorthand_values()
            .iter()
            .find(|declaration| declaration.matches_name("all"))
            .map(|declaration| declaration.value()),
        Some("var(--reset)")
    );
    let changed = mutate_style_rule_declaration(
        &rule,
        DeclarationPropertyInput::new("view-transition-scope", "all"),
        CssomDeclarationPriority::Normal,
    )
    .unwrap();
    assert!(
        !changed
            .payload()
            .declaration_block()
            .unwrap()
            .shorthand_values()
            .iter()
            .any(|declaration| declaration.matches_name("all"))
    );
}
