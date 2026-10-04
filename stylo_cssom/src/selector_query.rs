use selectors::{SelectorList, parser::ParseRelative};
use style::{
    selector_parser::{PseudoElement, SelectorImpl, SelectorParser},
    stylesheets::{Namespaces, Origin, UrlExtraData},
};

pub type ParsedSelectorList = SelectorList<SelectorImpl>;

pub fn parse_selector(selector: &str) -> Result<ParsedSelectorList, String> {
    if selector.trim().is_empty() {
        return Err("empty selector".to_owned());
    }
    let url_data = UrlExtraData::from(crate::context::ABOUT_BLANK.clone());
    let rewritten = crate::active_view_transition_selector::rewrite_selector(selector);
    SelectorParser::parse_author_origin_no_namespace(&rewritten, &url_data)
        .map_err(|error| format!("{error:?}"))
}

fn targets_element(selector: &selectors::parser::Selector<SelectorImpl>) -> bool {
    !selector.has_pseudo_element() && !selector.is_part() && !selector.is_slotted()
}

pub fn parse_dom_selector(selector: &str) -> Result<ParsedSelectorList, String> {
    let list = parse_selector(selector)?;
    let elements = list
        .slice()
        .iter()
        .filter(|selector| targets_element(selector))
        .cloned()
        .collect::<Vec<_>>();
    Ok(SelectorList::from_iter(elements.into_iter()))
}

struct ContentLanguageReads(bool);

impl selectors::visitor::SelectorVisitor for ContentLanguageReads {
    type Impl = SelectorImpl;

    fn visit_simple_selector(
        &mut self,
        component: &selectors::parser::Component<SelectorImpl>,
    ) -> bool {
        self.0 |= matches!(
            component,
            selectors::parser::Component::NonTSPseudoClass(
                style::selector_parser::NonTSPseudoClass::Lang(_)
            )
        );
        !self.0
    }

    fn visit_relative_selector_list(
        &mut self,
        list: &[selectors::parser::RelativeSelector<SelectorImpl>],
    ) -> bool {
        list.iter().all(|relative| relative.selector.visit(self))
    }
}

/// Whether a match of the list can read the content language of an element.
#[must_use]
pub fn reads_content_language(selectors: &ParsedSelectorList) -> bool {
    let mut reads = ContentLanguageReads(false);
    selectors
        .slice()
        .iter()
        .all(|selector| selector.visit(&mut reads));
    reads.0
}

pub fn selector_specificity(selector: &str) -> Result<u32, String> {
    let list = parse_selector(selector)?;
    let [selector] = list.slice() else {
        return Err("expected a single selector without commas".to_owned());
    };
    Ok(selector.specificity())
}

pub fn parse_cssom_pseudo_element(selector: &str) -> Result<PseudoElement, String> {
    let mut input = cssparser::ParserInput::new(selector);
    let mut input = cssparser::Parser::new(&mut input);
    let start = input.state();
    let mut trailing_whitespace = false;
    while let Ok(token) = input.next_including_whitespace() {
        trailing_whitespace = matches!(token, cssparser::Token::WhiteSpace(_));
    }
    if trailing_whitespace {
        return Err("pseudo-element selector cannot end in whitespace".to_owned());
    }
    input.reset(&start);
    let namespaces = Namespaces::default();
    let url_data = UrlExtraData::from(crate::context::ABOUT_BLANK.clone());
    let parser = SelectorParser {
        stylesheet_origin: Origin::Author,
        namespaces: &namespaces,
        url_data: &url_data,
        for_supports_rule: false,
    };
    let parsed = input
        .parse_entirely(|input| SelectorList::parse(&parser, input, ParseRelative::No))
        .map_err(|error| format!("{error:?}"))?;
    let [selector] = parsed.slice() else {
        return Err("expected one pseudo-element selector".to_owned());
    };
    if selector.len() != 2 || selector.is_part() || selector.is_slotted() {
        return Err("expected only a pseudo-element selector".to_owned());
    }
    selector
        .pseudo_element()
        .cloned()
        .ok_or_else(|| "expected a pseudo-element selector".to_owned())
}

pub fn serialize_selector_list(selectors: &ParsedSelectorList) -> String {
    cssparser::ToCss::to_css_string(selectors)
}

