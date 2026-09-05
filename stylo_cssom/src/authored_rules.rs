use std::{ops::Range, sync::Arc as StdArc};

use cssparser::{
    Parser, ParserInput, SourceLocation, SourcePosition, ToCss as CssParserToCss, Token,
};
use selectors::{
    matching::QuirksMode,
    parser::{ParseRelative, SelectorList},
};
use servo_arc::Arc as ServoArc;
use style::{
    media_queries::MediaList,
    selector_parser::SelectorParser,
    shared_lock::{Locked, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard},
    stylesheets::{
        AllowImportRules, CssRule, Origin, Stylesheet, StylesheetLoader, UrlExtraData,
        import_rule::{ImportLayer, ImportRule, ImportSheet, ImportSupportsCondition},
    },
    values::CssUrl,
};
use style_traits::{CssWriter, ToCss};
use url::Url;

use crate::context::ABOUT_BLANK;

pub type AuthoredTopLevelRule = ValidImportRule;

#[must_use]
#[derive(Clone, Debug, Default)]
pub struct ParsedStylesheet {
    rules: Vec<stylo_cssom_model::RuleNode>,
    linked_source: Option<ParsedLinkedStylesheetSource>,
}

#[derive(Debug)]
pub struct ParsedStylesheetGraph {
    stylesheet: ParsedStylesheet,
    imports: Box<[ParsedStylesheetImport]>,
}

#[derive(Debug)]
pub struct ParsedStylesheetImport {
    rule_index: usize,
    resolved_url: String,
    child: Option<Box<ParsedStylesheetGraph>>,
}

