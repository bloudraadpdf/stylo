use stylo_cssom_model::RuleNode;

pub fn project_rule_sources(
    rules: &[RuleNode],
    rewrite: &mut impl FnMut(&str) -> String,
) -> Vec<RuleNode> {
    map_rule_sources(rules, &mut |rule| {
        rule.clone()
            .with_projection_serialization(rewrite(&rule.projection_serialization()))
    })
}

pub fn map_rule_sources(
    rules: &[RuleNode],
    project: &mut impl FnMut(&RuleNode) -> RuleNode,
) -> Vec<RuleNode> {
    rules
        .iter()
        .map(|rule| {
            let projected = project(rule);
            let css = projected.projection_serialization();
            let nested = map_rule_sources(rule.payload().nested(), project);
            projected.with_projected_nested(nested, css)
        })
        .collect()
}
