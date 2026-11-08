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
        map_pending_declarations(projected, &mut *rewrite, None, None)
    })
}

pub(crate) fn map_pending_declarations(
    rule: RuleNode,
    mut rewrite_tokens: impl FnMut(&str) -> String,
    base_url: Option<&str>,
    referrer_source: Option<&str>,
) -> RuleNode {
    let Some(block) = rule.payload().declaration_block() else {
        return rule;
    };
    let declarations = block
        .declarations()
        .iter()
        .map(|declaration| {
            if declaration.pending_substitution().is_none()
                && !declaration_uses_resource_context(declaration.value())
            {
                return declaration.clone();
            }
            let projected = match declaration.pending_substitution() {
                Some(pending) => stylo_cssom_model::RuleDeclaration::from_pending_substitution(
                    declaration.name(),
                    pending.shorthand(),
                    rewrite_tokens(pending.tokens()),
                    base_url.unwrap_or_else(|| pending.base_url()),
                )
                .expect("source projection retains the shorthand member identity")
                .with_importance(declaration.important()),
                None => declaration.clone(),
            };
            let projected = match base_url {
                Some(base) => projected.with_source_base_url(base),
                None => projected,
            };
            match referrer_source.or_else(|| declaration.source_referrer()) {
                Some(source) => projected.with_source_referrer(source),
                None => projected,
            }
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

fn declaration_uses_resource_context(css: &str) -> bool {
    use stylo_cssom_model::{SpecifiedComponentValue as Component, SpecifiedStyleValue as Value};
    fn components(values: &[Component]) -> bool {
        values.iter().any(|value| match value {
            Component::Url { .. } => true,
            Component::Function { name, arguments } => {
                name.starts_with("--")
                    || [
                        "url",
                        "src",
                        "image",
                        "image-set",
                        "-webkit-image-set",
                        "var",
                        "attr",
                        "if",
                    ]
                    .iter()
                    .any(|function| name.eq_ignore_ascii_case(function))
                    || components(arguments)
            },
            Component::Block { values, .. } => components(values),
            _ => false,
        })
    }
    fn value_uses_context(value: &Value) -> bool {
        match value {
            Value::Components(values) => components(values),
            Value::List { values, .. } => values.iter().any(value_uses_context),
            Value::ParsedCalculation { source, .. } => value_uses_context(source),
            _ => false,
        }
    }
    crate::declaration_parser::specified_style_value_from_css(
        css,
        &std::sync::Arc::from("about:blank"),
    )
    .as_ref()
    .is_some_and(value_uses_context)
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
