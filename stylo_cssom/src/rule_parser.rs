use cssparser::{Parser, ParserInput, ToCss as CssParserToCss, Token};
use servo_arc::Arc;
use style::{
    properties::PropertyDeclarationBlock,
    shared_lock::{SharedRwLockReadGuard, ToCssWithGuard},
    stylesheets::{
        CssRule, CssRules, FontPaletteValuesRule, MarginRule, Origin, PageRule,
        container_rule::{ContainerCondition, ContainerConditions},
    },
};
use style_traits::{CssWriter, ToCss};

pub mod declaration_list;
mod font_feature_values;
mod keyframes;
#[cfg(test)]
mod native_common_properties;
mod source;
pub use font_feature_values::{font_feature_values_node, replace_font_feature_family};
use keyframes::{CanonicalKeyframeRule, CanonicalKeyframesRule};
pub use keyframes::{
    parse_keyframe_rule, parse_keyframe_selector, replace_keyframe_declarations,
    replace_keyframe_selector, replace_keyframes_name, serialize_keyframe_selector,
};
pub use source::{forgiving_rule_sources, stylesheet_parser_input};

#[derive(Clone, Copy, Debug)]
pub struct RuleInput<'a>(&'a str);

impl<'a> RuleInput<'a> {
    #[must_use]
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }

    pub const fn text(self) -> &'a str {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Observable rule text cannot become rule parser input.
///
/// ```compile_fail
/// fn parse(_: stylo_cssom::RuleInput<'_>) {}
/// fn reject(output: stylo_cssom::RuleSerialization) {
///     parse(output);
/// }
/// ```
pub struct RuleSerialization(String);

impl RuleSerialization {
    #[must_use]
    pub fn into_css_text(self) -> String {
        self.0
    }
}

#[must_use]
pub fn serialize_rule_node(node: &stylo_cssom_model::RuleNode) -> RuleSerialization {
    RuleSerialization(node.serialization())
}

/// The standards-defined serialization of one successfully parsed CSS rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedCssRule {
    serialization: String,
    projection: Option<String>,
    source_location: Option<cssparser::SourceLocation>,
    namespaces: stylo_cssom_model::RuleNamespaceContext,
    kind: ParsedCssRuleKind,
    interface_name: CssomRuleInterfaceName,
    grammar: stylo_cssom_model::RuleGrammar,
}

#[derive(Clone, Copy)]
struct RuleSource<'a> {
    source: &'a source::AuthoredSource<'a>,
    rule: Option<&'a str>,
    end: Option<usize>,
    namespaces: &'a stylo_cssom_model::RuleNamespaceContext,
}

fn model_namespaces(
    namespaces: &style::stylesheets::Namespaces,
) -> stylo_cssom_model::RuleNamespaceContext {
    stylo_cssom_model::RuleNamespaceContext::new(
        namespaces
            .default
            .as_ref()
            .map(|namespace| namespace.as_ref().into()),
        namespaces
            .prefixes
            .iter()
            .map(|(prefix, namespace)| (prefix.as_ref().into(), namespace.as_ref().into())),
    )
}

macro_rules! cssom_rule_interface_names {
    ($($variant:ident => $name:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum CssomRuleInterfaceName {
            $($variant),+
        }

        impl CssomRuleInterfaceName {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name),+
                }
            }
        }
    };
}

