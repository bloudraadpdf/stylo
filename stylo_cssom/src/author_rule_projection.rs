use stylo_cssom_model::RuleNode;

pub fn project_rule_sources(
    rules: &[RuleNode],
    rewrite: &mut impl FnMut(&str) -> String,
) -> Vec<RuleNode> {
    map_rule_sources(rules, &mut |rule| {
        let source = rule.projection_serialization();
        let rewritten = rewrite(&source);
        if rule.grammar() == stylo_cssom_model::RuleGrammar::Unknown
            && rewritten != source
            && let Some(parsed) = crate::rule_parser::ParsedCssRule::parse(&rewritten)
        {
            return parsed.to_rule_node();
        }
        let projected = rule.clone().with_projection_serialization(rewritten);
        map_pending_declarations(projected, &mut *rewrite, None)
    })
}

pub(crate) fn map_pending_declarations(
    rule: RuleNode,
    mut rewrite_tokens: impl FnMut(&str) -> String,
    base_url: Option<&str>,
) -> RuleNode {
    let Some(block) = rule.payload().declaration_block().filter(|block| {
        block
            .declarations()
            .iter()
            .any(|declaration| declaration.pending_substitution().is_some())
    }) else {
        return rule;
    };
    let declarations = block
        .declarations()
        .iter()
        .map(|declaration| {
            let Some(pending) = declaration.pending_substitution() else {
                return declaration.clone();
            };
            stylo_cssom_model::RuleDeclaration::from_pending_substitution(
                declaration.name(),
                pending.shorthand(),
                rewrite_tokens(pending.tokens()),
                base_url.unwrap_or_else(|| pending.base_url()),
            )
            .expect("source projection retains the shorthand member identity")
            .with_importance(declaration.important())
        })
        .collect::<Vec<_>>();
    let projected = stylo_cssom_model::RuleDeclarationBlock::new(
        block.domain(),
        block.serialization(),
        declarations,
    )
    .with_namespaces(block.namespaces().clone())
    .with_shorthand_values(block.shorthand_values());
    rule.with_declaration_block(projected)
}

pub fn map_rule_sources(
    rules: &[RuleNode],
    project: &mut impl FnMut(&RuleNode) -> RuleNode,
) -> Vec<RuleNode> {
    rules
        .iter()
        .filter_map(|rule| {
            let projected = project(rule);
            let css = projected.projection_serialization();
            if css.trim().is_empty() {
                return None;
            }
            let nested = map_rule_sources(projected.payload().nested(), project);
            Some(projected.with_projected_nested(nested, css))
        })
        .collect()
}
