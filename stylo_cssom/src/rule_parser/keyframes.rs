use super::{
    CanonicalCssDeclarationBlock, CssomRuleInterfaceName, ParsedCssRule, ParsedCssRuleKind,
    RuleInput, canonical_declaration_block,
};
use crate::declaration_parser::{
    CssomDeclarationContext, CssomDeclarationPriority, mutate_rule_declaration_block,
    parse_cssom_declaration_block, rule_declaration_block_from_cssom,
};
use cssparser::{Parser, ParserInput};
use style::{
    shared_lock::SharedRwLockReadGuard,
    stylesheets::{
        Origin,
        keyframes_rule::{Keyframe, KeyframePercentage, KeyframeSelector},
    },
    values::CustomIdent,
};
use style_traits::ToCss;
use stylo_cssom_model::{
    RuleCssomData, RuleDeclarationBlock, RuleDeclarationDomain, RuleGrammar, RuleGroupHeader,
    RuleKeyframeSelector, RuleNode,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CanonicalKeyframesRule {
    pub name: String,
    pub frames: Box<[ParsedCssRule]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CanonicalKeyframeRule {
    pub selector: RuleKeyframeSelector,
    pub declarations: CanonicalCssDeclarationBlock,
}

fn model_selector(selector: &KeyframeSelector) -> RuleKeyframeSelector {
    RuleKeyframeSelector::new(
        selector
            .percentages()
            .iter()
            .map(|value| value.0)
            .collect::<Vec<_>>(),
    )
    .expect("parsed keyframe percentages are nonempty and in range")
}

pub fn parse_keyframe_selector(text: &str) -> Option<RuleKeyframeSelector> {
    let mut input = ParserInput::new(text);
    Parser::new(&mut input)
        .parse_entirely(KeyframeSelector::parse)
        .ok()
        .map(|selector| model_selector(&selector))
}

pub fn serialize_keyframe_selector(selector: &RuleKeyframeSelector) -> String {
    selector
        .percentages()
        .iter()
        .map(|value| KeyframePercentage::new(*value).to_css_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn serialize_keyframe(selector: &RuleKeyframeSelector, declarations: &str) -> String {
    let separator = if declarations.is_empty() { "" } else { " " };
    format!(
        "{} {{ {declarations}{separator}}}",
        serialize_keyframe_selector(selector)
    )
}

pub(super) fn keyframes_header(name: &str) -> RuleGroupHeader {
    let mut serialized_name = String::new();
    let location = cssparser::SourceLocation { line: 0, column: 1 };
    if CustomIdent::from_ident(location, &name.into(), &["none"]).is_ok() {
        cssparser::serialize_identifier(name, &mut serialized_name)
            .expect("writing a string cannot fail");
    } else {
        cssparser::serialize_string(name, &mut serialized_name)
            .expect("writing a string cannot fail");
    }
    RuleGroupHeader::new(format!("@keyframes {serialized_name}"))
}

pub(super) fn serialize_keyframes<'a>(
    name: &str,
    frames: impl IntoIterator<Item = &'a str>,
) -> String {
    let mut result = format!("{} {{\n", keyframes_header(name).as_str());
    for frame in frames {
        result.push_str("  ");
        result.push_str(frame);
        result.push('\n');
    }
    result.push('}');
    result
}

pub(super) fn canonical_keyframe(
    frame: &Keyframe,
    guard: &SharedRwLockReadGuard,
    namespaces: &stylo_cssom_model::RuleNamespaceContext,
    source: &super::source::AuthoredSource<'_>,
) -> ParsedCssRule {
    let selector = model_selector(&frame.selector);
    let declarations = canonical_declaration_block(frame.block.read_with(guard));
    ParsedCssRule {
        serialization: serialize_keyframe(&selector, &declarations.serialization),
        source_location: Some(frame.source_location),
        projection: source
            .span(frame.source_location)
            .map(|span| span.text(source.text()).to_owned()),
        namespaces: namespaces.clone(),
        kind: ParsedCssRuleKind::Keyframe(CanonicalKeyframeRule {
            selector,
            declarations,
        }),
        interface_name: CssomRuleInterfaceName::Keyframe,
        grammar: RuleGrammar::Keyframe,
    }
}

pub fn parse_keyframe_rule(input: RuleInput<'_>) -> Option<RuleNode> {
    crate::context::initialise_required_servo_style_prefs();
    let (stylesheet, lock) = crate::context::parse_stylesheet_fragment("", Origin::Author);
    let guard = lock.read();
    let contents = stylesheet.contents.read_with(&guard);
    let frame = Keyframe::parse(input.text(), contents, &lock).ok()?;
    Some(
        canonical_keyframe(
            frame.read_with(&guard),
            &guard,
            &super::model_namespaces(&contents.namespaces),
            &super::source::AuthoredSource::new(
                input.text(),
                cssparser::UrlErrorRecovery::CssSyntax3,
            ),
        )
        .to_rule_node(),
    )
}

fn keyframe_node(selector: RuleKeyframeSelector, block: RuleDeclarationBlock) -> RuleNode {
    RuleNode::authored(
        RuleGrammar::Keyframe,
        serialize_keyframe(&selector, block.serialization()),
        [],
    )
    .with_declaration_block(block)
    .with_cssom_data(RuleCssomData::Keyframe { selector })
    .expect("keyframe data matches keyframe grammar")
}

pub fn replace_keyframe_selector(
    node: &RuleNode,
    selector: RuleKeyframeSelector,
) -> Option<RuleNode> {
    let RuleCssomData::Keyframe { .. } = node.cssom_data()? else {
        return None;
    };
    Some(keyframe_node(
        selector,
        node.payload().declaration_block()?.clone(),
    ))
}

pub fn replace_keyframe_declarations(node: &RuleNode, declarations: &str) -> Option<RuleNode> {
    let RuleCssomData::Keyframe { selector } = node.cssom_data()? else {
        return None;
    };
    let parsed = parse_cssom_declaration_block(declarations, CssomDeclarationContext::Keyframe);
    let block = rule_declaration_block_from_cssom(&parsed, RuleDeclarationDomain::Keyframe)
        .with_namespaces(node.payload().declaration_block()?.namespaces().clone());
    Some(keyframe_node(selector.clone(), block))
}

pub(super) fn mutate_keyframe_declaration(
    node: &RuleNode,
    property: &str,
    value: &str,
    priority: CssomDeclarationPriority,
) -> Option<RuleNode> {
    let RuleCssomData::Keyframe { selector } = node.cssom_data()? else {
        return None;
    };
    let block = mutate_rule_declaration_block(
        node.payload().declaration_block()?,
        property,
        value,
        priority,
        CssomDeclarationContext::Keyframe,
    )?;
    Some(keyframe_node(selector.clone(), block))
}

pub fn replace_keyframes_name(node: &RuleNode, name: &str) -> Option<RuleNode> {
    let RuleCssomData::Keyframes { .. } = node.cssom_data()? else {
        return None;
    };
    let children = node.payload().nested();
    let texts = children
        .iter()
        .map(RuleNode::serialization)
        .collect::<Vec<_>>();
    RuleNode::authored_with_group_header(
        RuleGrammar::Keyframes,
        serialize_keyframes(name, texts.iter().map(String::as_str)),
        children.to_vec(),
        keyframes_header(name),
    )
    .with_cssom_data(RuleCssomData::Keyframes { name: name.into() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selectors_preserve_order_duplicates_and_percentage_precision() {
        let selector = parse_keyframe_selector("to, from, 12.5%, 12.5%").unwrap();
        assert_eq!(selector.percentages(), &[1.0, 0.0, 0.125, 0.125]);
        assert_eq!(
            serialize_keyframe_selector(&selector),
            "100%, 0%, 12.5%, 12.5%"
        );
        assert_eq!(
            parse_keyframe_selector("FROM, TO"),
            parse_keyframe_selector("0%,100%")
        );
        for invalid in ["", "0", "-1%", "101%", "from,", "from {}", "50% 60%"] {
            assert!(parse_keyframe_selector(invalid).is_none(), "{invalid}");
        }
        for invalid in [
            vec![],
            vec![f32::NAN],
            vec![f32::INFINITY],
            vec![-0.1],
            vec![1.1],
        ] {
            assert!(RuleKeyframeSelector::new(invalid).is_none());
        }
    }

    #[test]
    fn one_keyframe_rule_has_its_own_parser_context() {
        let rule = parse_keyframe_rule(RuleInput::new(
            "from { opacity: .5; width: 10px !important; }",
        ))
        .unwrap();
        assert_eq!(rule.serialization(), "0% { opacity: 0.5; }");
        assert_eq!(
            rule.payload().declaration_block().unwrap().domain(),
            RuleDeclarationDomain::Keyframe
        );
        assert_eq!(
            parse_keyframe_rule(RuleInput::new("to {}"))
                .unwrap()
                .serialization(),
            "100% { }"
        );
        for invalid in ["p {}", "101% {}", "from {} to {}", "@media all {}"] {
            assert!(
                parse_keyframe_rule(RuleInput::new(invalid)).is_none(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn standalone_keyframe_source_uses_its_native_recovery_boundary() {
        for source in ["from { opacity:.5 }", "from { background:url(foo\"bar) }"] {
            let css = format!("{source} /* outside the native rule */");
            let rule = parse_keyframe_rule(RuleInput::new(&css))
                .expect("the standalone native rule must parse");
            assert_eq!(rule.grammar(), RuleGrammar::Keyframe);
            assert_eq!(rule.projection_serialization(), source);
        }
    }

    #[test]
    fn cssom_group_serialization_preserves_escaped_keyframes_name() {
        use stylo_cssom_model::{
            StyleDocumentHandle, StyleOrigin, StyleSheetCandidate, StyleSheetSourceContext,
            StyleState,
        };

        for (source, header) in [
            (r"@keyframes abc\{\}oops {}", r"@keyframes abc\{\}oops"),
            (r"@keyframes tail\  {}", r"@keyframes tail\ "),
        ] {
            let node = ParsedCssRule::parse(source)
                .expect("escaped characters belong to the keyframes name")
                .to_rule_node();
            let mut state = StyleState::new(StyleDocumentHandle::allocate());
            let context = StyleSheetSourceContext::constructed(StyleOrigin::Author);
            let sheet = state
                .create_stylesheet(StyleSheetCandidate::new(context.clone(), [node]))
                .expect("the stylesheet must bind");
            let empty = format!("{header} {{\n}}");
            assert_eq!(sheet.serialise(), empty);

            let rule = sheet.top_list().rule(0).unwrap();
            let nested = rule.nested_list().unwrap();
            let frame = parse_keyframe_rule(RuleInput::new("from { opacity: .5; }")).unwrap();
            let insert = state
                .prepare_insert_rule(&sheet, &nested, 0, frame)
                .unwrap();
            state.commit_rule_graph_update(insert).unwrap();
            let populated = format!("{header} {{\n  0% {{ opacity: 0.5; }}\n}}");
            assert_eq!(sheet.serialise(), populated);

            let snapshot = state
                .create_stylesheet(StyleSheetCandidate::new(context, [rule.snapshot()]))
                .unwrap();
            assert_eq!(snapshot.serialise(), populated);

            let delete = state.prepare_delete_rule(&sheet, &nested, 0).unwrap();
            state.commit_rule_graph_update(delete).unwrap();
            assert_eq!(sheet.serialise(), empty);
            assert_eq!(snapshot.serialise(), populated);

            let renamed = replace_keyframes_name(&rule.snapshot(), "renamed{} ").unwrap();
            let update = state
                .prepare_mutate_rule(&sheet, &sheet.top_list(), 0, renamed)
                .unwrap();
            state.commit_rule_graph_update(update).unwrap();
            assert_eq!(sheet.serialise(), "@keyframes renamed\\{\\}\\  {\n}");
        }
    }

    #[test]
    fn keyframes_name_serialization_uses_the_cssom_keyword_rules() {
        for keyword in [
            "initial",
            "inherit",
            "unset",
            "revert",
            "revert-layer",
            "default",
            "none",
            "NONE",
        ] {
            assert_eq!(
                serialize_keyframes(keyword, []),
                format!("@keyframes \"{keyword}\" {{\n}}")
            );
        }
        assert_eq!(serialize_keyframes("a b", []), "@keyframes a\\ b {\n}");
    }
}