impl ParsedStylesheetGraph {
    #[must_use]
    pub fn new(stylesheet: ParsedStylesheet, imports: Vec<ParsedStylesheetImport>) -> Self {
        Self {
            stylesheet,
            imports: imports.into_boxed_slice(),
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (ParsedStylesheet, Box<[ParsedStylesheetImport]>) {
        (self.stylesheet, self.imports)
    }
}

impl ParsedStylesheetImport {
    #[must_use]
    pub fn loaded(rule_index: usize, resolved_url: String, child: ParsedStylesheetGraph) -> Self {
        Self {
            rule_index,
            resolved_url,
            child: Some(Box::new(child)),
        }
    }

    #[must_use]
    pub fn failed(rule_index: usize, resolved_url: String) -> Self {
        Self {
            rule_index,
            resolved_url,
            child: None,
        }
    }

    fn into_model(
        self,
        document: stylo_cssom_model::StyleDocumentHandle,
    ) -> stylo_cssom_model::StyleSheetImportCandidate {
        match self.child {
            Some(child) => stylo_cssom_model::StyleSheetImportCandidate::loaded(
                self.rule_index,
                self.resolved_url,
                child.into_imported_model(document),
            ),
            None => stylo_cssom_model::StyleSheetImportCandidate::failed(
                self.rule_index,
                self.resolved_url,
            ),
        }
    }
}

impl ParsedStylesheetGraph {
    pub fn into_imported_model(
        self,
        document: stylo_cssom_model::StyleDocumentHandle,
    ) -> stylo_cssom_model::StyleSheetGraphCandidate {
        let (stylesheet, imports) = self.into_parts();
        let (source_url, encoding) = stylesheet
            .linked_source()
            .expect("a loaded imported stylesheet graph must retain its decoded source");
        let source = stylo_cssom_model::StyleSheetSourceContext {
            kind: stylo_cssom_model::StyleSheetSourceKind::Imported,
            origin: stylo_cssom_model::StyleOrigin::Author,
            document: Some(document),
            source_url: Some(source_url.into()),
            base_url: Some(source_url.into()),
            encoding: Some(encoding),
        };
        let candidate =
            stylo_cssom_model::StyleSheetCandidate::new(source, stylesheet.rule_nodes().to_vec());
        let imports = imports
            .into_vec()
            .into_iter()
            .map(|import| import.into_model(document))
            .collect::<Vec<_>>();
        stylo_cssom_model::StyleSheetGraphCandidate::new(candidate, imports)
    }

    #[must_use]
    pub fn into_model_imports(
        self,
        document: stylo_cssom_model::StyleDocumentHandle,
    ) -> (
        ParsedStylesheet,
        Vec<stylo_cssom_model::StyleSheetImportCandidate>,
    ) {
        let (stylesheet, imports) = self.into_parts();
        (
            stylesheet,
            imports
                .into_vec()
                .into_iter()
                .map(|import| import.into_model(document))
                .collect(),
        )
    }
}

#[derive(Clone, Debug)]
struct ParsedLinkedStylesheetSource {
    url: String,
    encoding: stylo_cssom_model::CssEncoding,
}

pub fn scan_stylesheet_rule_sources(
    css: &str,
) -> Result<Vec<String>, stylo_cssom_model::CssomStylesheetError> {
    scan_rule_sources(css)
}

fn scan_rule_sources(css: &str) -> Result<Vec<String>, stylo_cssom_model::CssomStylesheetError> {
    let bytes = css.as_bytes();
    let mut rules = Vec::new();
    let mut start = 0;
    let mut index = 0;
    let mut braces = 0_u32;
    let mut component_braces = 0_u32;
    let mut brackets = 0_u32;
    let mut parentheses = 0_u32;
    let mut quote = None;
    let mut escaped = false;
    let mut comment = false;

    while index < bytes.len() {
        let byte = bytes[index];
        let next = bytes.get(index + 1).copied();
        if comment {
            if byte == b'*' && next == Some(b'/') {
                comment = false;
                index += 2;
                continue;
            }
            index += 1;
            continue;
        }
        if let Some(delimiter) = quote {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == delimiter {
                quote = None;
            }
            index += 1;
            continue;
        }
        if escaped {
            escaped = false;
            index += 1;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            index += 1;
            continue;
        }
        if byte == b'/' && next == Some(b'*') {
            comment = true;
            index += 2;
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'{' => {
                if brackets != 0 || parentheses != 0 || component_braces != 0 {
                    component_braces += 1;
                } else {
                    braces += 1;
                }
            },
            b'}' => {
                if component_braces != 0 {
                    component_braces -= 1;
                    index += 1;
                    continue;
                }
                if brackets != 0 || parentheses != 0 {
                    index += 1;
                    continue;
                }
                if braces == 0 {
                    return Err(stylo_cssom_model::CssomStylesheetError::Unterminated);
                }
                braces -= 1;
            },
            b'[' => brackets += 1,
            b']' => {
                close_component_block(&mut brackets)?;
            },
            b'(' => parentheses += 1,
            b')' => {
                close_component_block(&mut parentheses)?;
            },
            _ => {},
        }
        let ends_statement = byte == b';' && braces == 0 && brackets == 0 && parentheses == 0;
        let ends_block = byte == b'}' && braces == 0 && brackets == 0 && parentheses == 0;
        if ends_statement || ends_block {
            let rule = css[start..=index].trim();
            if !rule.is_empty() {
                rules.push(rule.to_owned());
            }
            start = index + 1;
        }
        index += 1;
    }
    if quote.is_some()
        || comment
        || braces != 0
        || component_braces != 0
        || brackets != 0
        || parentheses != 0
    {
        return Err(stylo_cssom_model::CssomStylesheetError::Unterminated);
    }
    let trailing = css[start..].trim();
    if !trailing.is_empty() {
        rules.push(trailing.to_owned());
    }
    Ok(rules)
}

fn close_component_block(depth: &mut u32) -> Result<(), stylo_cssom_model::CssomStylesheetError> {
    *depth = depth
        .checked_sub(1)
        .ok_or(stylo_cssom_model::CssomStylesheetError::Unterminated)?;
    Ok(())
}

impl ParsedStylesheet {
    pub fn parse(css: &str) -> Result<Self, stylo_cssom_model::CssomStylesheetError> {
        let sources = crate::rule_parser::forgiving_rule_sources(css);
        let validated = crate::rule_parser::ParsedCssRule::parse_stylesheet(css);
        let mut validated = validated.iter().peekable();
        let rules = sources
            .into_iter()
            .filter_map(|source| {
                if validated
                    .peek()
                    .is_some_and(|rule| rule.source_location() == Some(source.location))
                {
                    let rule = validated
                        .next()
                        .expect("the exact native source location matched");
                    Some(authored_rule_node(rule, &source.text))
                } else {
                    parse_authored_compatibility_rule(&source.text)
                }
            })
            .collect();
        Ok(Self {
            rules,
            linked_source: None,
        })
    }

    pub fn with_linked_source(
        mut self,
        url: String,
        encoding: stylo_cssom_model::CssEncoding,
    ) -> Self {
        self.linked_source = Some(ParsedLinkedStylesheetSource { url, encoding });
        self
    }

    pub fn linked_source(&self) -> Option<(&str, stylo_cssom_model::CssEncoding)> {
        self.linked_source
            .as_ref()
            .map(|source| (source.url.as_str(), source.encoding))
    }

    pub fn rule(&self, index: usize) -> Option<String> {
        self.rules
            .get(index)
            .map(stylo_cssom_model::RuleNode::serialization)
    }

    pub fn rules(&self) -> impl ExactSizeIterator<Item = String> + '_ {
        self.rules
            .iter()
            .map(stylo_cssom_model::RuleNode::serialization)
    }

    pub fn rule_nodes(&self) -> &[stylo_cssom_model::RuleNode] {
        &self.rules
    }

    pub fn insert_rule(
        &mut self,
        index: usize,
        rule: &str,
    ) -> Result<(), stylo_cssom_model::CssomStylesheetError> {
        let rule = Self::parse_single_rule(rule)?;
        if index > self.rules.len() {
            return Err(
                stylo_cssom_model::CssomStylesheetError::InvalidInsertionIndex {
                    index,
                    len: self.rules.len(),
                },
            );
        }
        self.rules.insert(index, rule);
        Ok(())
    }

    pub fn delete_rule(
        &mut self,
        index: usize,
    ) -> Result<(), stylo_cssom_model::CssomStylesheetError> {
        if index >= self.rules.len() {
            return Err(
                stylo_cssom_model::CssomStylesheetError::InvalidDeletionIndex {
                    index,
                    len: self.rules.len(),
                },
            );
        }
        self.rules.remove(index);
        Ok(())
    }

    pub fn retain_rules(&mut self, mut predicate: impl FnMut(&str) -> bool) {
        self.rules.retain(|rule| predicate(&rule.serialization()));
    }

    pub fn replace_rule(
        &mut self,
        index: usize,
        rule: &str,
    ) -> Result<(), stylo_cssom_model::CssomStylesheetError> {
        let rule = Self::parse_single_rule(rule)?;
        let len = self.rules.len();
        let Some(slot) = self.rules.get_mut(index) else {
            return Err(
                stylo_cssom_model::CssomStylesheetError::InvalidDeletionIndex { index, len },
            );
        };
        *slot = rule;
        Ok(())
    }

    fn parse_single_rule(
        rule: &str,
    ) -> Result<stylo_cssom_model::RuleNode, stylo_cssom_model::CssomStylesheetError> {
        let mut sources = scan_stylesheet_rule_sources(rule)?;
        if sources.len() != 1 {
            return Err(stylo_cssom_model::CssomStylesheetError::ExpectedSingleRule);
        }
        parse_rule_node(&sources.pop().expect("one scanned rule"))
            .ok_or(stylo_cssom_model::CssomStylesheetError::ExpectedSingleRule)
    }

    pub fn serialise(&self) -> String {
        self.rules().collect::<Vec<_>>().join("\n")
    }

    pub fn materialise_imports(
        &self,
        mut replacement: impl FnMut(&ValidImportRule) -> Option<ParsedImportReplacement>,
    ) -> Self {
        let mut imports_allowed = true;
        let rules = self
            .rules
            .iter()
            .flat_map(|rule| {
                let Some(stylo_cssom_model::RuleCssomData::Import { request }) = rule.cssom_data()
                else {
                    imports_allowed &= matches!(
                        rule.grammar(),
                        stylo_cssom_model::RuleGrammar::LayerStatement
                            | stylo_cssom_model::RuleGrammar::Unknown
                            | stylo_cssom_model::RuleGrammar::CustomMedia
                    );
                    return vec![rule.clone()];
                };
                if !imports_allowed {
                    return vec![rule.clone()];
                }
                replacement(&ValidImportRule::from(request))
                    .map(ParsedImportReplacement::into_rules)
                    .unwrap_or_default()
            })
            .collect();
        Self {
            rules,
            linked_source: self.linked_source.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ValidatedStylesheetRuleList {
    stylesheet: ParsedStylesheet,
}

impl ValidatedStylesheetRuleList {
    pub fn parse(css: &str) -> Self {
        let css = normalise_page_rules(css).unwrap_or_else(|| css.to_owned());
        let lock = StdArc::new(SharedRwLock::new());
        let loader = NonLoadingImportLoader;
        let stylesheet = ImportPreludeWalker::parse_standalone_stylesheet(
            &css,
            &lock,
            Some(&loader),
            AllowImportRules::Yes,
        );
        let guard = lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let validated = contents
            .rules(&guard)
            .iter()
            .map(|rule| rule.to_css_string(&guard))
            .collect::<Vec<_>>();
        let mut next_validated = 0;
        let mut serialised = Vec::with_capacity(validated.len());
        if let Ok(authored) = ParsedStylesheet::parse(&css) {
            for source in authored.rules() {
                let Some(candidate) = serialise_single_valid_rule(&source) else {
                    continue;
                };
                if validated.get(next_validated) != Some(&candidate) {
                    continue;
                }
                serialised.push(if contains_page_rule(&source) {
                    source
                } else {
                    candidate
                });
                next_validated += 1;
            }
        }
        serialised.extend(validated.into_iter().skip(next_validated));
        let serialised = serialised.join("\n");
        let stylesheet = ParsedStylesheet::parse(&serialised)
            .expect("serialised parsed rules must remain complete CSS rules");
        Self { stylesheet }
    }

    pub fn into_stylesheet(self) -> ParsedStylesheet {
        self.stylesheet
    }
}

pub fn typed_rule_nodes(stylesheet: &ParsedStylesheet) -> Vec<stylo_cssom_model::RuleNode> {
    stylesheet.rule_nodes().to_vec()
}

pub fn parse_rule_node(source: &str) -> Option<stylo_cssom_model::RuleNode> {
    crate::rule_parser::ParsedCssRule::parse(source)
        .or_else(|| crate::rule_parser::ParsedCssRule::parse_page_child(source))
        .map(|rule| authored_rule_node(&rule, source))
        .or_else(|| parse_authored_compatibility_rule(source))
}

fn authored_rule_node(
    rule: &crate::rule_parser::ParsedCssRule,
    source: &str,
) -> stylo_cssom_model::RuleNode {
    let node = rule.to_rule_node().with_projection_serialization(source);
    if parsed_rule_contains_page(rule) {
        node.with_authored_serialization(source)
    } else {
        node
    }
}

pub fn parse_scope_rule_node(source: &str) -> Option<stylo_cssom_model::RuleNode> {
    let authored = scan_stylesheet_rule_sources(source).ok()?;
    if authored.len() != 1 {
        return None;
    }
    let wrapper = crate::rule_parser::ParsedCssRule::parse(&format!("@scope {{ {source} }}"))?;
    let [rule] = wrapper.nested_rules()? else {
        return None;
    };
    (rule.grammar() == stylo_cssom_model::RuleGrammar::Style).then(|| rule.to_rule_node())
}

pub fn parse_nested_declarations_rule_node(source: &str) -> Option<stylo_cssom_model::RuleNode> {
    crate::declaration_parser::parse_nested_rule_declarations(source)
        .map(stylo_cssom_model::RuleNode::nested_declarations)
}

#[must_use]
pub fn parse_nested_declarations_input(
    input: crate::rule_parser::RuleInput<'_>,
) -> Option<stylo_cssom_model::RuleNode> {
    parse_nested_declarations_rule_node(input.text())
}

fn parse_authored_compatibility_rule(source: &str) -> Option<stylo_cssom_model::RuleNode> {
    parse_highlight_compatibility_rule(source)
        .or_else(|| parse_view_transition_compatibility_rule(source))
        .or_else(|| {
            (source.trim_start().starts_with('@')
                && crate::rule_parser::ParsedCssRule::retain_scanned_rule(source))
            .then(|| {
                stylo_cssom_model::RuleNode::authored(
                    stylo_cssom_model::RuleGrammar::Unknown,
                    source,
                    Vec::<stylo_cssom_model::RuleNode>::new(),
                )
            })
        })
}

fn parse_view_transition_compatibility_rule(source: &str) -> Option<stylo_cssom_model::RuleNode> {
    let open = source.find('{')?;
    let selector = source[..open].trim();
    let selector_bytes = selector.as_bytes();
    let has_view_transition_selector = [
        b"::view-transition".as_slice(),
        b":active-view-transition".as_slice(),
    ]
    .into_iter()
    .any(|needle| {
        selector_bytes
            .windows(needle.len())
            .any(|candidate| candidate.eq_ignore_ascii_case(needle))
    });
    if !has_view_transition_selector || selector.starts_with('@') {
        return None;
    }

    let active = crate::active_view_transition_selector::rewrite_stylesheet(source);
    let projected = crate::view_transition_root_rewrite::rewrite_view_transition_root(&active);
    if crate::rule_parser::ParsedCssRule::parse_stylesheet(&projected).is_empty() {
        return None;
    }

    let placeholder = format!(":where(*) {}", &source[open..]);
    let rule = crate::rule_parser::ParsedCssRule::parse(&placeholder)?;
    rule.to_rule_node()
        .with_projection_serialization(source)
        .with_cssom_data(stylo_cssom_model::RuleCssomData::Style {
            selector: selector.into(),
        })
}

fn parse_highlight_compatibility_rule(source: &str) -> Option<stylo_cssom_model::RuleNode> {
    let translated = crate::highlight_projection::project_highlight_selectors(source);
    if translated == source {
        return None;
    }
    let rule = crate::rule_parser::ParsedCssRule::parse(&translated)?;
    if rule.grammar() != stylo_cssom_model::RuleGrammar::Style {
        return None;
    }
    let selector = source.get(..source.find('{')?)?.trim();
    rule.to_rule_node()
        .with_projection_serialization(source)
        .with_cssom_data(stylo_cssom_model::RuleCssomData::Style {
            selector: selector.into(),
        })
}

fn serialise_single_valid_rule(css: &str) -> Option<String> {
    let lock = StdArc::new(SharedRwLock::new());
    let loader = NonLoadingImportLoader;
    let stylesheet = ImportPreludeWalker::parse_standalone_stylesheet(
        css,
        &lock,
        Some(&loader),
        AllowImportRules::Yes,
    );
    let guard = lock.read();
    let contents = stylesheet.contents.read_with(&guard);
    let [rule] = contents.rules(&guard) else {
        return None;
    };
    Some(rule.to_css_string(&guard))
}

fn contains_page_rule(css: &str) -> bool {
    let Some(rule) = crate::rule_parser::ParsedCssRule::parse(css) else {
        return false;
    };
    parsed_rule_contains_page(&rule)
}

fn parsed_rule_contains_page(rule: &crate::rule_parser::ParsedCssRule) -> bool {
    rule.page_selector_text().is_some()
        || rule
            .nested_rules()
            .is_some_and(|children| children.iter().any(parsed_rule_contains_page))
}

fn normalise_page_rules(css: &str) -> Option<String> {
    let rules = scan_stylesheet_rule_sources(css).ok()?;
    Some(
        rules
            .into_iter()
            .map(|source| {
                let Some(rule) = crate::rule_parser::ParsedCssRule::parse(&source) else {
                    return source;
                };
                if rule.page_selector_text().is_some() {
                    return rule.as_str().to_owned();
                }
                let Some(condition) = rule.media_condition_text() else {
                    return source.to_owned();
                };
                let Some(open) = source.find('{') else {
                    return source.to_owned();
                };
                let Some(close) = source.rfind('}') else {
                    return source.to_owned();
                };
                let Some(children) = normalise_page_rules(&source[open + 1..close]) else {
                    return source.to_owned();
                };
                format!("@media {condition} {{ {children} }}")
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

#[derive(Debug)]
pub struct ValidatedCssRule {
    node: stylo_cssom_model::RuleNode,
}

impl ValidatedCssRule {
    pub fn parse(input: crate::rule_parser::RuleInput<'_>) -> Option<Self> {
        Self::parse_text(input.text())
    }

    fn parse_text(css: &str) -> Option<Self> {
        let authored = match scan_stylesheet_rule_sources(css) {
            Ok(mut rules) => {
                if rules.len() != 1 {
                    return None;
                }
                rules.pop()
            },
            Err(_) => None,
        };
        let validated = ValidatedStylesheetRuleList::parse(css).into_stylesheet();
        if validated.rule_nodes().len() != 1 {
            return None;
        }
        let node = authored
            .as_deref()
            .and_then(parse_rule_node)
            .unwrap_or_else(|| validated.rule_nodes()[0].clone());
        Some(Self { node })
    }

    #[must_use]
    pub fn into_rule_node(self) -> stylo_cssom_model::RuleNode {
        self.node
    }

    pub fn parse_parent_dependent(input: crate::rule_parser::RuleInput<'_>) -> Option<Self> {
        let css = input.text();
        let authored = scan_stylesheet_rule_sources(css).ok()?;
        if authored.len() != 1 {
            return None;
        }
        let source = css.trim_start();
        matches!(source.as_bytes().first(), Some(b'>' | b'+' | b'~'))
            .then(|| Self::parse_text(&format!("& {source}")))
            .flatten()
    }

    pub fn parse_page_child(input: crate::rule_parser::RuleInput<'_>) -> Option<Self> {
        let css = input.text();
        let authored = scan_stylesheet_rule_sources(css).ok()?;
        if authored.len() != 1 {
            return None;
        }
        let rule = crate::rule_parser::ParsedCssRule::parse_page_child(css)?;
        Some(Self {
            node: rule.to_rule_node(),
        })
    }

    pub fn parse_scope_child(input: crate::rule_parser::RuleInput<'_>) -> Option<Self> {
        parse_scope_rule_node(input.text()).map(|node| Self { node })
    }
}

#[must_use]
pub fn rule_selector_namespaces(
    stylesheet: &stylo_cssom_model::StyleSheetLease,
    rule: &stylo_cssom_model::RuleLease,
) -> stylo_cssom_model::RuleNamespaceContext {
    use stylo_cssom_model::{RuleCssomData, RuleNamespaceContext};

    if stylesheet.rule_path(rule.handle()).is_none() {
        return RuleNamespaceContext::default();
    }
    let mut default = None;
    let mut prefixes = Vec::new();
    let rules = stylesheet.top_list();
    for rule in (0..rules.len()).filter_map(|index| rules.rule(index)) {
        if let Some(RuleCssomData::Namespace { prefix, uri }) = rule.node().cssom_data() {
            if prefix.is_empty() {
                default = Some(uri.clone());
            } else {
                prefixes.push((prefix.clone(), uri.clone()));
            }
        }
    }
    RuleNamespaceContext::new(default, prefixes)
}

#[derive(Debug)]
pub struct ValidatedSelectorText {
    css: String,
}

impl ValidatedSelectorText {
    pub fn parse(
        selector: &str,
        namespaces: &stylo_cssom_model::RuleNamespaceContext,
    ) -> Option<Self> {
        let namespaces = crate::declaration_parser::stylo_namespaces(namespaces);
        let url_data = UrlExtraData::from(ABOUT_BLANK.clone());
        let parser = SelectorParser {
            stylesheet_origin: Origin::Author,
            namespaces: &namespaces,
            url_data: &url_data,
            for_supports_rule: false,
        };
        let mut input = ParserInput::new(selector);
        let selectors = Parser::new(&mut input)
            .parse_entirely(|input| SelectorList::parse(&parser, input, ParseRelative::No))
            .ok()?;
        let mut css = String::new();
        CssParserToCss::to_css(&selectors, &mut css).ok()?;
        Some(Self { css })
    }

    pub fn as_str(&self) -> &str {
        &self.css
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidImportRule {
    pub url: stylo_cssom_model::CssResourceUrl,

    pub layer: Option<Option<String>>,

    pub supports: Option<String>,

    pub media: Option<String>,
}

impl From<&stylo_cssom_model::RuleImportRequest> for ValidImportRule {
    fn from(request: &stylo_cssom_model::RuleImportRequest) -> Self {
        let cors = request.cors().map(|mode| match mode {
            stylo_cssom_model::RuleImportCorsMode::Anonymous => {
                stylo_cssom_model::CssUrlCorsMode::Anonymous
            },
            stylo_cssom_model::RuleImportCorsMode::UseCredentials => {
                stylo_cssom_model::CssUrlCorsMode::UseCredentials
            },
        });
        let referrer_policy = request.referrer_policy().map(|policy| match policy {
            stylo_cssom_model::RuleImportReferrerPolicy::NoReferrer => {
                stylo_cssom_model::CssUrlReferrerPolicy::NoReferrer
            },
            stylo_cssom_model::RuleImportReferrerPolicy::NoReferrerWhenDowngrade => {
                stylo_cssom_model::CssUrlReferrerPolicy::NoReferrerWhenDowngrade
            },
            stylo_cssom_model::RuleImportReferrerPolicy::SameOrigin => {
                stylo_cssom_model::CssUrlReferrerPolicy::SameOrigin
            },
            stylo_cssom_model::RuleImportReferrerPolicy::Origin => {
                stylo_cssom_model::CssUrlReferrerPolicy::Origin
            },
            stylo_cssom_model::RuleImportReferrerPolicy::StrictOrigin => {
                stylo_cssom_model::CssUrlReferrerPolicy::StrictOrigin
            },
            stylo_cssom_model::RuleImportReferrerPolicy::OriginWhenCrossOrigin => {
                stylo_cssom_model::CssUrlReferrerPolicy::OriginWhenCrossOrigin
            },
            stylo_cssom_model::RuleImportReferrerPolicy::StrictOriginWhenCrossOrigin => {
                stylo_cssom_model::CssUrlReferrerPolicy::StrictOriginWhenCrossOrigin
            },
            stylo_cssom_model::RuleImportReferrerPolicy::UnsafeUrl => {
                stylo_cssom_model::CssUrlReferrerPolicy::UnsafeUrl
            },
        });
        Self {
            url: stylo_cssom_model::CssResourceUrl::new(
                request.url(),
                stylo_cssom_model::CssUrlRequestModifiers::new(
                    cors,
                    request.integrity().map(str::to_owned),
                    referrer_policy,
                ),
            ),
            layer: match request.layer() {
                stylo_cssom_model::RuleImportLayer::Absent => None,
                stylo_cssom_model::RuleImportLayer::Anonymous => Some(None),
                stylo_cssom_model::RuleImportLayer::Named(name) => Some(Some(name.to_string())),
            },
            supports: request.supports().map(str::to_owned),
            media: request.media().map(str::to_owned),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedImportReplacement {
    rules: Vec<stylo_cssom_model::RuleNode>,
}

impl ParsedImportReplacement {
    pub fn parse(css: &str, import: &ValidImportRule) -> Self {
        let lock = StdArc::new(SharedRwLock::new());
        let stylesheet = ImportPreludeWalker::parse_standalone_stylesheet(
            css,
            &lock,
            None,
            AllowImportRules::No,
        );
        let guard = lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let serialised = contents
            .rules(&guard)
            .iter()
            .map(|rule| rule.to_css_string(&guard))
            .collect::<Vec<_>>()
            .join("\n");
        let mut rules = ParsedStylesheet::parse(&serialised)
            .expect("serialised parsed import rules must remain valid")
            .rules;
        if let Some(media) = &import.media {
            rules = vec![stylo_cssom_model::RuleNode::media(media.as_str(), rules)];
        }
        if let Some(supports) = &import.supports {
            rules = vec![stylo_cssom_model::RuleNode::supports(
                supports.as_str(),
                rules,
            )];
        }
        if let Some(layer) = &import.layer {
            rules = match layer {
                Some(name) => vec![stylo_cssom_model::RuleNode::layer(
                    Some(name.as_str()),
                    rules,
                )],
                None => vec![stylo_cssom_model::RuleNode::layer(
                    None::<StdArc<str>>,
                    rules,
                )],
            };
        }
        Self { rules }
    }

    fn into_rules(self) -> Vec<stylo_cssom_model::RuleNode> {
        self.rules
    }

    pub fn serialise(&self) -> String {
        self.rules
            .iter()
            .map(stylo_cssom_model::RuleNode::serialization)
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn walk_authored_stylesheet(css: &str) -> Vec<AuthoredTopLevelRule> {
    let mut spans = Vec::new();
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    ImportPreludeWalker::walk(css, &mut parser, &mut spans);
    NonOverlappingOrderedSpans::from_parser_order(spans)
        .expect("cssparser yields ordered non-overlapping top-level rules")
        .iter()
        .map(|span| span.rule.clone())
        .collect()
}

#[derive(Debug)]
struct ImportRuleSpan {
    source: Range<usize>,
    rule: ValidImportRule,
}

#[derive(Debug)]
struct NonOverlappingOrderedSpans(Vec<ImportRuleSpan>);

impl NonOverlappingOrderedSpans {
    fn from_parser_order(spans: Vec<ImportRuleSpan>) -> Option<Self> {
        spans
            .windows(2)
            .all(|pair| pair[0].source.end <= pair[1].source.start)
            .then_some(Self(spans))
    }

    fn iter(&self) -> impl Iterator<Item = &ImportRuleSpan> {
        self.0.iter()
    }
}

struct ImportPreludeWalker;

impl ImportPreludeWalker {
    fn walk(css: &str, parser: &mut Parser<'_, '_>, out: &mut Vec<ImportRuleSpan>) {
        while let Some(rule) = next_top_level_rule(css, parser) {
            match Self::effect(rule) {
                ImportPreludeEffect::MaterialisableImport(import) => out.push(import),
                ImportPreludeEffect::Preserve => {},
                ImportPreludeEffect::Close => return,
            }
        }
    }

    fn effect(rule: SlicedTopLevelRule<'_>) -> ImportPreludeEffect {
        match rule {
            SlicedTopLevelRule::Import { source, css } => Self::parse_import(css)
                .map_or(ImportPreludeEffect::Preserve, |rule| {
                    ImportPreludeEffect::MaterialisableImport(ImportRuleSpan { source, rule })
                }),
            SlicedTopLevelRule::PreludeException => ImportPreludeEffect::Preserve,
            SlicedTopLevelRule::NeedsParsedValidity { css } => {
                match Self::parsed_rule_validity(css) {
                    ParsedRuleValidity::IgnoredInvalid => ImportPreludeEffect::Preserve,
                    ParsedRuleValidity::Valid => ImportPreludeEffect::Close,
                }
            },
        }
    }

    fn parse_import(slice: &str) -> Option<ValidImportRule> {
        let lock = StdArc::new(SharedRwLock::new());
        let loader = NonLoadingImportLoader;
        let stylesheet = Self::parse_standalone_stylesheet(
            slice,
            &lock,
            Some(&loader as &dyn StylesheetLoader),
            AllowImportRules::Yes,
        );
        let guard = lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        for rule in contents.rules(&guard) {
            if let CssRule::Import(locked) = rule {
                let import = locked.read_with(&guard);
                return Some(project_import(import, &guard));
            }
        }
        None
    }

    fn parsed_rule_validity(slice: &str) -> ParsedRuleValidity {
        let lock = StdArc::new(SharedRwLock::new());
        let stylesheet =
            Self::parse_standalone_stylesheet(slice, &lock, None, AllowImportRules::No);
        let guard = lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        if contents.rules(&guard).is_empty() {
            ParsedRuleValidity::IgnoredInvalid
        } else {
            ParsedRuleValidity::Valid
        }
    }

    fn parse_standalone_stylesheet(
        css: &str,
        lock: &StdArc<SharedRwLock>,
        loader: Option<&dyn StylesheetLoader>,
        allow_import_rules: AllowImportRules,
    ) -> Stylesheet {
        crate::context::initialise_required_servo_style_prefs();
        Stylesheet::from_str(
            css,
            UrlExtraData::from(ABOUT_BLANK.clone()),
            Origin::Author,
            ServoArc::new(lock.wrap(MediaList::empty())),
            (**lock).clone(),
            loader,
            None,
            QuirksMode::NoQuirks,
            allow_import_rules,
        )
    }
}

enum ImportPreludeEffect {
    MaterialisableImport(ImportRuleSpan),
    Preserve,
    Close,
}

enum ParsedRuleValidity {
    IgnoredInvalid,
    Valid,
}

enum SlicedTopLevelRule<'a> {
    Import { source: Range<usize>, css: &'a str },
    PreludeException,
    NeedsParsedValidity { css: &'a str },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TopLevelRuleEnd {
    Semicolon,
    Block,
    End,
}

fn next_top_level_rule<'a>(
    css: &'a str,
    parser: &mut Parser<'_, '_>,
) -> Option<SlicedTopLevelRule<'a>> {
    let rule_start = loop {
        let before = parser.state();
        #[allow(clippy::needless_continue)]
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::WhiteSpace(_) | Token::Comment(_)) => continue,
            Ok(_) => {
                parser.reset(&before);
                break before.position().byte_index();
            },
            Err(_) => return None,
        }
    };

    let before = parser.state();
    let at_keyword = match parser.next_including_whitespace_and_comments() {
        Ok(Token::AtKeyword(name)) => Some(name.as_ref().to_owned()),
        Ok(_) => None,
        Err(_) => return None,
    };
    parser.reset(&before);

    let end = consume_top_level_rule(parser, at_keyword.is_some());
    let rule_end = parser.position().byte_index();
    match at_keyword.as_deref() {
        Some(name) if name.eq_ignore_ascii_case("import") => Some(SlicedTopLevelRule::Import {
            source: rule_start..rule_end,
            css: &css[rule_start..rule_end],
        }),
        Some(name)
            if name.eq_ignore_ascii_case("charset")
                || name.eq_ignore_ascii_case("layer") && end == TopLevelRuleEnd::Semicolon =>
        {
            Some(SlicedTopLevelRule::PreludeException)
        },
        _ => Some(SlicedTopLevelRule::NeedsParsedValidity {
            css: &css[rule_start..rule_end],
        }),
    }
}

fn consume_top_level_rule(parser: &mut Parser<'_, '_>, is_at_rule: bool) -> TopLevelRuleEnd {
    loop {
        let token = match parser.next_including_whitespace_and_comments() {
            Ok(tok) => tok.clone(),
            Err(_) => return TopLevelRuleEnd::End,
        };
        match token {
            Token::Semicolon if is_at_rule => return TopLevelRuleEnd::Semicolon,
            Token::CurlyBracketBlock => {
                consume_nested_block(parser);
                return TopLevelRuleEnd::Block;
            },
            Token::Function(_) | Token::ParenthesisBlock | Token::SquareBracketBlock => {
                consume_nested_block(parser);
            },
            _ => {},
        }
    }
}

fn consume_nested_block(parser: &mut Parser<'_, '_>) {
    let _ = parser.parse_nested_block(
        |_: &mut Parser<'_, '_>| -> Result<(), cssparser::ParseError<'_, ()>> { Ok(()) },
    );
}

pub struct NonLoadingImportLoader;

impl StylesheetLoader for NonLoadingImportLoader {
    fn request_stylesheet(
        &self,
        url: CssUrl,
        location: SourceLocation,
        lock: &SharedRwLock,
        media: ServoArc<Locked<MediaList>>,
        supports: Option<ImportSupportsCondition>,
        layer: ImportLayer,
    ) -> ServoArc<Locked<ImportRule>> {
        let placeholder = Stylesheet::from_str(
            "",
            UrlExtraData::from(ABOUT_BLANK.clone()),
            Origin::Author,
            media,
            lock.clone(),
            None,
            None,
            QuirksMode::NoQuirks,
            AllowImportRules::No,
        );
        let sheet = ImportSheet::new(ServoArc::new(placeholder));
        ServoArc::new(lock.wrap(ImportRule {
            url,
            stylesheet: sheet,
            supports,
            layer,
            source_location: location,
        }))
    }
}

fn serialise_css_url_original(css_url: &CssUrl) -> String {
    css_url
        .original()
        .unwrap_or_else(|| css_url.as_str())
        .to_owned()
}

fn project_url_modifiers(css_url: &CssUrl) -> stylo_cssom_model::CssUrlRequestModifiers {
    use style::servo::url::{UrlCorsMode, UrlReferrerPolicy};

    let modifiers = css_url.request_modifiers();
    let cors = modifiers.cors().map(|mode| match mode {
        UrlCorsMode::Anonymous => stylo_cssom_model::CssUrlCorsMode::Anonymous,
        UrlCorsMode::UseCredentials => stylo_cssom_model::CssUrlCorsMode::UseCredentials,
    });
    let referrer_policy = modifiers.referrer_policy().map(|policy| match policy {
        UrlReferrerPolicy::NoReferrer => stylo_cssom_model::CssUrlReferrerPolicy::NoReferrer,
        UrlReferrerPolicy::NoReferrerWhenDowngrade => {
            stylo_cssom_model::CssUrlReferrerPolicy::NoReferrerWhenDowngrade
        },
        UrlReferrerPolicy::SameOrigin => stylo_cssom_model::CssUrlReferrerPolicy::SameOrigin,
        UrlReferrerPolicy::Origin => stylo_cssom_model::CssUrlReferrerPolicy::Origin,
        UrlReferrerPolicy::StrictOrigin => stylo_cssom_model::CssUrlReferrerPolicy::StrictOrigin,
        UrlReferrerPolicy::OriginWhenCrossOrigin => {
            stylo_cssom_model::CssUrlReferrerPolicy::OriginWhenCrossOrigin
        },
        UrlReferrerPolicy::StrictOriginWhenCrossOrigin => {
            stylo_cssom_model::CssUrlReferrerPolicy::StrictOriginWhenCrossOrigin
        },
        UrlReferrerPolicy::UnsafeUrl => stylo_cssom_model::CssUrlReferrerPolicy::UnsafeUrl,
    });
    stylo_cssom_model::CssUrlRequestModifiers::new(
        cors,
        modifiers.integrity().map(str::to_owned),
        referrer_policy,
    )
}

fn project_import(import: &ImportRule, guard: &SharedRwLockReadGuard) -> ValidImportRule {
    let url = stylo_cssom_model::CssResourceUrl::new(
        serialise_css_url_original(&import.url),
        project_url_modifiers(&import.url),
    );

    let layer = match &import.layer {
        ImportLayer::None => None,
        ImportLayer::Anonymous => Some(None),
        ImportLayer::Named(name) => {
            let mut buffer = String::new();
            let _ = name.to_css(&mut CssWriter::new(&mut buffer));
            Some(Some(buffer))
        },
    };

    let supports = import.supports.as_ref().map(|support| {
        let mut buffer = String::new();
        buffer.push('(');
        let _ = support.condition.to_css(&mut CssWriter::new(&mut buffer));
        buffer.push(')');
        buffer
    });

    let media = import.stylesheet.media(guard).and_then(|media_list| {
        if media_list.is_empty() {
            None
        } else {
            let mut buffer = String::new();
            let _ = media_list.to_css(&mut CssWriter::new(&mut buffer));
            Some(buffer)
        }
    });

    ValidImportRule {
        url,
        layer,
        supports,
        media,
    }
}

#[must_use]
pub fn project_stylesheet_rule_urls(
    rules: &[stylo_cssom_model::RuleNode],
    base_url: &crate::CssStylesheetBaseUrl,
) -> Vec<stylo_cssom_model::RuleNode> {
    crate::author_rule_projection::map_rule_sources(rules, &mut |rule| {
        let projected = rule
            .clone()
            .with_projection_serialization(rewrite_css_url_tokens(
                &rule.projection_serialization(),
                base_url.as_str(),
            ));
        let Some(block) = rule.payload().declaration_block().filter(|block| {
            block
                .declarations()
                .iter()
                .any(|declaration| declaration.pending_substitution().is_some())
        }) else {
            return projected;
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
                    pending.tokens(),
                    base_url.as_str(),
                )
                .expect("a retained pending value keeps its validated shorthand member")
                .with_importance(declaration.important())
            })
            .collect::<Vec<_>>();
        projected.with_declaration_block(
            stylo_cssom_model::RuleDeclarationBlock::new(
                block.domain(),
                block.serialization(),
                declarations,
            )
            .with_namespaces(block.namespaces().clone())
            .with_shorthand_values(block.shorthand_values()),
        )
    })
}

pub fn rewrite_css_url_tokens(css: &str, base_url: &str) -> String {
    let Ok(base) = Url::parse(base_url) else {
        return css.to_owned();
    };
    rewrite_css_urls(css, &|raw| resolve_if_relative(raw, &base))
}

pub fn rewrite_css_urls(css: &str, resolve: &impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(css.len());
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    let mut last_flushed: usize = 0;
    rewrite_tokens_recursive(css, &mut parser, resolve, &mut out, &mut last_flushed);
    out.push_str(&css[last_flushed..]);
    out
}

fn rewrite_tokens_recursive(
    css: &str,
    parser: &mut Parser<'_, '_>,
    resolve: &impl Fn(&str) -> Option<String>,
    out: &mut String,
    last_flushed: &mut usize,
) {
    loop {
        let token_start = parser.position();

        let token = match parser.next_including_whitespace_and_comments() {
            Ok(tok) => tok.clone(),
            Err(_) => break,
        };
        match token {
            Token::UnquotedUrl(raw) => {
                let token_end = parser.position();
                if let Some(resolved) = resolve(raw.as_ref()) {
                    out.push_str(&css[*last_flushed..token_start.byte_index()]);
                    write_url_token(out, &resolved);
                    *last_flushed = token_end.byte_index();
                }
            },
            Token::Function(ref name) if name.eq_ignore_ascii_case("url") => {
                let quoted = parser.parse_nested_block(
                    |inner| -> Result<
                        Option<(String, SourcePosition, SourcePosition)>,
                        cssparser::ParseError<'_, ()>,
                    > { Ok(extract_url_quoted_string_span(inner)) },
                );
                if let Ok(Some((raw, string_start, string_end))) = quoted
                    && let Some(resolved) = resolve(&raw)
                {
                    out.push_str(&css[*last_flushed..string_start.byte_index()]);
                    write_css_string(out, &resolved);
                    *last_flushed = string_end.byte_index();
                }
            },
            Token::Function(_)
            | Token::ParenthesisBlock
            | Token::SquareBracketBlock
            | Token::CurlyBracketBlock => {
                let _ = parser.parse_nested_block(
                    |inner| -> Result<(), cssparser::ParseError<'_, ()>> {
                        rewrite_tokens_recursive(css, inner, resolve, out, last_flushed);
                        Ok(())
                    },
                );
            },
            _ => {},
        }
    }
}

fn extract_url_quoted_string_span(
    inner: &mut Parser<'_, '_>,
) -> Option<(String, SourcePosition, SourcePosition)> {
    inner.skip_whitespace();
    let start = inner.position();
    let value = inner.expect_string().ok()?.to_string();
    let end = inner.position();
    style::servo::url::UrlRequestModifiers::parse(inner).ok()?;
    inner.expect_exhausted().ok()?;
    Some((value, start, end))
}

fn resolve_if_relative(raw: &str, base_url: &Url) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || should_skip_css_url_rewrite(trimmed) {
        return None;
    }
    base_url.join(trimmed).ok().map(|url| url.to_string())
}

fn write_url_token(out: &mut String, url: &str) {
    out.push_str("url(\"");
    write_css_string_contents(out, url);
    out.push_str("\")");
}

fn write_css_string(out: &mut String, value: &str) {
    out.push('"');
    write_css_string_contents(out, value);
    out.push('"');
}

fn write_css_string_contents(out: &mut String, value: &str) {
    for ch in value.chars() {
        if ch == char::from(34) {
            out.push(char::from(92));
            out.push(char::from(34));
        } else if ch == char::from(92) {
            out.push(char::from(92));
            out.push(char::from(92));
        } else {
            out.push(ch);
        }
    }
}

fn should_skip_css_url_rewrite(raw_url: &str) -> bool {
    if raw_url.starts_with('#')
        || raw_url.starts_with("//")
        || raw_url.starts_with("data:")
        || raw_url.starts_with("blob:")
    {
        return true;
    }

    if let Some((scheme, _)) = raw_url.split_once(':')
        && !scheme.is_empty()
        && scheme
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
    {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsed_stylesheets_preserve_nested_blocks_and_import_statements() {
        let sheet = ParsedStylesheet::parse(
            "@import url('a;b.css'); @media all { a { color: red } } b { color: green }",
        )
        .expect("valid stylesheet must parse into rules");
        assert_eq!(
            sheet
                .rule_nodes()
                .iter()
                .map(stylo_cssom_model::RuleNode::grammar)
                .collect::<Vec<_>>(),
            [
                stylo_cssom_model::RuleGrammar::Import,
                stylo_cssom_model::RuleGrammar::Media,
                stylo_cssom_model::RuleGrammar::Style,
            ]
        );
    }

    #[test]
    fn parsed_stylesheets_recover_an_unterminated_import_string_at_eof() {
        let sheet = ParsedStylesheet::parse("@import \"support/eof-green.css")
            .expect("stylesheet parsing must close open constructs at EOF");

        assert_eq!(sheet.rule_nodes().len(), 1);
        assert_eq!(
            sheet.rule_nodes()[0].grammar(),
            stylo_cssom_model::RuleGrammar::Import
        );
        assert!(
            sheet.rule_nodes()[0]
                .projection_serialization()
                .contains("support/eof-green.css")
        );
    }

    #[test]
    fn parsed_stylesheets_keep_authored_projection_separate_from_cssom_text() {
        let stylesheet =
            ParsedStylesheet::parse("a{color:red}").expect("the authored rule must parse");
        let rule = &stylesheet.rule_nodes()[0];

        assert_eq!(rule.serialization(), "a { color: red; }");
        assert_eq!(rule.projection_serialization(), "a{color:red}");
    }

    #[test]
    fn validated_rule_lists_retain_empty_media_groups() {
        let stylesheet = ValidatedStylesheetRuleList::parse("@media all {}").into_stylesheet();

        assert_eq!(stylesheet.rules().collect::<Vec<_>>(), ["@media all {\n}"]);
    }

    #[test]
    fn validated_page_rules_retain_typed_non_applicable_declarations() {
        let source = "@media print { @page :left { column-count: 7; color: red; } }";
        assert!(contains_page_rule(source));
        let stylesheet = ValidatedStylesheetRuleList::parse(source).into_stylesheet();

        assert_eq!(
            stylesheet.rules().collect::<Vec<_>>(),
            ["@media print { @page :left { column-count: 7; color: red; } }"]
        );
    }

    #[test]
    fn validated_css_rules_accept_typed_logical_selectors() {
        for selector in [":is(div)", ":where(div)", ":not(div)", ":has(> div)"] {
            assert!(
                ValidatedCssRule::parse(crate::rule_parser::RuleInput::new(&format!(
                    "{selector} {{}}"
                )))
                .is_some(),
                "{selector} must remain valid at the CSSOM rule boundary"
            );
        }
    }

    #[test]
    fn validated_css_rules_reject_content_after_one_complete_rule() {
        for css in [
            "@media print {} trailing",
            "a { color: red } trailing",
            "@media print {} @media screen {}",
            "a {} @media print {}",
        ] {
            assert!(
                ValidatedCssRule::parse(crate::rule_parser::RuleInput::new(css)).is_none(),
                "{css}"
            );
        }
    }

    #[test]
    fn parent_dependent_rules_are_strict_but_accept_relative_selectors() {
        assert_eq!(
            ValidatedCssRule::parse_parent_dependent(crate::rule_parser::RuleInput::new(
                "> .target {}"
            ))
            .map(ValidatedCssRule::into_rule_node)
            .map(|rule| rule.serialization()),
            Some("& > .target { }".to_owned())
        );
        for css in ["@media print {} trailing", "p {} trailing"] {
            assert!(
                ValidatedCssRule::parse_parent_dependent(crate::rule_parser::RuleInput::new(css))
                    .is_none(),
                "{css}"
            );
        }
        assert!(
            ValidatedCssRule::parse_page_child(crate::rule_parser::RuleInput::new(
                "@top-center {} trailing"
            ))
            .is_none()
        );
        assert_eq!(
            ValidatedCssRule::parse_page_child(crate::rule_parser::RuleInput::new(
                "@top-center {}"
            ))
            .map(ValidatedCssRule::into_rule_node)
            .map(|rule| rule.serialization()),
            Some("@top-center {  }".to_owned())
        );
        assert_eq!(
            ValidatedCssRule::parse_scope_child(crate::rule_parser::RuleInput::new("> .target {}"))
                .map(ValidatedCssRule::into_rule_node)
                .map(|rule| rule.serialization()),
            Some("> .target { }".to_owned())
        );
        assert_eq!(
            parse_nested_declarations_rule_node("z-index: 3;").map(|rule| rule.serialization()),
            Some("z-index: 3;".to_owned())
        );
        assert!(parse_nested_declarations_rule_node("not-a-property: 3;").is_none());
    }

    #[test]
    fn validated_css_rules_retain_one_recovered_font_face_rule() {
        let css = "@font-face { font-familly: foo; src: url('font'; }";

        assert_eq!(
            ValidatedCssRule::parse(crate::rule_parser::RuleInput::new(css))
                .map(ValidatedCssRule::into_rule_node)
                .map(|rule| rule.serialization()),
            Some("@font-face { }".to_owned())
        );
    }

    #[test]
    fn validated_css_rules_recover_font_feature_values_at_eof() {
        for rule in [
            "@font-feature-values bongo { @styleset { abc: 1 2 3; }",
            "@font-feature-values bongo { @styleset { abc: 1 2 3",
        ] {
            assert_eq!(
                ValidatedCssRule::parse(crate::rule_parser::RuleInput::new(rule))
                    .map(ValidatedCssRule::into_rule_node)
                    .map(|rule| rule.serialization()),
                Some(
                    "@font-feature-values bongo {\n  @styleset {\n    abc: 1 2 3;\n  }\n}"
                        .to_owned()
                )
            );
        }
    }

    fn unqualified_import(url: &str) -> ValidImportRule {
        ValidImportRule {
            url: stylo_cssom_model::CssResourceUrl::without_modifiers(url),
            layer: None,
            supports: None,
            media: None,
        }
    }

    fn splice_child(parent: &str, child: &str) -> String {
        ParsedStylesheet::parse(parent)
            .expect("the parent stylesheet must parse")
            .materialise_imports(|import| Some(ParsedImportReplacement::parse(child, import)))
            .serialise()
    }

    fn parse_final_stylesheet(css: &str) -> String {
        ParsedImportReplacement::parse(css, &unqualified_import("final.css")).serialise()
    }

    fn parse_child_and_parent_rules(css: &str) -> String {
        let parsed = parse_final_stylesheet(css);
        assert!(parsed.contains(".child"));
        assert!(parsed.contains(".parent"));
        parsed
    }

    #[test]
    fn parsed_import_replacement_preserves_the_implicit_counter_increment() {
        let parsed = parse_final_stylesheet("grid { counter-increment: grid }");

        assert!(parsed.contains("counter-increment: grid 1"), "{parsed}");
    }

    #[test]
    fn parsed_import_replacement_keeps_malformed_child_tokens_inside_the_child() {
        let parent = "@import 'child.css'; .parent { color: green }";
        let import = unqualified_import("child.css");

        let materialised = ParsedStylesheet::parse(parent)
            .expect("the parent stylesheet must parse")
            .materialise_imports(|_| {
                Some(ParsedImportReplacement::parse(
                    ".child { color: red } /*",
                    &import,
                ))
            })
            .serialise();

        let parsed = parse_child_and_parent_rules(&materialised);
        assert!(!parsed.contains("/*"));
    }

    #[test]
    fn parsed_import_replacement_keeps_valid_child_rules_after_invalid_rules() {
        let parent = "@import 'child.css'; .parent { color: green }";
        let import = unqualified_import("child.css");
        let child = "# { color: red } .child { color: green }";

        let materialised = ParsedStylesheet::parse(parent)
            .expect("the parent stylesheet must parse")
            .materialise_imports(|_| Some(ParsedImportReplacement::parse(child, &import)))
            .serialise();

        let parsed = parse_child_and_parent_rules(&materialised);
        assert!(!parsed.contains("# {"));
    }

    #[test]
    fn parsed_import_replacement_keeps_valid_child_before_parent() {
        let materialised = splice_child(
            "@import 'child.css'; .parent { color: green }",
            ".child { color: blue }",
        );

        assert!(
            materialised
                .find(".child")
                .expect("the child rule must remain")
                < materialised
                    .find(".parent")
                    .expect("the parent rule must remain")
        );
    }

    #[test]
    fn parsed_import_replacement_cannot_expose_a_child_import() {
        let materialised = splice_child(
            "@import 'child.css'; .parent { color: green }",
            "@import 'nested.css'; .child { color: blue }",
        );

        assert!(!materialised.contains("nested.css"));
        assert!(materialised.contains(".child"));
    }

    #[test]
    fn parsed_import_replacement_preserves_resolved_child_urls() {
        let child = rewrite_css_url_tokens(
            ".child { background: url('../images/marker.png') }",
            "https://fixtures.test/styles/child.css",
        );
        let materialised = splice_child("@import 'child.css';", &child);

        assert!(materialised.contains("https://fixtures.test/images/marker.png"));
    }

    #[test]
    fn url_rewrite_preserves_request_modifiers_after_the_resolved_string() {
        let rewritten = rewrite_css_url_tokens(
            r#".child { background: url('../images/marker.svg' cross-origin(anonymous) integrity("sha256-fixture") referrer-policy(origin)) }"#,
            "https://fixtures.test/styles/child.css",
        );

        assert!(rewritten.contains(
            r#"url("https://fixtures.test/images/marker.svg" cross-origin(anonymous) integrity("sha256-fixture") referrer-policy(origin))"#
        ));
    }

    #[test]
    fn external_font_face_urls_resolve_against_the_stylesheet() {
        let rewritten = rewrite_css_url_tokens(
            "@font-face { font-family: Diagnostic; src: url('./Diagnostic.ttf') format('opentype') }",
            "https://fixtures.test/fonts/diagnostic/style.css",
        );

        assert!(
            rewritten.contains("https://fixtures.test/fonts/diagnostic/Diagnostic.ttf"),
            "{rewritten}"
        );
    }

    #[test]
    fn parsed_import_replacement_applies_media_and_supports_qualifiers() {
        let materialised = splice_child(
            "@import 'child.css' supports(display: block) print;",
            ".child { color: green }",
        );

        assert!(materialised.contains("@supports (display: block)"));
        assert!(materialised.contains("@media print"));
        assert!(
            materialised
                .find("@supports")
                .expect("the supports qualifier must remain")
                < materialised
                    .find("@media")
                    .expect("the media qualifier must remain")
        );
    }

    #[test]
    fn parsed_import_replacement_applies_named_and_anonymous_layers() {
        let named = splice_child(
            "@import 'child.css' layer(theme);",
            ".child { color: green }",
        );
        let anonymous = splice_child("@import 'child.css' layer;", ".child { color: green }");

        assert!(named.contains("@layer theme"));
        assert!(anonymous.contains("@layer {"));
    }

    #[test]
    fn stylesheet_without_valid_imports_keeps_typed_rules() {
        let css = concat!(
            "/* lead */\n",
            "#three { background-color: green; }\n",
            "#foo { background: url(foo\"bar) }\n",
            "#three { background-color: red; }\n",
        );

        let sources = crate::rule_parser::forgiving_rule_sources(css);
        assert!(sources[0].text.starts_with("/* lead */"));
        assert_eq!(
            sources
                .iter()
                .map(|source| source.text.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            css.trim_end()
        );
        let native = crate::rule_parser::ParsedCssRule::parse_stylesheet(css);
        assert_eq!(sources.len(), native.len());
        let parsed = ParsedStylesheet::parse(css).expect("the stylesheet must recover");
        assert_eq!(parsed.rule_nodes().len(), native.len());
        assert_eq!(
            parsed
                .rule_nodes()
                .iter()
                .map(stylo_cssom_model::RuleNode::projection_serialization)
                .collect::<Vec<_>>()
                .join("\n"),
            css.trim_end()
        );
        let materialised = parsed.materialise_imports(|_| None);

        assert_eq!(materialised.rule_nodes(), parsed.rule_nodes());
    }

    #[test]
    fn stylesheet_namespace_context_reaches_qualified_selectors() {
        let css = "@namespace x url(https://example.test/ns); x|item { color: green }";

        let parsed = ParsedStylesheet::parse(css).expect("the namespace fixture must parse");

        assert_eq!(parsed.rule_nodes().len(), 2);
        assert_eq!(
            parsed.rule_nodes()[1].grammar(),
            stylo_cssom_model::RuleGrammar::Style
        );
        assert_eq!(
            parsed.rule_nodes()[1].projection_serialization(),
            "x|item { color: green }"
        );
    }

    #[test]
    fn malformed_trailing_layer_does_not_discard_prior_valid_layers() {
        let css = concat!(
            "@layer A { @layer A { target { color: red; } } }",
            "@layer B.A { target { color: green; } }",
            "@layer A.A target { color: red; } }",
        );

        let parsed = ParsedStylesheet::parse(css).expect("the layer fixture must parse");

        assert_eq!(parsed.rule_nodes().len(), 2);
        assert!(
            parsed
                .rule_nodes()
                .iter()
                .all(|rule| rule.grammar() == stylo_cssom_model::RuleGrammar::LayerBlock)
        );
    }

    #[test]
    fn mismatched_brace_inside_parentheses_does_not_end_the_style_rule() {
        let css = "#test { --test: green; --test: (}); background-color: var(--test); }";

        let sources = scan_stylesheet_rule_sources(css).expect("the complete style rule must scan");
        let parsed = ParsedStylesheet::parse(css).expect("the stylesheet must parse");

        assert_eq!(sources, [css]);
        assert_eq!(parsed.rule_nodes().len(), 1);
        assert!(!parsed.serialise().contains("(})"));
    }

    #[test]
    fn strict_source_scanning_rejects_unmatched_component_closers() {
        for closer in [")", "]"] {
            assert!(
                scan_stylesheet_rule_sources(&format!(".valid {{ color:green }} {closer}"))
                    .is_err()
            );
        }
    }

    #[test]
    fn stylesheet_url_projection_preserves_pending_sources_and_namespace_context() {
        let parsed = ParsedStylesheet::parse("@namespace x 'urn:source'; x|target { all:initial; background:var(--colour) url(image.svg) !important; background-size:1px }").unwrap();
        let root = stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::Author,
            parsed.rule_nodes().to_vec(),
        );
        let base = crate::CssStylesheetBaseUrl::from_absolute(
            Url::parse("https://source.test/assets/style.css").unwrap(),
        );
        let projected = project_stylesheet_rule_urls(root.rules(), &base);
        let original = root.rules()[1].payload().declaration_block().unwrap();
        let block = projected[1].payload().declaration_block().unwrap();
        for name in [
            "view-transition-scope",
            "view-transition-group",
            "grid-lanes-direction",
            "-webkit-box-orient",
        ] {
            let original = original
                .declarations()
                .iter()
                .find(|declaration| declaration.matches_name(name))
                .unwrap();
            let projected = block
                .declarations()
                .iter()
                .find(|declaration| declaration.matches_name(name))
                .unwrap();
            assert_eq!(original, projected);
            assert_eq!(projected.value(), "initial");
        }
        assert_eq!(block.namespaces(), original.namespaces());
        assert!(
            block
                .declarations()
                .iter()
                .any(|declaration| declaration.pending_substitution().is_some())
        );
        for declaration in block
            .declarations()
            .iter()
            .filter(|declaration| declaration.pending_substitution().is_some())
        {
            let pending = declaration.pending_substitution().unwrap();
            assert_eq!(pending.base_url(), base.as_str());
            assert!(pending.tokens().contains("image.svg"));
            assert!(!pending.tokens().contains("https://"));
            assert_eq!(declaration.value(), "");
            assert!(declaration.important());
        }
        assert!(
            projected[1]
                .projection_serialization()
                .contains("https://source.test/assets/image.svg")
        );
    }

    #[test]
    fn balanced_brace_blocks_inside_parentheses_remain_component_values() {
        let supports = "@supports ({x}) {}";
        let mixin = "#target { @apply --m({red !important}); }";

        assert_eq!(scan_stylesheet_rule_sources(supports).unwrap(), [supports]);
        assert_eq!(scan_stylesheet_rule_sources(mixin).unwrap(), [mixin]);
        assert_eq!(
            ParsedStylesheet::parse(supports)
                .unwrap()
                .rule_nodes()
                .len(),
            1
        );
        assert_eq!(
            ParsedStylesheet::parse(mixin).unwrap().rule_nodes().len(),
            1
        );
    }

    #[test]
    fn malformed_font_feature_member_does_not_expose_a_following_member() {
        let css = "@font-feature-values bongo { @blah } @styleset { abc: 1 2 3; } }";

        let rule = parse_rule_node(css).expect("the font-feature-values rule must recover");

        assert_eq!(
            rule.grammar(),
            stylo_cssom_model::RuleGrammar::FontFeatureValues
        );
        assert!(!rule.serialization().contains("@styleset"));
    }

    #[test]
    fn nested_scope_declarations_keep_their_top_level_style_rule() {
        let css = concat!(
            "#child { color: green; }",
            ".b { #child { @scope (&) { --x: 1; color: red; } } }",
        );

        let parsed = ParsedStylesheet::parse(css).expect("the nested scope fixture must parse");

        assert_eq!(parsed.rule_nodes().len(), 2);
        let nested = parsed.rule_nodes()[1].payload().nested()[0]
            .payload()
            .nested()[0]
            .payload()
            .nested();
        assert_eq!(nested.len(), 1);
        assert_eq!(
            nested[0].grammar(),
            stylo_cssom_model::RuleGrammar::NestedDeclarations
        );
    }

    #[test]
    fn stylesheet_keeps_custom_highlight_selectors_for_projection() {
        let css = "::highlight(note) { color: blue }";

        let parsed = ParsedStylesheet::parse(css).expect("the highlight fixture must parse");

        assert_eq!(parsed.rule_nodes().len(), 1);
        assert_eq!(
            parsed.rule_nodes()[0].grammar(),
            stylo_cssom_model::RuleGrammar::Style
        );
        assert_eq!(parsed.rule_nodes()[0].projection_serialization(), css);
    }

    #[test]
    fn stylesheet_keeps_grouped_highlight_selectors_for_projection() {
        let css = "@container (width >= 400px) { ::selection { color: green } ::highlight(note) { color: green } }";

        let parsed =
            ParsedStylesheet::parse(css).expect("the grouped highlight fixture must parse");

        assert_eq!(parsed.rule_nodes().len(), 1);
        assert_eq!(
            parsed.rule_nodes()[0].grammar(),
            stylo_cssom_model::RuleGrammar::Container
        );
        assert_eq!(parsed.rule_nodes()[0].projection_serialization(), css);
    }

    #[test]
    fn stylesheet_keeps_view_transition_selectors_for_later_projection() {
        let css = "html:active-view-transition #target { color: green } html::view-transition-old(root) { opacity: 1 }";

        let parsed = ParsedStylesheet::parse(css).expect("the view-transition fixture must parse");

        assert_eq!(parsed.rule_nodes().len(), 2);
        assert!(
            parsed
                .rule_nodes()
                .iter()
                .all(|rule| rule.grammar() == stylo_cssom_model::RuleGrammar::Style)
        );
        assert_eq!(
            parsed.rule_nodes()[0].projection_serialization(),
            "html:active-view-transition #target { color: green }"
        );
        assert_eq!(
            parsed.rule_nodes()[1].projection_serialization(),
            "html::view-transition-old(root) { opacity: 1 }"
        );
    }

    #[test]
    fn splice_replaces_only_valid_early_import_ranges_in_source_order() {
        let css = concat!(
            "/* lead */ @import \"one.css\";\n",
            "/* between */ @import \"two.css\";\n",
            ".body { color: green } @import \"late.css\";",
        );
        let mut urls = Vec::new();

        let materialised = ParsedStylesheet::parse(css)
            .expect("the stylesheet must parse")
            .materialise_imports(|import| {
                urls.push(import.url.as_str().to_owned());
                Some(ParsedImportReplacement::parse(
                    &format!(".{} {{ display: block }}", import.url.as_str()),
                    import,
                ))
            })
            .serialise();

        assert_eq!(urls, ["one.css", "two.css"]);
        let one = materialised
            .find(".one.css")
            .expect("the first child rule must remain");
        let two = materialised
            .find(".two.css")
            .expect("the second child rule must remain");
        let body = materialised
            .find(".body")
            .expect("the parent rule must remain");
        assert!(one < two && two < body);
        assert!(materialised.contains("@import") && materialised.contains("late.css"));
    }

    #[test]
    fn invalid_import_is_dropped_and_does_not_hide_a_later_valid_import() {
        let css = "@import ; @import 'valid.css'; .body {}";

        let materialised = ParsedStylesheet::parse(css)
            .expect("the stylesheet must parse")
            .materialise_imports(|import| {
                assert_eq!(import.url.as_str(), "valid.css");
                Some(ParsedImportReplacement::parse(".imported {}", import))
            })
            .serialise();

        assert!(!materialised.contains("@import ;"));
        let imported = materialised
            .find(".imported")
            .expect("the valid import replacement must remain");
        let body = materialised
            .find(".body")
            .expect("the parent rule must remain");
        assert!(imported < body);
    }

    #[test]
    fn whole_stylesheet_rejection_cannot_expose_scanner_suffixes() {
        let garbage_import = ParsedStylesheet::parse(
            "# :unknownpseudo @import 'import-red.css'; .import { color: red; } p { color: green; }",
        )
        .expect("the invalid stylesheet remains structurally complete")
        .serialise();
        assert!(
            !garbage_import.contains("import-red.css"),
            "{garbage_import}"
        );
        assert!(!garbage_import.contains(".import"), "{garbage_import}");
        assert!(garbage_import.contains("color: green"), "{garbage_import}");

        let unknown_suffix =
            ParsedStylesheet::parse("p { color: green; } @foo {}; p { color: red; }")
                .expect("the unknown at-rule fixture remains structurally complete")
                .serialise();
        assert!(unknown_suffix.contains("color: green"), "{unknown_suffix}");
        assert!(!unknown_suffix.contains("color: red"), "{unknown_suffix}");

        let charset_suffix =
            ParsedStylesheet::parse("test; @charset \"UTF-8\"; .target { color: red; }")
                .expect("the charset fixture remains structurally complete")
                .serialise();
        assert!(!charset_suffix.contains("color: red"), "{charset_suffix}");
    }

    #[test]
    fn wholly_invalid_unclosed_rules_do_not_become_unknown_cssom_rules() {
        for css in ["@scope (", "@starting-style ( {}", "[foo["] {
            let parsed = ParsedStylesheet::parse(css)
                .expect("invalid stylesheet recovery must be deterministic");
            assert!(parsed.rule_nodes().is_empty(), "{css:?}");
        }
    }

    fn import_urls(css: &str) -> Vec<String> {
        walk_authored_stylesheet(css)
            .into_iter()
            .map(|rule| rule.url.as_str().to_owned())
            .collect()
    }

    #[test]
    fn valid_style_rule_closes_the_import_prelude() {
        let rules = walk_authored_stylesheet(".valid { color: green } @import 'late.css';");

        assert!(rules.is_empty());
    }

    #[test]
    fn initial_import_remains_a_materialisable_import() {
        let rules = walk_authored_stylesheet("@import 'early.css'; .valid { color: green }");

        assert!(matches!(rules.as_slice(), [import] if import.url.as_str() == "early.css"));
    }

    #[test]
    fn quoted_import_retains_typed_request_modifiers() {
        let rules = walk_authored_stylesheet(
            r#"@import url("child.css" referrer-policy(no-referrer) integrity("sha256-fixture") cross-origin(use-credentials));"#,
        );

        let [import] = rules.as_slice() else {
            panic!("the valid import must be projected");
        };
        assert_eq!(import.url.as_str(), "child.css");
        assert_eq!(
            import.url.modifiers().cors(),
            Some(stylo_cssom_model::CssUrlCorsMode::UseCredentials)
        );
        assert_eq!(import.url.modifiers().integrity(), Some("sha256-fixture"));
        assert_eq!(
            import.url.modifiers().referrer_policy(),
            Some(stylo_cssom_model::CssUrlReferrerPolicy::NoReferrer)
        );
    }

    #[test]
    fn comments_charset_and_layer_statement_preserve_the_import_prelude() {
        let rules = import_urls(
            r#"/* lead */ @charset "UTF-8"; @layer reset, theme;
               @import "one.css"; /* between */ @import "two.css";"#,
        );

        assert_eq!(rules, ["one.css", "two.css"]);
    }

    #[test]
    fn layer_block_closes_the_import_prelude() {
        let rules = import_urls("@layer reset {} @import 'late.css';");

        assert!(rules.is_empty());
    }

    #[test]
    fn ignored_unknown_at_rule_preserves_the_import_prelude() {
        let rules = import_urls("@badat-rule foo; @import 'green.css';");

        assert_eq!(rules, ["green.css"]);
    }

    #[test]
    fn ignored_invalid_selectors_preserve_the_import_prelude() {
        let rules =
            import_urls("# { color: red } :unknownpseudo { color: red } @import 'green.css';");

        assert_eq!(rules, ["green.css"]);
    }

    #[test]
    fn ignored_bad_selector_block_preserves_the_import_prelude() {
        let rules = import_urls("1badselector { someprop: someval; } @import 'green.css';");

        assert_eq!(rules, ["green.css"]);
    }

    #[test]
    fn ignored_malformed_known_at_rules_preserve_the_import_prelude() {
        let rules = import_urls("@media; @page; @charset; @import 'green.css';");

        assert_eq!(rules, ["green.css"]);
    }

    #[test]
    fn valid_conditional_rule_closes_the_import_prelude() {
        let rules = import_urls("@media print {} @import 'late.css';");

        assert!(rules.is_empty());
    }

    #[test]
    fn ignored_block_does_not_expose_its_nested_import() {
        let rules = import_urls("@badat-rule { @import 'nested.css'; } @import 'green.css';");

        assert_eq!(rules, ["green.css"]);
    }
}
