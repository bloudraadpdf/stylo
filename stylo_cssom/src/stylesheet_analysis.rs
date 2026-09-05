use selectors::parser::Component;
use std::collections::BTreeSet;
use style::selector_parser::SelectorImpl;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BodySignature {
    pub tags: BTreeSet<String>,
    pub ids: BTreeSet<String>,
    pub classes: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reachability {
    Reachable,

    Ambiguous(&'static str),

    Unreachable(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnreferencedDraftCss {
    FloatOffset,
    FloatDefer,
    BlockFloat,
    SnapFloat,
    ExtendedClear,
    InlineOrRegionFloatReference,
    ExclusionWrapping,
}

impl UnreferencedDraftCss {
    pub const fn label(self) -> &'static str {
        match self {
            Self::FloatOffset => "float-offset",
            Self::FloatDefer => "float-defer",
            Self::BlockFloat => "float: block-start | block-end",
            Self::SnapFloat => "float: snap-block() | snap-inline()",
            Self::ExtendedClear => "extended clear values",
            Self::InlineOrRegionFloatReference => "float-reference: inline | region",
            Self::ExclusionWrapping => "wrap-flow | wrap-through",
        }
    }
}

#[must_use]
pub fn detect_unreferenced_draft_css(css: &str) -> BTreeSet<UnreferencedDraftCss> {
    let mut found = BTreeSet::new();

    for declaration in crate::source::stylesheet_declarations(css) {
        let property = declaration.property.as_str();
        let value = declaration.value.to_ascii_lowercase();
        let value = value.as_str();

        match property {
            "float-offset" => {
                found.insert(UnreferencedDraftCss::FloatOffset);
            },
            "float-defer" => {
                found.insert(UnreferencedDraftCss::FloatDefer);
            },
            "float" if matches!(value, "block-start" | "block-end") => {
                found.insert(UnreferencedDraftCss::BlockFloat);
            },
            "float"
                if matches!(value, "snap-block" | "snap-inline")
                    || value.starts_with("snap-block(")
                    || value.starts_with("snap-inline(") =>
            {
                found.insert(UnreferencedDraftCss::SnapFloat);
            },
            "clear"
                if matches!(
                    value,
                    "block-start" | "block-end" | "both-block" | "both-inline" | "top" | "bottom"
                ) =>
            {
                found.insert(UnreferencedDraftCss::ExtendedClear);
            },
            "float-reference" if matches!(value, "inline" | "region") => {
                found.insert(UnreferencedDraftCss::InlineOrRegionFloatReference);
            },
            "wrap-flow" | "wrap-through" => {
                found.insert(UnreferencedDraftCss::ExclusionWrapping);
            },
            _ => {},
        }
    }

    found
}

pub fn extract_selectors_from_css(css: &str) -> Vec<String> {
    fn collect(nodes: &[stylo_cssom_model::RuleNode], output: &mut Vec<String>) {
        for node in nodes {
            if let Some(stylo_cssom_model::RuleCssomData::Style { selector }) = node.cssom_data() {
                output.push(selector.to_string());
            }
            collect(node.payload().nested(), output);
        }
    }
    let Ok(stylesheet) = crate::ParsedStylesheet::parse(css) else {
        return Vec::new();
    };
    let mut output = Vec::new();
    collect(stylesheet.rule_nodes(), &mut output);
    output
}

pub fn selector_reachable(selector: &str, body: &BodySignature) -> Reachability {
    let Ok(list) = crate::selector_query::parse_selector(selector) else {
        return Reachability::Ambiguous("invalid selector");
    };
    if list
        .slice()
        .iter()
        .flat_map(|selector| selector.iter_raw_match_order())
        .any(|component| {
            !matches!(
                component,
                Component::LocalName(_)
                    | Component::ID(_)
                    | Component::Class(_)
                    | Component::ExplicitUniversalType
                    | Component::Combinator(_)
            )
        })
    {
        return Reachability::Ambiguous("complex construct");
    }
    for selector in list.slice() {
        for component in selector.iter_raw_match_order() {
            match component {
                Component::LocalName(name) => {
                    if !body.tags.contains(name.lower_name.as_ref()) {
                        return Reachability::Unreachable(format!(
                            "tag `{}` absent from body",
                            name.lower_name.as_ref()
                        ));
                    }
                },
                Component::ID(name) => {
                    if !body.ids.contains(name.as_ref()) {
                        return Reachability::Unreachable(format!(
                            "id `#{}` absent from body",
                            name.as_ref()
                        ));
                    }
                },
                Component::Class(name) => {
                    if !body.classes.contains(name.as_ref()) {
                        return Reachability::Unreachable(format!(
                            "class `.{}` absent from body",
                            name.as_ref()
                        ));
                    }
                },
                Component::ExplicitUniversalType | Component::Combinator(_) => {},
                _ => return Reachability::Ambiguous("complex construct"),
            }
        }
    }
    Reachability::Reachable
}

pub fn css_contains_class_or_id_selector(css: &str) -> bool {
    struct Visitor(bool);
    impl selectors::visitor::SelectorVisitor for Visitor {
        type Impl = SelectorImpl;
        fn visit_simple_selector(&mut self, component: &Component<SelectorImpl>) -> bool {
            self.0 |= matches!(component, Component::ID(_) | Component::Class(_));
            !self.0
        }
    }
    extract_selectors_from_css(css).iter().any(|source| {
        crate::selector_query::parse_selector(source).is_ok_and(|list| {
            let mut visitor = Visitor(false);
            for selector in list.slice() {
                selector.visit(&mut visitor);
            }
            visitor.0
        })
    })
}

pub fn stylesheet_property_labels(source: &str) -> Vec<String> {
    property_labels(crate::source::stylesheet_declarations(source))
}

pub fn declaration_property_labels(source: &str) -> Vec<String> {
    property_labels(crate::source::declarations(source))
}

fn property_labels(declarations: Vec<crate::source::SourceDeclaration<'_>>) -> Vec<String> {
    let mut labels = Vec::new();
    for declaration in declarations {
        if declaration.property.starts_with('-') {
            continue;
        }
        let prefix = if declaration.at_rules.iter().any(|name| name == "page") {
            "@page:"
        } else {
            ""
        };
        labels.push(format!("{prefix}{}", declaration.property));
        if matches!(declaration.property.as_str(), "display" | "position") {
            let mut input = cssparser::ParserInput::new(declaration.value);
            if let Ok(value) = cssparser::Parser::new(&mut input).expect_ident_cloned() {
                labels.push(format!(
                    "{prefix}{}:{}",
                    declaration.property,
                    value.to_ascii_lowercase()
                ));
            }
        }
    }
    labels
}
