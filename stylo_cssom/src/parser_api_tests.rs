use super::*;

#[test]
fn generic_rules_keep_unknown_names_and_option_selected_bodies() {
    let mut options = SyntaxOptions::default();
    options.set_at_rule("MeDiA", SyntaxBodyKind::Rules);
    let rules = parse_stylesheet(
        "<!-- @media print { strange ?? { novel: fn(1, [a,b]); } } --> @future yes;",
        &options,
    )
    .unwrap();
    assert_eq!(rules.len(), 2);
    let SyntaxRule::AtRule {
        body: Some(body), ..
    } = &rules[0]
    else {
        panic!("media block")
    };
    let SyntaxRule::QualifiedRule { body, .. } = &body[0] else {
        panic!("generic rule")
    };
    assert!(matches!(&body[0], SyntaxRule::Declaration { name, .. } if name.as_ref() == "novel"));
    assert!(matches!(&rules[1], SyntaxRule::AtRule { body: None, .. }));
}

#[test]
fn component_values_keep_whitespace_and_split_only_top_level_commas() {
    let values = parse_comma_value_list("a/**/b,fn(x,y),[z,w]").unwrap();
    assert_eq!(values.len(), 3);
    assert_eq!(serialize_values(&values[0]), "a/**/b");
    assert!(matches!(&values[1][0], SyntaxValue::Function { args, .. } if args.len() == 2));
    assert!(matches!(&values[2][0], SyntaxValue::Block { body, .. } if body.len() == 3));
    assert_eq!(
        serialize_values(&parse_value_list("a\r\nb\x0cc\0").unwrap()),
        "a\nb\nc�"
    );
    assert!(parse_comma_value_list("").unwrap().is_empty());
}

#[test]
fn single_parsers_reject_empty_and_multiple_results() {
    let options = SyntaxOptions::default();
    assert_eq!(parse_rule("", &options), Err(SyntaxParseError::InvalidRule));
    assert_eq!(
        parse_rule("a{} b{}", &options),
        Err(SyntaxParseError::InvalidRule)
    );
    assert_eq!(
        parse_declaration("not-a-declaration"),
        Err(SyntaxParseError::InvalidDeclaration)
    );
    assert_eq!(parse_value("a b"), Err(SyntaxParseError::InvalidValue));
    assert_eq!(parse_value(" \t"), Err(SyntaxParseError::InvalidValue));
}

#[test]
fn declaration_lists_recover_without_dropping_later_declarations() {
    let rules = parse_declaration_list(
        "broken; a: fn(1;2); @unknown; b: two !IMPORTANT;",
        &SyntaxOptions::default(),
    )
    .unwrap();
    assert_eq!(rules.len(), 3);
    assert_eq!(serialize_rule(&rules[0]), "a: fn(1;2);");
    assert!(matches!(&rules[1], SyntaxRule::AtRule { body: None, .. }));
    assert_eq!(serialize_rule(&rules[2]), "b: two !IMPORTANT;");
}

#[test]
fn syntax_serialisation_preserves_boundaries_and_autocloses() {
    let options = SyntaxOptions::default();
    let rule = parse_rule("@unknown { thing: fn(a/**/b, [1", &options).unwrap();
    let text = serialize_rule(&rule);
    assert_eq!(parse_rule(&text, &options).unwrap(), rule);
    assert_eq!(
        serialize_values(&[
            SyntaxValue::Token("1".into()),
            SyntaxValue::Token("px".into())
        ]),
        "1/**/px"
    );
    assert_eq!(
        serialize_values(&[
            SyntaxValue::Token("/".into()),
            SyntaxValue::Token("*".into())
        ]),
        "//**/*"
    );
    assert_eq!(
        parse_value_list(&"(".repeat(300)),
        Err(SyntaxParseError::NestingLimit)
    );
}
