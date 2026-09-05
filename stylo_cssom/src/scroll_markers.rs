#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollMarkerRuleTarget {
    OriginatingElement,
    GroupPseudo,
    MarkerPseudo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollMarkerGroupPosition {
    Before,
    After,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollMarkerGroupMode {
    Links,
    Tabs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredScrollMarkerDeclaration {
    pub property: String,
    pub value: String,
    pub important: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredScrollMarkerRule {
    pub selector: String,
    pub target: ScrollMarkerRuleTarget,
    pub declarations: Vec<AuthoredScrollMarkerDeclaration>,
}

pub fn extract_scroll_marker_rules(css: &str) -> Vec<AuthoredScrollMarkerRule> {
    let Ok(stylesheet) = crate::ParsedStylesheet::parse(css) else {
        return Vec::new();
    };
    let mut rules = Vec::new();
    scan_nodes(stylesheet.rule_nodes(), None, &mut rules);
    rules
}

fn scan_nodes(
    nodes: &[stylo_cssom_model::RuleNode],
    parent_selector: Option<&str>,
    out: &mut Vec<AuthoredScrollMarkerRule>,
) {
    for node in nodes {
        let Some(stylo_cssom_model::RuleCssomData::Style { selector }) = node.cssom_data() else {
            continue;
        };
        let Some(selector) = resolve_nested_selector(parent_selector, selector) else {
            continue;
        };
        scan_nodes(node.payload().nested(), Some(&selector), out);
        let declarations = node
            .payload()
            .declaration_block()
            .map(|block| {
                block
                    .declarations()
                    .iter()
                    .map(|declaration| AuthoredScrollMarkerDeclaration {
                        property: declaration.name().to_owned(),
                        value: declaration.value().to_owned(),
                        important: declaration.important(),
                    })
                    .collect()
            })
            .unwrap_or_default();
        push_scroll_marker_rule(selector, declarations, out);
    }
}

pub fn extract_bound_scroll_marker_rules(
    list: &stylo_cssom_model::RuleListLease,
    context: stylo_cssom_model::ImportBindingContext,
) -> Vec<AuthoredScrollMarkerRule> {
    let mut rules = Vec::new();
    scan_typed_rule_list(list, context, None, &mut rules);
    rules
}

fn scan_typed_rule_list(
    list: &stylo_cssom_model::RuleListLease,
    import_context: stylo_cssom_model::ImportBindingContext,
    parent_selector: Option<&str>,
    out: &mut Vec<AuthoredScrollMarkerRule>,
) {
    for index in 0..list.len() {
        let Some(rule) = list.rule(index) else {
            continue;
        };
        let node = rule.node();
        if node.grammar() == stylo_cssom_model::RuleGrammar::Import {
            if let Some(child) = rule
                .import_bindings()
                .into_iter()
                .find(|binding| binding.context() == import_context)
                .and_then(|binding| binding.loaded_child())
            {
                scan_typed_rule_list(
                    &child.top_list(),
                    stylo_cssom_model::ImportBindingContext::Source,
                    parent_selector,
                    out,
                );
            }
            continue;
        }
        if node.grammar() != stylo_cssom_model::RuleGrammar::Style {
            continue;
        }
        let Some(stylo_cssom_model::RuleCssomData::Style { selector }) = node.cssom_data() else {
            continue;
        };
        let Some(selector) = resolve_nested_selector(parent_selector, selector) else {
            continue;
        };
        let declarations = rule
            .block()
            .map(|block| {
                block
                    .declarations()
                    .iter()
                    .map(|declaration| AuthoredScrollMarkerDeclaration {
                        property: declaration.name().to_owned(),
                        value: declaration.value().to_owned(),
                        important: declaration.important(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if let Some(nested) = rule.nested_list() {
            scan_typed_rule_list(&nested, import_context, Some(&selector), out);
        }
        push_scroll_marker_rule(selector, declarations, out);
    }
}

fn push_scroll_marker_rule(
    selector: String,
    declarations: Vec<AuthoredScrollMarkerDeclaration>,
    out: &mut Vec<AuthoredScrollMarkerRule>,
) {
    let target = if selector.ends_with("::scroll-marker-group") {
        Some(ScrollMarkerRuleTarget::GroupPseudo)
    } else if selector.ends_with("::scroll-marker") {
        Some(ScrollMarkerRuleTarget::MarkerPseudo)
    } else if declarations
        .iter()
        .any(|declaration| declaration.property == "scroll-marker-group")
    {
        Some(ScrollMarkerRuleTarget::OriginatingElement)
    } else {
        None
    };
    if let Some(target) = target {
        out.push(AuthoredScrollMarkerRule {
            selector,
            target,
            declarations,
        });
    }
}

fn resolve_nested_selector(parent: Option<&str>, selector: &str) -> Option<String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.starts_with('@') || selector.contains(',') {
        return None;
    }
    match parent {
        Some(parent) if selector.contains('&') => Some(selector.replace('&', parent)),
        Some(parent) => Some(format!("{parent} {selector}")),
        None => Some(selector.to_string()),
    }
}
