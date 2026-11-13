use stylo_cssom_model::RuleNode;

fn is_empty_projection(css: &str) -> bool {
    let mut input = cssparser::ParserInput::new(css);
    cssparser::Parser::new(&mut input).is_exhausted()
}

pub fn project_rule_sources(
    rules: &[RuleNode],
    rewrite: &mut impl FnMut(&str) -> String,
) -> Vec<RuleNode> {
    let mut output = Vec::new();
    for rule in rules {
        let source = rule.projection_serialization();
        let rewritten = rewrite(&source);
        if is_empty_projection(&rewritten) {
            continue;
        }
        let reparsed = if rewritten != source
            && (rule.grammar() == stylo_cssom_model::RuleGrammar::Unknown
                || rule.grammar() == stylo_cssom_model::RuleGrammar::Style
                    && crate::rule_parser::starts_at_rule(&source))
        {
            crate::authored_rules::ParsedStylesheet::parse(&rewritten)
                .ok()
                .map(|parsed| parsed.rule_nodes().to_vec())
        } else {
            None
        };
        let projected = reparsed.unwrap_or_else(|| {
            let projected = rule.clone().with_projection_serialization(rewritten);
            vec![map_pending_declarations(
                projected,
                &mut *rewrite,
                None,
                None,
            )]
        });
        for rule in projected {
            let css = rule.projection_serialization();
            let nested = project_rule_sources(rule.payload().nested(), rewrite);
            output.push(rule.with_projected_nested(nested, css));
        }
    }
    output
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
    let projected = if declarations.as_slice() == block.declarations() {
        block.clone().with_native_backing(None)
    } else {
        stylo_cssom_model::RuleDeclarationBlock::new(
            block.domain(),
            block.shared_serialization(),
            declarations,
        )
        .with_namespaces(block.namespaces().clone())
        .with_shorthand_values(block.shorthand_values())
    };
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
            if is_empty_projection(&css) {
                return None;
            }
            let nested = map_rule_sources(projected.payload().nested(), project);
            Some(projected.with_projected_nested(nested, css))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_projection_shares_only_unchanged_pending_declarations() {
        use stylo_cssom_model::{
            RuleDeclaration, RuleDeclarationBlock, RuleDeclarationDomain, RuleNode,
        };
        crate::context::initialise_required_servo_style_prefs();
        let shorthand = stylo_cssom_model::property_schema("margin").unwrap().id;
        let declaration = RuleDeclaration::from_pending_substitution(
            "margin-top",
            shorthand,
            "var(--before)",
            "https://styles.test/",
        )
        .unwrap()
        .with_importance(true)
        .with_source_referrer("https://response.test/style.css");
        let block = RuleDeclarationBlock::new(
            RuleDeclarationDomain::Style,
            "margin:var(--before)!important",
            [declaration],
        );
        let original =
            RuleNode::style("p", block.serialization()).with_declaration_block(block.clone());
        let unchanged =
            super::map_pending_declarations(original.clone(), str::to_owned, None, None);
        assert_eq!(
            unchanged
                .payload()
                .declaration_block()
                .unwrap()
                .declarations()
                .as_ptr(),
            block.declarations().as_ptr()
        );
        let changed = super::map_pending_declarations(
            original,
            |tokens| tokens.replace("--before", "--after"),
            None,
            None,
        );
        let after = changed.payload().declaration_block().unwrap();
        assert_ne!(after.declarations().as_ptr(), block.declarations().as_ptr());
        let declaration = &after.declarations()[0];
        let pending = declaration.pending_substitution().unwrap();
        assert_eq!(pending.tokens(), "var(--after)");
        assert_eq!(pending.shorthand(), shorthand);
        assert_eq!(pending.base_url(), "https://styles.test/");
        assert!(declaration.important());
        assert_eq!(
            declaration.source_referrer(),
            Some("https://response.test/style.css")
        );
        assert!(after.native_backing().is_none());
    }

    #[test]
    fn source_context_projection_reuses_block_text_and_resets_native_backing() {
        use std::sync::Arc;
        use stylo_cssom_model::{
            NativeDeclarationBacking, RetainedNativeDeclarations, RuleDeclaration,
            RuleDeclarationBlock, RuleDeclarationDomain, RuleNamespaceContext,
        };
        #[derive(Debug)]
        struct Backing;
        impl NativeDeclarationBacking for Backing {
            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
            fn equivalent(&self, other: &dyn NativeDeclarationBacking) -> bool {
                other.as_any().is::<Self>()
            }
        }
        crate::context::initialise_required_servo_style_prefs();
        let declaration = RuleDeclaration::new("background-image", "url(image.png)")
            .with_importance(true)
            .with_source_base_url("https://before.test/")
            .with_source_referrer("https://before.test/source.css");
        let block = RuleDeclarationBlock::new(
            RuleDeclarationDomain::Style,
            "background-image: url(image.png) !important;",
            [declaration],
        )
        .with_namespaces(RuleNamespaceContext::new(
            Some("urn:default".into()),
            [("p".into(), "urn:prefix".into())],
        ))
        .with_shorthand_values([RuleDeclaration::new("background", "url(image.png)")])
        .with_native_backing(Some(RetainedNativeDeclarations(Arc::new(Backing))));
        let original = stylo_cssom_model::RuleNode::style("p", block.serialization())
            .with_declaration_block(block.clone());
        for (base, referrer) in [
            (None, None),
            (
                Some("https://before.test/"),
                Some("https://before.test/source.css"),
            ),
        ] {
            let unchanged =
                super::map_pending_declarations(original.clone(), str::to_owned, base, referrer);
            let after = unchanged.payload().declaration_block().unwrap();
            assert_eq!(after.declarations(), block.declarations());
            assert_eq!(after.declarations().as_ptr(), block.declarations().as_ptr());
            assert_eq!(
                after.shorthand_values().as_ptr(),
                block.shorthand_values().as_ptr()
            );
            assert!(after.native_backing().is_none());
            assert!(
                original
                    .payload()
                    .declaration_block()
                    .unwrap()
                    .native_backing()
                    .is_some()
            );
        }
        let projected = super::map_pending_declarations(
            original.clone(),
            str::to_owned,
            Some("https://after.test/"),
            Some("https://after.test/changed.css"),
        );
        let after = projected.payload().declaration_block().unwrap();
        assert_eq!(after.serialization(), block.serialization());
        assert_eq!(after.domain(), block.domain());
        assert_eq!(after.namespaces(), block.namespaces());
        assert_eq!(after.shorthand_values(), block.shorthand_values());
        assert_ne!(after.declarations().as_ptr(), block.declarations().as_ptr());
        assert!(after.native_backing().is_none());
        assert!(
            original
                .payload()
                .declaration_block()
                .unwrap()
                .native_backing()
                .is_some()
        );
        let changed = &after.declarations()[0];
        assert_eq!(changed.name(), "background-image");
        assert_eq!(changed.value(), "url(image.png)");
        assert!(changed.important());
        assert_eq!(changed.source_base_url(), Some("https://after.test/"));
        assert_eq!(
            changed.source_referrer(),
            Some("https://after.test/changed.css")
        );
        assert_eq!(
            block.declarations()[0].source_base_url(),
            Some("https://before.test/")
        );
        assert_eq!(
            after.serialization().as_ptr(),
            block.serialization().as_ptr()
        );
    }

    #[test]
    fn rejected_selector_comments_do_not_leave_style_rule_nodes() {
        use stylo_cssom_model::{InternalStylesheetRoot, StyleOrigin};

        crate::context::initialise_required_servo_style_prefs();
        let parsed = crate::authored_rules::ParsedStylesheet::parse(
            "/* rejected */ a:has(> code) { width:var(--width) } \
             @media print { /* nested */ b:has(code) { color:red } \
             /* retained */ c { width:var(--width) } }",
        )
        .unwrap();
        let root = InternalStylesheetRoot::new(StyleOrigin::Author, parsed.rule_nodes().to_vec());
        let (projected, _) = crate::compat::translate::project_compat_root(
            &root,
            crate::compat::CompatMode::PdfReactor,
        );
        assert_eq!(projected.rules().len(), 1);
        assert_eq!(projected.rules()[0].payload().nested().len(), 1);
        let expanded =
            crate::rule_declaration_lowering::expand_view_transition_rules(projected.rules())
                .unwrap();
        assert!(expanded[0].payload().nested()[0]
            .payload()
            .declaration_block()
            .unwrap()
            .declarations()
            .iter()
            .any(
                |declaration| declaration.name() == "width" && declaration.value().contains("var(")
            ));
    }

    #[test]
    fn compatibility_projection_keeps_generated_preference_rules_during_expansion() {
        crate::context::initialise_required_servo_style_prefs();
        let parsed = crate::authored_rules::ParsedStylesheet::parse(
            ":root{--ink:red}p{color:var(--ink)}@-ro-preferences{first-page-side:verso}",
        )
        .unwrap();
        let root = stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::Author,
            parsed.rule_nodes().to_vec(),
        );
        let (projected, _) = crate::compat::translate::project_compat_root(
            &root,
            crate::compat::CompatMode::PdfReactor,
        );
        let expanded =
            crate::rule_declaration_lowering::expand_view_transition_rules(projected.rules())
                .unwrap();
        assert!(expanded.iter().any(|rule| rule
            .projection_serialization()
            .contains("-bd-first-page-side: verso")));
    }
}
