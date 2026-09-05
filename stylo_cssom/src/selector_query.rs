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

pub fn selector_specificity(selector: &str) -> Result<u32, String> {
    let list = parse_selector(selector)?;
    let [selector] = list.slice() else {
        return Err("expected a single selector without commas".to_owned());
    };
    Ok(selector.specificity())
}

pub fn parse_cssom_pseudo_element(selector: &str) -> Result<PseudoElement, String> {
    if selector
        .as_bytes()
        .last()
        .is_some_and(|byte| crate::css_scan::is_css_whitespace(*byte))
    {
        return Err("pseudo-element selector cannot end in whitespace".to_owned());
    }
    let namespaces = Namespaces::default();
    let url_data = UrlExtraData::from(crate::context::ABOUT_BLANK.clone());
    let parser = SelectorParser {
        stylesheet_origin: Origin::Author,
        namespaces: &namespaces,
        url_data: &url_data,
        for_supports_rule: false,
    };
    let mut input = cssparser::ParserInput::new(selector);
    let parsed = cssparser::Parser::new(&mut input)
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
        .copied()
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