pub fn selector_serializations(source: &str) -> Vec<String> {
    parse_selector(source)
        .map(|list| {
            list.slice()
                .iter()
                .map(cssparser::ToCss::to_css_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn selector_targets_pseudo_element(source: &str) -> bool {
    parse_selector(source).is_ok_and(|list| {
        list.slice()
            .iter()
            .any(|selector| !targets_element(selector))
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_dom_selector, selector_targets_pseudo_element};

    #[test]
    fn cssom_pseudos_retain_names_and_require_double_colons() {
        use cssparser::ToCss;
        for source in [
            "::picker-icon",
            "::grammar-error",
            "::spelling-error",
            "::highlight(name)",
            "::view-transition",
            "::view-transition-group(name)",
            "::view-transition-image-pair(name)",
            "::view-transition-old(name)",
            "::view-transition-new(name)",
        ] {
            let pseudo = super::parse_cssom_pseudo_element(source).unwrap();
            assert_eq!(pseudo.to_css_string(), source);
            assert!(
                super::parse_cssom_pseudo_element(&source[1..]).is_err(),
                "{source}"
            );
            let other = source.replace("(name)", "(other)");
            if other != source {
                assert_ne!(pseudo, super::parse_cssom_pseudo_element(&other).unwrap());
            }
        }
        for source in [
            "::highlight()",
            "::highlight(inherit)",
            "::view-transition-old(none)",
            "::picker-icon ",
        ] {
            assert!(
                super::parse_cssom_pseudo_element(source).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn cssom_form_control_pseudos_use_standard_grammar() {
        use cssparser::ToCss;
        for source in ["::checkmark", "::picker(select)"] {
            let pseudo = super::parse_cssom_pseudo_element(source).unwrap();
            assert_eq!(pseudo.to_css_string(), source);
            assert!(super::parse_cssom_pseudo_element(&source[1..]).is_err());
        }
        for source in [
            "::picker",
            "::picker()",
            "::picker(div)",
            "::picker(select)a",
            "::picker( )",
            "::checkmark()",
        ] {
            assert!(
                super::parse_cssom_pseudo_element(source).is_err(),
                "{source}"
            );
        }
        for source in [
            "::picker(SELECT)",
            "::picker(select select)",
            "::picker(select\t",
        ] {
            assert_eq!(
                super::parse_cssom_pseudo_element(source)
                    .unwrap()
                    .to_css_string(),
                "::picker(select)",
                "{source}",
            );
        }
    }

    #[test]
    fn cssom_function_whitespace_obeys_component_boundaries() {
        use cssparser::ToCss;
        for source in [
            "::highlight(name\t",
            "::highlight( name ",
            "::highlight(\nname",
            "::highlight( name )",
        ] {
            assert_eq!(
                super::parse_cssom_pseudo_element(source)
                    .unwrap()
                    .to_css_string(),
                "::highlight(name)",
                "{source}",
            );
        }
        for source in [
            "::highlight(name) ",
            "::highlight(name)\t",
            "::before ",
            "::before\n",
        ] {
            assert!(
                super::parse_cssom_pseudo_element(source).is_err(),
                "{source}"
            );
        }
    }

    #[test]
    fn only_a_lang_selector_reads_the_content_language() {
        for (source, expected) in [
            (".transition", false),
            ("#fixture > .container:not(.to)", false),
            ("p:lang(en)", true),
            ("div, p:lang(en)", true),
            ("div:not(:lang(en))", true),
            ("div:is(.a, :lang(fr))", true),
            ("div:has(> p:lang(de))", true),
        ] {
            let parsed = super::parse_selector(source).expect("the selector is valid");
            assert_eq!(super::reads_content_language(&parsed), expected, "{source}");
        }
    }

    #[test]
    fn static_form_selector_retains_its_pseudo_class_specificity() {
        for source in [
            "select[size]:-bd-static-form option[selected]",
            "select[size]:-ro-static-form option[selected]",
        ] {
            let parsed = super::parse_selector(source).expect("static form selectors are admitted");
            assert_eq!(
                super::serialize_selector_list(&parsed),
                "select[size]:-bd-static-form option[selected]"
            );
            assert_eq!(
                super::selector_specificity(source).unwrap(),
                super::selector_specificity("select[size]:enabled option[selected]").unwrap()
            );
        }
    }

    #[test]
    fn dom_selector_lists_retain_only_element_targets() {
        for selector in [
            "::part(test):is(:focus)",
            "::slotted(button)",
            "button::before",
        ] {
            assert!(
                parse_dom_selector(selector).unwrap().slice().is_empty(),
                "{selector}"
            );
        }
        assert_eq!(
            parse_dom_selector("button, ::part(test):is(:focus)")
                .unwrap()
                .slice()
                .len(),
            1
        );
        assert!(parse_dom_selector("button,").is_err());
    }

    #[test]
    fn shadow_pseudo_elements_retain_their_target_classification() {
        for selector in [
            "::part(test):is(:focus)",
            "::slotted(button)",
            "button::before",
        ] {
            assert!(selector_targets_pseudo_element(selector), "{selector}");
        }
        assert!(!selector_targets_pseudo_element("button:is(:focus)"));
    }
}