cssom_rule_interface_names! {
    CssRule => "CSSRule",
    Style => "CSSStyleRule",
    Namespace => "CSSNamespaceRule",
    Import => "CSSImportRule",
    Media => "CSSMediaRule",
    Supports => "CSSSupportsRule",
    Container => "CSSContainerRule",
    FontFace => "CSSFontFaceRule",
    FontFeatureValues => "CSSFontFeatureValuesRule",
    FontPaletteValues => "CSSFontPaletteValuesRule",
    CounterStyle => "CSSCounterStyleRule",
    Keyframes => "CSSKeyframesRule",
    Keyframe => "CSSKeyframeRule",
    Margin => "CSSMarginRule",
    Page => "CSSPageRule",
    Property => "CSSPropertyRule",
    LayerBlock => "CSSLayerBlockRule",
    LayerStatement => "CSSLayerStatementRule",
    Scope => "CSSScopeRule",
    StartingStyle => "CSSStartingStyleRule",
    PositionTry => "CSSPositionTryRule",
    NestedDeclarations => "CSSNestedDeclarations",
    ColorProfile => "CSSColorProfileRule",
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CssomRuleInterfaceParent {
    None,
    CssRule,
    GroupingRule,
    ConditionRule,
}

impl CssomRuleInterfaceName {
    pub const CSS_RULE: Self = Self::CssRule;

    #[cfg(test)]
    fn from_at_keyword(name: &str) -> Self {
        match name.to_ascii_lowercase().as_str() {
            "import" => Self::Import,
            "namespace" => Self::Namespace,
            "media" => Self::Media,
            "supports" => Self::Supports,
            "container" => Self::Container,
            "font-face" => Self::FontFace,
            "font-feature-values" => Self::FontFeatureValues,
            "font-palette-values" => Self::FontPaletteValues,
            "counter-style" => Self::CounterStyle,
            "keyframes" | "-webkit-keyframes" => Self::Keyframes,
            "page" => Self::Page,
            "top-left-corner"
            | "top-left"
            | "top-center"
            | "top-right"
            | "top-right-corner"
            | "right-top"
            | "right-middle"
            | "right-bottom"
            | "bottom-right-corner"
            | "bottom-right"
            | "bottom-center"
            | "bottom-left"
            | "bottom-left-corner"
            | "left-bottom"
            | "left-middle"
            | "left-top" => Self::Margin,
            "property" => Self::Property,
            "scope" => Self::Scope,
            "starting-style" => Self::StartingStyle,
            "position-try" => Self::PositionTry,
            "color-profile" => Self::ColorProfile,
            _ => Self::CssRule,
        }
    }

    pub const fn parent(self) -> CssomRuleInterfaceParent {
        match self {
            Self::CssRule => CssomRuleInterfaceParent::None,
            Self::Style | Self::Page | Self::LayerBlock | Self::Scope | Self::StartingStyle => {
                CssomRuleInterfaceParent::GroupingRule
            },
            Self::Media | Self::Supports | Self::Container => {
                CssomRuleInterfaceParent::ConditionRule
            },
            Self::Namespace
            | Self::Import
            | Self::FontFace
            | Self::FontFeatureValues
            | Self::FontPaletteValues
            | Self::CounterStyle
            | Self::Keyframes
            | Self::Keyframe
            | Self::Margin
            | Self::Property
            | Self::LayerStatement
            | Self::PositionTry
            | Self::NestedDeclarations
            | Self::ColorProfile => CssomRuleInterfaceParent::CssRule,
        }
    }

    pub const fn legacy_type(self) -> Option<CssomLegacyRuleType> {
        match self {
            Self::Style => Some(CssomLegacyRuleType::Style),
            Self::Namespace => Some(CssomLegacyRuleType::Namespace),
            Self::Import => Some(CssomLegacyRuleType::Import),
            Self::Media => Some(CssomLegacyRuleType::Media),
            Self::Supports => Some(CssomLegacyRuleType::Supports),
            Self::FontFace => Some(CssomLegacyRuleType::FontFace),
            Self::FontFeatureValues => Some(CssomLegacyRuleType::FontFeatureValues),
            Self::CounterStyle => Some(CssomLegacyRuleType::CounterStyle),
            Self::Keyframes => Some(CssomLegacyRuleType::Keyframes),
            Self::Keyframe => Some(CssomLegacyRuleType::Keyframe),
            Self::Margin => Some(CssomLegacyRuleType::Margin),
            Self::Page => Some(CssomLegacyRuleType::Page),
            Self::CssRule
            | Self::Container
            | Self::FontPaletteValues
            | Self::Property
            | Self::LayerBlock
            | Self::LayerStatement
            | Self::Scope
            | Self::StartingStyle
            | Self::PositionTry
            | Self::NestedDeclarations
            | Self::ColorProfile => None,
        }
    }
}

#[cfg(test)]
pub fn cssom_rule_interface_name(css: &str) -> CssomRuleInterfaceName {
    if let Some(rule) = ParsedCssRule::parse(css) {
        return rule.interface_name();
    }
    first_at_keyword(css).as_deref().map_or(
        CssomRuleInterfaceName::CSS_RULE,
        CssomRuleInterfaceName::from_at_keyword,
    )
}

pub const fn cssom_rule_interface_name_for_grammar(
    grammar: stylo_cssom_model::RuleGrammar,
) -> CssomRuleInterfaceName {
    use stylo_cssom_model::RuleGrammar;

    match grammar {
        RuleGrammar::Style => CssomRuleInterfaceName::Style,
        RuleGrammar::Namespace => CssomRuleInterfaceName::Namespace,
        RuleGrammar::Import => CssomRuleInterfaceName::Import,
        RuleGrammar::Media => CssomRuleInterfaceName::Media,
        RuleGrammar::Supports => CssomRuleInterfaceName::Supports,
        RuleGrammar::Container => CssomRuleInterfaceName::Container,
        RuleGrammar::FontFace => CssomRuleInterfaceName::FontFace,
        RuleGrammar::FontFeatureValues => CssomRuleInterfaceName::FontFeatureValues,
        RuleGrammar::FontPaletteValues => CssomRuleInterfaceName::FontPaletteValues,
        RuleGrammar::CounterStyle => CssomRuleInterfaceName::CounterStyle,
        RuleGrammar::Keyframes => CssomRuleInterfaceName::Keyframes,
        RuleGrammar::Keyframe => CssomRuleInterfaceName::Keyframe,
        RuleGrammar::Margin => CssomRuleInterfaceName::Margin,
        RuleGrammar::Page => CssomRuleInterfaceName::Page,
        RuleGrammar::Property => CssomRuleInterfaceName::Property,
        RuleGrammar::LayerBlock => CssomRuleInterfaceName::LayerBlock,
        RuleGrammar::LayerStatement => CssomRuleInterfaceName::LayerStatement,
        RuleGrammar::Scope => CssomRuleInterfaceName::Scope,
        RuleGrammar::StartingStyle => CssomRuleInterfaceName::StartingStyle,
        RuleGrammar::PositionTry => CssomRuleInterfaceName::PositionTry,
        RuleGrammar::NestedDeclarations => CssomRuleInterfaceName::NestedDeclarations,
        RuleGrammar::ColorProfile => CssomRuleInterfaceName::ColorProfile,
        RuleGrammar::When
        | RuleGrammar::Else
        | RuleGrammar::Document
        | RuleGrammar::CustomMedia
        | RuleGrammar::Region
        | RuleGrammar::Footnote
        | RuleGrammar::Sidenote
        | RuleGrammar::BdColour
        | RuleGrammar::Unknown => CssomRuleInterfaceName::CssRule,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCssDeclarationBlock {
    serialization: String,
    declarations: Box<[stylo_cssom_model::RuleDeclaration]>,
    shorthand_values: Box<[stylo_cssom_model::RuleDeclaration]>,
}

impl CanonicalCssDeclarationBlock {
    fn from_serialization(serialization: String) -> Self {
        let declarations = declaration_slices(&serialization)
            .filter_map(|declaration| {
                let (name, value) = declaration.split_once(':')?;
                let value = value.trim();
                let important_start = value
                    .to_ascii_lowercase()
                    .rfind("!important")
                    .filter(|start| value[*start + "!important".len()..].trim().is_empty());
                Some(
                    stylo_cssom_model::RuleDeclaration::new(
                        name.trim(),
                        important_start.map_or(value, |start| value[..start].trim_end()),
                    )
                    .with_importance(important_start.is_some()),
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            serialization,
            declarations,
            shorthand_values: Box::new([]),
        }
    }

    fn rule_block(
        &self,
        domain: stylo_cssom_model::RuleDeclarationDomain,
        namespaces: &stylo_cssom_model::RuleNamespaceContext,
    ) -> stylo_cssom_model::RuleDeclarationBlock {
        stylo_cssom_model::RuleDeclarationBlock::new(
            domain,
            self.serialization.as_str(),
            self.declarations.to_vec(),
        )
        .with_namespaces(namespaces.clone())
        .with_shorthand_values(self.shorthand_values.to_vec())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ParsedCssRuleKind {
    Style(CanonicalStyleRule),
    Keyframes(CanonicalKeyframesRule),
    Keyframe(CanonicalKeyframeRule),
    Namespace(CanonicalNamespaceRule),
    FontFace(CanonicalCssDeclarationBlock),
    ConditionalGrouping {
        condition: CanonicalCssConditionalRule,
        grouping: CanonicalCssGroupingRule,
    },
    Grouping {
        header: stylo_cssom_model::RuleGroupHeader,
        grouping: CanonicalCssGroupingRule,
    },
    Container(CanonicalCssContainerRule),
    Import(CanonicalCssImportRule),
    LayerBlock(CanonicalCssLayerBlockRule),
    LayerStatement(CanonicalCssLayerStatementRule),
    Scope(CanonicalCssScopeRule),
    NestedDeclarations(CanonicalCssDeclarationBlock),
    Page(CanonicalPageRule),
    Margin(CanonicalMarginRule),
    FontFeatureValues(stylo_cssom_model::RuleFontFeatureValues),
    FontPaletteValues(CanonicalFontPaletteValuesRule),
    CounterStyle(CanonicalCounterStyleRule),
    Property(CanonicalPropertyRule),
    PositionTry(CanonicalPositionTryRule),
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalNamespaceRule {
    prefix: String,
    namespace_uri: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCounterStyleRule {
    name: String,
    system: String,
    negative: String,
    prefix: String,
    suffix: String,
    range: String,
    pad: String,
    fallback: String,
    symbols: String,
    additive_symbols: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalPositionTryRule {
    name: String,
    declarations: CanonicalCssDeclarationBlock,
}

fn canonical_position_try_declarations(
    block: &PropertyDeclarationBlock,
) -> CanonicalCssDeclarationBlock {
    let mut declarations = canonical_declaration_block(block);
    let retain_accepted = |values: Box<[stylo_cssom_model::RuleDeclaration]>| {
        values
            .into_vec()
            .into_iter()
            .filter(|declaration| {
                !declaration.important()
                    && stylo_cssom_model::PositionTryDescriptorName::parse(declaration.name())
                        .is_some()
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    };
    declarations.declarations = retain_accepted(declarations.declarations);
    declarations.shorthand_values = retain_accepted(declarations.shorthand_values);
    declarations.serialization = declarations
        .declarations
        .iter()
        .map(|declaration| format!("{}: {};", declaration.name(), declaration.value()))
        .collect::<Vec<_>>()
        .join(" ");
    declarations
}

fn optional_css<T: ToCss>(value: Option<&T>) -> String {
    value.map(ToCss::to_css_string).unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalStyleRule {
    selector_text: String,
    declarations: CanonicalCssDeclarationBlock,
    grouping: CanonicalCssGroupingRule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCssGroupingRule {
    nested_rules: Box<[ParsedCssRule]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCssContainerRule {
    conditions: CanonicalCssContainerConditions,
    condition_text: String,
    grouping: CanonicalCssGroupingRule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCssImportRule {
    request: stylo_cssom_model::RuleImportRequest,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CssomImportLayerName<'a> {
    Null,
    String(&'a str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCssLayerBlockRule {
    name: String,
    grouping: CanonicalCssGroupingRule,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCssLayerStatementRule {
    names: Box<[String]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalPropertyRule {
    name: String,
    syntax: String,
    inherits: bool,
    initial_value: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCssContainerConditions {
    first: CanonicalCssContainerCondition,
    rest: Box<[CanonicalCssContainerCondition]>,
}

impl CanonicalCssContainerConditions {
    fn iter(&self) -> impl Iterator<Item = &CanonicalCssContainerCondition> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }

    fn condition_text(&self) -> String {
        self.iter()
            .map(CanonicalCssContainerCondition::condition_text)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCssContainerCondition(CanonicalCssContainerConditionKind);

#[derive(Clone, Debug)]
struct NonEmptyCssList<T> {
    first: T,
    remaining: Box<[T]>,
}

#[derive(Clone, Debug)]
pub struct ParsedContainerQueryCondition {
    condition: Arc<ContainerConditions>,
    serialization: Box<str>,
}

impl ParsedContainerQueryCondition {
    pub fn lookup(&self) -> &ContainerCondition {
        self.condition
            .iter()
            .next()
            .expect("a parsed container condition list is non-empty")
    }

    pub fn query(&self) -> Option<&style::queries::QueryCondition> {
        self.lookup().query()
    }

    pub const fn serialization(&self) -> &str {
        &self.serialization
    }
}

#[derive(Clone, Debug)]
pub struct ParsedContainerQueryList {
    conditions: NonEmptyCssList<ParsedContainerQueryCondition>,
    serialization: Box<str>,
}

impl ParsedContainerQueryList {
    #[must_use]
    pub fn parse(source: &str) -> Option<Self> {
        Self::parse_with_base_url(source, crate::context::ABOUT_BLANK.clone().into())
    }

    #[must_use]
    pub fn parse_with_base_url(
        source: &str,
        url_data: style::stylesheets::UrlExtraData,
    ) -> Option<Self> {
        let mut input = ParserInput::new(source);
        let mut parser = Parser::new(&mut input);
        let conditions = parser
            .parse_comma_separated(|input| {
                let start = input.position();
                consume_component_values(input)?;
                let source = input.slice_from(start).trim();
                parse_typed_container_condition_with_url_data(source, url_data.clone())
                    .ok_or_else(|| input.new_custom_error::<_, ()>(()))
            })
            .ok()
            .and_then(NonEmptyCssList::from_vec)?;
        let serialization = conditions
            .iter()
            .map(ParsedContainerQueryCondition::serialization)
            .collect::<Vec<_>>()
            .join(", ")
            .into_boxed_str();
        Some(Self {
            conditions,
            serialization,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &ParsedContainerQueryCondition> {
        self.conditions.iter()
    }

    pub const fn serialization(&self) -> &str {
        &self.serialization
    }
}

impl<T> NonEmptyCssList<T> {
    fn from_vec(mut values: Vec<T>) -> Option<Self> {
        if values.is_empty() {
            return None;
        }
        let remaining = values.split_off(1).into_boxed_slice();
        Some(Self {
            first: values
                .pop()
                .expect("a checked non-empty CSS list has a first value"),
            remaining,
        })
    }

    fn iter(&self) -> impl Iterator<Item = &T> {
        std::iter::once(&self.first).chain(self.remaining.iter())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CanonicalCssContainerConditionKind {
    Name(String),
    Query(String),
    NamedQuery { name: String, query: String },
}

impl CanonicalCssContainerCondition {
    fn new(name: Option<String>, query: Option<String>) -> Option<Self> {
        Some(Self(match (name, query) {
            (Some(name), None) => CanonicalCssContainerConditionKind::Name(name),
            (None, Some(query)) => CanonicalCssContainerConditionKind::Query(query),
            (Some(name), Some(query)) => {
                CanonicalCssContainerConditionKind::NamedQuery { name, query }
            },
            (None, None) => return None,
        }))
    }

    pub fn name(&self) -> &str {
        match &self.0 {
            CanonicalCssContainerConditionKind::Name(name)
            | CanonicalCssContainerConditionKind::NamedQuery { name, .. } => name,
            CanonicalCssContainerConditionKind::Query(_) => "",
        }
    }

    pub fn query(&self) -> &str {
        match &self.0 {
            CanonicalCssContainerConditionKind::Query(query)
            | CanonicalCssContainerConditionKind::NamedQuery { query, .. } => query,
            CanonicalCssContainerConditionKind::Name(_) => "",
        }
    }

    fn condition_text(&self) -> String {
        match &self.0 {
            CanonicalCssContainerConditionKind::Name(name) => name.clone(),
            CanonicalCssContainerConditionKind::Query(query) => query.clone(),
            CanonicalCssContainerConditionKind::NamedQuery { name, query } => {
                format!("{name} {query}")
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalCssScopeRule {
    start: Option<String>,
    end: Option<String>,
    grouping: CanonicalCssGroupingRule,
}

/// The CSSOM condition exposed by a successfully parsed conditional grouping
/// rule. Private variant payloads prevent callers from fabricating an
/// unparsed condition string.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CanonicalCssConditionalRule {
    Media(CanonicalCssMediaRule),
    Supports(CanonicalCssSupportsRule),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCssMediaRule {
    condition_text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalCssSupportsRule {
    condition_text: String,
}

impl CanonicalCssConditionalRule {
    pub fn condition_text(&self) -> &str {
        match self {
            Self::Media(rule) => &rule.condition_text,
            Self::Supports(rule) => &rule.condition_text,
        }
    }
}

fn consume_component_values<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<(), cssparser::ParseError<'i, ()>> {
    while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
        if matches!(
            token,
            Token::Function(_)
                | Token::ParenthesisBlock
                | Token::SquareBracketBlock
                | Token::CurlyBracketBlock
        ) {
            input.parse_nested_block(consume_component_values)?;
        }
    }
    Ok(())
}

fn parse_typed_container_condition_with_url_data(
    condition: &str,
    url_data: style::stylesheets::UrlExtraData,
) -> Option<ParsedContainerQueryCondition> {
    let css = format!("@container {condition} {{}}");
    let (stylesheet, lock) =
        crate::context::parse_stylesheet_fragment_with_url_data(&css, Origin::Author, url_data);
    let guard = lock.read();
    let contents = stylesheet.contents.read_with(&guard);
    let rules = contents.rules.read_with(&guard);
    let [CssRule::Container(rule)] = rules.0.as_slice() else {
        return None;
    };
    let condition = rule.single_condition()?;
    let name = condition
        .name()
        .map_or_else(String::new, ToCss::to_css_string);
    let query = condition
        .query()
        .map_or_else(String::new, ToCss::to_css_string);
    let serialization = match (name.is_empty(), query.is_empty()) {
        (false, false) => format!("{name} {query}"),
        (false, true) => name.clone(),
        (true, false) => query.clone(),
        (true, true) => return None,
    };
    Some(ParsedContainerQueryCondition {
        condition: Arc::clone(&rule.conditions),
        serialization: serialization.into_boxed_str(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalPageRule {
    selector: CanonicalPageSelector,
    declarations: CanonicalCssDeclarationBlock,
    nested_rules: Box<[ParsedCssRule]>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum CanonicalPageSelector {
    Anonymous,
    Named(String),
}

impl CanonicalPageSelector {
    fn from_serialised(selector: String) -> Self {
        if selector.is_empty() {
            Self::Anonymous
        } else {
            Self::Named(selector)
        }
    }

    fn as_str(&self) -> &str {
        match self {
            Self::Anonymous => "",
            Self::Named(selector) => selector,
        }
    }

    fn serialise_rule(&self, body: &str) -> String {
        match (self, body.is_empty()) {
            (Self::Anonymous, true) => "@page { }".to_owned(),
            (Self::Anonymous, false) => format!("@page {{ {body} }}"),
            (Self::Named(selector), true) => format!("@page {selector} {{ }}"),
            (Self::Named(selector), false) => format!("@page {selector} {{ {body} }}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalMarginRule {
    name: String,
    declarations: CanonicalCssDeclarationBlock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PageOrientationDescriptor {
    Upright,
    RotateLeft,
    RotateRight,
}

impl PageOrientationDescriptor {
    fn parse(value: &str) -> Option<Self> {
        Some(match value.trim().to_ascii_lowercase().as_str() {
            "upright" => Self::Upright,
            "rotate-left" => Self::RotateLeft,
            "rotate-right" => Self::RotateRight,
            _ => return None,
        })
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Upright => "upright",
            Self::RotateLeft => "rotate-left",
            Self::RotateRight => "rotate-right",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalFontPaletteValuesRule {
    name: String,
    font_family: String,
    base_palette: String,
    override_colors: String,
}

macro_rules! cssom_legacy_rule_types {
    ($($variant:ident => ($constant:literal, $value:literal)),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum CssomLegacyRuleType {
            $($variant),+
        }

        impl CssomLegacyRuleType {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn constant_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $constant),+
                }
            }

            pub const fn value(self) -> u16 {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }
    };
}

cssom_legacy_rule_types! {
    Style => ("STYLE_RULE", 1),
    Charset => ("CHARSET_RULE", 2),
    Import => ("IMPORT_RULE", 3),
    Media => ("MEDIA_RULE", 4),
    FontFace => ("FONT_FACE_RULE", 5),
    Page => ("PAGE_RULE", 6),
    Keyframes => ("KEYFRAMES_RULE", 7),
    Keyframe => ("KEYFRAME_RULE", 8),
    Margin => ("MARGIN_RULE", 9),
    Namespace => ("NAMESPACE_RULE", 10),
    CounterStyle => ("COUNTER_STYLE_RULE", 11),
    Supports => ("SUPPORTS_RULE", 12),
    FontFeatureValues => ("FONT_FEATURE_VALUES_RULE", 14),
}

impl ParsedCssRule {
    pub fn parse_stylesheet(css: &str) -> Vec<Self> {
        Self::parse_rules(css, None)
    }

    fn parse_rules(css: &str, authored: Option<&str>) -> Vec<Self> {
        crate::context::initialise_required_servo_style_prefs();
        let (stylesheet, lock) = crate::context::parse_stylesheet_fragment(css, Origin::Author);
        let guard = lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let namespaces = model_namespaces(&contents.namespaces);
        let source = source::AuthoredSource::new(css, cssparser::UrlErrorRecovery::Css2);
        contents
            .rules(&guard)
            .iter()
            .filter_map(|rule| {
                Self::from_stylo_rule(
                    rule,
                    &guard,
                    RuleSource {
                        source: &source,
                        rule: authored,
                        end: None,
                        namespaces: &namespaces,
                    },
                )
            })
            .collect()
    }

    /// Parse exactly one rule and serialize its typed representation.
    pub fn parse(css: &str) -> Option<Self> {
        let mut rules = Self::parse_rules(css, Some(css));
        if rules.len() != 1 {
            return None;
        }
        rules.pop()
    }

    pub const fn source_location(&self) -> Option<cssparser::SourceLocation> {
        self.source_location
    }

    fn from_stylo_rule(
        rule: &CssRule,
        guard: &SharedRwLockReadGuard,
        authored: RuleSource<'_>,
    ) -> Option<Self> {
        let location = source::location(rule, guard);
        let text = authored.source.text();
        let span = authored.source.span(location);
        let projection = if matches!(rule, CssRule::NestedDeclarations(_)) {
            authored
                .source
                .position(location)
                .zip(authored.end)
                .map(|(start, end)| &text[start..end])
        } else {
            span.as_ref().map(|span| span.text(text))
        };
        let authored = RuleSource {
            rule: projection.or(authored.rule),
            end: span
                .as_ref()
                .and_then(|span| span.block_end)
                .or(authored.end),
            ..authored
        };
        let serialization = rule.to_css_string(guard);
        let declaration_block_from_serialization = || {
            let open = serialization
                .find('{')
                .expect("a declaration rule has an opening block");
            let close = serialization
                .rfind('}')
                .expect("a declaration rule has a closing block");
            CanonicalCssDeclarationBlock::from_serialization(
                serialization[open + 1..close].trim().to_owned(),
            )
        };
        let interface_name = interface_name_for_rule(rule);
        let grouping_rule = |rules: &CssRules| ParsedCssRuleKind::Grouping {
            header: source::canonical_group_header(&serialization),
            grouping: CanonicalCssGroupingRule {
                nested_rules: canonical_nested_rules(rules, guard, authored),
            },
        };
        let kind = match rule {
            CssRule::Style(rule) => {
                let rule = rule.read_with(guard);
                let grouping = CanonicalCssGroupingRule {
                    nested_rules: rule.rules.as_ref().map_or_else(Box::default, |rules| {
                        canonical_nested_rules(rules.read_with(guard), guard, authored)
                    }),
                };
                ParsedCssRuleKind::Style(CanonicalStyleRule {
                    selector_text: rule.selectors.to_css_string(),
                    declarations: canonical_declaration_block(rule.block.read_with(guard)),
                    grouping,
                })
            },
            CssRule::Namespace(rule) => {
                ParsedCssRuleKind::Namespace(canonical_namespace_rule(rule))
            },
            CssRule::Keyframes(rule) => {
                let rule = rule.read_with(guard);
                ParsedCssRuleKind::Keyframes(CanonicalKeyframesRule {
                    name: rule.name.as_atom().to_string(),
                    frames: rule
                        .keyframes
                        .iter()
                        .map(|frame| {
                            keyframes::canonical_keyframe(
                                frame.read_with(guard),
                                guard,
                                authored.namespaces,
                                authored.source,
                            )
                        })
                        .collect(),
                })
            },
            CssRule::FontFace(_) => {
                ParsedCssRuleKind::FontFace(declaration_block_from_serialization())
            },
            CssRule::Media(rule) => ParsedCssRuleKind::ConditionalGrouping {
                condition: CanonicalCssConditionalRule::Media(CanonicalCssMediaRule {
                    condition_text: rule.media_queries.read_with(guard).to_css_string(),
                }),
                grouping: CanonicalCssGroupingRule {
                    nested_rules: canonical_nested_rules(
                        rule.rules.read_with(guard),
                        guard,
                        authored,
                    ),
                },
            },
            CssRule::Supports(rule) => ParsedCssRuleKind::ConditionalGrouping {
                condition: CanonicalCssConditionalRule::Supports(CanonicalCssSupportsRule {
                    condition_text: rule.condition.to_css_string(),
                }),
                grouping: CanonicalCssGroupingRule {
                    nested_rules: canonical_nested_rules(
                        rule.rules.read_with(guard),
                        guard,
                        authored,
                    ),
                },
            },
            CssRule::When(rule) => grouping_rule(rule.rules.read_with(guard)),
            CssRule::Else(rule) => grouping_rule(rule.rules.read_with(guard)),
            CssRule::StartingStyle(rule) => grouping_rule(rule.rules.read_with(guard)),
            CssRule::Container(rule) => {
                let mut conditions = rule
                    .conditions()
                    .map(|condition| {
                        CanonicalCssContainerCondition::new(
                            condition.name().map(ToCss::to_css_string),
                            condition.query().map(ToCss::to_css_string),
                        )
                    })
                    .collect::<Option<Vec<_>>>()?
                    .into_iter();
                let first = conditions.next()?;
                let conditions = CanonicalCssContainerConditions {
                    first,
                    rest: conditions.collect(),
                };
                let condition_text = conditions.condition_text();
                ParsedCssRuleKind::Container(CanonicalCssContainerRule {
                    conditions,
                    condition_text,
                    grouping: CanonicalCssGroupingRule {
                        nested_rules: canonical_nested_rules(
                            rule.rules.read_with(guard),
                            guard,
                            authored,
                        ),
                    },
                })
            },
            CssRule::Import(rule) => canonical_import_rule(rule.read_with(guard), guard),
            CssRule::LayerBlock(rule) => canonical_layer_block_rule(rule, guard, authored),
            CssRule::LayerStatement(rule) => canonical_layer_statement_rule(rule),
            CssRule::Scope(rule) => ParsedCssRuleKind::Scope(CanonicalCssScopeRule {
                start: rule
                    .bounds
                    .start
                    .as_ref()
                    .map(CssParserToCss::to_css_string),
                end: rule.bounds.end.as_ref().map(CssParserToCss::to_css_string),
                grouping: CanonicalCssGroupingRule {
                    nested_rules: canonical_nested_rules(
                        rule.rules.read_with(guard),
                        guard,
                        authored,
                    ),
                },
            }),
            CssRule::NestedDeclarations(rule) => ParsedCssRuleKind::NestedDeclarations(
                canonical_declaration_block(rule.read_with(guard).block.read_with(guard)),
            ),
            CssRule::Page(rule) => canonical_page_rule(rule.read_with(guard), guard, authored)?,
            CssRule::Margin(rule) => ParsedCssRuleKind::Margin(canonical_margin_rule(rule, guard)),
            CssRule::FontFeatureValues(rule) => ParsedCssRuleKind::FontFeatureValues(
                font_feature_values::canonical_rule(rule, authored.source),
            ),
            CssRule::CounterStyle(rule) => {
                let rule = rule.read_with(guard);
                ParsedCssRuleKind::CounterStyle(CanonicalCounterStyleRule {
                    name: rule.name().to_css_string(),
                    system: optional_css(rule.system()),
                    negative: optional_css(rule.negative()),
                    prefix: optional_css(rule.prefix()),
                    suffix: optional_css(rule.suffix()),
                    range: optional_css(rule.range()),
                    pad: optional_css(rule.pad()),
                    fallback: optional_css(rule.fallback()),
                    symbols: optional_css(rule.symbols()),
                    additive_symbols: optional_css(rule.additive_symbols()),
                })
            },
            CssRule::FontPaletteValues(rule) => canonical_font_palette_values_rule(rule),
            CssRule::Property(rule) => ParsedCssRuleKind::Property(CanonicalPropertyRule {
                name: rule.name.to_css_string(),
                syntax: rule
                    .data
                    .syntax
                    .specified_string()
                    .unwrap_or("*")
                    .to_owned(),
                inherits: rule.inherits(),
                initial_value: rule
                    .data
                    .initial_value
                    .as_ref()
                    .map(|value| value.css_text().to_owned()),
            }),
            CssRule::PositionTry(rule) => {
                let rule = rule.read_with(guard);
                ParsedCssRuleKind::PositionTry(CanonicalPositionTryRule {
                    name: rule.name.to_css_string(),
                    declarations: canonical_position_try_declarations(rule.block.read_with(guard)),
                })
            },
            _ => ParsedCssRuleKind::Other,
        };
        if let ParsedCssRuleKind::Page(page) = kind {
            let mut parsed = Self::from_page_parts(
                page.selector,
                page.declarations,
                page.nested_rules,
                authored.namespaces.clone(),
            );
            parsed.projection = projection.map(str::to_owned);
            parsed.source_location = Some(location);
            return Some(parsed);
        }
        Some(Self {
            serialization,
            projection: projection.map(str::to_owned),
            source_location: Some(location),
            namespaces: authored.namespaces.clone(),
            kind,
            interface_name,
            grammar: stylo_rule_grammar(rule),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.serialization
    }

    pub const fn grammar(&self) -> stylo_cssom_model::RuleGrammar {
        self.grammar
    }

    #[must_use]
    pub fn to_rule_node(&self) -> stylo_cssom_model::RuleNode {
        if let ParsedCssRuleKind::FontFeatureValues(values) = &self.kind {
            return font_feature_values_node(values.clone());
        }
        let nested_rules = self.nested_rules();
        let nested = nested_rules
            .unwrap_or_default()
            .iter()
            .map(Self::to_rule_node)
            .collect::<Vec<_>>();
        let node = match self.group_header() {
            Some(header) => stylo_cssom_model::RuleNode::authored_with_group_header(
                self.grammar,
                self.serialization.as_str(),
                nested,
                header,
            ),
            None => stylo_cssom_model::RuleNode::authored(
                self.grammar,
                self.serialization.as_str(),
                nested,
            ),
        };
        let node = match self.rule_declaration_block() {
            Some(block) => node.with_declaration_block(block),
            None => node,
        };
        let node = match &self.projection {
            Some(projection) => node.with_projection_serialization(projection.as_str()),
            None => node,
        };
        match self.rule_cssom_data() {
            Some(data) => node
                .with_cssom_data(data)
                .expect("typed CSSOM member data must match its rule grammar"),
            None => node,
        }
    }

    fn rule_cssom_data(&self) -> Option<stylo_cssom_model::RuleCssomData> {
        use stylo_cssom_model::{RuleConditionKind, RuleContainerCondition, RuleCssomData};

        Some(match &self.kind {
            ParsedCssRuleKind::FontFeatureValues(values) => RuleCssomData::FontFeatureValues {
                values: values.clone(),
            },
            ParsedCssRuleKind::Keyframes(rule) => RuleCssomData::Keyframes {
                name: rule.name.as_str().into(),
            },
            ParsedCssRuleKind::Keyframe(rule) => RuleCssomData::Keyframe {
                selector: rule.selector.clone(),
            },
            ParsedCssRuleKind::Style(rule) => RuleCssomData::Style {
                selector: rule.selector_text.as_str().into(),
            },
            ParsedCssRuleKind::Namespace(rule) => RuleCssomData::Namespace {
                prefix: rule.prefix.as_str().into(),
                uri: rule.namespace_uri.as_str().into(),
            },
            ParsedCssRuleKind::Import(rule) => RuleCssomData::Import {
                request: rule.request.clone(),
            },
            ParsedCssRuleKind::ConditionalGrouping { condition, .. } => {
                let kind = match condition {
                    CanonicalCssConditionalRule::Media(_) => RuleConditionKind::Media,
                    CanonicalCssConditionalRule::Supports(_) => RuleConditionKind::Supports,
                };
                RuleCssomData::Conditional {
                    kind,
                    condition: condition.condition_text().into(),
                }
            },
            ParsedCssRuleKind::Container(rule) => RuleCssomData::Container {
                condition: rule.condition_text.as_str().into(),
                conditions: rule
                    .conditions
                    .iter()
                    .map(|condition| {
                        RuleContainerCondition::new(condition.name(), condition.query())
                    })
                    .collect::<Vec<_>>()
                    .into(),
            },
            ParsedCssRuleKind::FontPaletteValues(rule) => RuleCssomData::FontPaletteValues {
                name: rule.name.as_str().into(),
                font_family: rule.font_family.as_str().into(),
                base_palette: rule.base_palette.as_str().into(),
                override_colors: rule.override_colors.as_str().into(),
            },
            ParsedCssRuleKind::CounterStyle(rule) => RuleCssomData::CounterStyle {
                name: rule.name.as_str().into(),
                system: rule.system.as_str().into(),
                negative: rule.negative.as_str().into(),
                prefix: rule.prefix.as_str().into(),
                suffix: rule.suffix.as_str().into(),
                range: rule.range.as_str().into(),
                pad: rule.pad.as_str().into(),
                fallback: rule.fallback.as_str().into(),
                symbols: rule.symbols.as_str().into(),
                additive_symbols: rule.additive_symbols.as_str().into(),
            },
            ParsedCssRuleKind::Property(rule) => RuleCssomData::Property {
                name: rule.name.as_str().into(),
                syntax: rule.syntax.as_str().into(),
                inherits: rule.inherits,
                initial_value: rule.initial_value.as_deref().map(Into::into),
            },
            ParsedCssRuleKind::PositionTry(rule) => RuleCssomData::PositionTry {
                name: rule.name.as_str().into(),
            },
            ParsedCssRuleKind::Margin(rule) => RuleCssomData::Margin {
                name: rule.name.as_str().into(),
            },
            ParsedCssRuleKind::Page(rule) => RuleCssomData::Page {
                selector: rule.selector.as_str().into(),
            },
            ParsedCssRuleKind::LayerBlock(rule) => RuleCssomData::LayerBlock {
                name: rule.name.as_str().into(),
            },
            ParsedCssRuleKind::LayerStatement(rule) => RuleCssomData::LayerStatement {
                names: rule
                    .names
                    .iter()
                    .map(|name| name.as_str().into())
                    .collect::<Vec<_>>()
                    .into(),
            },
            ParsedCssRuleKind::Scope(rule) => RuleCssomData::Scope {
                start: rule.start.as_deref().map(Into::into),
                end: rule.end.as_deref().map(Into::into),
            },
            ParsedCssRuleKind::FontFace(_)
            | ParsedCssRuleKind::NestedDeclarations(_)
            | ParsedCssRuleKind::Grouping { .. }
            | ParsedCssRuleKind::Other => {
                return None;
            },
        })
    }

    fn rule_declaration_block(&self) -> Option<stylo_cssom_model::RuleDeclarationBlock> {
        let domain = match &self.kind {
            ParsedCssRuleKind::Keyframe(_) => stylo_cssom_model::RuleDeclarationDomain::Keyframe,
            ParsedCssRuleKind::Style(_) => stylo_cssom_model::RuleDeclarationDomain::Style,
            ParsedCssRuleKind::FontFace(_) => {
                stylo_cssom_model::RuleDeclarationDomain::FontFaceDescriptor
            },
            ParsedCssRuleKind::NestedDeclarations(_) => {
                stylo_cssom_model::RuleDeclarationDomain::Nested
            },
            ParsedCssRuleKind::Page(_) => stylo_cssom_model::RuleDeclarationDomain::Page,
            ParsedCssRuleKind::Margin(_) => stylo_cssom_model::RuleDeclarationDomain::Margin,
            ParsedCssRuleKind::PositionTry(_) => {
                stylo_cssom_model::RuleDeclarationDomain::PositionTry
            },
            ParsedCssRuleKind::Keyframes(_)
            | ParsedCssRuleKind::Namespace(_)
            | ParsedCssRuleKind::ConditionalGrouping { .. }
            | ParsedCssRuleKind::Grouping { .. }
            | ParsedCssRuleKind::Container(_)
            | ParsedCssRuleKind::Import(_)
            | ParsedCssRuleKind::LayerBlock(_)
            | ParsedCssRuleKind::LayerStatement(_)
            | ParsedCssRuleKind::Scope(_)
            | ParsedCssRuleKind::FontFeatureValues(_)
            | ParsedCssRuleKind::FontPaletteValues(_)
            | ParsedCssRuleKind::CounterStyle(_)
            | ParsedCssRuleKind::Property(_)
            | ParsedCssRuleKind::Other => return None,
        };
        Some(self.declarations()?.rule_block(domain, &self.namespaces))
    }

    pub fn declaration_value(&self, property: &str) -> Option<String> {
        match &self.kind {
            ParsedCssRuleKind::Keyframe(rule) => {
                cssom_declaration_value(&rule.declarations, property)
            },
            ParsedCssRuleKind::Style(rule) => cssom_declaration_value(&rule.declarations, property),
            ParsedCssRuleKind::FontFace(block) | ParsedCssRuleKind::NestedDeclarations(block) => {
                block.property_value(property)
            },
            ParsedCssRuleKind::Page(rule) => cssom_declaration_value(&rule.declarations, property),
            ParsedCssRuleKind::Margin(rule) => {
                cssom_declaration_value(&rule.declarations, property)
            },
            ParsedCssRuleKind::PositionTry(rule) => {
                cssom_declaration_value(&rule.declarations, property)
            },
            _ => None,
        }
    }

    #[cfg(test)]
    pub const fn interface_name(&self) -> CssomRuleInterfaceName {
        self.interface_name
    }

    fn declarations(&self) -> Option<&CanonicalCssDeclarationBlock> {
        match &self.kind {
            ParsedCssRuleKind::Keyframe(rule) => Some(&rule.declarations),
            ParsedCssRuleKind::Style(rule) => Some(&rule.declarations),
            ParsedCssRuleKind::FontFace(block) | ParsedCssRuleKind::NestedDeclarations(block) => {
                Some(block)
            },
            ParsedCssRuleKind::Page(rule) => Some(&rule.declarations),
            ParsedCssRuleKind::Margin(rule) => Some(&rule.declarations),
            ParsedCssRuleKind::PositionTry(rule) => Some(&rule.declarations),
            ParsedCssRuleKind::Keyframes(_)
            | ParsedCssRuleKind::FontFeatureValues(_)
            | ParsedCssRuleKind::Namespace(_)
            | ParsedCssRuleKind::Import(_)
            | ParsedCssRuleKind::FontPaletteValues(_)
            | ParsedCssRuleKind::CounterStyle(_)
            | ParsedCssRuleKind::Property(_)
            | ParsedCssRuleKind::ConditionalGrouping { .. }
            | ParsedCssRuleKind::Grouping { .. }
            | ParsedCssRuleKind::Container(_)
            | ParsedCssRuleKind::LayerBlock(_)
            | ParsedCssRuleKind::LayerStatement(_)
            | ParsedCssRuleKind::Scope(_)
            | ParsedCssRuleKind::Other => None,
        }
    }

    pub fn page_selector_text(&self) -> Option<&str> {
        let ParsedCssRuleKind::Page(rule) = &self.kind else {
            return None;
        };
        Some(rule.selector.as_str())
    }

    pub fn margin_rule_name(&self) -> Option<&str> {
        let ParsedCssRuleKind::Margin(rule) = &self.kind else {
            return None;
        };
        Some(&rule.name)
    }

    pub fn media_condition_text(&self) -> Option<&str> {
        let ParsedCssRuleKind::ConditionalGrouping {
            condition: CanonicalCssConditionalRule::Media(rule),
            ..
        } = &self.kind
        else {
            return None;
        };
        Some(&rule.condition_text)
    }

    pub fn nested_rules(&self) -> Option<&[Self]> {
        Some(match &self.kind {
            ParsedCssRuleKind::Keyframes(rule) => &rule.frames,
            ParsedCssRuleKind::Style(rule) => &rule.grouping.nested_rules,
            ParsedCssRuleKind::ConditionalGrouping { grouping, .. }
            | ParsedCssRuleKind::Grouping { grouping, .. } => &grouping.nested_rules,
            ParsedCssRuleKind::Container(rule) => &rule.grouping.nested_rules,
            ParsedCssRuleKind::LayerBlock(rule) => &rule.grouping.nested_rules,
            ParsedCssRuleKind::Scope(rule) => &rule.grouping.nested_rules,
            ParsedCssRuleKind::Page(rule) => &rule.nested_rules,
            _ => return None,
        })
    }

    fn group_header(&self) -> Option<stylo_cssom_model::RuleGroupHeader> {
        let header = match &self.kind {
            ParsedCssRuleKind::Grouping { header, .. } => return Some(header.clone()),
            ParsedCssRuleKind::Keyframes(rule) => {
                return Some(keyframes::keyframes_header(&rule.name));
            },
            ParsedCssRuleKind::Style(rule) => rule.selector_text.clone(),
            ParsedCssRuleKind::ConditionalGrouping { condition, .. } => match condition {
                CanonicalCssConditionalRule::Media(rule) => {
                    format!("@media {}", rule.condition_text)
                },
                CanonicalCssConditionalRule::Supports(rule) => {
                    format!("@supports {}", rule.condition_text)
                },
            },
            ParsedCssRuleKind::Container(rule) => {
                format!("@container {}", rule.condition_text)
            },
            ParsedCssRuleKind::LayerBlock(rule) => {
                if rule.name.is_empty() {
                    "@layer".to_owned()
                } else {
                    format!("@layer {}", rule.name)
                }
            },
            ParsedCssRuleKind::Scope(rule) => {
                let start = rule
                    .start
                    .as_ref()
                    .map_or_else(String::new, |value| format!(" ({value})"));
                let end = rule
                    .end
                    .as_ref()
                    .map_or_else(String::new, |value| format!(" to ({value})"));
                format!("@scope{start}{end}")
            },
            ParsedCssRuleKind::Page(rule) => match &rule.selector {
                CanonicalPageSelector::Anonymous => "@page".to_owned(),
                CanonicalPageSelector::Named(selector) => format!("@page {selector}"),
            },
            _ => return None,
        };
        Some(stylo_cssom_model::RuleGroupHeader::new(header))
    }

    pub fn parse_page_child(css: &str) -> Option<Self> {
        Self::parse_margin_rule(css, &stylo_cssom_model::RuleNamespaceContext::default())
    }

    /// Whether a scanned source rule belongs in the CSSOM rule list.
    ///
    /// The structural scanner deliberately preserves unsupported rules. A
    /// recognised rule family is different: if its typed parse fails, CSS
    /// Syntax requires that malformed rule to be discarded.
    pub fn retain_scanned_rule(css: &str) -> bool {
        Self::parse(css).is_some()
            || if css.trim_start().starts_with('@') {
                first_at_keyword(css).is_some() && !starts_with_typed_rule_at_keyword(css)
            } else {
                true
            }
    }

    fn from_margin_rule(
        rule: &MarginRule,
        guard: &SharedRwLockReadGuard,
        namespaces: &stylo_cssom_model::RuleNamespaceContext,
        source: &source::AuthoredSource<'_>,
    ) -> Self {
        Self {
            serialization: rule.to_css_string(guard),
            source_location: Some(rule.source_location),
            projection: source
                .span(rule.source_location)
                .map(|span| span.text(source.text()).to_owned()),
            namespaces: namespaces.clone(),
            kind: ParsedCssRuleKind::Margin(canonical_margin_rule(rule, guard)),
            interface_name: CssomRuleInterfaceName::Margin,
            grammar: stylo_cssom_model::RuleGrammar::Margin,
        }
    }

    fn parse_margin_rule(
        css: &str,
        namespaces: &stylo_cssom_model::RuleNamespaceContext,
    ) -> Option<Self> {
        let wrapper = Self::parse(&format!("@page {{ {css} }}"))?;
        let name = wrapper
            .nested_rules()?
            .first()?
            .margin_rule_name()?
            .to_owned();
        let open = css.find('{')?;
        let close = css.rfind('}')?;
        if close <= open {
            return None;
        }
        let declarations = canonical_cssom_declaration_block(
            css[open + 1..close].trim(),
            crate::declaration_parser::CssomDeclarationContext::Margin,
        );
        let serialization = format!("@{name} {{ {} }}", declarations.serialization);
        Some(Self {
            serialization,
            projection: Some(css.to_owned()),
            source_location: None,
            namespaces: namespaces.clone(),
            kind: ParsedCssRuleKind::Margin(CanonicalMarginRule { name, declarations }),
            interface_name: CssomRuleInterfaceName::Margin,
            grammar: stylo_cssom_model::RuleGrammar::Margin,
        })
    }

    fn from_page_parts(
        selector: CanonicalPageSelector,
        declarations: CanonicalCssDeclarationBlock,
        nested_rules: Box<[Self]>,
        namespaces: stylo_cssom_model::RuleNamespaceContext,
    ) -> Self {
        let nested = nested_rules
            .iter()
            .map(Self::as_str)
            .collect::<Vec<_>>()
            .join(" ");
        let body = [declarations.serialization.as_str(), nested.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        let serialization = selector.serialise_rule(&body);
        Self {
            serialization,
            projection: None,
            source_location: None,
            namespaces,
            kind: ParsedCssRuleKind::Page(CanonicalPageRule {
                selector,
                declarations,
                nested_rules,
            }),
            interface_name: CssomRuleInterfaceName::Page,
            grammar: stylo_cssom_model::RuleGrammar::Page,
        }
    }
}

#[cfg(test)]
impl ParsedCssRule {
    const fn legacy_type(&self) -> Option<CssomLegacyRuleType> {
        self.interface_name.legacy_type()
    }

    fn declaration_names(&self) -> Option<Vec<String>> {
        Some(
            self.declarations()?
                .declarations
                .iter()
                .map(|declaration| declaration.name().to_owned())
                .collect(),
        )
    }

    fn import_layer_name(&self) -> Option<CssomImportLayerName<'_>> {
        let ParsedCssRuleKind::Import(rule) = &self.kind else {
            return None;
        };
        Some(match rule.request.layer() {
            stylo_cssom_model::RuleImportLayer::Absent => CssomImportLayerName::Null,
            stylo_cssom_model::RuleImportLayer::Anonymous => CssomImportLayerName::String(""),
            stylo_cssom_model::RuleImportLayer::Named(name) => CssomImportLayerName::String(name),
        })
    }

    fn layer_block_name(&self) -> Option<&str> {
        let ParsedCssRuleKind::LayerBlock(rule) = &self.kind else {
            return None;
        };
        Some(&rule.name)
    }

    fn layer_statement_names(&self) -> Option<&[String]> {
        let ParsedCssRuleKind::LayerStatement(rule) = &self.kind else {
            return None;
        };
        Some(&rule.names)
    }

    fn scope_bounds(&self) -> Option<(Option<&str>, Option<&str>)> {
        let ParsedCssRuleKind::Scope(rule) = &self.kind else {
            return None;
        };
        Some((rule.start.as_deref(), rule.end.as_deref()))
    }

    fn container_condition(&self) -> Option<(&str, &str)> {
        let ParsedCssRuleKind::Container(rule) = &self.kind else {
            return None;
        };
        let Some(condition) = rule
            .conditions
            .rest
            .is_empty()
            .then_some(&rule.conditions.first)
        else {
            return Some(("", ""));
        };
        Some((condition.name(), condition.query()))
    }

    fn container_conditions(
        &self,
    ) -> Option<impl Iterator<Item = &CanonicalCssContainerCondition>> {
        let ParsedCssRuleKind::Container(rule) = &self.kind else {
            return None;
        };
        Some(rule.conditions.iter())
    }

    fn condition_text(&self) -> Option<&str> {
        match &self.kind {
            ParsedCssRuleKind::ConditionalGrouping { condition, .. } => {
                Some(condition.condition_text())
            },
            ParsedCssRuleKind::Container(rule) => Some(&rule.condition_text),
            _ => None,
        }
    }

    fn conditional_rule(&self) -> Option<&CanonicalCssConditionalRule> {
        let ParsedCssRuleKind::ConditionalGrouping { condition, .. } = &self.kind else {
            return None;
        };
        Some(condition)
    }

    fn with_page_nested_rules<'a>(&self, rules: impl IntoIterator<Item = &'a str>) -> Option<Self> {
        let ParsedCssRuleKind::Page(page) = &self.kind else {
            return None;
        };
        let nested = rules
            .into_iter()
            .map(|rule| Self::parse_margin_rule(rule, &self.namespaces))
            .collect::<Option<Vec<_>>>()?;
        Some(Self::from_page_parts(
            page.selector.clone(),
            page.declarations.clone(),
            nested.into_boxed_slice(),
            self.namespaces.clone(),
        ))
    }

    fn with_page_selector_text(&self, selector_text: &str) -> Option<Self> {
        let ParsedCssRuleKind::Page(page) = &self.kind else {
            return None;
        };
        let validated = Self::parse(&format!("@page {selector_text} {{}}"))?;
        let ParsedCssRuleKind::Page(validated_page) = validated.kind else {
            return None;
        };
        Some(Self::from_page_parts(
            validated_page.selector,
            page.declarations.clone(),
            page.nested_rules.clone(),
            self.namespaces.clone(),
        ))
    }
}

pub const fn stylo_rule_grammar(rule: &CssRule) -> stylo_cssom_model::RuleGrammar {
    use stylo_cssom_model::RuleGrammar;

    match rule {
        CssRule::Style(_) => RuleGrammar::Style,
        CssRule::Namespace(_) => RuleGrammar::Namespace,
        CssRule::Import(_) => RuleGrammar::Import,
        CssRule::Media(_) => RuleGrammar::Media,
        CssRule::CustomMedia(_) => RuleGrammar::CustomMedia,
        CssRule::Container(_) => RuleGrammar::Container,
        CssRule::FontFace(_) => RuleGrammar::FontFace,
        CssRule::FontFeatureValues(_) => RuleGrammar::FontFeatureValues,
        CssRule::FontPaletteValues(_) => RuleGrammar::FontPaletteValues,
        CssRule::CounterStyle(_) => RuleGrammar::CounterStyle,
        CssRule::Keyframes(_) => RuleGrammar::Keyframes,
        CssRule::Margin(_) => RuleGrammar::Margin,
        CssRule::Footnote(_) => RuleGrammar::Footnote,
        CssRule::Sidenote(_) => RuleGrammar::Sidenote,
        CssRule::BdColour(_) => RuleGrammar::BdColour,
        CssRule::ColorProfile(_) => RuleGrammar::ColorProfile,
        CssRule::Region(_) => RuleGrammar::Region,
        CssRule::Supports(_) => RuleGrammar::Supports,
        CssRule::When(_) => RuleGrammar::When,
        CssRule::Else(_) => RuleGrammar::Else,
        CssRule::Page(_) => RuleGrammar::Page,
        CssRule::Property(_) => RuleGrammar::Property,
        CssRule::Document(_) => RuleGrammar::Document,
        CssRule::LayerBlock(_) => RuleGrammar::LayerBlock,
        CssRule::LayerStatement(_) => RuleGrammar::LayerStatement,
        CssRule::Scope(_) => RuleGrammar::Scope,
        CssRule::StartingStyle(_) => RuleGrammar::StartingStyle,
        CssRule::PositionTry(_) => RuleGrammar::PositionTry,
        CssRule::NestedDeclarations(_) => RuleGrammar::NestedDeclarations,
    }
}

fn canonical_declaration_block(block: &PropertyDeclarationBlock) -> CanonicalCssDeclarationBlock {
    let mut serialization = String::new();
    let _ = block.to_css(&mut serialization);
    let declarations = block
        .declaration_importance_iter()
        .map(|(declaration, importance)| {
            crate::declaration_parser::rule_declaration_from_stylo(declaration, importance)
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let shorthand_values =
        crate::declaration_parser::declaration_block_shorthand_values(block).into_boxed_slice();
    CanonicalCssDeclarationBlock {
        serialization,
        declarations,
        shorthand_values,
    }
}

fn canonical_namespace_rule(rule: &style::stylesheets::NamespaceRule) -> CanonicalNamespaceRule {
    CanonicalNamespaceRule {
        prefix: rule
            .prefix
            .as_ref()
            .map_or_else(String::new, CssParserToCss::to_css_string),
        namespace_uri: rule.url.to_string(),
    }
}

fn canonical_import_rule(
    rule: &style::stylesheets::ImportRule,
    guard: &SharedRwLockReadGuard,
) -> ParsedCssRuleKind {
    use style::servo::url::{UrlCorsMode, UrlReferrerPolicy};
    use stylo_cssom_model::{
        RuleImportCorsMode, RuleImportLayer, RuleImportReferrerPolicy, RuleImportRequest,
    };

    let layer = match &rule.layer {
        style::stylesheets::import_rule::ImportLayer::None => RuleImportLayer::Absent,
        style::stylesheets::import_rule::ImportLayer::Anonymous => RuleImportLayer::Anonymous,
        style::stylesheets::import_rule::ImportLayer::Named(name) => {
            RuleImportLayer::Named(name.to_css_string().into())
        },
    };
    let modifiers = rule.url.request_modifiers();
    let cors = modifiers.cors().map(|mode| match mode {
        UrlCorsMode::Anonymous => RuleImportCorsMode::Anonymous,
        UrlCorsMode::UseCredentials => RuleImportCorsMode::UseCredentials,
    });
    let referrer_policy = modifiers.referrer_policy().map(|policy| match policy {
        UrlReferrerPolicy::NoReferrer => RuleImportReferrerPolicy::NoReferrer,
        UrlReferrerPolicy::NoReferrerWhenDowngrade => {
            RuleImportReferrerPolicy::NoReferrerWhenDowngrade
        },
        UrlReferrerPolicy::SameOrigin => RuleImportReferrerPolicy::SameOrigin,
        UrlReferrerPolicy::Origin => RuleImportReferrerPolicy::Origin,
        UrlReferrerPolicy::StrictOrigin => RuleImportReferrerPolicy::StrictOrigin,
        UrlReferrerPolicy::OriginWhenCrossOrigin => RuleImportReferrerPolicy::OriginWhenCrossOrigin,
        UrlReferrerPolicy::StrictOriginWhenCrossOrigin => {
            RuleImportReferrerPolicy::StrictOriginWhenCrossOrigin
        },
        UrlReferrerPolicy::UnsafeUrl => RuleImportReferrerPolicy::UnsafeUrl,
    });
    let supports = rule.supports.as_ref().map(|support| {
        let mut css = String::from("(");
        let _ = support.condition.to_css(&mut CssWriter::new(&mut css));
        css.push(')');
        css
    });
    let media = rule
        .stylesheet
        .media(guard)
        .and_then(|media| (!media.is_empty()).then(|| media.to_css_string()));
    let mut prelude = rule.url.to_css_string();
    if !matches!(
        rule.layer,
        style::stylesheets::import_rule::ImportLayer::None
    ) {
        prelude.push(' ');
        prelude.push_str(&rule.layer.to_css_string());
    }
    if let Some(supports) = &rule.supports {
        prelude.push_str(" supports(");
        let _ = supports.condition.to_css(&mut CssWriter::new(&mut prelude));
        prelude.push(')');
    }
    let request = RuleImportRequest::new(
        rule.url.original().unwrap_or_else(|| rule.url.as_str()),
        layer,
        stylo_cssom_model::RuleImportPrelude::new(prelude),
    )
    .with_request_modifiers(
        cors,
        modifiers.integrity().map(str::to_owned),
        referrer_policy,
    )
    .with_conditions(supports, media);
    ParsedCssRuleKind::Import(CanonicalCssImportRule { request })
}

fn canonical_layer_block_rule(
    rule: &style::stylesheets::LayerBlockRule,
    guard: &SharedRwLockReadGuard,
    authored: RuleSource<'_>,
) -> ParsedCssRuleKind {
    ParsedCssRuleKind::LayerBlock(CanonicalCssLayerBlockRule {
        name: rule
            .name
            .as_ref()
            .map_or_else(String::new, ToCss::to_css_string),
        grouping: CanonicalCssGroupingRule {
            nested_rules: canonical_nested_rules(rule.rules.read_with(guard), guard, authored),
        },
    })
}

fn canonical_layer_statement_rule(
    rule: &style::stylesheets::LayerStatementRule,
) -> ParsedCssRuleKind {
    ParsedCssRuleKind::LayerStatement(CanonicalCssLayerStatementRule {
        names: rule.names.iter().map(ToCss::to_css_string).collect(),
    })
}

fn canonical_page_rule(
    rule: &PageRule,
    guard: &SharedRwLockReadGuard,
    authored: RuleSource<'_>,
) -> Option<ParsedCssRuleKind> {
    if rule
        .selectors
        .as_slice()
        .iter()
        .any(|selector| selector.name.0.as_ref().starts_with("--"))
    {
        return None;
    }
    let nested_rules = rule
        .rules
        .read_with(guard)
        .0
        .iter()
        .filter_map(|nested| match nested {
            CssRule::Margin(rule) => Some(ParsedCssRule::from_margin_rule(
                rule,
                guard,
                authored.namespaces,
                authored.source,
            )),
            _ => None,
        })
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let declarations = authored.rule.map_or_else(
        || canonical_declaration_block(rule.block.read_with(guard)),
        |source| {
            let body =
                outer_block_contents(source).expect("a typed page rule has an opening block");
            canonical_page_declaration_block(&body)
        },
    );
    Some(ParsedCssRuleKind::Page(CanonicalPageRule {
        selector: CanonicalPageSelector::from_serialised(rule.selectors.to_css_string()),
        declarations,
        nested_rules,
    }))
}

fn canonical_font_palette_values_rule(rule: &FontPaletteValuesRule) -> ParsedCssRuleKind {
    ParsedCssRuleKind::FontPaletteValues(CanonicalFontPaletteValuesRule {
        name: rule.name.0.to_string(),
        font_family: if rule.family_names.is_empty() {
            String::new()
        } else {
            rule.family_names.to_css_string()
        },
        base_palette: rule
            .base_palette
            .as_ref()
            .map(ToCss::to_css_string)
            .unwrap_or_default(),
        override_colors: if rule.override_colors.is_empty() {
            String::new()
        } else {
            rule.override_colors.to_css_string()
        },
    })
}

fn canonical_nested_rules(
    rules: &CssRules,
    guard: &SharedRwLockReadGuard,
    authored_parent: RuleSource<'_>,
) -> Box<[ParsedCssRule]> {
    rules
        .0
        .iter()
        .enumerate()
        .filter_map(|(index, rule)| {
            let end = rules
                .0
                .get(index + 1)
                .and_then(|next| {
                    authored_parent
                        .source
                        .position(source::location(next, guard))
                })
                .or(authored_parent.end);
            ParsedCssRule::from_stylo_rule(
                rule,
                guard,
                RuleSource {
                    source: authored_parent.source,
                    rule: None,
                    end,
                    namespaces: authored_parent.namespaces,
                },
            )
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

fn outer_block_contents(css: &str) -> Option<String> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    while let Ok(token) = parser.next_including_whitespace_and_comments().cloned() {
        match token {
            Token::CurlyBracketBlock => {
                return parser
                    .parse_nested_block(|input| {
                        let start = input.position();
                        while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
                            if matches!(
                                token,
                                Token::CurlyBracketBlock
                                    | Token::ParenthesisBlock
                                    | Token::SquareBracketBlock
                                    | Token::Function(_)
                            ) {
                                let _ = input.parse_nested_block(
                                    |_: &mut Parser<'_, '_>| -> Result<(), cssparser::ParseError<'_, ()>> { Ok(()) },
                                );
                            }
                        }
                        Ok::<_, cssparser::ParseError<'_, ()>>(input.slice_from(start).to_owned())
                    })
                    .ok();
            },
            Token::ParenthesisBlock | Token::SquareBracketBlock | Token::Function(_) => {
                let _ = parser.parse_nested_block(
                    |_: &mut Parser<'_, '_>| -> Result<(), cssparser::ParseError<'_, ()>> {
                        Ok(())
                    },
                );
            },
            _ => {},
        }
    }
    None
}

fn cssom_declaration_value(block: &CanonicalCssDeclarationBlock, property: &str) -> Option<String> {
    block
        .shorthand_values
        .iter()
        .find(|declaration| declaration.matches_name(property))
        .map(|declaration| declaration.value().to_owned())
        .or_else(|| block.property_value(property))
}

pub fn rule_block_declaration_value(
    block: &stylo_cssom_model::RuleBlock,
    property: &str,
) -> Option<crate::value_serialization::ResolvedValueSerialization> {
    block
        .shorthand_values()
        .iter()
        .find(|declaration| declaration.matches_name(property))
        .map(|declaration| declaration.value().to_owned())
        .or_else(|| {
            block
                .declarations()
                .iter()
                .rev()
                .find(|declaration| declaration.matches_name(property))
                .map(|declaration| declaration.value().to_owned())
        })
        .map(crate::value_serialization::ResolvedValueSerialization::new)
}

#[must_use]
pub fn rule_block_typed_om_value(
    block: &stylo_cssom_model::RuleBlock,
    property: &str,
) -> Option<crate::value_serialization::TypedOmDeclaredValue> {
    use crate::value_serialization::TypedOmDeclaredValue;

    if block
        .declarations()
        .iter()
        .rev()
        .find(|declaration| declaration.matches_name(property))
        .is_some_and(|declaration| declaration.pending_substitution().is_some())
    {
        return Some(TypedOmDeclaredValue::PendingSubstitution);
    }
    rule_block_declaration_value(block, property).map(TypedOmDeclaredValue::Serialized)
}

pub fn rule_block_declaration_is_important(
    block: &stylo_cssom_model::RuleBlock,
    property: &str,
) -> bool {
    let declarations = block.declarations();
    let important = |property: &str| {
        declarations
            .iter()
            .rev()
            .find(|declaration| declaration.matches_name(property))
            .is_some_and(stylo_cssom_model::RuleDeclaration::important)
    };
    crate::declaration_parser::inline_style_cssom_property_schema(property)
        .filter(|schema| schema.kind == stylo_cssom_model::PropertyKind::Shorthand)
        .map_or_else(
            || important(property),
            |schema| {
                schema
                    .shorthand_expansion
                    .iter()
                    .all(|name| important(name))
            },
        )
}

pub const fn rule_node_exposes_style(node: &stylo_cssom_model::RuleNode) -> bool {
    matches!(
        node.grammar(),
        stylo_cssom_model::RuleGrammar::Style
            | stylo_cssom_model::RuleGrammar::Keyframe
            | stylo_cssom_model::RuleGrammar::FontFace
            | stylo_cssom_model::RuleGrammar::NestedDeclarations
            | stylo_cssom_model::RuleGrammar::Page
            | stylo_cssom_model::RuleGrammar::Margin
            | stylo_cssom_model::RuleGrammar::PositionTry
    )
}

pub fn replace_rule_selector(
    node: &stylo_cssom_model::RuleNode,
    selector_text: &str,
    namespaces: &stylo_cssom_model::RuleNamespaceContext,
) -> Option<stylo_cssom_model::RuleNode> {
    use stylo_cssom_model::RuleCssomData;

    let selector = match node.cssom_data()? {
        RuleCssomData::Style { selector: current } => {
            let selector = crate::ValidatedSelectorText::parse(selector_text, namespaces)?
                .as_str()
                .to_owned();
            if current.starts_with('&') && !selector.contains('&') {
                format!("& {selector}")
            } else {
                selector
            }
        },
        RuleCssomData::Page { .. } => {
            let validated = ParsedCssRule::parse(&format!("@page {selector_text} {{}}"))?;
            validated.page_selector_text()?.to_owned()
        },
        RuleCssomData::Keyframes { .. }
        | RuleCssomData::FontFeatureValues { .. }
        | RuleCssomData::Keyframe { .. }
        | RuleCssomData::Namespace { .. }
        | RuleCssomData::Import { .. }
        | RuleCssomData::Conditional { .. }
        | RuleCssomData::Container { .. }
        | RuleCssomData::FontPaletteValues { .. }
        | RuleCssomData::CounterStyle { .. }
        | RuleCssomData::Property { .. }
        | RuleCssomData::PositionTry { .. }
        | RuleCssomData::Margin { .. }
        | RuleCssomData::LayerBlock { .. }
        | RuleCssomData::LayerStatement { .. }
        | RuleCssomData::Scope { .. } => return None,
    };
    node.clone().with_cssom_selector(selector)
}

pub fn mutate_non_style_rule_declaration(
    node: &stylo_cssom_model::RuleNode,
    property: &str,
    value: &str,
    priority: crate::declaration_parser::CssomDeclarationPriority,
) -> Option<stylo_cssom_model::RuleNode> {
    use stylo_cssom_model::{RuleDeclaration, RuleDeclarationBlock, RuleDeclarationDomain};

    let block = node.payload().declaration_block()?;
    if block.domain() == RuleDeclarationDomain::Keyframe {
        return keyframes::mutate_keyframe_declaration(node, property, value, priority);
    }
    let updated = match block.domain() {
        RuleDeclarationDomain::Page | RuleDeclarationDomain::Margin => {
            let context = match block.domain() {
                RuleDeclarationDomain::Page => {
                    crate::declaration_parser::CssomDeclarationContext::Page
                },
                RuleDeclarationDomain::Margin => {
                    crate::declaration_parser::CssomDeclarationContext::Margin
                },
                RuleDeclarationDomain::Style
                | RuleDeclarationDomain::FontFaceDescriptor
                | RuleDeclarationDomain::Keyframe
                | RuleDeclarationDomain::PositionTry
                | RuleDeclarationDomain::Nested => unreachable!(),
            };
            crate::declaration_parser::mutate_rule_declaration_block(
                block, property, value, priority, context,
            )?
        },
        RuleDeclarationDomain::FontFaceDescriptor => {
            let property = property.trim().to_ascii_lowercase();
            if property.is_empty() {
                return None;
            }
            let mut declarations = block
                .declarations()
                .iter()
                .filter(|declaration| !declaration.name().eq_ignore_ascii_case(&property))
                .cloned()
                .collect::<Vec<_>>();
            if !value.is_empty() {
                if !is_single_css_value(value) {
                    return None;
                }
                let probe =
                    ParsedCssRule::parse(&format!("@font-face {{ {property}: {value}; }}"))?;
                let canonical_value = probe.declaration_value(&property)?;
                declarations.push(RuleDeclaration::new(property, canonical_value));
            }
            RuleDeclarationBlock::from_declarations(block.domain(), declarations)
        },
        RuleDeclarationDomain::PositionTry => {
            if priority == crate::declaration_parser::CssomDeclarationPriority::Important
                || stylo_cssom_model::PositionTryDescriptorName::parse(property.trim()).is_none()
            {
                return None;
            }
            crate::declaration_parser::mutate_rule_declaration_block(
                block,
                property,
                value,
                priority,
                crate::declaration_parser::CssomDeclarationContext::Style,
            )?
        },
        RuleDeclarationDomain::Nested => {
            return crate::declaration_parser::mutate_style_rule_declaration(
                node,
                crate::declaration_parser::DeclarationPropertyInput::new(property, value),
                priority,
            );
        },
        RuleDeclarationDomain::Style | RuleDeclarationDomain::Keyframe => return None,
    };
    Some(node.clone().with_cssom_declaration_block(updated))
}

pub fn replace_position_try_rule_declarations(
    node: &stylo_cssom_model::RuleNode,
    declarations: &str,
) -> Option<stylo_cssom_model::RuleNode> {
    if !matches!(
        node.cssom_data(),
        Some(stylo_cssom_model::RuleCssomData::PositionTry { .. })
    ) {
        return None;
    }
    let parsed = ParsedCssRule::parse(&format!("@position-try --cssom {{ {declarations} }}"))?;
    let block = parsed.to_rule_node().payload().declaration_block()?.clone();
    Some(node.clone().with_cssom_declaration_block(block))
}

pub fn replace_page_or_margin_rule_declarations(
    node: &stylo_cssom_model::RuleNode,
    declarations: &str,
) -> Option<stylo_cssom_model::RuleNode> {
    use stylo_cssom_model::RuleDeclarationDomain;

    let parsed = match node.payload().declaration_block()?.domain() {
        RuleDeclarationDomain::Page => {
            ParsedCssRule::parse(&format!("@page {{ {declarations} }}"))?
        },
        RuleDeclarationDomain::Margin => {
            ParsedCssRule::parse_page_child(&format!("@top-left {{ {declarations} }}"))?
        },
        RuleDeclarationDomain::Style
        | RuleDeclarationDomain::FontFaceDescriptor
        | RuleDeclarationDomain::Keyframe
        | RuleDeclarationDomain::PositionTry
        | RuleDeclarationDomain::Nested => return None,
    };
    let block = parsed.to_rule_node().payload().declaration_block()?.clone();
    Some(node.clone().with_cssom_declaration_block(block))
}

fn interface_name_for_rule(rule: &CssRule) -> CssomRuleInterfaceName {
    match rule {
        CssRule::Style(_) => CssomRuleInterfaceName::Style,
        CssRule::Namespace(_) => CssomRuleInterfaceName::Namespace,
        CssRule::Import(_) => CssomRuleInterfaceName::Import,
        CssRule::Media(_) => CssomRuleInterfaceName::Media,
        CssRule::Container(_) => CssomRuleInterfaceName::Container,
        CssRule::FontFace(_) => CssomRuleInterfaceName::FontFace,
        CssRule::FontFeatureValues(_) => CssomRuleInterfaceName::FontFeatureValues,
        CssRule::FontPaletteValues(_) => CssomRuleInterfaceName::FontPaletteValues,
        CssRule::CounterStyle(_) => CssomRuleInterfaceName::CounterStyle,
        CssRule::Keyframes(_) => CssomRuleInterfaceName::Keyframes,
        CssRule::Margin(_) => CssomRuleInterfaceName::Margin,
        CssRule::Supports(_) => CssomRuleInterfaceName::Supports,
        CssRule::Page(_) => CssomRuleInterfaceName::Page,
        CssRule::Property(_) => CssomRuleInterfaceName::Property,
        CssRule::LayerBlock(_) => CssomRuleInterfaceName::LayerBlock,
        CssRule::LayerStatement(_) => CssomRuleInterfaceName::LayerStatement,
        CssRule::Scope(_) => CssomRuleInterfaceName::Scope,
        CssRule::StartingStyle(_) => CssomRuleInterfaceName::StartingStyle,
        CssRule::PositionTry(_) => CssomRuleInterfaceName::PositionTry,
        CssRule::NestedDeclarations(_) => CssomRuleInterfaceName::NestedDeclarations,
        CssRule::ColorProfile(_) => CssomRuleInterfaceName::ColorProfile,
        _ => CssomRuleInterfaceName::CssRule,
    }
}

fn canonical_cssom_declaration_block(
    css: &str,
    context: crate::declaration_parser::CssomDeclarationContext,
) -> CanonicalCssDeclarationBlock {
    let block = crate::declaration_parser::parse_cssom_declaration_block(css, context);
    canonical_cssom_declaration_block_from_typed(&block)
}

fn canonical_cssom_declaration_block_from_typed(
    block: &crate::declaration_parser::CssomDeclarationBlock,
) -> CanonicalCssDeclarationBlock {
    canonical_declaration_block(block.as_typed())
}

fn canonical_page_declaration_block(authored_body: &str) -> CanonicalCssDeclarationBlock {
    let orientation = declaration_slices(authored_body).fold(None, |orientation, declaration| {
        let Some((name, value)) = declaration.split_once(':') else {
            return orientation;
        };
        if !name.trim().eq_ignore_ascii_case("page-orientation") {
            return orientation;
        }
        PageOrientationDescriptor::parse(value).or(orientation)
    });
    let context = crate::declaration_parser::CssomDeclarationContext::Page;
    let mut block = crate::declaration_parser::parse_cssom_declaration_block("", context);
    for declaration in declaration_slices(authored_body) {
        if !declaration.contains(':') {
            continue;
        }
        let page = crate::declaration_parser::parse_cssom_declaration_block(declaration, context);
        crate::declaration_parser::cssom_declaration_merge(&mut block, &page);
        let style = crate::declaration_parser::parse_cssom_declaration_block(
            declaration,
            crate::declaration_parser::CssomDeclarationContext::Margin,
        );
        crate::declaration_parser::cssom_declaration_merge(&mut block, &style);
    }
    let _ = crate::declaration_parser::cssom_declaration_remove_property(
        &mut block,
        "page-orientation",
    );
    if let Some(orientation) = orientation {
        let _ = crate::declaration_parser::cssom_declaration_set_property(
            &mut block,
            "page-orientation",
            orientation.as_str(),
            crate::declaration_parser::CssomDeclarationPriority::Normal,
            context,
        );
    }
    canonical_cssom_declaration_block_from_typed(&block)
}

fn canonical_margin_rule(rule: &MarginRule, guard: &SharedRwLockReadGuard) -> CanonicalMarginRule {
    CanonicalMarginRule {
        name: rule.name().to_owned(),
        declarations: canonical_declaration_block(rule.block.read_with(guard)),
    }
}

fn starts_with_typed_rule_at_keyword(css: &str) -> bool {
    first_at_keyword(css).is_some_and(|name| {
        [
            "charset",
            "import",
            "namespace",
            "media",
            "supports",
            "container",
            "font-face",
            "font-feature-values",
            "font-palette-values",
            "counter-style",
            "keyframes",
            "-webkit-keyframes",
            "page",
            "property",
            "layer",
            "scope",
            "starting-style",
            "position-try",
            "color-profile",
            "when",
            "else",
            "document",
            "-moz-document",
            "custom-media",
            "region",
            "footnote",
            "sidenote",
            "bd-colour",
            "location",
        ]
        .iter()
        .any(|expected| name.eq_ignore_ascii_case(expected))
    })
}

fn first_at_keyword(css: &str) -> Option<String> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    loop {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::WhiteSpace(_) | Token::Comment(_)) => {},
            Ok(Token::AtKeyword(name)) => return Some(name.to_string()),
            Ok(_) | Err(_) => return None,
        }
    }
}

impl CanonicalCssDeclarationBlock {
    fn property_value(&self, property: &str) -> Option<String> {
        self.declarations
            .iter()
            .filter(|declaration| declaration.matches_name(property))
            .map(|declaration| declaration.value().to_owned())
            .next_back()
    }
}

fn declaration_slices(declarations: &str) -> impl Iterator<Item = &str> {
    let bytes = declarations.as_bytes();
    let mut boundaries = Vec::new();
    let mut declaration_start = 0;
    let mut quote = None;
    let mut escaped = false;
    let mut parentheses = 0_u32;
    let mut brackets = 0_u32;
    let mut braces = 0_u32;

    for (index, byte) in bytes.iter().copied().enumerate() {
        if let Some(delimiter) = quote {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                _ if byte == delimiter => quote = None,
                _ => {},
            }
            continue;
        }
        match byte {
            b'\'' | b'"' => quote = Some(byte),
            b'(' => parentheses += 1,
            b')' => parentheses = parentheses.saturating_sub(1),
            b'[' => brackets += 1,
            b']' => brackets = brackets.saturating_sub(1),
            b'{' => braces += 1,
            b'}' => {
                braces = braces.saturating_sub(1);
                if braces == 0 {
                    declaration_start = index + 1;
                }
            },
            b';' if parentheses == 0 && brackets == 0 && braces == 0 => {
                boundaries.push((declaration_start, index));
                declaration_start = index + 1;
            },
            _ => {},
        }
    }
    boundaries.push((declaration_start, declarations.len()));
    boundaries
        .into_iter()
        .map(move |(start, end)| &declarations[start..end])
}

fn is_single_css_value(value: &str) -> bool {
    let mut input = ParserInput::new(value);
    let mut parser = Parser::new(&mut input);
    while !parser.is_exhausted() {
        match parser.next_including_whitespace_and_comments() {
            Ok(Token::Semicolon) | Err(_) => return false,
            Ok(_) => {},
        }
    }
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn css_keyframes_parser_preserves_child_rules_and_declarations() {
        let parsed = super::ParsedCssRule::parse(
            "@keyframes fade { from, to { opacity: 0; } 50% { opacity: 1; } }",
        )
        .expect("keyframes parse");
        let frames = parsed
            .nested_rules()
            .expect("keyframes expose their child rules");
        assert_eq!(frames.len(), 2);
        assert_eq!(
            frames[0].grammar(),
            stylo_cssom_model::RuleGrammar::Keyframe
        );
        assert_eq!(frames[0].as_str(), "0%, 100% { opacity: 0; }");
        assert_eq!(frames[1].declaration_value("opacity"), Some("1".to_owned()));
        let graph = parsed.to_rule_node();
        assert!(graph.accepts_nested_rules());
        assert_eq!(graph.payload().nested().len(), 2);
    }

    use super::{
        CssomImportLayerName, CssomLegacyRuleType, CssomRuleInterfaceName,
        CssomRuleInterfaceParent, ParsedCssRule,
    };
    use style::{
        properties::PropertyDeclaration, shared_lock::ToCssWithGuard, stylesheets::CssRule,
    };
    use style_traits::ToCss;

    #[allow(clippy::too_many_lines)]
    fn reconstruct_pinned_declaration(declaration: &PropertyDeclaration) -> PropertyDeclaration {
        match declaration {
            PropertyDeclaration::AlignItems(value) => PropertyDeclaration::AlignItems(*value),
            PropertyDeclaration::AlignmentBaseline(value) => {
                PropertyDeclaration::AlignmentBaseline(*value)
            },
            PropertyDeclaration::Appearance(value) => PropertyDeclaration::Appearance(*value),
            PropertyDeclaration::BackfaceVisibility(value) => {
                PropertyDeclaration::BackfaceVisibility(*value)
            },
            PropertyDeclaration::BaselineSource(value) => {
                PropertyDeclaration::BaselineSource(*value)
            },
            PropertyDeclaration::BdBarcodeCheckdigitMode(value) => {
                PropertyDeclaration::BdBarcodeCheckdigitMode(*value)
            },
            PropertyDeclaration::BdBarcodeCompositeType(value) => {
                PropertyDeclaration::BdBarcodeCompositeType(*value)
            },
            PropertyDeclaration::BdBarcodeEccLevel(value) => {
                PropertyDeclaration::BdBarcodeEccLevel(*value)
            },
            PropertyDeclaration::BdBarcodeEncoding(value) => {
                PropertyDeclaration::BdBarcodeEncoding(*value)
            },
            PropertyDeclaration::BdBarcodeHumanReadablePosition(value) => {
                PropertyDeclaration::BdBarcodeHumanReadablePosition(*value)
            },
            PropertyDeclaration::BdBarcodeReaderInitialization(value) => {
                PropertyDeclaration::BdBarcodeReaderInitialization(*value)
            },
            PropertyDeclaration::BdBarcodeType(value) => PropertyDeclaration::BdBarcodeType(*value),
            PropertyDeclaration::BdChangeBarAlign(value) => {
                PropertyDeclaration::BdChangeBarAlign(*value)
            },
            PropertyDeclaration::BdChangeBarExclusion(value) => {
                PropertyDeclaration::BdChangeBarExclusion(*value)
            },
            PropertyDeclaration::BdFloatDeferColumn(value) => {
                PropertyDeclaration::BdFloatDeferColumn(*value)
            },
            PropertyDeclaration::BdFloatDeferPage(value) => {
                PropertyDeclaration::BdFloatDeferPage(*value)
            },
            PropertyDeclaration::BdFloatDisplace(value) => {
                PropertyDeclaration::BdFloatDisplace(*value)
            },
            PropertyDeclaration::BdFloatModifier(value) => {
                PropertyDeclaration::BdFloatModifier(*value)
            },
            PropertyDeclaration::BdFloatPolicy(value) => PropertyDeclaration::BdFloatPolicy(*value),
            PropertyDeclaration::BdFloatTail(value) => PropertyDeclaration::BdFloatTail(*value),
            PropertyDeclaration::BdIndexGrouping(value) => {
                PropertyDeclaration::BdIndexGrouping(*value)
            },
            PropertyDeclaration::BdPagePrintMarkSet(value) => {
                PropertyDeclaration::BdPagePrintMarkSet(*value)
            },
            PropertyDeclaration::BdPdfAnnotationHidden(value) => {
                PropertyDeclaration::BdPdfAnnotationHidden(*value)
            },
            PropertyDeclaration::BdPdfFormFieldFlags(value) => {
                PropertyDeclaration::BdPdfFormFieldFlags(*value)
            },
            PropertyDeclaration::BdPdfFormFieldMaxlength(value) => {
                PropertyDeclaration::BdPdfFormFieldMaxlength(*value)
            },
            PropertyDeclaration::BdPdfFormFieldMkRotation(value) => {
                PropertyDeclaration::BdPdfFormFieldMkRotation(*value)
            },
            PropertyDeclaration::BdPdfFormFieldMkTextPosition(value) => {
                PropertyDeclaration::BdPdfFormFieldMkTextPosition(*value)
            },
            PropertyDeclaration::BdPdfLayerVisible(value) => {
                PropertyDeclaration::BdPdfLayerVisible(*value)
            },
            PropertyDeclaration::BdPdfLinkArea(value) => PropertyDeclaration::BdPdfLinkArea(*value),
            PropertyDeclaration::BdPdfLinkBorderStyle(value) => {
                PropertyDeclaration::BdPdfLinkBorderStyle(*value)
            },
            PropertyDeclaration::BdPdfMarkColourBarPosition(value) => {
                PropertyDeclaration::BdPdfMarkColourBarPosition(*value)
            },
            PropertyDeclaration::BdPdfMarkRegistrationPosition(value) => {
                PropertyDeclaration::BdPdfMarkRegistrationPosition(*value)
            },
            PropertyDeclaration::BdPdfMultimedia(value) => {
                PropertyDeclaration::BdPdfMultimedia(*value)
            },
            PropertyDeclaration::BdPdfMultimediaFormat(value) => {
                PropertyDeclaration::BdPdfMultimediaFormat(*value)
            },
            PropertyDeclaration::BdPdfSignatureFieldLock(value) => {
                PropertyDeclaration::BdPdfSignatureFieldLock(*value)
            },
            PropertyDeclaration::BdPdfTagged(value) => PropertyDeclaration::BdPdfTagged(*value),
            PropertyDeclaration::BdPdfTrapped(value) => PropertyDeclaration::BdPdfTrapped(*value),
            PropertyDeclaration::BdRunningCopy(value) => PropertyDeclaration::BdRunningCopy(*value),
            PropertyDeclaration::BdTextEmphasisSkip(value) => {
                PropertyDeclaration::BdTextEmphasisSkip(*value)
            },
            PropertyDeclaration::BdTextUnderlinePosition(value) => {
                PropertyDeclaration::BdTextUnderlinePosition(*value)
            },
            PropertyDeclaration::BlockStepAlign(value) => {
                PropertyDeclaration::BlockStepAlign(*value)
            },
            PropertyDeclaration::BlockStepInsert(value) => {
                PropertyDeclaration::BlockStepInsert(*value)
            },
            PropertyDeclaration::BlockStepRound(value) => {
                PropertyDeclaration::BlockStepRound(*value)
            },
            PropertyDeclaration::BookmarkLevel(value) => PropertyDeclaration::BookmarkLevel(*value),
            PropertyDeclaration::BookmarkState(value) => PropertyDeclaration::BookmarkState(*value),
            PropertyDeclaration::BorderCollapse(value) => {
                PropertyDeclaration::BorderCollapse(*value)
            },
            PropertyDeclaration::BoxDecorationBreak(value) => {
                PropertyDeclaration::BoxDecorationBreak(*value)
            },
            PropertyDeclaration::BoxSizing(value) => PropertyDeclaration::BoxSizing(*value),
            PropertyDeclaration::BoxSnap(value) => PropertyDeclaration::BoxSnap(*value),
            PropertyDeclaration::BreakInside(value) => PropertyDeclaration::BreakInside(*value),
            PropertyDeclaration::CaptionSide(value) => PropertyDeclaration::CaptionSide(*value),
            PropertyDeclaration::Clear(value) => PropertyDeclaration::Clear(*value),
            PropertyDeclaration::ColorInterpolation(value) => {
                PropertyDeclaration::ColorInterpolation(*value)
            },
            PropertyDeclaration::ColorInterpolationFilters(value) => {
                PropertyDeclaration::ColorInterpolationFilters(*value)
            },
            PropertyDeclaration::ColumnFill(value) => PropertyDeclaration::ColumnFill(*value),
            PropertyDeclaration::ColumnSpan(value) => PropertyDeclaration::ColumnSpan(*value),
            PropertyDeclaration::ColumnWrap(value) => PropertyDeclaration::ColumnWrap(*value),
            PropertyDeclaration::Contain(value) => PropertyDeclaration::Contain(*value),
            PropertyDeclaration::ContainerType(value) => PropertyDeclaration::ContainerType(*value),
            PropertyDeclaration::ContentVisibility(value) => {
                PropertyDeclaration::ContentVisibility(*value)
            },
            PropertyDeclaration::Continue(value) => PropertyDeclaration::Continue(*value),
            PropertyDeclaration::Direction(value) => PropertyDeclaration::Direction(*value),
            PropertyDeclaration::Display(value) => PropertyDeclaration::Display(*value),
            PropertyDeclaration::DominantBaseline(value) => {
                PropertyDeclaration::DominantBaseline(*value)
            },
            PropertyDeclaration::DynamicRangeLimit(value) => {
                PropertyDeclaration::DynamicRangeLimit(*value)
            },
            PropertyDeclaration::EmptyCells(value) => PropertyDeclaration::EmptyCells(*value),
            PropertyDeclaration::FlexDirection(value) => PropertyDeclaration::FlexDirection(*value),
            PropertyDeclaration::FlexWrap(value) => PropertyDeclaration::FlexWrap(*value),
            PropertyDeclaration::FloatDefer(value) => PropertyDeclaration::FloatDefer(*value),
            PropertyDeclaration::FloatReference(value) => {
                PropertyDeclaration::FloatReference(*value)
            },
            PropertyDeclaration::FontKerning(value) => PropertyDeclaration::FontKerning(*value),
            PropertyDeclaration::FontLanguageOverride(value) => {
                PropertyDeclaration::FontLanguageOverride(*value)
            },
            PropertyDeclaration::FontOpticalSizing(value) => {
                PropertyDeclaration::FontOpticalSizing(*value)
            },
            PropertyDeclaration::FontStyle(value) => PropertyDeclaration::FontStyle(*value),
            PropertyDeclaration::FontSynthesisStyle(value) => {
                PropertyDeclaration::FontSynthesisStyle(*value)
            },
            PropertyDeclaration::FontVariantCaps(value) => {
                PropertyDeclaration::FontVariantCaps(*value)
            },
            PropertyDeclaration::FontVariantEastAsian(value) => {
                PropertyDeclaration::FontVariantEastAsian(*value)
            },
            PropertyDeclaration::FontVariantEmoji(value) => {
                PropertyDeclaration::FontVariantEmoji(*value)
            },
            PropertyDeclaration::FontVariantLigatures(value) => {
                PropertyDeclaration::FontVariantLigatures(*value)
            },
            PropertyDeclaration::FontVariantNumeric(value) => {
                PropertyDeclaration::FontVariantNumeric(*value)
            },
            PropertyDeclaration::FontVariantPosition(value) => {
                PropertyDeclaration::FontVariantPosition(*value)
            },
            PropertyDeclaration::FootnoteDisplay(value) => {
                PropertyDeclaration::FootnoteDisplay(*value)
            },
            PropertyDeclaration::FootnotePolicy(value) => {
                PropertyDeclaration::FootnotePolicy(*value)
            },
            PropertyDeclaration::GridAutoFlow(value) => PropertyDeclaration::GridAutoFlow(*value),
            PropertyDeclaration::GridLanesDirection(value) => {
                PropertyDeclaration::GridLanesDirection(*value)
            },
            PropertyDeclaration::HangingPunctuation(value) => {
                PropertyDeclaration::HangingPunctuation(*value)
            },
            PropertyDeclaration::Hyphens(value) => PropertyDeclaration::Hyphens(*value),
            PropertyDeclaration::ImageOrientation(value) => {
                PropertyDeclaration::ImageOrientation(*value)
            },
            PropertyDeclaration::ImageRendering(value) => {
                PropertyDeclaration::ImageRendering(*value)
            },
            PropertyDeclaration::InitialLetterAlign(value) => {
                PropertyDeclaration::InitialLetterAlign(*value)
            },
            PropertyDeclaration::InitialLetterWrap(value) => {
                PropertyDeclaration::InitialLetterWrap(*value)
            },
            PropertyDeclaration::Isolation(value) => PropertyDeclaration::Isolation(*value),
            PropertyDeclaration::JustifyItems(value) => PropertyDeclaration::JustifyItems(*value),
            PropertyDeclaration::LeadingTrim(value) => PropertyDeclaration::LeadingTrim(*value),
            PropertyDeclaration::LineBreak(value) => PropertyDeclaration::LineBreak(*value),
            PropertyDeclaration::LineGrid(value) => PropertyDeclaration::LineGrid(*value),
            PropertyDeclaration::ListStylePosition(value) => {
                PropertyDeclaration::ListStylePosition(*value)
            },
            PropertyDeclaration::MarginBreak(value) => PropertyDeclaration::MarginBreak(*value),
            PropertyDeclaration::MarginTrim(value) => PropertyDeclaration::MarginTrim(*value),
            PropertyDeclaration::MarkerSide(value) => PropertyDeclaration::MarkerSide(*value),
            PropertyDeclaration::MaskType(value) => PropertyDeclaration::MaskType(*value),
            PropertyDeclaration::MasonryAutoFlow(value) => {
                PropertyDeclaration::MasonryAutoFlow(*value)
            },
            PropertyDeclaration::MinIntrinsicSizing(value) => {
                PropertyDeclaration::MinIntrinsicSizing(*value)
            },
            PropertyDeclaration::MixBlendMode(value) => PropertyDeclaration::MixBlendMode(*value),
            PropertyDeclaration::ObjectFit(value) => PropertyDeclaration::ObjectFit(*value),
            PropertyDeclaration::OffsetRotate(value) => PropertyDeclaration::OffsetRotate(*value),
            PropertyDeclaration::OutlineStyle(value) => PropertyDeclaration::OutlineStyle(*value),
            PropertyDeclaration::OverflowAnchor(value) => {
                PropertyDeclaration::OverflowAnchor(*value)
            },
            PropertyDeclaration::OverflowWrap(value) => PropertyDeclaration::OverflowWrap(*value),
            PropertyDeclaration::PageOrientation(value) => {
                PropertyDeclaration::PageOrientation(*value)
            },
            PropertyDeclaration::PaintOrder(value) => PropertyDeclaration::PaintOrder(*value),
            PropertyDeclaration::PointerEvents(value) => PropertyDeclaration::PointerEvents(*value),
            PropertyDeclaration::PositionArea(value) => PropertyDeclaration::PositionArea(*value),
            PropertyDeclaration::PositionTryOrder(value) => {
                PropertyDeclaration::PositionTryOrder(*value)
            },
            PropertyDeclaration::PositionVisibility(value) => {
                PropertyDeclaration::PositionVisibility(*value)
            },
            PropertyDeclaration::PrintColorAdjust(value) => {
                PropertyDeclaration::PrintColorAdjust(*value)
            },
            PropertyDeclaration::ReadingFlow(value) => PropertyDeclaration::ReadingFlow(*value),
            PropertyDeclaration::RegionFragment(value) => {
                PropertyDeclaration::RegionFragment(*value)
            },
            PropertyDeclaration::Resize(value) => PropertyDeclaration::Resize(*value),
            PropertyDeclaration::RubyAlign(value) => PropertyDeclaration::RubyAlign(*value),
            PropertyDeclaration::RubyMerge(value) => PropertyDeclaration::RubyMerge(*value),
            PropertyDeclaration::RubyOverhang(value) => PropertyDeclaration::RubyOverhang(*value),
            PropertyDeclaration::RubyPosition(value) => PropertyDeclaration::RubyPosition(*value),
            PropertyDeclaration::RuleOverlap(value) => PropertyDeclaration::RuleOverlap(*value),
            PropertyDeclaration::ScrollBehavior(value) => {
                PropertyDeclaration::ScrollBehavior(*value)
            },
            PropertyDeclaration::ScrollSnapAlign(value) => {
                PropertyDeclaration::ScrollSnapAlign(*value)
            },
            PropertyDeclaration::ScrollSnapStop(value) => {
                PropertyDeclaration::ScrollSnapStop(*value)
            },
            PropertyDeclaration::ScrollSnapType(value) => {
                PropertyDeclaration::ScrollSnapType(*value)
            },
            PropertyDeclaration::ScrollbarGutter(value) => {
                PropertyDeclaration::ScrollbarGutter(*value)
            },
            PropertyDeclaration::ScrollbarWidth(value) => {
                PropertyDeclaration::ScrollbarWidth(*value)
            },
            PropertyDeclaration::ServoOverflowClipBox(value) => {
                PropertyDeclaration::ServoOverflowClipBox(*value)
            },
            PropertyDeclaration::ServoTopLayer(value) => PropertyDeclaration::ServoTopLayer(*value),
            PropertyDeclaration::ShapeRendering(value) => {
                PropertyDeclaration::ShapeRendering(*value)
            },
            PropertyDeclaration::Speak(value) => PropertyDeclaration::Speak(*value),
            PropertyDeclaration::StrokeLinecap(value) => PropertyDeclaration::StrokeLinecap(*value),
            PropertyDeclaration::StrokeLinejoin(value) => {
                PropertyDeclaration::StrokeLinejoin(*value)
            },
            PropertyDeclaration::TableLayout(value) => PropertyDeclaration::TableLayout(*value),
            PropertyDeclaration::TextAlignAll(value) => PropertyDeclaration::TextAlignAll(*value),
            PropertyDeclaration::TextAlignLast(value) => PropertyDeclaration::TextAlignLast(*value),
            PropertyDeclaration::TextAnchor(value) => PropertyDeclaration::TextAnchor(*value),
            PropertyDeclaration::TextAutospace(value) => PropertyDeclaration::TextAutospace(*value),
            PropertyDeclaration::TextBoxEdge(value) => PropertyDeclaration::TextBoxEdge(*value),
            PropertyDeclaration::TextDecorationLine(value) => {
                PropertyDeclaration::TextDecorationLine(*value)
            },
            PropertyDeclaration::TextDecorationSkipInk(value) => {
                PropertyDeclaration::TextDecorationSkipInk(*value)
            },
            PropertyDeclaration::TextDecorationStyle(value) => {
                PropertyDeclaration::TextDecorationStyle(*value)
            },
            PropertyDeclaration::TextEmphasisPosition(value) => {
                PropertyDeclaration::TextEmphasisPosition(*value)
            },
            PropertyDeclaration::TextEmphasisSkip(value) => {
                PropertyDeclaration::TextEmphasisSkip(*value)
            },
            PropertyDeclaration::TextJustify(value) => PropertyDeclaration::TextJustify(*value),
            PropertyDeclaration::TextOrientation(value) => {
                PropertyDeclaration::TextOrientation(*value)
            },
            PropertyDeclaration::TextRendering(value) => PropertyDeclaration::TextRendering(*value),
            PropertyDeclaration::TextSpacingTrim(value) => {
                PropertyDeclaration::TextSpacingTrim(*value)
            },
            PropertyDeclaration::TextTransform(value) => PropertyDeclaration::TextTransform(*value),
            PropertyDeclaration::TextUnderlinePosition(value) => {
                PropertyDeclaration::TextUnderlinePosition(*value)
            },
            PropertyDeclaration::TextWrapMode(value) => PropertyDeclaration::TextWrapMode(*value),
            PropertyDeclaration::TextWrapStyle(value) => PropertyDeclaration::TextWrapStyle(*value),
            PropertyDeclaration::TouchAction(value) => PropertyDeclaration::TouchAction(*value),
            PropertyDeclaration::TransformBox(value) => PropertyDeclaration::TransformBox(*value),
            PropertyDeclaration::TransformStyle(value) => {
                PropertyDeclaration::TransformStyle(*value)
            },
            PropertyDeclaration::UnicodeBidi(value) => PropertyDeclaration::UnicodeBidi(*value),
            PropertyDeclaration::UserSelect(value) => PropertyDeclaration::UserSelect(*value),
            PropertyDeclaration::VectorEffect(value) => PropertyDeclaration::VectorEffect(*value),
            PropertyDeclaration::Visibility(value) => PropertyDeclaration::Visibility(*value),
            PropertyDeclaration::WebkitTextSecurity(value) => {
                PropertyDeclaration::WebkitTextSecurity(*value)
            },
            PropertyDeclaration::WhiteSpaceCollapse(value) => {
                PropertyDeclaration::WhiteSpaceCollapse(*value)
            },
            PropertyDeclaration::WhiteSpaceTrim(value) => {
                PropertyDeclaration::WhiteSpaceTrim(*value)
            },
            PropertyDeclaration::WordBreak(value) => PropertyDeclaration::WordBreak(*value),
            PropertyDeclaration::WordSpaceTransform(value) => {
                PropertyDeclaration::WordSpaceTransform(*value)
            },
            PropertyDeclaration::WrapFlow(value) => PropertyDeclaration::WrapFlow(*value),
            PropertyDeclaration::WrapThrough(value) => PropertyDeclaration::WrapThrough(*value),
            PropertyDeclaration::WritingMode(value) => PropertyDeclaration::WritingMode(*value),
            PropertyDeclaration::BdBarcodeStructuredAppend(value) => {
                PropertyDeclaration::BdBarcodeStructuredAppend(*value)
            },
            PropertyDeclaration::BdBarcodeStructuredAppendPosition(value) => {
                PropertyDeclaration::BdBarcodeStructuredAppendPosition(*value)
            },
            PropertyDeclaration::BorderImageRepeat(value) => {
                PropertyDeclaration::BorderImageRepeat(*value)
            },
            PropertyDeclaration::MaskBorderRepeat(value) => {
                PropertyDeclaration::MaskBorderRepeat(*value)
            },
            PropertyDeclaration::BreakAfter(value) => PropertyDeclaration::BreakAfter(*value),
            PropertyDeclaration::BreakBefore(value) => PropertyDeclaration::BreakBefore(*value),
            PropertyDeclaration::AlignContent(value) => PropertyDeclaration::AlignContent(*value),
            PropertyDeclaration::JustifyContent(value) => {
                PropertyDeclaration::JustifyContent(*value)
            },
            PropertyDeclaration::ClipRule(value) => PropertyDeclaration::ClipRule(*value),
            PropertyDeclaration::FillRule(value) => PropertyDeclaration::FillRule(*value),
            PropertyDeclaration::ColumnRuleBreak(value) => {
                PropertyDeclaration::ColumnRuleBreak(*value)
            },
            PropertyDeclaration::RowRuleBreak(value) => PropertyDeclaration::RowRuleBreak(*value),
            PropertyDeclaration::ColumnRuleVisibilityItems(value) => {
                PropertyDeclaration::ColumnRuleVisibilityItems(*value)
            },
            PropertyDeclaration::RowRuleVisibilityItems(value) => {
                PropertyDeclaration::RowRuleVisibilityItems(*value)
            },
            PropertyDeclaration::AlignSelf(value) => PropertyDeclaration::AlignSelf(*value),
            PropertyDeclaration::JustifySelf(value) => PropertyDeclaration::JustifySelf(*value),
            PropertyDeclaration::FontSynthesisPosition(value) => {
                PropertyDeclaration::FontSynthesisPosition(*value)
            },
            PropertyDeclaration::FontSynthesisSmallCaps(value) => {
                PropertyDeclaration::FontSynthesisSmallCaps(*value)
            },
            PropertyDeclaration::FontSynthesisWeight(value) => {
                PropertyDeclaration::FontSynthesisWeight(*value)
            },
            PropertyDeclaration::CornerBottomLeftShape(value) => {
                PropertyDeclaration::CornerBottomLeftShape(*value)
            },
            PropertyDeclaration::CornerBottomRightShape(value) => {
                PropertyDeclaration::CornerBottomRightShape(*value)
            },
            PropertyDeclaration::CornerTopLeftShape(value) => {
                PropertyDeclaration::CornerTopLeftShape(*value)
            },
            PropertyDeclaration::CornerTopRightShape(value) => {
                PropertyDeclaration::CornerTopRightShape(*value)
            },
            PropertyDeclaration::OverflowBlock(value) => PropertyDeclaration::OverflowBlock(*value),
            PropertyDeclaration::OverflowInline(value) => {
                PropertyDeclaration::OverflowInline(*value)
            },
            PropertyDeclaration::OverflowX(value) => PropertyDeclaration::OverflowX(*value),
            PropertyDeclaration::OverflowY(value) => PropertyDeclaration::OverflowY(*value),
            PropertyDeclaration::OverscrollBehaviorBlock(value) => {
                PropertyDeclaration::OverscrollBehaviorBlock(*value)
            },
            PropertyDeclaration::OverscrollBehaviorInline(value) => {
                PropertyDeclaration::OverscrollBehaviorInline(*value)
            },
            PropertyDeclaration::OverscrollBehaviorX(value) => {
                PropertyDeclaration::OverscrollBehaviorX(*value)
            },
            PropertyDeclaration::OverscrollBehaviorY(value) => {
                PropertyDeclaration::OverscrollBehaviorY(*value)
            },
            PropertyDeclaration::TextDecorationSkipBox(value) => {
                PropertyDeclaration::TextDecorationSkipBox(*value)
            },
            PropertyDeclaration::TextDecorationSkipInset(value) => {
                PropertyDeclaration::TextDecorationSkipInset(*value)
            },
            PropertyDeclaration::TextDecorationSkipSelf(value) => {
                PropertyDeclaration::TextDecorationSkipSelf(*value)
            },
            PropertyDeclaration::TextDecorationSkipSpaces(value) => {
                PropertyDeclaration::TextDecorationSkipSpaces(*value)
            },
            PropertyDeclaration::BdPdfMarkBleedEnabled(value) => {
                PropertyDeclaration::BdPdfMarkBleedEnabled(*value)
            },
            PropertyDeclaration::BdPdfMarkColourBarEnabled(value) => {
                PropertyDeclaration::BdPdfMarkColourBarEnabled(*value)
            },
            PropertyDeclaration::BdPdfMarkCornerRegistrationEnabled(value) => {
                PropertyDeclaration::BdPdfMarkCornerRegistrationEnabled(*value)
            },
            PropertyDeclaration::BdPdfMarkCropEnabled(value) => {
                PropertyDeclaration::BdPdfMarkCropEnabled(*value)
            },
            PropertyDeclaration::BdPdfMarkPageInfoEnabled(value) => {
                PropertyDeclaration::BdPdfMarkPageInfoEnabled(*value)
            },
            PropertyDeclaration::BdPdfMarkRegistrationEnabled(value) => {
                PropertyDeclaration::BdPdfMarkRegistrationEnabled(*value)
            },
            PropertyDeclaration::BorderBlockEndStyle(value) => {
                PropertyDeclaration::BorderBlockEndStyle(*value)
            },
            PropertyDeclaration::BorderBlockStartStyle(value) => {
                PropertyDeclaration::BorderBlockStartStyle(*value)
            },
            PropertyDeclaration::BorderBottomStyle(value) => {
                PropertyDeclaration::BorderBottomStyle(*value)
            },
            PropertyDeclaration::BorderInlineEndStyle(value) => {
                PropertyDeclaration::BorderInlineEndStyle(*value)
            },
            PropertyDeclaration::BorderInlineStartStyle(value) => {
                PropertyDeclaration::BorderInlineStartStyle(*value)
            },
            PropertyDeclaration::BorderLeftStyle(value) => {
                PropertyDeclaration::BorderLeftStyle(*value)
            },
            PropertyDeclaration::BorderRightStyle(value) => {
                PropertyDeclaration::BorderRightStyle(*value)
            },
            PropertyDeclaration::BorderTopStyle(value) => {
                PropertyDeclaration::BorderTopStyle(*value)
            },
            PropertyDeclaration::AccentColor(value) => {
                PropertyDeclaration::AccentColor(value.clone())
            },
            PropertyDeclaration::AlignTracks(value) => {
                PropertyDeclaration::AlignTracks(value.clone())
            },
            PropertyDeclaration::AnchorName(value) => {
                PropertyDeclaration::AnchorName(value.clone())
            },
            PropertyDeclaration::AnchorScope(value) => {
                PropertyDeclaration::AnchorScope(value.clone())
            },
            PropertyDeclaration::AnimationComposition(value) => {
                PropertyDeclaration::AnimationComposition(value.clone())
            },
            PropertyDeclaration::AnimationDelay(value) => {
                PropertyDeclaration::AnimationDelay(value.clone())
            },
            PropertyDeclaration::AnimationDirection(value) => {
                PropertyDeclaration::AnimationDirection(value.clone())
            },
            PropertyDeclaration::AnimationDuration(value) => {
                PropertyDeclaration::AnimationDuration(value.clone())
            },
            PropertyDeclaration::AnimationFillMode(value) => {
                PropertyDeclaration::AnimationFillMode(value.clone())
            },
            PropertyDeclaration::AnimationIterationCount(value) => {
                PropertyDeclaration::AnimationIterationCount(value.clone())
            },
            PropertyDeclaration::AnimationName(value) => {
                PropertyDeclaration::AnimationName(value.clone())
            },
            PropertyDeclaration::AnimationPlayState(value) => {
                PropertyDeclaration::AnimationPlayState(value.clone())
            },
            PropertyDeclaration::AnimationTimeline(value) => {
                PropertyDeclaration::AnimationTimeline(value.clone())
            },
            PropertyDeclaration::AnimationTimingFunction(value) => {
                PropertyDeclaration::AnimationTimingFunction(value.clone())
            },
            PropertyDeclaration::AspectRatio(value) => {
                PropertyDeclaration::AspectRatio(value.clone())
            },
            PropertyDeclaration::BackdropFilter(value) => {
                PropertyDeclaration::BackdropFilter(value.clone())
            },
            PropertyDeclaration::BackgroundAttachment(value) => {
                PropertyDeclaration::BackgroundAttachment(value.clone())
            },
            PropertyDeclaration::BackgroundBlendMode(value) => {
                PropertyDeclaration::BackgroundBlendMode(value.clone())
            },
            PropertyDeclaration::BackgroundClip(value) => {
                PropertyDeclaration::BackgroundClip(value.clone())
            },
            PropertyDeclaration::BackgroundImage(value) => {
                PropertyDeclaration::BackgroundImage(value.clone())
            },
            PropertyDeclaration::BackgroundOrigin(value) => {
                PropertyDeclaration::BackgroundOrigin(value.clone())
            },
            PropertyDeclaration::BackgroundPositionX(value) => {
                PropertyDeclaration::BackgroundPositionX(value.clone())
            },
            PropertyDeclaration::BackgroundPositionY(value) => {
                PropertyDeclaration::BackgroundPositionY(value.clone())
            },
            PropertyDeclaration::BackgroundRepeat(value) => {
                PropertyDeclaration::BackgroundRepeat(value.clone())
            },
            PropertyDeclaration::BackgroundSize(value) => {
                PropertyDeclaration::BackgroundSize(value.clone())
            },
            PropertyDeclaration::BaselineShift(value) => {
                PropertyDeclaration::BaselineShift(value.clone())
            },
            PropertyDeclaration::BdBarcodeFontFamily(value) => {
                PropertyDeclaration::BdBarcodeFontFamily(value.clone())
            },
            PropertyDeclaration::BdBarcodeHumanReadableAffix(value) => {
                PropertyDeclaration::BdBarcodeHumanReadableAffix(value.clone())
            },
            PropertyDeclaration::BdBarcodeSize(value) => {
                PropertyDeclaration::BdBarcodeSize(value.clone())
            },
            PropertyDeclaration::BdBaselineGrid(value) => {
                PropertyDeclaration::BdBaselineGrid(value.clone())
            },
            PropertyDeclaration::BdBlankPageContent(value) => {
                PropertyDeclaration::BdBlankPageContent(value.clone())
            },
            PropertyDeclaration::BdBorderClip(value) => {
                PropertyDeclaration::BdBorderClip(value.clone())
            },
            PropertyDeclaration::BdCaptionPage(value) => {
                PropertyDeclaration::BdCaptionPage(value.clone())
            },
            PropertyDeclaration::BdChangeBarColour(value) => {
                PropertyDeclaration::BdChangeBarColour(value.clone())
            },
            PropertyDeclaration::BdChangeBarName(value) => {
                PropertyDeclaration::BdChangeBarName(value.clone())
            },
            PropertyDeclaration::BdChangeBarOffset(value) => {
                PropertyDeclaration::BdChangeBarOffset(value.clone())
            },
            PropertyDeclaration::BdChangeBarWidth(value) => {
                PropertyDeclaration::BdChangeBarWidth(value.clone())
            },
            PropertyDeclaration::BdChangeLineBreaksForPagination(value) => {
                PropertyDeclaration::BdChangeLineBreaksForPagination(value.clone())
            },
            PropertyDeclaration::BdColorFunction(value) => {
                PropertyDeclaration::BdColorFunction(value.clone())
            },
            PropertyDeclaration::BdColumnClip(value) => {
                PropertyDeclaration::BdColumnClip(value.clone())
            },
            PropertyDeclaration::BdDestinationArea(value) => {
                PropertyDeclaration::BdDestinationArea(value.clone())
            },
            PropertyDeclaration::BdFilterResolution(value) => {
                PropertyDeclaration::BdFilterResolution(value.clone())
            },
            PropertyDeclaration::BdFloatReferenceSidenote(value) => {
                PropertyDeclaration::BdFloatReferenceSidenote(value.clone())
            },
            PropertyDeclaration::BdFlow(value) => PropertyDeclaration::BdFlow(value.clone()),
            PropertyDeclaration::BdFlowFrom(value) => {
                PropertyDeclaration::BdFlowFrom(value.clone())
            },
            PropertyDeclaration::BdFlowInto(value) => {
                PropertyDeclaration::BdFlowInto(value.clone())
            },
            PropertyDeclaration::BdFontEmbeddingType(value) => {
                PropertyDeclaration::BdFontEmbeddingType(value.clone())
            },
            PropertyDeclaration::BdFootnoteFragmentation(value) => {
                PropertyDeclaration::BdFootnoteFragmentation(value.clone())
            },
            PropertyDeclaration::BdFootnoteRuleLength(value) => {
                PropertyDeclaration::BdFootnoteRuleLength(value.clone())
            },
            PropertyDeclaration::BdForcedBreaks(value) => {
                PropertyDeclaration::BdForcedBreaks(value.clone())
            },
            PropertyDeclaration::BdGlyphLayoutMode(value) => {
                PropertyDeclaration::BdGlyphLayoutMode(value.clone())
            },
            PropertyDeclaration::BdHyphenateLimitLines(value) => {
                PropertyDeclaration::BdHyphenateLimitLines(value.clone())
            },
            PropertyDeclaration::BdHyphenateLines(value) => {
                PropertyDeclaration::BdHyphenateLines(value.clone())
            },
            PropertyDeclaration::BdHyphenatePatterns(value) => {
                PropertyDeclaration::BdHyphenatePatterns(value.clone())
            },
            PropertyDeclaration::BdHyphenateWordLength(value) => {
                PropertyDeclaration::BdHyphenateWordLength(value.clone())
            },
            PropertyDeclaration::BdImageClipPath(value) => {
                PropertyDeclaration::BdImageClipPath(value.clone())
            },
            PropertyDeclaration::BdImageInteractivity(value) => {
                PropertyDeclaration::BdImageInteractivity(value.clone())
            },
            PropertyDeclaration::BdImageMagic(value) => {
                PropertyDeclaration::BdImageMagic(value.clone())
            },
            PropertyDeclaration::BdImageOrientation(value) => {
                PropertyDeclaration::BdImageOrientation(value.clone())
            },
            PropertyDeclaration::BdImageRecompression(value) => {
                PropertyDeclaration::BdImageRecompression(value.clone())
            },
            PropertyDeclaration::BdImageResampling(value) => {
                PropertyDeclaration::BdImageResampling(value.clone())
            },
            PropertyDeclaration::BdIndex(value) => PropertyDeclaration::BdIndex(value.clone()),
            PropertyDeclaration::BdInitialPage(value) => {
                PropertyDeclaration::BdInitialPage(value.clone())
            },
            PropertyDeclaration::BdInitialZoom(value) => {
                PropertyDeclaration::BdInitialZoom(value.clone())
            },
            PropertyDeclaration::BdKeepWithPrevious(value) => {
                PropertyDeclaration::BdKeepWithPrevious(value.clone())
            },
            PropertyDeclaration::BdLang(value) => PropertyDeclaration::BdLang(value.clone()),
            PropertyDeclaration::BdLineBreakChoices(value) => {
                PropertyDeclaration::BdLineBreakChoices(value.clone())
            },
            PropertyDeclaration::BdLineBreakOpportunity(value) => {
                PropertyDeclaration::BdLineBreakOpportunity(value.clone())
            },
            PropertyDeclaration::BdLineGrid(value) => {
                PropertyDeclaration::BdLineGrid(value.clone())
            },
            PropertyDeclaration::BdLineSnap(value) => {
                PropertyDeclaration::BdLineSnap(value.clone())
            },
            PropertyDeclaration::BdLineStackingStrategy(value) => {
                PropertyDeclaration::BdLineStackingStrategy(value.clone())
            },
            PropertyDeclaration::BdLinebreakMagic(value) => {
                PropertyDeclaration::BdLinebreakMagic(value.clone())
            },
            PropertyDeclaration::BdLink(value) => PropertyDeclaration::BdLink(value.clone()),
            PropertyDeclaration::BdLinkArea(value) => {
                PropertyDeclaration::BdLinkArea(value.clone())
            },
            PropertyDeclaration::BdNLines(value) => PropertyDeclaration::BdNLines(value.clone()),
            PropertyDeclaration::BdObjectSlice(value) => {
                PropertyDeclaration::BdObjectSlice(value.clone())
            },
            PropertyDeclaration::BdOrphansFragments(value) => {
                PropertyDeclaration::BdOrphansFragments(value.clone())
            },
            PropertyDeclaration::BdPageFill(value) => {
                PropertyDeclaration::BdPageFill(value.clone())
            },
            PropertyDeclaration::BdPageGroup(value) => {
                PropertyDeclaration::BdPageGroup(value.clone())
            },
            PropertyDeclaration::BdPageMarksColour(value) => {
                PropertyDeclaration::BdPageMarksColour(value.clone())
            },
            PropertyDeclaration::BdPageMarksWidth(value) => {
                PropertyDeclaration::BdPageMarksWidth(value.clone())
            },
            PropertyDeclaration::BdPagesCounterOffset(value) => {
                PropertyDeclaration::BdPagesCounterOffset(value.clone())
            },
            PropertyDeclaration::BdPaintReordering(value) => {
                PropertyDeclaration::BdPaintReordering(value.clone())
            },
            PropertyDeclaration::BdPdfArtBox(value) => {
                PropertyDeclaration::BdPdfArtBox(value.clone())
            },
            PropertyDeclaration::BdPdfArtSize(value) => {
                PropertyDeclaration::BdPdfArtSize(value.clone())
            },
            PropertyDeclaration::BdPdfAttachmentIcon(value) => {
                PropertyDeclaration::BdPdfAttachmentIcon(value.clone())
            },
            PropertyDeclaration::BdPdfAttachmentLocation(value) => {
                PropertyDeclaration::BdPdfAttachmentLocation(value.clone())
            },
            PropertyDeclaration::BdPdfAttachmentModificationDate(value) => {
                PropertyDeclaration::BdPdfAttachmentModificationDate(value.clone())
            },
            PropertyDeclaration::BdPdfAttachmentOrder(value) => {
                PropertyDeclaration::BdPdfAttachmentOrder(value.clone())
            },
            PropertyDeclaration::BdPdfAttachmentRelationship(value) => {
                PropertyDeclaration::BdPdfAttachmentRelationship(value.clone())
            },
            PropertyDeclaration::BdPdfAttachmentUrl(value) => {
                PropertyDeclaration::BdPdfAttachmentUrl(value.clone())
            },
            PropertyDeclaration::BdPdfBleedBox(value) => {
                PropertyDeclaration::BdPdfBleedBox(value.clone())
            },
            PropertyDeclaration::BdPdfBookmarksEnabled(value) => {
                PropertyDeclaration::BdPdfBookmarksEnabled(value.clone())
            },
            PropertyDeclaration::BdPdfColourConversion(value) => {
                PropertyDeclaration::BdPdfColourConversion(value.clone())
            },
            PropertyDeclaration::BdPdfColourOptions(value) => {
                PropertyDeclaration::BdPdfColourOptions(value.clone())
            },
            PropertyDeclaration::BdPdfComment(value) => {
                PropertyDeclaration::BdPdfComment(value.clone())
            },
            PropertyDeclaration::BdPdfCommentAuthor(value) => {
                PropertyDeclaration::BdPdfCommentAuthor(value.clone())
            },
            PropertyDeclaration::BdPdfCommentColour(value) => {
                PropertyDeclaration::BdPdfCommentColour(value.clone())
            },
            PropertyDeclaration::BdPdfCommentDateFormat(value) => {
                PropertyDeclaration::BdPdfCommentDateFormat(value.clone())
            },
            PropertyDeclaration::BdPdfCommentIcon(value) => {
                PropertyDeclaration::BdPdfCommentIcon(value.clone())
            },
            PropertyDeclaration::BdPdfCommentOpen(value) => {
                PropertyDeclaration::BdPdfCommentOpen(value.clone())
            },
            PropertyDeclaration::BdPdfCommentPosition(value) => {
                PropertyDeclaration::BdPdfCommentPosition(value.clone())
            },
            PropertyDeclaration::BdPdfCommentState(value) => {
                PropertyDeclaration::BdPdfCommentState(value.clone())
            },
            PropertyDeclaration::BdPdfCommentStateModel(value) => {
                PropertyDeclaration::BdPdfCommentStateModel(value.clone())
            },
            PropertyDeclaration::BdPdfCommentSubject(value) => {
                PropertyDeclaration::BdPdfCommentSubject(value.clone())
            },
            PropertyDeclaration::BdPdfConformance(value) => {
                PropertyDeclaration::BdPdfConformance(value.clone())
            },
            PropertyDeclaration::BdPdfCropBox(value) => {
                PropertyDeclaration::BdPdfCropBox(value.clone())
            },
            PropertyDeclaration::BdPdfCropSize(value) => {
                PropertyDeclaration::BdPdfCropSize(value.clone())
            },
            PropertyDeclaration::BdPdfCustomProperty(value) => {
                PropertyDeclaration::BdPdfCustomProperty(value.clone())
            },
            PropertyDeclaration::BdPdfEventScripts(value) => {
                PropertyDeclaration::BdPdfEventScripts(value.clone())
            },
            PropertyDeclaration::BdPdfFallbackCmykProfile(value) => {
                PropertyDeclaration::BdPdfFallbackCmykProfile(value.clone())
            },
            PropertyDeclaration::BdPdfFormFieldMkIconFit(value) => {
                PropertyDeclaration::BdPdfFormFieldMkIconFit(value.clone())
            },
            PropertyDeclaration::BdPdfFormat(value) => {
                PropertyDeclaration::BdPdfFormat(value.clone())
            },
            PropertyDeclaration::BdPdfLayer(value) => {
                PropertyDeclaration::BdPdfLayer(value.clone())
            },
            PropertyDeclaration::BdPdfLayerIntent(value) => {
                PropertyDeclaration::BdPdfLayerIntent(value.clone())
            },
            PropertyDeclaration::BdPdfLinkBorder(value) => {
                PropertyDeclaration::BdPdfLinkBorder(value.clone())
            },
            PropertyDeclaration::BdPdfLinkBorderColor(value) => {
                PropertyDeclaration::BdPdfLinkBorderColor(value.clone())
            },
            PropertyDeclaration::BdPdfLinkBorderWidth(value) => {
                PropertyDeclaration::BdPdfLinkBorderWidth(value.clone())
            },
            PropertyDeclaration::BdPdfLinkType(value) => {
                PropertyDeclaration::BdPdfLinkType(value.clone())
            },
            PropertyDeclaration::BdPdfMarkBleedColor(value) => {
                PropertyDeclaration::BdPdfMarkBleedColor(value.clone())
            },
            PropertyDeclaration::BdPdfMarkColourBarSwatches(value) => {
                PropertyDeclaration::BdPdfMarkColourBarSwatches(value.clone())
            },
            PropertyDeclaration::BdPdfMarkCropColor(value) => {
                PropertyDeclaration::BdPdfMarkCropColor(value.clone())
            },
            PropertyDeclaration::BdPdfMarkRegistrationColor(value) => {
                PropertyDeclaration::BdPdfMarkRegistrationColor(value.clone())
            },
            PropertyDeclaration::BdPdfMarkSidenoteGlyph(value) => {
                PropertyDeclaration::BdPdfMarkSidenoteGlyph(value.clone())
            },
            PropertyDeclaration::BdPdfMarkSidenoteOffset(value) => {
                PropertyDeclaration::BdPdfMarkSidenoteOffset(value.clone())
            },
            PropertyDeclaration::BdPdfMediaSize(value) => {
                PropertyDeclaration::BdPdfMediaSize(value.clone())
            },
            PropertyDeclaration::BdPdfOpenActionScript(value) => {
                PropertyDeclaration::BdPdfOpenActionScript(value.clone())
            },
            PropertyDeclaration::BdPdfOutputCondition(value) => {
                PropertyDeclaration::BdPdfOutputCondition(value.clone())
            },
            PropertyDeclaration::BdPdfOutputIntent(value) => {
                PropertyDeclaration::BdPdfOutputIntent(value.clone())
            },
            PropertyDeclaration::BdPdfOutputRegistryName(value) => {
                PropertyDeclaration::BdPdfOutputRegistryName(value.clone())
            },
            PropertyDeclaration::BdPdfOverprint(value) => {
                PropertyDeclaration::BdPdfOverprint(value.clone())
            },
            PropertyDeclaration::BdPdfOverprintContent(value) => {
                PropertyDeclaration::BdPdfOverprintContent(value.clone())
            },
            PropertyDeclaration::BdPdfPageClip(value) => {
                PropertyDeclaration::BdPdfPageClip(value.clone())
            },
            PropertyDeclaration::BdPdfPageColourspace(value) => {
                PropertyDeclaration::BdPdfPageColourspace(value.clone())
            },
            PropertyDeclaration::BdPdfPageRotation(value) => {
                PropertyDeclaration::BdPdfPageRotation(value.clone())
            },
            PropertyDeclaration::BdPdfPassdownStyles(value) => {
                PropertyDeclaration::BdPdfPassdownStyles(value.clone())
            },
            PropertyDeclaration::BdPdfRasterAccessibility(value) => {
                PropertyDeclaration::BdPdfRasterAccessibility(value.clone())
            },
            PropertyDeclaration::BdPdfRoleMap(value) => {
                PropertyDeclaration::BdPdfRoleMap(value.clone())
            },
            PropertyDeclaration::BdPdfScript(value) => {
                PropertyDeclaration::BdPdfScript(value.clone())
            },
            PropertyDeclaration::BdPdfShapeOptimization(value) => {
                PropertyDeclaration::BdPdfShapeOptimization(value.clone())
            },
            PropertyDeclaration::BdPdfSignature(value) => {
                PropertyDeclaration::BdPdfSignature(value.clone())
            },
            PropertyDeclaration::BdPdfSignatureFieldLockFields(value) => {
                PropertyDeclaration::BdPdfSignatureFieldLockFields(value.clone())
            },
            PropertyDeclaration::BdPdfSignatureFieldName(value) => {
                PropertyDeclaration::BdPdfSignatureFieldName(value.clone())
            },
            PropertyDeclaration::BdPdfStampIcon(value) => {
                PropertyDeclaration::BdPdfStampIcon(value.clone())
            },
            PropertyDeclaration::BdPdfStampIntent(value) => {
                PropertyDeclaration::BdPdfStampIntent(value.clone())
            },
            PropertyDeclaration::BdPdfTag(value) => PropertyDeclaration::BdPdfTag(value.clone()),
            PropertyDeclaration::BdPdfTagForm(value) => {
                PropertyDeclaration::BdPdfTagForm(value.clone())
            },
            PropertyDeclaration::BdPdfTagFormChecked(value) => {
                PropertyDeclaration::BdPdfTagFormChecked(value.clone())
            },
            PropertyDeclaration::BdPdfTagFormName(value) => {
                PropertyDeclaration::BdPdfTagFormName(value.clone())
            },
            PropertyDeclaration::BdPdfTagHeaderCellScope(value) => {
                PropertyDeclaration::BdPdfTagHeaderCellScope(value.clone())
            },
            PropertyDeclaration::BdPdfTagNamespace(value) => {
                PropertyDeclaration::BdPdfTagNamespace(value.clone())
            },
            PropertyDeclaration::BdPdfTagTableSummary(value) => {
                PropertyDeclaration::BdPdfTagTableSummary(value.clone())
            },
            PropertyDeclaration::BdPdfTextRendering(value) => {
                PropertyDeclaration::BdPdfTextRendering(value.clone())
            },
            PropertyDeclaration::BdPdfTrimBox(value) => {
                PropertyDeclaration::BdPdfTrimBox(value.clone())
            },
            PropertyDeclaration::BdPdfVersion(value) => {
                PropertyDeclaration::BdPdfVersion(value.clone())
            },
            PropertyDeclaration::BdPdfViewerDirection(value) => {
                PropertyDeclaration::BdPdfViewerDirection(value.clone())
            },
            PropertyDeclaration::BdPdfViewerDuplex(value) => {
                PropertyDeclaration::BdPdfViewerDuplex(value.clone())
            },
            PropertyDeclaration::BdPdfViewerNonFullscreenPageMode(value) => {
                PropertyDeclaration::BdPdfViewerNonFullscreenPageMode(value.clone())
            },
            PropertyDeclaration::BdPdfViewerNumCopies(value) => {
                PropertyDeclaration::BdPdfViewerNumCopies(value.clone())
            },
            PropertyDeclaration::BdPdfViewerPageLayout(value) => {
                PropertyDeclaration::BdPdfViewerPageLayout(value.clone())
            },
            PropertyDeclaration::BdPdfViewerPageMode(value) => {
                PropertyDeclaration::BdPdfViewerPageMode(value.clone())
            },
            PropertyDeclaration::BdPdfViewerPrintPageRange(value) => {
                PropertyDeclaration::BdPdfViewerPrintPageRange(value.clone())
            },
            PropertyDeclaration::BdPdfViewerPrintScaling(value) => {
                PropertyDeclaration::BdPdfViewerPrintScaling(value.clone())
            },
            PropertyDeclaration::BdPositionOrigin(value) => {
                PropertyDeclaration::BdPositionOrigin(value.clone())
            },
            PropertyDeclaration::BdPrinceBleed(value) => {
                PropertyDeclaration::BdPrinceBleed(value.clone())
            },
            PropertyDeclaration::BdRasterization(value) => {
                PropertyDeclaration::BdRasterization(value.clone())
            },
            PropertyDeclaration::BdRasterizationMaxSize(value) => {
                PropertyDeclaration::BdRasterizationMaxSize(value.clone())
            },
            PropertyDeclaration::BdRasterizationSupersampling(value) => {
                PropertyDeclaration::BdRasterizationSupersampling(value.clone())
            },
            PropertyDeclaration::BdRegionFragment(value) => {
                PropertyDeclaration::BdRegionFragment(value.clone())
            },
            PropertyDeclaration::BdReplacedelement(value) => {
                PropertyDeclaration::BdReplacedelement(value.clone())
            },
            PropertyDeclaration::BdResizeAdjust(value) => {
                PropertyDeclaration::BdResizeAdjust(value.clone())
            },
            PropertyDeclaration::BdResizeOptions(value) => {
                PropertyDeclaration::BdResizeOptions(value.clone())
            },
            PropertyDeclaration::BdRotateBody(value) => {
                PropertyDeclaration::BdRotateBody(value.clone())
            },
            PropertyDeclaration::BdScaleContent(value) => {
                PropertyDeclaration::BdScaleContent(value.clone())
            },
            PropertyDeclaration::BdShrinkToFit(value) => {
                PropertyDeclaration::BdShrinkToFit(value.clone())
            },
            PropertyDeclaration::BdSidenoteAlign(value) => {
                PropertyDeclaration::BdSidenoteAlign(value.clone())
            },
            PropertyDeclaration::BdSidenoteAvoid(value) => {
                PropertyDeclaration::BdSidenoteAvoid(value.clone())
            },
            PropertyDeclaration::BdSidenoteOffset(value) => {
                PropertyDeclaration::BdSidenoteOffset(value.clone())
            },
            PropertyDeclaration::BdSidenoteSide(value) => {
                PropertyDeclaration::BdSidenoteSide(value.clone())
            },
            PropertyDeclaration::BdSource(value) => PropertyDeclaration::BdSource(value.clone()),
            PropertyDeclaration::BdSourceArea(value) => {
                PropertyDeclaration::BdSourceArea(value.clone())
            },
            PropertyDeclaration::BdSourcePage(value) => {
                PropertyDeclaration::BdSourcePage(value.clone())
            },
            PropertyDeclaration::BdSpreadLengthOptions(value) => {
                PropertyDeclaration::BdSpreadLengthOptions(value.clone())
            },
            PropertyDeclaration::BdTabSnap(value) => PropertyDeclaration::BdTabSnap(value.clone()),
            PropertyDeclaration::BdTabStops(value) => {
                PropertyDeclaration::BdTabStops(value.clone())
            },
            PropertyDeclaration::BdTargetCandidate(value) => {
                PropertyDeclaration::BdTargetCandidate(value.clone())
            },
            PropertyDeclaration::BdTextDecorationSkip(value) => {
                PropertyDeclaration::BdTextDecorationSkip(value.clone())
            },
            PropertyDeclaration::BdTextDecorationTrim(value) => {
                PropertyDeclaration::BdTextDecorationTrim(value.clone())
            },
            PropertyDeclaration::BdTextReplace(value) => {
                PropertyDeclaration::BdTextReplace(value.clone())
            },
            PropertyDeclaration::BdTextUnderlineOffset(value) => {
                PropertyDeclaration::BdTextUnderlineOffset(value.clone())
            },
            PropertyDeclaration::BdTextWrap(value) => {
                PropertyDeclaration::BdTextWrap(value.clone())
            },
            PropertyDeclaration::BdTooltip(value) => PropertyDeclaration::BdTooltip(value.clone()),
            PropertyDeclaration::BdTruncateMarginAfterBreak(value) => {
                PropertyDeclaration::BdTruncateMarginAfterBreak(value.clone())
            },
            PropertyDeclaration::BdWrapInside(value) => {
                PropertyDeclaration::BdWrapInside(value.clone())
            },
            PropertyDeclaration::Bleed(value) => PropertyDeclaration::Bleed(value.clone()),
            PropertyDeclaration::BlockEllipsis(value) => {
                PropertyDeclaration::BlockEllipsis(value.clone())
            },
            PropertyDeclaration::BlockStepSize(value) => {
                PropertyDeclaration::BlockStepSize(value.clone())
            },
            PropertyDeclaration::BookmarkLabel(value) => {
                PropertyDeclaration::BookmarkLabel(value.clone())
            },
            PropertyDeclaration::BookmarkTarget(value) => {
                PropertyDeclaration::BookmarkTarget(value.clone())
            },
            PropertyDeclaration::BorderShape(value) => {
                PropertyDeclaration::BorderShape(value.clone())
            },
            PropertyDeclaration::BorderSpacing(value) => {
                PropertyDeclaration::BorderSpacing(value.clone())
            },
            PropertyDeclaration::BoxShadow(value) => PropertyDeclaration::BoxShadow(value.clone()),
            PropertyDeclaration::CaretColor(value) => {
                PropertyDeclaration::CaretColor(value.clone())
            },
            PropertyDeclaration::Clip(value) => PropertyDeclaration::Clip(value.clone()),
            PropertyDeclaration::ClipPath(value) => PropertyDeclaration::ClipPath(value.clone()),
            PropertyDeclaration::Color(value) => PropertyDeclaration::Color(value.clone()),
            PropertyDeclaration::ColorScheme(value) => {
                PropertyDeclaration::ColorScheme(value.clone())
            },
            PropertyDeclaration::ColumnCount(value) => {
                PropertyDeclaration::ColumnCount(value.clone())
            },
            PropertyDeclaration::ContainerName(value) => {
                PropertyDeclaration::ContainerName(value.clone())
            },
            PropertyDeclaration::Content(value) => PropertyDeclaration::Content(value.clone()),
            PropertyDeclaration::CounterIncrement(value) => {
                PropertyDeclaration::CounterIncrement(value.clone())
            },
            PropertyDeclaration::CounterReset(value) => {
                PropertyDeclaration::CounterReset(value.clone())
            },
            PropertyDeclaration::CounterSet(value) => {
                PropertyDeclaration::CounterSet(value.clone())
            },
            PropertyDeclaration::Cursor(value) => PropertyDeclaration::Cursor(value.clone()),
            PropertyDeclaration::Filter(value) => PropertyDeclaration::Filter(value.clone()),
            PropertyDeclaration::FlexBasis(value) => PropertyDeclaration::FlexBasis(value.clone()),
            PropertyDeclaration::Float(value) => PropertyDeclaration::Float(value.clone()),
            PropertyDeclaration::FloatOffset(value) => {
                PropertyDeclaration::FloatOffset(value.clone())
            },
            PropertyDeclaration::FloatPlacement(value) => {
                PropertyDeclaration::FloatPlacement(value.clone())
            },
            PropertyDeclaration::FlowFrom(value) => PropertyDeclaration::FlowFrom(value.clone()),
            PropertyDeclaration::FlowInto(value) => PropertyDeclaration::FlowInto(value.clone()),
            PropertyDeclaration::FontFamily(value) => {
                PropertyDeclaration::FontFamily(value.clone())
            },
            PropertyDeclaration::FontFeatureSettings(value) => {
                PropertyDeclaration::FontFeatureSettings(value.clone())
            },
            PropertyDeclaration::FontPalette(value) => {
                PropertyDeclaration::FontPalette(value.clone())
            },
            PropertyDeclaration::FontSize(value) => PropertyDeclaration::FontSize(value.clone()),
            PropertyDeclaration::FontSizeAdjust(value) => {
                PropertyDeclaration::FontSizeAdjust(value.clone())
            },
            PropertyDeclaration::FontStretch(value) => {
                PropertyDeclaration::FontStretch(value.clone())
            },
            PropertyDeclaration::FontVariantAlternates(value) => {
                PropertyDeclaration::FontVariantAlternates(value.clone())
            },
            PropertyDeclaration::FontVariationSettings(value) => {
                PropertyDeclaration::FontVariationSettings(value.clone())
            },
            PropertyDeclaration::FontWeight(value) => {
                PropertyDeclaration::FontWeight(value.clone())
            },
            PropertyDeclaration::FootnoteStylePosition(value) => {
                PropertyDeclaration::FootnoteStylePosition(value.clone())
            },
            PropertyDeclaration::GridTemplateAreas(value) => {
                PropertyDeclaration::GridTemplateAreas(value.clone())
            },
            PropertyDeclaration::HyphenateCharacter(value) => {
                PropertyDeclaration::HyphenateCharacter(value.clone())
            },
            PropertyDeclaration::HyphenateLimitChars(value) => {
                PropertyDeclaration::HyphenateLimitChars(value.clone())
            },
            PropertyDeclaration::InitialLetter(value) => {
                PropertyDeclaration::InitialLetter(value.clone())
            },
            PropertyDeclaration::JustifyTracks(value) => {
                PropertyDeclaration::JustifyTracks(value.clone())
            },
            PropertyDeclaration::LetterSpacing(value) => {
                PropertyDeclaration::LetterSpacing(value.clone())
            },
            PropertyDeclaration::LineHeight(value) => {
                PropertyDeclaration::LineHeight(value.clone())
            },
            PropertyDeclaration::LineSnap(value) => PropertyDeclaration::LineSnap(value.clone()),
            PropertyDeclaration::ListStyleType(value) => {
                PropertyDeclaration::ListStyleType(value.clone())
            },
            PropertyDeclaration::Marks(value) => PropertyDeclaration::Marks(value.clone()),
            PropertyDeclaration::MaskBorderMode(value) => {
                PropertyDeclaration::MaskBorderMode(value.clone())
            },
            PropertyDeclaration::MaskClip(value) => PropertyDeclaration::MaskClip(value.clone()),
            PropertyDeclaration::MaskComposite(value) => {
                PropertyDeclaration::MaskComposite(value.clone())
            },
            PropertyDeclaration::MaskImage(value) => PropertyDeclaration::MaskImage(value.clone()),
            PropertyDeclaration::MaskMode(value) => PropertyDeclaration::MaskMode(value.clone()),
            PropertyDeclaration::MaskOrigin(value) => {
                PropertyDeclaration::MaskOrigin(value.clone())
            },
            PropertyDeclaration::MaskPositionX(value) => {
                PropertyDeclaration::MaskPositionX(value.clone())
            },
            PropertyDeclaration::MaskPositionY(value) => {
                PropertyDeclaration::MaskPositionY(value.clone())
            },
            PropertyDeclaration::MaskRepeat(value) => {
                PropertyDeclaration::MaskRepeat(value.clone())
            },
            PropertyDeclaration::MaskSize(value) => PropertyDeclaration::MaskSize(value.clone()),
            PropertyDeclaration::MasonrySlack(value) => {
                PropertyDeclaration::MasonrySlack(value.clone())
            },
            PropertyDeclaration::MaxLines(value) => PropertyDeclaration::MaxLines(value.clone()),
            PropertyDeclaration::ObjectViewBox(value) => {
                PropertyDeclaration::ObjectViewBox(value.clone())
            },
            PropertyDeclaration::OffsetAnchor(value) => {
                PropertyDeclaration::OffsetAnchor(value.clone())
            },
            PropertyDeclaration::OffsetPath(value) => {
                PropertyDeclaration::OffsetPath(value.clone())
            },
            PropertyDeclaration::OffsetPosition(value) => {
                PropertyDeclaration::OffsetPosition(value.clone())
            },
            PropertyDeclaration::OutlineOffset(value) => {
                PropertyDeclaration::OutlineOffset(value.clone())
            },
            PropertyDeclaration::OutputColorModel(value) => {
                PropertyDeclaration::OutputColorModel(value.clone())
            },
            PropertyDeclaration::Overlay(value) => PropertyDeclaration::Overlay(value.clone()),
            PropertyDeclaration::Page(value) => PropertyDeclaration::Page(value.clone()),
            PropertyDeclaration::Perspective(value) => {
                PropertyDeclaration::Perspective(value.clone())
            },
            PropertyDeclaration::Position(value) => PropertyDeclaration::Position(value.clone()),
            PropertyDeclaration::PositionAnchor(value) => {
                PropertyDeclaration::PositionAnchor(value.clone())
            },
            PropertyDeclaration::PositionTryFallbacks(value) => {
                PropertyDeclaration::PositionTryFallbacks(value.clone())
            },
            PropertyDeclaration::Quotes(value) => PropertyDeclaration::Quotes(value.clone()),
            PropertyDeclaration::Rotate(value) => PropertyDeclaration::Rotate(value.clone()),
            PropertyDeclaration::Scale(value) => PropertyDeclaration::Scale(value.clone()),
            PropertyDeclaration::ScrollMarkerGroup(value) => {
                PropertyDeclaration::ScrollMarkerGroup(value.clone())
            },
            PropertyDeclaration::ScrollbarColor(value) => {
                PropertyDeclaration::ScrollbarColor(value.clone())
            },
            PropertyDeclaration::ShapeOutside(value) => {
                PropertyDeclaration::ShapeOutside(value.clone())
            },
            PropertyDeclaration::Size(value) => PropertyDeclaration::Size(value.clone()),
            PropertyDeclaration::StringSet(value) => PropertyDeclaration::StringSet(value.clone()),
            PropertyDeclaration::StrokeDasharray(value) => {
                PropertyDeclaration::StrokeDasharray(value.clone())
            },
            PropertyDeclaration::StrokeDashoffset(value) => {
                PropertyDeclaration::StrokeDashoffset(value.clone())
            },
            PropertyDeclaration::StrokeWidth(value) => {
                PropertyDeclaration::StrokeWidth(value.clone())
            },
            PropertyDeclaration::TabSize(value) => PropertyDeclaration::TabSize(value.clone()),
            PropertyDeclaration::TextCombineUpright(value) => {
                PropertyDeclaration::TextCombineUpright(value.clone())
            },
            PropertyDeclaration::TextDecorationThickness(value) => {
                PropertyDeclaration::TextDecorationThickness(value.clone())
            },
            PropertyDeclaration::TextDecorationTrim(value) => {
                PropertyDeclaration::TextDecorationTrim(value.clone())
            },
            PropertyDeclaration::TextEmphasisStyle(value) => {
                PropertyDeclaration::TextEmphasisStyle(value.clone())
            },
            PropertyDeclaration::TextIndent(value) => {
                PropertyDeclaration::TextIndent(value.clone())
            },
            PropertyDeclaration::TextOverflow(value) => {
                PropertyDeclaration::TextOverflow(value.clone())
            },
            PropertyDeclaration::TextShadow(value) => {
                PropertyDeclaration::TextShadow(value.clone())
            },
            PropertyDeclaration::TextSizeAdjust(value) => {
                PropertyDeclaration::TextSizeAdjust(value.clone())
            },
            PropertyDeclaration::TextUnderlineOffset(value) => {
                PropertyDeclaration::TextUnderlineOffset(value.clone())
            },
            PropertyDeclaration::Transform(value) => PropertyDeclaration::Transform(value.clone()),
            PropertyDeclaration::TransformOrigin(value) => {
                PropertyDeclaration::TransformOrigin(value.clone())
            },
            PropertyDeclaration::TransitionBehavior(value) => {
                PropertyDeclaration::TransitionBehavior(value.clone())
            },
            PropertyDeclaration::TransitionDelay(value) => {
                PropertyDeclaration::TransitionDelay(value.clone())
            },
            PropertyDeclaration::TransitionDuration(value) => {
                PropertyDeclaration::TransitionDuration(value.clone())
            },
            PropertyDeclaration::TransitionProperty(value) => {
                PropertyDeclaration::TransitionProperty(value.clone())
            },
            PropertyDeclaration::TransitionTimingFunction(value) => {
                PropertyDeclaration::TransitionTimingFunction(value.clone())
            },
            PropertyDeclaration::Translate(value) => PropertyDeclaration::Translate(value.clone()),
            PropertyDeclaration::ViewTransitionClass(value) => {
                PropertyDeclaration::ViewTransitionClass(value.clone())
            },
            PropertyDeclaration::ViewTransitionName(value) => {
                PropertyDeclaration::ViewTransitionName(value.clone())
            },
            PropertyDeclaration::ViewTransitionGroup(value) => {
                PropertyDeclaration::ViewTransitionGroup(value.clone())
            },
            PropertyDeclaration::ViewTransitionScope(value) => {
                PropertyDeclaration::ViewTransitionScope(*value)
            },
            PropertyDeclaration::WebkitBoxOrient(value) => {
                PropertyDeclaration::WebkitBoxOrient(*value)
            },
            PropertyDeclaration::WebkitLineClamp(value) => {
                PropertyDeclaration::WebkitLineClamp(value.clone())
            },
            PropertyDeclaration::WillChange(value) => {
                PropertyDeclaration::WillChange(value.clone())
            },
            PropertyDeclaration::WordSpacing(value) => {
                PropertyDeclaration::WordSpacing(value.clone())
            },
            PropertyDeclaration::XLang(value) => PropertyDeclaration::XLang(value.clone()),
            PropertyDeclaration::ZIndex(value) => PropertyDeclaration::ZIndex(value.clone()),
            PropertyDeclaration::Zoom(value) => PropertyDeclaration::Zoom(value.clone()),
            PropertyDeclaration::BorderImageSlice(value) => {
                PropertyDeclaration::BorderImageSlice(value.clone())
            },
            PropertyDeclaration::MaskBorderSlice(value) => {
                PropertyDeclaration::MaskBorderSlice(value.clone())
            },
            PropertyDeclaration::BorderImageWidth(value) => {
                PropertyDeclaration::BorderImageWidth(value.clone())
            },
            PropertyDeclaration::MaskBorderWidth(value) => {
                PropertyDeclaration::MaskBorderWidth(value.clone())
            },
            PropertyDeclaration::BorderImageOutset(value) => {
                PropertyDeclaration::BorderImageOutset(value.clone())
            },
            PropertyDeclaration::MaskBorderOutset(value) => {
                PropertyDeclaration::MaskBorderOutset(value.clone())
            },
            PropertyDeclaration::ObjectPosition(value) => {
                PropertyDeclaration::ObjectPosition(value.clone())
            },
            PropertyDeclaration::PerspectiveOrigin(value) => {
                PropertyDeclaration::PerspectiveOrigin(value.clone())
            },
            PropertyDeclaration::Fill(value) => PropertyDeclaration::Fill(value.clone()),
            PropertyDeclaration::Stroke(value) => PropertyDeclaration::Stroke(value.clone()),
            PropertyDeclaration::BdBarcodeCompositeContent(value) => {
                PropertyDeclaration::BdBarcodeCompositeContent(value.clone())
            },
            PropertyDeclaration::BdBarcodeContent(value) => {
                PropertyDeclaration::BdBarcodeContent(value.clone())
            },
            PropertyDeclaration::BdFirstPageSide(value) => {
                PropertyDeclaration::BdFirstPageSide(value.clone())
            },
            PropertyDeclaration::BdFirstPageSideView(value) => {
                PropertyDeclaration::BdFirstPageSideView(value.clone())
            },
            PropertyDeclaration::BdImageResolution(value) => {
                PropertyDeclaration::BdImageResolution(value.clone())
            },
            PropertyDeclaration::ImageResolution(value) => {
                PropertyDeclaration::ImageResolution(value.clone())
            },
            PropertyDeclaration::BdPageBleedMarkLength(value) => {
                PropertyDeclaration::BdPageBleedMarkLength(value.clone())
            },
            PropertyDeclaration::BdPageCropMarkLength(value) => {
                PropertyDeclaration::BdPageCropMarkLength(value.clone())
            },
            PropertyDeclaration::BdPdfCommentContents(value) => {
                PropertyDeclaration::BdPdfCommentContents(value.clone())
            },
            PropertyDeclaration::BdPdfCommentTitle(value) => {
                PropertyDeclaration::BdPdfCommentTitle(value.clone())
            },
            PropertyDeclaration::BdPdfFormFieldMkDownCaption(value) => {
                PropertyDeclaration::BdPdfFormFieldMkDownCaption(value.clone())
            },
            PropertyDeclaration::BdPdfFormFieldMkRolloverCaption(value) => {
                PropertyDeclaration::BdPdfFormFieldMkRolloverCaption(value.clone())
            },
            PropertyDeclaration::BdPdfFormFieldMkBackgroundColour(value) => {
                PropertyDeclaration::BdPdfFormFieldMkBackgroundColour(value.clone())
            },
            PropertyDeclaration::BdPdfFormFieldMkBorderColour(value) => {
                PropertyDeclaration::BdPdfFormFieldMkBorderColour(value.clone())
            },
            PropertyDeclaration::BdPdfFormFieldMkAlternateIcon(value) => {
                PropertyDeclaration::BdPdfFormFieldMkAlternateIcon(value.clone())
            },
            PropertyDeclaration::BdPdfFormFieldMkRolloverIcon(value) => {
                PropertyDeclaration::BdPdfFormFieldMkRolloverIcon(value.clone())
            },
            PropertyDeclaration::BdPdfTagExpanded(value) => {
                PropertyDeclaration::BdPdfTagExpanded(value.clone())
            },
            PropertyDeclaration::BdPdfTagTitle(value) => {
                PropertyDeclaration::BdPdfTagTitle(value.clone())
            },
            PropertyDeclaration::ColumnRuleColor(value) => {
                PropertyDeclaration::ColumnRuleColor(value.clone())
            },
            PropertyDeclaration::RowRuleColor(value) => {
                PropertyDeclaration::RowRuleColor(value.clone())
            },
            PropertyDeclaration::ColumnRuleStyle(value) => {
                PropertyDeclaration::ColumnRuleStyle(value.clone())
            },
            PropertyDeclaration::RowRuleStyle(value) => {
                PropertyDeclaration::RowRuleStyle(value.clone())
            },
            PropertyDeclaration::ColumnRuleWidth(value) => {
                PropertyDeclaration::ColumnRuleWidth(value.clone())
            },
            PropertyDeclaration::RowRuleWidth(value) => {
                PropertyDeclaration::RowRuleWidth(value.clone())
            },
            PropertyDeclaration::GridTemplateColumns(value) => {
                PropertyDeclaration::GridTemplateColumns(value.clone())
            },
            PropertyDeclaration::GridTemplateRows(value) => {
                PropertyDeclaration::GridTemplateRows(value.clone())
            },
            PropertyDeclaration::GridAutoColumns(value) => {
                PropertyDeclaration::GridAutoColumns(value.clone())
            },
            PropertyDeclaration::GridAutoRows(value) => {
                PropertyDeclaration::GridAutoRows(value.clone())
            },
            PropertyDeclaration::Order(value) => PropertyDeclaration::Order(value.clone()),
            PropertyDeclaration::ReadingOrder(value) => {
                PropertyDeclaration::ReadingOrder(value.clone())
            },
            PropertyDeclaration::Orphans(value) => PropertyDeclaration::Orphans(value.clone()),
            PropertyDeclaration::Widows(value) => PropertyDeclaration::Widows(value.clone()),
            PropertyDeclaration::FillOpacity(value) => {
                PropertyDeclaration::FillOpacity(value.clone())
            },
            PropertyDeclaration::StrokeOpacity(value) => {
                PropertyDeclaration::StrokeOpacity(value.clone())
            },
            PropertyDeclaration::ColumnHeight(value) => {
                PropertyDeclaration::ColumnHeight(value.clone())
            },
            PropertyDeclaration::ColumnWidth(value) => {
                PropertyDeclaration::ColumnWidth(value.clone())
            },
            PropertyDeclaration::ColumnGap(value) => PropertyDeclaration::ColumnGap(value.clone()),
            PropertyDeclaration::RowGap(value) => PropertyDeclaration::RowGap(value.clone()),
            PropertyDeclaration::BdPageCropMarkOffset(value) => {
                PropertyDeclaration::BdPageCropMarkOffset(value.clone())
            },
            PropertyDeclaration::BdPageMarksOffset(value) => {
                PropertyDeclaration::BdPageMarksOffset(value.clone())
            },
            PropertyDeclaration::BdPageRegistrationMarkOffset(value) => {
                PropertyDeclaration::BdPageRegistrationMarkOffset(value.clone())
            },
            PropertyDeclaration::BdPdfCommentCreatedate(value) => {
                PropertyDeclaration::BdPdfCommentCreatedate(value.clone())
            },
            PropertyDeclaration::BdPdfCommentDate(value) => {
                PropertyDeclaration::BdPdfCommentDate(value.clone())
            },
            PropertyDeclaration::BdPdfCommentModifydate(value) => {
                PropertyDeclaration::BdPdfCommentModifydate(value.clone())
            },
            PropertyDeclaration::BdPdfStampContents(value) => {
                PropertyDeclaration::BdPdfStampContents(value.clone())
            },
            PropertyDeclaration::BdPdfStampSubject(value) => {
                PropertyDeclaration::BdPdfStampSubject(value.clone())
            },
            PropertyDeclaration::BdPdfStampTitle(value) => {
                PropertyDeclaration::BdPdfStampTitle(value.clone())
            },
            PropertyDeclaration::BdPdfBlur(value) => PropertyDeclaration::BdPdfBlur(value.clone()),
            PropertyDeclaration::BdPdfCalculate(value) => {
                PropertyDeclaration::BdPdfCalculate(value.clone())
            },
            PropertyDeclaration::BdPdfFocus(value) => {
                PropertyDeclaration::BdPdfFocus(value.clone())
            },
            PropertyDeclaration::BdTextLinethroughColor(value) => {
                PropertyDeclaration::BdTextLinethroughColor(value.clone())
            },
            PropertyDeclaration::BdTextOverlineColor(value) => {
                PropertyDeclaration::BdTextOverlineColor(value.clone())
            },
            PropertyDeclaration::BdTextUnderlineColor(value) => {
                PropertyDeclaration::BdTextUnderlineColor(value.clone())
            },
            PropertyDeclaration::BdTextLinethroughStyle(value) => {
                PropertyDeclaration::BdTextLinethroughStyle(value.clone())
            },
            PropertyDeclaration::BdTextOverlineStyle(value) => {
                PropertyDeclaration::BdTextOverlineStyle(value.clone())
            },
            PropertyDeclaration::BdTextUnderlineStyle(value) => {
                PropertyDeclaration::BdTextUnderlineStyle(value.clone())
            },
            PropertyDeclaration::BdTextLinethroughThickness(value) => {
                PropertyDeclaration::BdTextLinethroughThickness(value.clone())
            },
            PropertyDeclaration::BdTextOverlineThickness(value) => {
                PropertyDeclaration::BdTextOverlineThickness(value.clone())
            },
            PropertyDeclaration::BdTextUnderlineThickness(value) => {
                PropertyDeclaration::BdTextUnderlineThickness(value.clone())
            },
            PropertyDeclaration::BorderImageSource(value) => {
                PropertyDeclaration::BorderImageSource(value.clone())
            },
            PropertyDeclaration::ListStyleImage(value) => {
                PropertyDeclaration::ListStyleImage(value.clone())
            },
            PropertyDeclaration::MaskBorderSource(value) => {
                PropertyDeclaration::MaskBorderSource(value.clone())
            },
            PropertyDeclaration::BdBarcodeFontSize(value) => {
                PropertyDeclaration::BdBarcodeFontSize(value.clone())
            },
            PropertyDeclaration::BdBarcodeSymbolWidth(value) => {
                PropertyDeclaration::BdBarcodeSymbolWidth(value.clone())
            },
            PropertyDeclaration::LineHeightStep(value) => {
                PropertyDeclaration::LineHeightStep(value.clone())
            },
            PropertyDeclaration::FlexGrow(value) => PropertyDeclaration::FlexGrow(value.clone()),
            PropertyDeclaration::FlexShrink(value) => {
                PropertyDeclaration::FlexShrink(value.clone())
            },
            PropertyDeclaration::StrokeMiterlimit(value) => {
                PropertyDeclaration::StrokeMiterlimit(value.clone())
            },
            PropertyDeclaration::MarkerEnd(value) => PropertyDeclaration::MarkerEnd(value.clone()),
            PropertyDeclaration::MarkerMid(value) => PropertyDeclaration::MarkerMid(value.clone()),
            PropertyDeclaration::MarkerStart(value) => {
                PropertyDeclaration::MarkerStart(value.clone())
            },
            PropertyDeclaration::GridColumnEnd(value) => {
                PropertyDeclaration::GridColumnEnd(value.clone())
            },
            PropertyDeclaration::GridColumnStart(value) => {
                PropertyDeclaration::GridColumnStart(value.clone())
            },
            PropertyDeclaration::GridRowEnd(value) => {
                PropertyDeclaration::GridRowEnd(value.clone())
            },
            PropertyDeclaration::GridRowStart(value) => {
                PropertyDeclaration::GridRowStart(value.clone())
            },
            PropertyDeclaration::BdListitemValue(value) => {
                PropertyDeclaration::BdListitemValue(value.clone())
            },
            PropertyDeclaration::BdTableBaseline(value) => {
                PropertyDeclaration::BdTableBaseline(value.clone())
            },
            PropertyDeclaration::BdTableColumnSpan(value) => {
                PropertyDeclaration::BdTableColumnSpan(value.clone())
            },
            PropertyDeclaration::BdTableRowSpan(value) => {
                PropertyDeclaration::BdTableRowSpan(value.clone())
            },
            PropertyDeclaration::BdPdfViewerPrintArea(value) => {
                PropertyDeclaration::BdPdfViewerPrintArea(value.clone())
            },
            PropertyDeclaration::BdPdfViewerPrintClip(value) => {
                PropertyDeclaration::BdPdfViewerPrintClip(value.clone())
            },
            PropertyDeclaration::BdPdfViewerViewArea(value) => {
                PropertyDeclaration::BdPdfViewerViewArea(value.clone())
            },
            PropertyDeclaration::BdPdfViewerViewClip(value) => {
                PropertyDeclaration::BdPdfViewerViewClip(value.clone())
            },
            PropertyDeclaration::ContainIntrinsicBlockSize(value) => {
                PropertyDeclaration::ContainIntrinsicBlockSize(value.clone())
            },
            PropertyDeclaration::ContainIntrinsicHeight(value) => {
                PropertyDeclaration::ContainIntrinsicHeight(value.clone())
            },
            PropertyDeclaration::ContainIntrinsicInlineSize(value) => {
                PropertyDeclaration::ContainIntrinsicInlineSize(value.clone())
            },
            PropertyDeclaration::ContainIntrinsicWidth(value) => {
                PropertyDeclaration::ContainIntrinsicWidth(value.clone())
            },
            PropertyDeclaration::MaxBlockSize(value) => {
                PropertyDeclaration::MaxBlockSize(value.clone())
            },
            PropertyDeclaration::MaxHeight(value) => PropertyDeclaration::MaxHeight(value.clone()),
            PropertyDeclaration::MaxInlineSize(value) => {
                PropertyDeclaration::MaxInlineSize(value.clone())
            },
            PropertyDeclaration::MaxWidth(value) => PropertyDeclaration::MaxWidth(value.clone()),
            PropertyDeclaration::FloodOpacity(value) => {
                PropertyDeclaration::FloodOpacity(value.clone())
            },
            PropertyDeclaration::Opacity(value) => PropertyDeclaration::Opacity(value.clone()),
            PropertyDeclaration::ShapeImageThreshold(value) => {
                PropertyDeclaration::ShapeImageThreshold(value.clone())
            },
            PropertyDeclaration::StopOpacity(value) => {
                PropertyDeclaration::StopOpacity(value.clone())
            },
            PropertyDeclaration::BdInsetInside(value) => {
                PropertyDeclaration::BdInsetInside(value.clone())
            },
            PropertyDeclaration::BdInsetOutside(value) => {
                PropertyDeclaration::BdInsetOutside(value.clone())
            },
            PropertyDeclaration::BdMarginAlt(value) => {
                PropertyDeclaration::BdMarginAlt(value.clone())
            },
            PropertyDeclaration::BdMarginInside(value) => {
                PropertyDeclaration::BdMarginInside(value.clone())
            },
            PropertyDeclaration::BdMarginOutside(value) => {
                PropertyDeclaration::BdMarginOutside(value.clone())
            },
            PropertyDeclaration::BdPdfAltText(value) => {
                PropertyDeclaration::BdPdfAltText(value.clone())
            },
            PropertyDeclaration::BdPdfTagActualText(value) => {
                PropertyDeclaration::BdPdfTagActualText(value.clone())
            },
            PropertyDeclaration::BdPdfTagAlt(value) => {
                PropertyDeclaration::BdPdfTagAlt(value.clone())
            },
            PropertyDeclaration::BdPdfTagLang(value) => {
                PropertyDeclaration::BdPdfTagLang(value.clone())
            },
            PropertyDeclaration::BdPdfTooltip(value) => {
                PropertyDeclaration::BdPdfTooltip(value.clone())
            },
            PropertyDeclaration::BdPageBleedBottom(value) => {
                PropertyDeclaration::BdPageBleedBottom(value.clone())
            },
            PropertyDeclaration::BdPageBleedLeft(value) => {
                PropertyDeclaration::BdPageBleedLeft(value.clone())
            },
            PropertyDeclaration::BdPageBleedMarkOffset(value) => {
                PropertyDeclaration::BdPageBleedMarkOffset(value.clone())
            },
            PropertyDeclaration::BdPageBleedRight(value) => {
                PropertyDeclaration::BdPageBleedRight(value.clone())
            },
            PropertyDeclaration::BdPageBleedTop(value) => {
                PropertyDeclaration::BdPageBleedTop(value.clone())
            },
            PropertyDeclaration::BdPageRegistrationMarkSize(value) => {
                PropertyDeclaration::BdPageRegistrationMarkSize(value.clone())
            },
            PropertyDeclaration::BdAnchor(value) => PropertyDeclaration::BdAnchor(value.clone()),
            PropertyDeclaration::BdPdfAttachmentDescription(value) => {
                PropertyDeclaration::BdPdfAttachmentDescription(value.clone())
            },
            PropertyDeclaration::BdPdfAttachmentMimeType(value) => {
                PropertyDeclaration::BdPdfAttachmentMimeType(value.clone())
            },
            PropertyDeclaration::BdPdfAttachmentName(value) => {
                PropertyDeclaration::BdPdfAttachmentName(value.clone())
            },
            PropertyDeclaration::BdPdfDestination(value) => {
                PropertyDeclaration::BdPdfDestination(value.clone())
            },
            PropertyDeclaration::BdPdfPageLabel(value) => {
                PropertyDeclaration::BdPdfPageLabel(value.clone())
            },
            PropertyDeclaration::BdPageColorbarOffset(value) => {
                PropertyDeclaration::BdPageColorbarOffset(value.clone())
            },
            PropertyDeclaration::Cx(value) => PropertyDeclaration::Cx(value.clone()),
            PropertyDeclaration::Cy(value) => PropertyDeclaration::Cy(value.clone()),
            PropertyDeclaration::OffsetDistance(value) => {
                PropertyDeclaration::OffsetDistance(value.clone())
            },
            PropertyDeclaration::BdPdfAuthor(value) => {
                PropertyDeclaration::BdPdfAuthor(value.clone())
            },
            PropertyDeclaration::BdPdfCreator(value) => {
                PropertyDeclaration::BdPdfCreator(value.clone())
            },
            PropertyDeclaration::BdPdfKeywords(value) => {
                PropertyDeclaration::BdPdfKeywords(value.clone())
            },
            PropertyDeclaration::BdPdfProducer(value) => {
                PropertyDeclaration::BdPdfProducer(value.clone())
            },
            PropertyDeclaration::BdPdfSubject(value) => {
                PropertyDeclaration::BdPdfSubject(value.clone())
            },
            PropertyDeclaration::BdPdfTitle(value) => {
                PropertyDeclaration::BdPdfTitle(value.clone())
            },
            PropertyDeclaration::BdPdfXmp(value) => PropertyDeclaration::BdPdfXmp(value.clone()),
            PropertyDeclaration::BdPdfViewerCenterWindow(value) => {
                PropertyDeclaration::BdPdfViewerCenterWindow(value.clone())
            },
            PropertyDeclaration::BdPdfViewerDisplayDocTitle(value) => {
                PropertyDeclaration::BdPdfViewerDisplayDocTitle(value.clone())
            },
            PropertyDeclaration::BdPdfViewerFitWindow(value) => {
                PropertyDeclaration::BdPdfViewerFitWindow(value.clone())
            },
            PropertyDeclaration::BdPdfViewerHideMenubar(value) => {
                PropertyDeclaration::BdPdfViewerHideMenubar(value.clone())
            },
            PropertyDeclaration::BdPdfViewerHideToolbar(value) => {
                PropertyDeclaration::BdPdfViewerHideToolbar(value.clone())
            },
            PropertyDeclaration::BdPdfViewerHideWindowUi(value) => {
                PropertyDeclaration::BdPdfViewerHideWindowUi(value.clone())
            },
            PropertyDeclaration::BdPdfViewerPickTrayByPdfSize(value) => {
                PropertyDeclaration::BdPdfViewerPickTrayByPdfSize(value.clone())
            },
            PropertyDeclaration::BorderBottomLeftRadius(value) => {
                PropertyDeclaration::BorderBottomLeftRadius(value.clone())
            },
            PropertyDeclaration::BorderBottomRightRadius(value) => {
                PropertyDeclaration::BorderBottomRightRadius(value.clone())
            },
            PropertyDeclaration::BorderEndEndRadius(value) => {
                PropertyDeclaration::BorderEndEndRadius(value.clone())
            },
            PropertyDeclaration::BorderEndStartRadius(value) => {
                PropertyDeclaration::BorderEndStartRadius(value.clone())
            },
            PropertyDeclaration::BorderStartEndRadius(value) => {
                PropertyDeclaration::BorderStartEndRadius(value.clone())
            },
            PropertyDeclaration::BorderStartStartRadius(value) => {
                PropertyDeclaration::BorderStartStartRadius(value.clone())
            },
            PropertyDeclaration::BorderTopLeftRadius(value) => {
                PropertyDeclaration::BorderTopLeftRadius(value.clone())
            },
            PropertyDeclaration::BorderTopRightRadius(value) => {
                PropertyDeclaration::BorderTopRightRadius(value.clone())
            },
            PropertyDeclaration::BdPageColorbarBottomLeft(value) => {
                PropertyDeclaration::BdPageColorbarBottomLeft(value.clone())
            },
            PropertyDeclaration::BdPageColorbarBottomRight(value) => {
                PropertyDeclaration::BdPageColorbarBottomRight(value.clone())
            },
            PropertyDeclaration::BdPageColorbarLeftBottom(value) => {
                PropertyDeclaration::BdPageColorbarLeftBottom(value.clone())
            },
            PropertyDeclaration::BdPageColorbarLeftTop(value) => {
                PropertyDeclaration::BdPageColorbarLeftTop(value.clone())
            },
            PropertyDeclaration::BdPageColorbarRightBottom(value) => {
                PropertyDeclaration::BdPageColorbarRightBottom(value.clone())
            },
            PropertyDeclaration::BdPageColorbarRightTop(value) => {
                PropertyDeclaration::BdPageColorbarRightTop(value.clone())
            },
            PropertyDeclaration::BdPageColorbarTopLeft(value) => {
                PropertyDeclaration::BdPageColorbarTopLeft(value.clone())
            },
            PropertyDeclaration::BdPageColorbarTopRight(value) => {
                PropertyDeclaration::BdPageColorbarTopRight(value.clone())
            },
            PropertyDeclaration::Bottom(value) => PropertyDeclaration::Bottom(value.clone()),
            PropertyDeclaration::InsetBlockEnd(value) => {
                PropertyDeclaration::InsetBlockEnd(value.clone())
            },
            PropertyDeclaration::InsetBlockStart(value) => {
                PropertyDeclaration::InsetBlockStart(value.clone())
            },
            PropertyDeclaration::InsetInlineEnd(value) => {
                PropertyDeclaration::InsetInlineEnd(value.clone())
            },
            PropertyDeclaration::InsetInlineStart(value) => {
                PropertyDeclaration::InsetInlineStart(value.clone())
            },
            PropertyDeclaration::Left(value) => PropertyDeclaration::Left(value.clone()),
            PropertyDeclaration::Right(value) => PropertyDeclaration::Right(value.clone()),
            PropertyDeclaration::Top(value) => PropertyDeclaration::Top(value.clone()),
            PropertyDeclaration::MarginBlockEnd(value) => {
                PropertyDeclaration::MarginBlockEnd(value.clone())
            },
            PropertyDeclaration::MarginBlockStart(value) => {
                PropertyDeclaration::MarginBlockStart(value.clone())
            },
            PropertyDeclaration::MarginBottom(value) => {
                PropertyDeclaration::MarginBottom(value.clone())
            },
            PropertyDeclaration::MarginInlineEnd(value) => {
                PropertyDeclaration::MarginInlineEnd(value.clone())
            },
            PropertyDeclaration::MarginInlineStart(value) => {
                PropertyDeclaration::MarginInlineStart(value.clone())
            },
            PropertyDeclaration::MarginLeft(value) => {
                PropertyDeclaration::MarginLeft(value.clone())
            },
            PropertyDeclaration::MarginRight(value) => {
                PropertyDeclaration::MarginRight(value.clone())
            },
            PropertyDeclaration::MarginTop(value) => PropertyDeclaration::MarginTop(value.clone()),
            PropertyDeclaration::OverflowClipMarginBlockEnd(value) => {
                PropertyDeclaration::OverflowClipMarginBlockEnd(value.clone())
            },
            PropertyDeclaration::OverflowClipMarginBlockStart(value) => {
                PropertyDeclaration::OverflowClipMarginBlockStart(value.clone())
            },
            PropertyDeclaration::OverflowClipMarginBottom(value) => {
                PropertyDeclaration::OverflowClipMarginBottom(value.clone())
            },
            PropertyDeclaration::OverflowClipMarginInlineEnd(value) => {
                PropertyDeclaration::OverflowClipMarginInlineEnd(value.clone())
            },
            PropertyDeclaration::OverflowClipMarginInlineStart(value) => {
                PropertyDeclaration::OverflowClipMarginInlineStart(value.clone())
            },
            PropertyDeclaration::OverflowClipMarginLeft(value) => {
                PropertyDeclaration::OverflowClipMarginLeft(value.clone())
            },
            PropertyDeclaration::OverflowClipMarginRight(value) => {
                PropertyDeclaration::OverflowClipMarginRight(value.clone())
            },
            PropertyDeclaration::OverflowClipMarginTop(value) => {
                PropertyDeclaration::OverflowClipMarginTop(value.clone())
            },
            PropertyDeclaration::BlockSize(value) => PropertyDeclaration::BlockSize(value.clone()),
            PropertyDeclaration::Height(value) => PropertyDeclaration::Height(value.clone()),
            PropertyDeclaration::InlineSize(value) => {
                PropertyDeclaration::InlineSize(value.clone())
            },
            PropertyDeclaration::MinBlockSize(value) => {
                PropertyDeclaration::MinBlockSize(value.clone())
            },
            PropertyDeclaration::MinHeight(value) => PropertyDeclaration::MinHeight(value.clone()),
            PropertyDeclaration::MinInlineSize(value) => {
                PropertyDeclaration::MinInlineSize(value.clone())
            },
            PropertyDeclaration::MinWidth(value) => PropertyDeclaration::MinWidth(value.clone()),
            PropertyDeclaration::Width(value) => PropertyDeclaration::Width(value.clone()),
            PropertyDeclaration::ColumnRuleInsetCapEnd(value) => {
                PropertyDeclaration::ColumnRuleInsetCapEnd(value.clone())
            },
            PropertyDeclaration::ColumnRuleInsetCapStart(value) => {
                PropertyDeclaration::ColumnRuleInsetCapStart(value.clone())
            },
            PropertyDeclaration::ColumnRuleInsetJunctionEnd(value) => {
                PropertyDeclaration::ColumnRuleInsetJunctionEnd(value.clone())
            },
            PropertyDeclaration::ColumnRuleInsetJunctionStart(value) => {
                PropertyDeclaration::ColumnRuleInsetJunctionStart(value.clone())
            },
            PropertyDeclaration::RowRuleInsetCapEnd(value) => {
                PropertyDeclaration::RowRuleInsetCapEnd(value.clone())
            },
            PropertyDeclaration::RowRuleInsetCapStart(value) => {
                PropertyDeclaration::RowRuleInsetCapStart(value.clone())
            },
            PropertyDeclaration::RowRuleInsetJunctionEnd(value) => {
                PropertyDeclaration::RowRuleInsetJunctionEnd(value.clone())
            },
            PropertyDeclaration::RowRuleInsetJunctionStart(value) => {
                PropertyDeclaration::RowRuleInsetJunctionStart(value.clone())
            },
            PropertyDeclaration::BorderBlockEndWidth(value) => {
                PropertyDeclaration::BorderBlockEndWidth(value.clone())
            },
            PropertyDeclaration::BorderBlockStartWidth(value) => {
                PropertyDeclaration::BorderBlockStartWidth(value.clone())
            },
            PropertyDeclaration::BorderBottomWidth(value) => {
                PropertyDeclaration::BorderBottomWidth(value.clone())
            },
            PropertyDeclaration::BorderInlineEndWidth(value) => {
                PropertyDeclaration::BorderInlineEndWidth(value.clone())
            },
            PropertyDeclaration::BorderInlineStartWidth(value) => {
                PropertyDeclaration::BorderInlineStartWidth(value.clone())
            },
            PropertyDeclaration::BorderLeftWidth(value) => {
                PropertyDeclaration::BorderLeftWidth(value.clone())
            },
            PropertyDeclaration::BorderRightWidth(value) => {
                PropertyDeclaration::BorderRightWidth(value.clone())
            },
            PropertyDeclaration::BorderTopWidth(value) => {
                PropertyDeclaration::BorderTopWidth(value.clone())
            },
            PropertyDeclaration::OutlineWidth(value) => {
                PropertyDeclaration::OutlineWidth(value.clone())
            },
            PropertyDeclaration::BdBarcodeLetterSpacing(value) => {
                PropertyDeclaration::BdBarcodeLetterSpacing(value.clone())
            },
            PropertyDeclaration::ScrollMarginBlockEnd(value) => {
                PropertyDeclaration::ScrollMarginBlockEnd(value.clone())
            },
            PropertyDeclaration::ScrollMarginBlockStart(value) => {
                PropertyDeclaration::ScrollMarginBlockStart(value.clone())
            },
            PropertyDeclaration::ScrollMarginBottom(value) => {
                PropertyDeclaration::ScrollMarginBottom(value.clone())
            },
            PropertyDeclaration::ScrollMarginInlineEnd(value) => {
                PropertyDeclaration::ScrollMarginInlineEnd(value.clone())
            },
            PropertyDeclaration::ScrollMarginInlineStart(value) => {
                PropertyDeclaration::ScrollMarginInlineStart(value.clone())
            },
            PropertyDeclaration::ScrollMarginLeft(value) => {
                PropertyDeclaration::ScrollMarginLeft(value.clone())
            },
            PropertyDeclaration::ScrollMarginRight(value) => {
                PropertyDeclaration::ScrollMarginRight(value.clone())
            },
            PropertyDeclaration::ScrollMarginTop(value) => {
                PropertyDeclaration::ScrollMarginTop(value.clone())
            },
            PropertyDeclaration::PaddingBlockEnd(value) => {
                PropertyDeclaration::PaddingBlockEnd(value.clone())
            },
            PropertyDeclaration::PaddingBlockStart(value) => {
                PropertyDeclaration::PaddingBlockStart(value.clone())
            },
            PropertyDeclaration::PaddingBottom(value) => {
                PropertyDeclaration::PaddingBottom(value.clone())
            },
            PropertyDeclaration::PaddingInlineEnd(value) => {
                PropertyDeclaration::PaddingInlineEnd(value.clone())
            },
            PropertyDeclaration::PaddingInlineStart(value) => {
                PropertyDeclaration::PaddingInlineStart(value.clone())
            },
            PropertyDeclaration::PaddingLeft(value) => {
                PropertyDeclaration::PaddingLeft(value.clone())
            },
            PropertyDeclaration::PaddingRight(value) => {
                PropertyDeclaration::PaddingRight(value.clone())
            },
            PropertyDeclaration::PaddingTop(value) => {
                PropertyDeclaration::PaddingTop(value.clone())
            },
            PropertyDeclaration::ShapeMargin(value) => {
                PropertyDeclaration::ShapeMargin(value.clone())
            },
            PropertyDeclaration::Rx(value) => PropertyDeclaration::Rx(value.clone()),
            PropertyDeclaration::Ry(value) => PropertyDeclaration::Ry(value.clone()),
            PropertyDeclaration::ScrollPaddingBlockEnd(value) => {
                PropertyDeclaration::ScrollPaddingBlockEnd(value.clone())
            },
            PropertyDeclaration::ScrollPaddingBlockStart(value) => {
                PropertyDeclaration::ScrollPaddingBlockStart(value.clone())
            },
            PropertyDeclaration::ScrollPaddingBottom(value) => {
                PropertyDeclaration::ScrollPaddingBottom(value.clone())
            },
            PropertyDeclaration::ScrollPaddingInlineEnd(value) => {
                PropertyDeclaration::ScrollPaddingInlineEnd(value.clone())
            },
            PropertyDeclaration::ScrollPaddingInlineStart(value) => {
                PropertyDeclaration::ScrollPaddingInlineStart(value.clone())
            },
            PropertyDeclaration::ScrollPaddingLeft(value) => {
                PropertyDeclaration::ScrollPaddingLeft(value.clone())
            },
            PropertyDeclaration::ScrollPaddingRight(value) => {
                PropertyDeclaration::ScrollPaddingRight(value.clone())
            },
            PropertyDeclaration::ScrollPaddingTop(value) => {
                PropertyDeclaration::ScrollPaddingTop(value.clone())
            },
            PropertyDeclaration::BackgroundColor(value) => {
                PropertyDeclaration::BackgroundColor(value.clone())
            },
            PropertyDeclaration::BdBarcodeColour(value) => {
                PropertyDeclaration::BdBarcodeColour(value.clone())
            },
            PropertyDeclaration::BorderBlockEndColor(value) => {
                PropertyDeclaration::BorderBlockEndColor(value.clone())
            },
            PropertyDeclaration::BorderBlockStartColor(value) => {
                PropertyDeclaration::BorderBlockStartColor(value.clone())
            },
            PropertyDeclaration::BorderBottomColor(value) => {
                PropertyDeclaration::BorderBottomColor(value.clone())
            },
            PropertyDeclaration::BorderInlineEndColor(value) => {
                PropertyDeclaration::BorderInlineEndColor(value.clone())
            },
            PropertyDeclaration::BorderInlineStartColor(value) => {
                PropertyDeclaration::BorderInlineStartColor(value.clone())
            },
            PropertyDeclaration::BorderLeftColor(value) => {
                PropertyDeclaration::BorderLeftColor(value.clone())
            },
            PropertyDeclaration::BorderRightColor(value) => {
                PropertyDeclaration::BorderRightColor(value.clone())
            },
            PropertyDeclaration::BorderTopColor(value) => {
                PropertyDeclaration::BorderTopColor(value.clone())
            },
            PropertyDeclaration::FillColor(value) => PropertyDeclaration::FillColor(value.clone()),
            PropertyDeclaration::FloodColor(value) => {
                PropertyDeclaration::FloodColor(value.clone())
            },
            PropertyDeclaration::LightingColor(value) => {
                PropertyDeclaration::LightingColor(value.clone())
            },
            PropertyDeclaration::OutlineColor(value) => {
                PropertyDeclaration::OutlineColor(value.clone())
            },
            PropertyDeclaration::StopColor(value) => PropertyDeclaration::StopColor(value.clone()),
            PropertyDeclaration::TextDecorationColor(value) => {
                PropertyDeclaration::TextDecorationColor(value.clone())
            },
            PropertyDeclaration::TextEmphasisColor(value) => {
                PropertyDeclaration::TextEmphasisColor(value.clone())
            },
            PropertyDeclaration::D(value) => PropertyDeclaration::D(value.clone()),
            PropertyDeclaration::X(value) => PropertyDeclaration::X(value.clone()),
            PropertyDeclaration::Y(value) => PropertyDeclaration::Y(value.clone()),
            PropertyDeclaration::R(value) => PropertyDeclaration::R(value.clone()),
            PropertyDeclaration::CSSWideKeyword(value) => {
                PropertyDeclaration::CSSWideKeyword(value.clone())
            },
            PropertyDeclaration::WithVariables(value) => {
                PropertyDeclaration::WithVariables(value.clone())
            },
            PropertyDeclaration::Custom(value) => PropertyDeclaration::Custom(value.clone()),
        }
    }

    fn pinned_css_rule_variant(rule: &CssRule) -> &'static str {
        match rule {
            CssRule::Style(_) => "Style",
            CssRule::Namespace(_) => "Namespace",
            CssRule::Import(_) => "Import",
            CssRule::Media(_) => "Media",
            CssRule::CustomMedia(_) => "CustomMedia",
            CssRule::Container(_) => "Container",
            CssRule::FontFace(_) => "FontFace",
            CssRule::FontFeatureValues(_) => "FontFeatureValues",
            CssRule::FontPaletteValues(_) => "FontPaletteValues",
            CssRule::CounterStyle(_) => "CounterStyle",
            CssRule::Keyframes(_) => "Keyframes",
            CssRule::Margin(_) => "Margin",
            CssRule::Footnote(_) => "Footnote",
            CssRule::Sidenote(_) => "Sidenote",
            CssRule::BdColour(_) => "BdColour",
            CssRule::ColorProfile(_) => "ColorProfile",
            CssRule::Region(_) => "Region",
            CssRule::Supports(_) => "Supports",
            CssRule::When(_) => "When",
            CssRule::Else(_) => "Else",
            CssRule::Page(_) => "Page",
            CssRule::Property(_) => "Property",
            CssRule::Document(_) => "Document",
            CssRule::LayerBlock(_) => "LayerBlock",
            CssRule::LayerStatement(_) => "LayerStatement",
            CssRule::Scope(_) => "Scope",
            CssRule::StartingStyle(_) => "StartingStyle",
            CssRule::PositionTry(_) => "PositionTry",
            CssRule::NestedDeclarations(_) => "NestedDeclarations",
        }
    }

    #[test]
    fn pinned_stylo_rule_variant_probe_is_exhaustive() {
        let probe: fn(&CssRule) -> &'static str = pinned_css_rule_variant;
        assert_eq!(
            std::mem::size_of_val(&probe),
            std::mem::size_of::<fn(&CssRule) -> &'static str>()
        );
    }

    #[test]
    fn pinned_stylo_declaration_variant_probe_is_exhaustive() {
        let probe: fn(&PropertyDeclaration) -> PropertyDeclaration = reconstruct_pinned_declaration;
        assert_eq!(
            std::mem::size_of_val(&probe),
            std::mem::size_of::<fn(&PropertyDeclaration) -> PropertyDeclaration>()
        );
    }

    #[test]
    fn cssom_interface_manifest_is_complete() {
        use CssomRuleInterfaceName as Name;
        use CssomRuleInterfaceParent as Parent;

        let expected = [
            (Name::CssRule, "CSSRule", Parent::None),
            (Name::Style, "CSSStyleRule", Parent::GroupingRule),
            (Name::Namespace, "CSSNamespaceRule", Parent::CssRule),
            (Name::Import, "CSSImportRule", Parent::CssRule),
            (Name::Media, "CSSMediaRule", Parent::ConditionRule),
            (Name::Supports, "CSSSupportsRule", Parent::ConditionRule),
            (Name::Container, "CSSContainerRule", Parent::ConditionRule),
            (Name::FontFace, "CSSFontFaceRule", Parent::CssRule),
            (
                Name::FontFeatureValues,
                "CSSFontFeatureValuesRule",
                Parent::CssRule,
            ),
            (
                Name::FontPaletteValues,
                "CSSFontPaletteValuesRule",
                Parent::CssRule,
            ),
            (Name::CounterStyle, "CSSCounterStyleRule", Parent::CssRule),
            (Name::Keyframes, "CSSKeyframesRule", Parent::CssRule),
            (Name::Keyframe, "CSSKeyframeRule", Parent::CssRule),
            (Name::Margin, "CSSMarginRule", Parent::CssRule),
            (Name::Page, "CSSPageRule", Parent::GroupingRule),
            (Name::Property, "CSSPropertyRule", Parent::CssRule),
            (Name::LayerBlock, "CSSLayerBlockRule", Parent::GroupingRule),
            (
                Name::LayerStatement,
                "CSSLayerStatementRule",
                Parent::CssRule,
            ),
            (Name::Scope, "CSSScopeRule", Parent::GroupingRule),
            (
                Name::StartingStyle,
                "CSSStartingStyleRule",
                Parent::GroupingRule,
            ),
            (Name::PositionTry, "CSSPositionTryRule", Parent::CssRule),
            (
                Name::NestedDeclarations,
                "CSSNestedDeclarations",
                Parent::CssRule,
            ),
            (Name::ColorProfile, "CSSColorProfileRule", Parent::CssRule),
        ];

        assert_eq!(CssomRuleInterfaceName::ALL.len(), expected.len());
        for (actual, (name, spelling, parent)) in CssomRuleInterfaceName::ALL.iter().zip(expected) {
            assert_eq!(
                (*actual, actual.as_str(), actual.parent()),
                (name, spelling, parent)
            );
        }
    }

    #[test]
    fn pinned_stylo_rule_fixtures_record_typed_variants_and_serialisation() {
        crate::context::initialise_required_servo_style_prefs();

        for (css, expected_variant) in [
            ("a, b:hover { color: red }", "Style"),
            (
                "@namespace svg url(http://www.w3.org/2000/svg);",
                "Namespace",
            ),
            (
                "@import url(sheet.css) layer(theme) supports(display: grid) screen;",
                "Import",
            ),
            ("@media print and (color) { a { color: red } }", "Media"),
            ("@custom-media --narrow (width < 30em);", "CustomMedia"),
            (
                "@container card (width > 1px) { a { color: red } }",
                "Container",
            ),
            (
                "@font-face { font-family: Fixture; src: url(f.woff2) }",
                "FontFace",
            ),
            (
                "@font-feature-values Fixture { @styleset { compact: 1 } }",
                "FontFeatureValues",
            ),
            (
                "@font-palette-values --fixture { font-family: Fixture; base-palette: 1 }",
                "FontPaletteValues",
            ),
            (
                "@counter-style fixture { system: cyclic; symbols: x }",
                "CounterStyle",
            ),
            (
                "@keyframes fixture { from, 25% { opacity: 0 } to { opacity: 1 } }",
                "Keyframes",
            ),
            (
                "@-bd-colour Spot { colour-values: red; alternate: rgb; }",
                "BdColour",
            ),
            (
                "@color-profile --press { src: url(press.icc); }",
                "ColorProfile",
            ),
            ("@region .flow > p { color: red }", "Region"),
            (
                "@supports selector(:has(*)) { a { color: red } }",
                "Supports",
            ),
            (
                "@property --size { syntax: '<length>'; inherits: false; initial-value: 1px }",
                "Property",
            ),
            ("@layer framework { a { color: red } }", "LayerBlock"),
            ("@layer reset, theme.components;", "LayerStatement"),
            ("@scope (.card) to (.limit) { a { color: red } }", "Scope"),
            ("@starting-style { a { opacity: 0 } }", "StartingStyle"),
            ("@position-try --fallback { inset: 1px }", "PositionTry"),
        ] {
            let (stylesheet, lock) =
                crate::context::parse_stylesheet_fragment(css, style::stylesheets::Origin::Author);
            let guard = lock.read();
            let contents = stylesheet.contents.read_with(&guard);
            let rules = contents.rules.read_with(&guard);
            let [rule] = rules.0.as_slice() else {
                panic!("fixture must parse as exactly one rule: {css}");
            };

            assert_eq!(pinned_css_rule_variant(rule), expected_variant, "{css}");
            assert!(!rule.to_css_string(&guard).is_empty(), "{css}");
        }

        // Document remains pinned by the exhaustive probe above but is
        // Gecko-only and therefore disabled by the current prefs.
        for css in ["@-moz-document url-prefix('') { a { color: red } }"] {
            let (stylesheet, lock) =
                crate::context::parse_stylesheet_fragment(css, style::stylesheets::Origin::Author);
            let guard = lock.read();
            assert!(
                stylesheet
                    .contents
                    .read_with(&guard)
                    .rules
                    .read_with(&guard)
                    .0
                    .is_empty(),
                "the current Servo configuration must reject {css}"
            );
        }
    }

    #[test]
    fn nested_rule_grammars_retain_typed_structure_and_when_chain_identity() {
        crate::context::initialise_required_servo_style_prefs();
        let css = "@page report:left { @top-left { content: 'folio' } @footnote { color: red } \
                   @-bd-sidenote notes { width: 10px } } \
                   @when supports(color: red) { a { color: red } } \
                   @else media(print) { a { color: blue } } @else { a { color: black } } \
                   @keyframes fade { from, 25% { opacity: 0 } to { opacity: 1 } } \
                   @scope (.outer) { color: red; @media print {} }";
        let (stylesheet, lock) =
            crate::context::parse_stylesheet_fragment(css, style::stylesheets::Origin::Author);
        let guard = lock.read();
        let contents = stylesheet.contents.read_with(&guard);
        let rules = contents.rules.read_with(&guard);

        assert_eq!(
            rules
                .0
                .iter()
                .map(pinned_css_rule_variant)
                .collect::<Vec<_>>(),
            ["Page", "When", "Else", "Else", "Keyframes", "Scope"]
        );

        let CssRule::Page(page) = &rules.0[0] else {
            unreachable!("the variant sequence was asserted above")
        };
        assert_eq!(
            page.read_with(&guard)
                .rules
                .read_with(&guard)
                .0
                .iter()
                .map(pinned_css_rule_variant)
                .collect::<Vec<_>>(),
            ["Margin", "Footnote", "Sidenote"]
        );

        let (CssRule::When(when), CssRule::Else(guarded), CssRule::Else(fallback)) =
            (&rules.0[1], &rules.0[2], &rules.0[3])
        else {
            unreachable!("the variant sequence was asserted above")
        };
        assert_eq!(
            (
                when.chain.len(),
                guarded.chain_position,
                fallback.chain_position
            ),
            (3, 1, 2)
        );
        assert!(std::ptr::eq(when.chain.as_ref(), guarded.chain.as_ref()));
        assert!(std::ptr::eq(when.chain.as_ref(), fallback.chain.as_ref()));
        assert!(guarded.condition.is_some());
        assert!(fallback.condition.is_none());

        let CssRule::Keyframes(keyframes) = &rules.0[4] else {
            unreachable!("the variant sequence was asserted above")
        };
        let keyframes = keyframes.read_with(&guard);
        assert_eq!(keyframes.keyframes.len(), 2);
        assert_eq!(
            keyframes.keyframes[0]
                .read_with(&guard)
                .selector
                .to_css_string(),
            "0%, 25%"
        );
        assert_eq!(
            keyframes.keyframes[1]
                .read_with(&guard)
                .selector
                .to_css_string(),
            "100%"
        );

        let CssRule::Scope(scope) = &rules.0[5] else {
            unreachable!("the variant sequence was asserted above")
        };
        assert_eq!(
            scope
                .rules
                .read_with(&guard)
                .0
                .iter()
                .map(pinned_css_rule_variant)
                .collect::<Vec<_>>(),
            ["NestedDeclarations", "Media"]
        );
    }

    #[test]
    fn font_feature_values_preserves_typed_duplicate_declarations() {
        let rule = ParsedCssRule::parse(
            "@font-feature-values bongo { @styleset { blah: 1; de-blah: 1; blah: 2; } }",
        )
        .expect("the fixture is a typed font feature values rule");

        assert_eq!(
            rule.legacy_type(),
            Some(CssomLegacyRuleType::FontFeatureValues)
        );
        assert_eq!(
            rule.as_str(),
            "@font-feature-values bongo {\n@styleset {\nblah: 1;\nde-blah: 1;\nblah: 2;\n}\n}"
        );
    }

    #[test]
    fn font_feature_values_exposes_typed_rule_data() {
        let rule = ParsedCssRule::parse(
            "@font-feature-values Fixture { @annotation { first: 2; } @styleset { choices: 4 9; } }",
        )
        .unwrap()
        .to_rule_node();
        assert!(
            rule.cssom_data().is_some(),
            "feature-value maps must survive parsing into the model"
        );
    }

    #[test]
    fn font_feature_values_retains_historical_forms_and_last_declarations() {
        use stylo_cssom_model::{FontFeatureKind, RuleCssomData};
        let rule = ParsedCssRule::parse("@font-feature-values Fixture { @annotation { a: 1; a: 2; A: 3; } @historical-forms { old: 1; bad: -1; extra: 1 2; } }").unwrap().to_rule_node();
        let RuleCssomData::FontFeatureValues { values } = rule.cssom_data().unwrap() else {
            panic!("typed font feature values required");
        };
        assert_eq!(values.map(FontFeatureKind::Annotation).len(), 2);
        assert_eq!(
            values.map(FontFeatureKind::Annotation).get("a"),
            Some(&[2][..])
        );
        assert_eq!(
            values.map(FontFeatureKind::HistoricalForms).get("old"),
            Some(&[1][..])
        );
        assert_eq!(values.map(FontFeatureKind::HistoricalForms).len(), 1);
        assert!(rule.serialization().contains("@historical-forms"));
    }

    #[test]
    fn font_feature_values_historical_forms_survive_ignored_siblings() {
        let rule = ParsedCssRule::parse(
            "@media all { @unknown {} @font-feature-values Fixture { @historical-forms { old: 1; } } }",
        )
        .unwrap()
        .to_rule_node();
        assert!(
            rule.payload().nested()[0]
                .serialization()
                .contains("old: 1;")
        );
    }

    #[test]
    fn font_face_descriptor_mutation_is_typed_and_transactional() {
        let rule =
            ParsedCssRule::parse("@font-face { src: url(valid.ttf); unicode-range: U+1357; }")
                .expect("the fixture is a typed font-face rule");
        let updated = super::mutate_non_style_rule_declaration(
            &rule.to_rule_node(),
            "unicode-range",
            "u+a?",
            crate::declaration_parser::CssomDeclarationPriority::Normal,
        )
        .expect("a valid Unicode range must produce a replacement rule");
        let declarations = updated
            .payload()
            .declaration_block()
            .expect("the font-face declaration block remains typed")
            .declarations();
        assert!(
            declarations
                .iter()
                .any(|declaration| declaration.name() == "unicode-range"
                    && declaration.value() == "U+A0-AF")
        );
        assert!(
            declarations
                .iter()
                .any(|declaration| declaration.name() == "src")
        );
        assert!(
            super::mutate_non_style_rule_declaration(
                &updated,
                "unicode-range",
                "u+efg",
                crate::declaration_parser::CssomDeclarationPriority::Normal,
            )
            .is_none()
        );
        assert!(
            super::mutate_non_style_rule_declaration(
                &updated,
                "unicode-range",
                "U+20; src: url(injected.ttf)",
                crate::declaration_parser::CssomDeclarationPriority::Normal,
            )
            .is_none()
        );
        let removed = super::mutate_non_style_rule_declaration(
            &updated,
            "unicode-range",
            "",
            crate::declaration_parser::CssomDeclarationPriority::Normal,
        )
        .expect("an empty value removes the descriptor");
        let declarations = removed
            .payload()
            .declaration_block()
            .expect("the font-face declaration block remains typed")
            .declarations();
        assert!(
            !declarations
                .iter()
                .any(|declaration| declaration.name() == "unicode-range")
        );
        assert!(
            declarations
                .iter()
                .any(|declaration| declaration.name() == "src")
        );
    }

    #[test]
    fn typed_rules_select_their_cssom_interface_names() {
        for (css, expected) in [
            ("a { color: red }", "CSSStyleRule"),
            ("@import url(x.css);", "CSSImportRule"),
            (
                "@namespace svg url(http://www.w3.org/2000/svg);",
                "CSSNamespaceRule",
            ),
            ("@media print {}", "CSSMediaRule"),
            ("@supports (color: red) {}", "CSSSupportsRule"),
            ("@container (width > 1px) {}", "CSSContainerRule"),
            (
                "@font-face { font-family: x; src: url(x) }",
                "CSSFontFaceRule",
            ),
            (
                "@font-feature-values x { @styleset { compact: 1 } }",
                "CSSFontFeatureValuesRule",
            ),
            (
                "@font-palette-values --x { font-family: x; base-palette: 1 }",
                "CSSFontPaletteValuesRule",
            ),
            (
                "@counter-style x { system: cyclic; symbols: x }",
                "CSSCounterStyleRule",
            ),
            ("@keyframes x { from { opacity: 0 } }", "CSSKeyframesRule"),
            ("@top-left {}", "CSSMarginRule"),
            ("@page {}", "CSSPageRule"),
            (
                "@property --x { syntax: '<length>'; inherits: false; initial-value: 1px }",
                "CSSPropertyRule",
            ),
            ("@layer x {}", "CSSLayerBlockRule"),
            ("@layer x;", "CSSLayerStatementRule"),
            ("@scope (.x) {}", "CSSScopeRule"),
            ("@starting-style {}", "CSSStartingStyleRule"),
            ("@position-try --x { inset: 1px }", "CSSPositionTryRule"),
            (
                "@color-profile --x { src: url(x.icc) }",
                "CSSColorProfileRule",
            ),
            ("@region .x { color: red }", "CSSRule"),
            ("@custom-media --x (color);", "CSSRule"),
            ("@-bd-colour Spot { colour-values: red }", "CSSRule"),
            ("@footnote {}", "CSSRule"),
            ("@-bd-sidenote notes {}", "CSSRule"),
            ("@when supports(color: red) {}", "CSSRule"),
            ("@else {}", "CSSRule"),
            ("@-moz-document url-prefix('') {}", "CSSRule"),
        ] {
            assert_eq!(
                super::cssom_rule_interface_name(css).as_str(),
                expected,
                "{css}"
            );
        }
    }

    #[test]
    fn page_rule_cssom_members_come_from_the_typed_rule_tree() {
        let rule = ParsedCssRule::parse(
            "@page named:left { margin: 10px 20px; @top-left { content: 'folio'; margin-left: 3px } }",
        )
        .expect("the fixture is a typed page rule");
        let margin = &rule
            .nested_rules()
            .expect("a page rule owns a nested rule list")[0];

        assert_eq!(rule.legacy_type(), Some(CssomLegacyRuleType::Page));
        assert_eq!(rule.page_selector_text(), Some("named:left"));
        assert_eq!(
            rule.declaration_names()
                .expect("page descriptors are exposed")
                .len(),
            4
        );
        assert_eq!(margin.interface_name().as_str(), "CSSMarginRule");
        assert_eq!(margin.margin_rule_name(), Some("top-left"));
        assert_eq!(
            margin.declaration_value("content").as_deref(),
            Some("\"folio\"")
        );
    }

    #[test]
    fn position_try_projection_preserves_css_wide_insets() {
        let rule =
            ParsedCssRule::parse("@position-try --pt { inset: unset; position-area: top left; }")
                .expect("a named fallback is valid");
        let node = rule.to_rule_node();
        let projection = node.projection_serialization();
        let reparsed = ParsedCssRule::parse(&projection).expect("the projected fallback is valid");
        for property in ["top", "right", "bottom", "left"] {
            assert_eq!(
                reparsed.declaration_value(property).as_deref(),
                Some("unset"),
                "{projection}"
            );
        }
        assert_eq!(
            reparsed.declaration_value("position-area").as_deref(),
            Some("left top")
        );
    }

    #[test]
    fn position_try_rule_preserves_its_typed_descriptor_block() {
        let rule =
            ParsedCssRule::parse("@position-try --fallback { top: 10px; color: red; left: 20px }")
                .expect("the fixture is a typed position-try rule");
        let node = rule.to_rule_node();
        let declarations = node
            .payload()
            .declaration_block()
            .expect("a position-try rule owns its descriptor block");

        assert_eq!(
            declarations.domain(),
            stylo_cssom_model::RuleDeclarationDomain::PositionTry
        );
        assert_eq!(
            declarations
                .declarations()
                .iter()
                .map(stylo_cssom_model::RuleDeclaration::name)
                .collect::<Vec<_>>(),
            ["top", "left"]
        );
    }

    #[test]
    fn anonymous_page_rule_serialisation_has_no_empty_selector_separator() {
        let rule = ParsedCssRule::parse("@page { @top-left { } }")
            .expect("the fixture is a typed anonymous page rule");

        assert_eq!(rule.as_str(), "@page { @top-left { } }");
    }

    #[test]
    fn style_rule_declaration_values_use_cssom_shorthand_serialisation() {
        let border = ParsedCssRule::parse(".a { border: 1px solid black; }")
            .expect("the border fixture is a typed style rule");
        let margin = ParsedCssRule::parse(".a { margin: initial; }")
            .expect("the margin fixture is a typed style rule");
        let all = ParsedCssRule::parse(".a { margin: initial; all: revert; }")
            .expect("the all fixture is a typed style rule");

        assert_eq!(
            border.declaration_value("border").as_deref(),
            Some("1px solid black")
        );
        assert_eq!(
            margin.declaration_value("margin").as_deref(),
            Some("initial")
        );
        assert_eq!(all.declaration_value("all").as_deref(), Some("revert"));
    }

    #[test]
    fn parsed_rule_mutation_keeps_pending_shorthand_longhands() {
        let rule =
            ParsedCssRule::parse(".a { transition:var(--timing);transition-delay:1s }").unwrap();
        let updated = crate::declaration_parser::mutate_style_rule_declaration(
            &rule.to_rule_node(),
            crate::declaration_parser::DeclarationPropertyInput::new("color", "red"),
            crate::declaration_parser::CssomDeclarationPriority::Normal,
        )
        .expect("parsed-rule pending longhands must survive a declaration mutation");
        let block = updated.payload().declaration_block().unwrap();
        assert_eq!(
            block
                .declarations()
                .iter()
                .find(|value| value.name() == "transition-duration")
                .map(stylo_cssom_model::RuleDeclaration::value),
            Some("")
        );
        assert_eq!(
            block
                .declarations()
                .iter()
                .find(|value| value.name() == "transition-delay")
                .map(stylo_cssom_model::RuleDeclaration::value),
            Some("1s")
        );
    }

    #[test]
    fn page_rule_cssom_mutations_preserve_their_typed_context() {
        let rule = ParsedCssRule::parse("@page named { margin: 1px; @top-left { content: 'x' } }")
            .expect("the fixture is a typed page rule");
        let updated = super::mutate_non_style_rule_declaration(
            &rule.to_rule_node(),
            "margin",
            "auto",
            crate::declaration_parser::CssomDeclarationPriority::Normal,
        )
        .expect("a page-context margin value is valid");
        let declarations = updated
            .payload()
            .declaration_block()
            .expect("the page declaration block remains typed");

        assert_eq!(declarations.declarations().len(), 4);
        assert_eq!(
            declarations
                .declarations()
                .iter()
                .find(|declaration| declaration.name() == "margin-left")
                .map(stylo_cssom_model::RuleDeclaration::value),
            Some("auto")
        );
        let removed = super::mutate_non_style_rule_declaration(
            &updated,
            "margin",
            "",
            crate::declaration_parser::CssomDeclarationPriority::Normal,
        )
        .expect("removing a shorthand keeps a valid page rule");
        assert!(
            removed
                .payload()
                .declaration_block()
                .expect("the page declaration block remains typed")
                .declarations()
                .is_empty()
        );
        assert!(rule.with_page_selector_text("1").is_none());
        assert!(rule.with_page_selector_text("--a").is_none());
        assert_eq!(
            rule.with_page_selector_text("")
                .expect("an empty page selector is valid")
                .page_selector_text(),
            Some("")
        );
        assert!(rule.with_page_nested_rules(["@media print {}"]).is_none());
        assert_eq!(
            ParsedCssRule::parse_page_child("@top-center {}")
                .expect("a top-center margin rule is valid in page context")
                .margin_rule_name(),
            Some("top-center")
        );
        let orientation = ParsedCssRule::parse(
            "@page named { page-orientation:rotate-right; page-orientation:initial; page-orientation:none }",
        )
        .expect("the fixture is a typed page rule");
        assert_eq!(
            orientation.declaration_value("page-orientation").as_deref(),
            Some("rotate-right")
        );
    }

    #[test]
    fn conditional_rules_retain_typed_cssom_conditions() {
        let media = ParsedCssRule::parse("@media print { a { color: red } }")
            .expect("the media control must parse as a typed rule");
        let media = media
            .conditional_rule()
            .expect("a media rule must expose a conditional grouping value");
        assert_eq!(media.condition_text(), "print");

        let supports = ParsedCssRule::parse("@supports (color: red) { a { color: green } }")
            .expect("the supports condition must parse as a typed rule");
        let supports = supports
            .conditional_rule()
            .expect("a supports rule must expose a conditional grouping value");
        assert_eq!(supports.condition_text(), "(color: red)");

        assert!(ParsedCssRule::parse("@supports {}").is_none());
        assert!(!ParsedCssRule::retain_scanned_rule("@supports {}"));
        assert!(
            ParsedCssRule::parse("@property --x { syntax: <color>; inherits: false }").is_none()
        );
        assert!(!ParsedCssRule::retain_scanned_rule(
            "@property --x { syntax: <color>; inherits: false }"
        ));
        for rule in [
            "@charset \"utf-8\";",
            "@keyframes initial {}",
            "@scope () {}",
            "@counter-style none {}",
            "@starting-style div {}",
        ] {
            assert!(!ParsedCssRule::retain_scanned_rule(rule), "{rule}");
        }
        assert!(ParsedCssRule::retain_scanned_rule("@unknown {}"));
        for rule in [
            "@ import 'red.css';",
            "@1import 'red.css';",
            "@-1import 'red.css';",
        ] {
            assert!(!ParsedCssRule::retain_scanned_rule(rule), "{rule}");
        }
    }

    #[test]
    fn container_rules_retain_every_typed_condition() {
        let rule = ParsedCssRule::parse("@container (width > 300px), Name (width < 1000px) {}")
            .expect("the container rule must parse");
        let conditions = rule
            .container_conditions()
            .expect("a container rule must expose its conditions")
            .collect::<Vec<_>>();

        assert_eq!(conditions.len(), 2);
        assert_eq!(
            (conditions[0].name(), conditions[0].query()),
            ("", "(width > 300px)")
        );
        assert_eq!(
            (conditions[1].name(), conditions[1].query()),
            ("Name", "(width < 1000px)")
        );
        assert_eq!(rule.container_condition(), Some(("", "")));
        assert_eq!(
            rule.condition_text(),
            Some("(width > 300px), Name (width < 1000px)")
        );

        let name_only = ParsedCssRule::parse("@container Name {}")
            .expect("a name-only container condition must parse");
        assert_eq!(name_only.container_condition(), Some(("Name", "")));
    }

    #[test]
    fn grouping_rules_retain_typed_recursive_children_and_members() {
        let scope = ParsedCssRule::parse(
            "@scope (.outer) { color:red; @scope (.inner) to (.limit) { \
             @container card (width > 1px) { a { color:green } } } }",
        )
        .expect("the scope fixture must parse as one typed rule");
        let (start, end) = scope.scope_bounds().expect("a scope rule has typed bounds");
        let children = scope
            .nested_rules()
            .expect("a scope rule has typed children");

        assert_eq!((start, end), (Some(".outer"), None));
        assert_eq!(children.len(), 2);
        assert_eq!(
            children[0].interface_name().as_str(),
            "CSSNestedDeclarations"
        );
        assert_eq!(
            children[0].declaration_value("color").as_deref(),
            Some("red")
        );
        let inner = &children[1];
        assert_eq!(inner.scope_bounds(), Some((Some(".inner"), Some(".limit"))));
        let container = &inner.nested_rules().expect("the inner scope has a child")[0];
        assert_eq!(
            container.container_condition(),
            Some(("card", "(width > 1px)"))
        );
        assert_eq!(
            container.nested_rules().expect("the container has a child")[0]
                .interface_name()
                .as_str(),
            "CSSStyleRule"
        );
    }

    #[test]
    fn cascade_layer_members_come_from_the_typed_rule_tree() {
        let block = ParsedCssRule::parse("@layer outer { @layer foo.bar {} }")
            .expect("the named layer block must parse");
        let nested = &block.nested_rules().expect("the layer block owns rules")[0];
        let statement = ParsedCssRule::parse("@layer alpha, beta.gamma;")
            .expect("the layer statement must parse");
        let anonymous =
            ParsedCssRule::parse("@layer {}").expect("the anonymous layer block must parse");

        assert_eq!(block.layer_block_name(), Some("outer"));
        assert_eq!(nested.layer_block_name(), Some("foo.bar"));
        assert_eq!(
            statement.layer_statement_names(),
            Some(["alpha".to_owned(), "beta.gamma".to_owned()].as_slice())
        );
        assert_eq!(anonymous.layer_block_name(), Some(""));
    }

    #[test]
    fn import_layer_name_preserves_the_standard_three_states() {
        let absent =
            ParsedCssRule::parse("@import url(a.css);").expect("the import rule must parse");
        let anonymous = ParsedCssRule::parse("@import url(a.css) layer;")
            .expect("the anonymous import layer must parse");
        let named = ParsedCssRule::parse(
            "@import url(a.css) layer(foo.bar) supports(display: grid) screen;",
        )
        .expect("the qualified named import must parse");

        assert_eq!(absent.import_layer_name(), Some(CssomImportLayerName::Null));
        assert_eq!(
            anonymous.import_layer_name(),
            Some(CssomImportLayerName::String(""))
        );
        assert_eq!(
            named.import_layer_name(),
            Some(CssomImportLayerName::String("foo.bar"))
        );
        let node = named.to_rule_node();
        let Some(stylo_cssom_model::RuleCssomData::Import { request }) = node.cssom_data() else {
            panic!("the import node must retain its typed request");
        };
        assert_eq!(request.url(), "a.css");
        assert_eq!(request.supports(), Some("(display: grid)"));
        assert_eq!(request.media(), Some("screen"));
        assert_eq!(request.cors(), None);
        assert_eq!(request.integrity(), None);
        assert_eq!(request.referrer_policy(), None);
    }

    #[test]
    fn rule_shorthand_priority_does_not_require_serializable_values() {
        use stylo_cssom_model::{
            StyleDocumentHandle, StyleOrigin, StyleSheetCandidate, StyleSheetSourceContext,
            StyleState,
        };

        for (shorthand, css) in [
            (
                "margin",
                "margin:1px!important;margin-top:initial!important",
            ),
            (
                "container",
                "container-name:initial!important;container-type:inline-size!important",
            ),
            (
                "position-try",
                "position-try-order:initial!important;position-try-fallbacks:none!important",
            ),
        ] {
            let rule = ParsedCssRule::parse(&format!("a{{{css}}}"))
                .expect("the longhand declarations must parse");
            let document = StyleDocumentHandle::allocate();
            let mut state = StyleState::new(document);
            let sheet = state
                .create_stylesheet(StyleSheetCandidate::new(
                    StyleSheetSourceContext::inline(
                        document,
                        StyleOrigin::Author,
                        "about:blank".into(),
                    ),
                    [rule.to_rule_node()],
                ))
                .expect("the rule must bind to its stylesheet");
            let block = sheet.top_list().rule(0).unwrap().block().unwrap();
            assert!(
                super::rule_block_declaration_is_important(&block, shorthand),
                "{shorthand}"
            );
        }
    }

    #[test]
    fn import_media_mutation_preserves_its_typed_request_prelude() {
        let parsed = ParsedCssRule::parse(
            r#"@import url("child.css" referrer-policy(no-referrer) integrity("sha256-fixture") cross-origin(use-credentials)) layer(foo.bar) supports(display: grid) screen;"#,
        )
        .expect("the import request must parse");
        let node = parsed.to_rule_node();
        let Some(stylo_cssom_model::RuleCssomData::Import { request: original }) =
            node.cssom_data()
        else {
            panic!("the import node must retain its typed request");
        };
        for media in ["print", ""] {
            let updated = node
                .clone()
                .with_cssom_media_condition(media)
                .expect("an import rule accepts a media-list mutation");
            let Some(stylo_cssom_model::RuleCssomData::Import { request }) = updated.cssom_data()
            else {
                panic!("the mutated import must retain its typed request");
            };
            assert_eq!(request.url(), original.url());
            assert_eq!(request.cors(), original.cors());
            assert_eq!(request.integrity(), original.integrity());
            assert_eq!(request.referrer_policy(), original.referrer_policy());
            assert_eq!(request.layer(), original.layer());
            assert_eq!(request.supports(), original.supports());
            assert_eq!(request.media(), (!media.is_empty()).then_some(media));
            let suffix = if media.is_empty() { "" } else { " print" };
            assert_eq!(
                updated.serialization(),
                format!(
                    r#"@import url("child.css" cross-origin(use-credentials) integrity("sha256-fixture") referrer-policy(no-referrer)) layer(foo.bar) supports(display: grid){suffix};"#
                ),
            );
        }
    }

    #[test]
    fn nested_declaration_mutation_preserves_ordinary_property_rules() {
        use crate::declaration_parser::CssomDeclarationPriority::{Important, Normal};

        let rule =
            crate::parse_nested_declarations_input(super::RuleInput::new("font-size: 20px;"))
                .expect("a declaration-only rule must parse");
        let changed = super::mutate_non_style_rule_declaration(&rule, "color", "green", Important)
            .expect("nested declarations accept ordinary CSS property mutation");
        let block = changed
            .payload()
            .declaration_block()
            .expect("the declaration block must remain");
        assert_eq!(
            block.domain(),
            stylo_cssom_model::RuleDeclarationDomain::Nested
        );
        assert!(block.declarations().iter().any(|declaration| {
            declaration.name() == "color"
                && declaration.value() == "green"
                && declaration.important()
        }));
        assert!(
            super::mutate_non_style_rule_declaration(&changed, "color", "invalid-colour", Normal)
                .is_none()
        );

        let removed = super::mutate_non_style_rule_declaration(&changed, "color", "", Normal)
            .expect("nested declaration removal must use the same grammar");
        assert_eq!(
            removed.grammar(),
            stylo_cssom_model::RuleGrammar::NestedDeclarations
        );
        assert_eq!(removed.serialization(), "font-size: 20px;");
    }

    #[test]
    fn selector_replacement_uses_the_rules_namespace_context() {
        let rules = ParsedCssRule::parse_stylesheet(concat!(
            "@namespace url(http://www.w3.org/1999/xhtml);",
            "@namespace svg url(http://www.w3.org/2000/svg);",
            "svg|*.old { color: blue !important; }",
        ));
        let rule = rules
            .last()
            .expect("the namespaced style rule must parse")
            .to_rule_node();
        let namespaces = rule
            .payload()
            .declaration_block()
            .expect("style declarations must remain")
            .namespaces();
        let replacement = super::replace_rule_selector(&rule, "svg|*.new  ", namespaces)
            .expect("a declared namespace remains valid during selector replacement");

        assert!(matches!(
            replacement.cssom_data(),
            Some(stylo_cssom_model::RuleCssomData::Style { selector }) if selector.as_ref() == "svg|*.new"
        ));
        assert_eq!(
            replacement.payload().declaration_block(),
            rule.payload().declaration_block()
        );
        for invalid in [
            "undeclared|*.new",
            "svg|*.new, undeclared|*",
            "svg|*.new {}",
        ] {
            assert!(
                super::replace_rule_selector(&replacement, invalid, namespaces).is_none(),
                "{invalid}"
            );
        }
    }

    #[test]
    fn selector_replacement_rebuilds_nested_rules_from_the_typed_tree() {
        let parsed = ParsedCssRule::parse(
            ".a { @scope (&) { :scope .x { .unused {} background-color: green; } } }",
        )
        .expect("the nested style rule must parse");
        let namespaces = stylo_cssom_model::RuleNamespaceContext::default();
        let updated = super::replace_rule_selector(&parsed.to_rule_node(), ".b", &namespaces)
            .expect("a style rule accepts a replacement selector");
        let source = updated.serialization();

        assert!(source.starts_with(".b { @scope (&) {"), "{source}");
        assert!(source.contains("background-color: green;"), "{source}");
        assert!(matches!(
            updated.cssom_data(),
            Some(stylo_cssom_model::RuleCssomData::Style { selector }) if selector.as_ref() == ".b"
        ));
        assert_eq!(updated.payload().nested().len(), 1);

        let nested = ParsedCssRule::parse("& .a1 { color: green; }")
            .expect("the nested selector must parse");
        let nested_source =
            super::replace_rule_selector(&nested.to_rule_node(), ".a2", &namespaces)
                .expect("the nested style rule accepts a replacement selector");
        let nested_source = nested_source.serialization();
        assert!(nested_source.starts_with("& .a2 {"), "{nested_source}");
    }

    #[test]
    fn native_source_spans_keep_duplicate_rules_after_a_rejected_sibling() {
        let parsed = ParsedCssRule::parse(
            "@media all { @unsupported {} .same { float:-ro-top } .same { float:-ro-bottom } }",
        )
        .unwrap();
        let node = parsed.to_rule_node();
        let children = node.payload().nested();
        assert_eq!(children.len(), 2);
        assert_eq!(
            children[0].projection_serialization(),
            ".same { float:-ro-top }"
        );
        assert_eq!(
            children[1].projection_serialization(),
            ".same { float:-ro-bottom }"
        );
    }

    #[test]
    fn native_declaration_run_source_spans_keep_all_members_before_the_next_rule() {
        let parsed =
            ParsedCssRule::parse(".parent { .first {} left:1px; right:var(--right); .last {} }")
                .unwrap();
        let node = parsed.to_rule_node();
        let children = node.payload().nested();
        assert_eq!(children.len(), 3);
        assert_eq!(
            children[1].grammar(),
            stylo_cssom_model::RuleGrammar::NestedDeclarations
        );
        let source = children[1].projection_serialization();
        assert!(source.contains("left:1px;") && source.contains("right:var(--right);"));
        assert!(!source.contains(".last"));
    }

    #[test]
    fn container_query_lists_parse_transactionally_and_serialize_canonically() {
        let list = super::ParsedContainerQueryList::parse(
            "(MIN-WIDTH:  300px), style-container style(--state: ready)",
        )
        .expect("both container conditions are valid");

        assert_eq!(
            list.serialization(),
            "(min-width: 300px), style-container style(--state: ready)"
        );
        assert!(super::ParsedContainerQueryList::parse("").is_none());
        assert!(super::ParsedContainerQueryList::parse("???").is_none());
        assert!(super::ParsedContainerQueryList::parse("(width > 100px),").is_none());
        assert!(super::ParsedContainerQueryList::parse(",(width > 100px)").is_none());
    }
}
