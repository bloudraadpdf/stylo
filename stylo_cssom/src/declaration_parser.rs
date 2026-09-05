use std::sync::Arc;

use cssparser::{Parser, ParserInput, Token, parse_important};
use style::values::specified::{
    ContainerName, ContainerType,
    position::{PositionTryFallbacks, PositionTryOrder},
};
use style::{
    parser::{Parse, ParserContext},
    properties::{
        CSSWideKeyword, Importance, PropertyDeclaration, PropertyId, SourcePropertyDeclaration,
        declaration_block::{
            SourcePropertyDeclarationUpdate, parse_one_declaration_into, parse_style_attribute,
        },
    },
    stylesheets::{CssRuleType, Namespaces, Origin, UrlExtraData},
};
use style_traits::ToCss;
use style_traits::{ParsingMode, StyleParseErrorKind};

use crate::{
    context::ABOUT_BLANK, style_fragment_parser::parse_style_fragment_with as parse_fragment_with,
};

pub mod compatibility;
pub mod mutation;

#[derive(Clone, Copy, Debug)]
pub struct DeclarationInput<'a>(&'a str);

impl<'a> DeclarationInput<'a> {
    #[must_use]
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DeclarationPropertyInput<'a> {
    property: &'a str,
    value: &'a str,
}

impl<'a> DeclarationPropertyInput<'a> {
    #[must_use]
    pub const fn new(property: &'a str, value: &'a str) -> Self {
        Self { property, value }
    }
}

/// Return whether a CSS component value contains a container-relative length.
#[must_use]
#[allow(missing_debug_implementations)]
pub struct InlineStyleBlock(style::properties::declaration_block::PropertyDeclarationBlock);

impl InlineStyleBlock {
    pub const fn as_typed(
        &self,
    ) -> &style::properties::declaration_block::PropertyDeclarationBlock {
        &self.0
    }
}

#[allow(missing_debug_implementations)]
pub struct CssomDeclarationBlock(style::properties::declaration_block::PropertyDeclarationBlock);

impl CssomDeclarationBlock {
    pub const fn as_typed(
        &self,
    ) -> &style::properties::declaration_block::PropertyDeclarationBlock {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssomDeclarationContext {
    Style,
    Keyframe,
    Page,
    Margin,
}

impl CssomDeclarationContext {
    pub const fn rule_type(self) -> CssRuleType {
        match self {
            Self::Keyframe => CssRuleType::Keyframe,
            Self::Page => CssRuleType::Page,
            // A page-margin box accepts ordinary CSS properties. Stylo's
            // `Margin` parser context is reserved for its internal restricted
            // declaration table, so CSSOM mutations use the typed style-property
            // grammar while the enclosing rule retains the margin role.
            Self::Style | Self::Margin => CssRuleType::Style,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SpecifiedContainerShorthand {
    CssWide(CSSWideKeyword),
    Longhands {
        name: ContainerName,
        container_type: ContainerType,
    },
}

impl SpecifiedContainerShorthand {
    fn parse(css: &str) -> Option<Self> {
        parse_fragment_with(css, |context, input| {
            if let Ok(keyword) = input.try_parse(|input| {
                let ident = input.expect_ident()?;
                CSSWideKeyword::from_ident(ident).map_err(|()| {
                    input.new_custom_error::<StyleParseErrorKind<'_>, StyleParseErrorKind<'_>>(
                        StyleParseErrorKind::UnspecifiedError,
                    )
                })
            }) {
                return Ok(Self::CssWide(keyword));
            }

            let name = ContainerName::parse(context, input)?;
            let container_type = if input.try_parse(|input| input.expect_delim('/')).is_ok() {
                ContainerType::parse(context, input)?
            } else {
                ContainerType::NORMAL
            };
            Ok(Self::Longhands {
                name,
                container_type,
            })
        })
    }

    fn declarations(self) -> Option<SourcePropertyDeclaration> {
        match self {
            Self::CssWide(keyword) => {
                css_wide_longhand_declarations(&["container-name", "container-type"], keyword)
            },
            Self::Longhands {
                name,
                container_type,
            } => {
                let mut declarations = SourcePropertyDeclaration::default();
                declarations.push(PropertyDeclaration::ContainerName(name));
                declarations.push(PropertyDeclaration::ContainerType(container_type));
                Some(declarations)
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum SpecifiedPositionTryShorthand {
    CssWide(CSSWideKeyword),
    Longhands {
        order: PositionTryOrder,
        fallbacks: PositionTryFallbacks,
    },
}

impl SpecifiedPositionTryShorthand {
    fn parse(css: &str) -> Option<Self> {
        if let Ok(keyword) = CSSWideKeyword::from_ident(css.trim()) {
            return Some(Self::CssWide(keyword));
        }
        parse_fragment_with(css, |context, input| {
            let order = input
                .try_parse(PositionTryOrder::parse)
                .unwrap_or_else(|_| PositionTryOrder::normal());
            let fallbacks = PositionTryFallbacks::parse(context, input)?;
            Ok(Self::Longhands { order, fallbacks })
        })
    }

    fn declarations(self) -> Option<SourcePropertyDeclaration> {
        match self {
            Self::CssWide(keyword) => css_wide_longhand_declarations(
                &["position-try-order", "position-try-fallbacks"],
                keyword,
            ),
            Self::Longhands { order, fallbacks } => {
                let mut declarations = SourcePropertyDeclaration::default();
                declarations.push(PropertyDeclaration::PositionTryOrder(order));
                declarations.push(PropertyDeclaration::PositionTryFallbacks(fallbacks));
                Some(declarations)
            },
        }
    }
}

pub fn position_try_shorthand_is_valid(value: &str) -> bool {
    SpecifiedPositionTryShorthand::parse(value).is_some()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionTryShorthandLonghands {
    order: String,
    fallbacks: String,
}

impl PositionTryShorthandLonghands {
    pub fn order(&self) -> &str {
        &self.order
    }

    pub fn fallbacks(&self) -> &str {
        &self.fallbacks
    }
}

pub fn position_try_shorthand_longhands(value: &str) -> Option<PositionTryShorthandLonghands> {
    Some(match SpecifiedPositionTryShorthand::parse(value)? {
        SpecifiedPositionTryShorthand::CssWide(keyword) => PositionTryShorthandLonghands {
            order: keyword.to_str().to_owned(),
            fallbacks: keyword.to_str().to_owned(),
        },
        SpecifiedPositionTryShorthand::Longhands { order, fallbacks } => {
            PositionTryShorthandLonghands {
                order: order.to_css_string(),
                fallbacks: fallbacks.to_css_string(),
            }
        },
    })
}

fn css_wide_longhand_declarations(
    properties: &[&str],
    keyword: CSSWideKeyword,
) -> Option<SourcePropertyDeclaration> {
    let mut declarations = SourcePropertyDeclaration::default();
    for property in properties {
        let id = PropertyId::parse_enabled_for_all_content(property).ok()?;
        let url_data: UrlExtraData = ABOUT_BLANK.clone().into();
        let mut longhand = SourcePropertyDeclaration::default();
        parse_one_declaration_into(
            &mut longhand,
            id,
            keyword.to_str(),
            Origin::Author,
            &url_data,
            None,
            ParsingMode::DEFAULT,
            selectors::matching::QuirksMode::NoQuirks,
            CssRuleType::Style,
        )
        .ok()?;
        for declaration in longhand.drain().declarations {
            declarations.push(declaration);
        }
    }
    Some(declarations)
}

fn apply_parsed_shorthand(
    block: &mut style::properties::declaration_block::PropertyDeclarationBlock,
    declarations: Option<SourcePropertyDeclaration>,
    priority: CssomDeclarationPriority,
) -> bool {
    declarations.is_some_and(|declarations| {
        declaration_block_apply_declarations(block, declarations, priority)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CssomDeclarationPriority {
    Normal,
    Important,
}

impl CssomDeclarationPriority {
    pub fn parse(value: &str) -> Option<Self> {
        if value.is_empty() {
            Some(Self::Normal)
        } else if value.eq_ignore_ascii_case("important") {
            Some(Self::Important)
        } else {
            None
        }
    }

    const fn importance(self) -> Importance {
        match self {
            Self::Normal => Importance::Normal,
            Self::Important => Importance::Important,
        }
    }
}

pub fn parse_inline_style_block(css: &str) -> InlineStyleBlock {
    crate::context::initialise_required_servo_style_prefs();
    let url_data: UrlExtraData = ABOUT_BLANK.clone().into();
    InlineStyleBlock(parse_style_attribute(
        css,
        &url_data,
        None,
        selectors::matching::QuirksMode::NoQuirks,
        CssRuleType::Style,
    ))
}

#[must_use]
pub fn parse_inline_style(input: DeclarationInput<'_>) -> InlineStyleBlock {
    parse_inline_style_block(input.0)
}

#[must_use]
pub fn parse_style_rule_declarations(
    input: DeclarationInput<'_>,
) -> stylo_cssom_model::RuleDeclarationBlock {
    let block = parse_cssom_declaration_block(input.0, CssomDeclarationContext::Style);
    rule_declaration_block_from_cssom(&block, stylo_cssom_model::RuleDeclarationDomain::Style)
}

fn ordinary_rule_declaration_block(
    node: &stylo_cssom_model::RuleNode,
) -> Option<&stylo_cssom_model::RuleDeclarationBlock> {
    node.payload().declaration_block().filter(|block| {
        matches!(
            block.domain(),
            stylo_cssom_model::RuleDeclarationDomain::Style
                | stylo_cssom_model::RuleDeclarationDomain::Nested
        )
    })
}

#[must_use]
pub fn replace_style_rule_declarations(
    node: &stylo_cssom_model::RuleNode,
    input: DeclarationInput<'_>,
) -> Option<stylo_cssom_model::RuleNode> {
    let original = ordinary_rule_declaration_block(node)?;
    let parsed = parse_cssom_declaration_block(input.0, CssomDeclarationContext::Style);
    let block = rule_declaration_block_from_cssom(&parsed, original.domain())
        .with_namespaces(original.namespaces().clone());
    Some(node.clone().with_cssom_declaration_block(block))
}

fn parse_specified_component_values(
    css: &str,
    base_url: &Arc<str>,
) -> Option<Box<[stylo_cssom_model::SpecifiedComponentValue]>> {
    fn parse(
        parser: &mut Parser<'_, '_>,
        base_url: &Arc<str>,
    ) -> Option<Vec<stylo_cssom_model::SpecifiedComponentValue>> {
        let mut values = Vec::new();
        loop {
            let start = parser.position();
            let Ok(token) = parser.next_including_whitespace_and_comments() else {
                break;
            };
            let token = token.clone();
            let token_serialization = Arc::from(parser.slice(start..parser.position()));
            let value = match token {
                Token::Ident(value) => {
                    stylo_cssom_model::SpecifiedComponentValue::Ident(Arc::from(value.as_ref()))
                },
                Token::AtKeyword(value) => {
                    stylo_cssom_model::SpecifiedComponentValue::AtKeyword(Arc::from(value.as_ref()))
                },
                Token::Hash(value) => stylo_cssom_model::SpecifiedComponentValue::Hash {
                    value: Arc::from(value.as_ref()),
                    id: false,
                },
                Token::IDHash(value) => stylo_cssom_model::SpecifiedComponentValue::Hash {
                    value: Arc::from(value.as_ref()),
                    id: true,
                },
                Token::QuotedString(value) => {
                    stylo_cssom_model::SpecifiedComponentValue::String(Arc::from(value.as_ref()))
                },
                Token::UnquotedUrl(value) => stylo_cssom_model::SpecifiedComponentValue::Url {
                    value: Arc::from(value.as_ref()),
                    source: stylo_cssom_model::UrlSourceContext {
                        base_url: base_url.clone(),
                    },
                },
                Token::Delim(value) => stylo_cssom_model::SpecifiedComponentValue::Delimiter(value),
                Token::Number { value, .. } => stylo_cssom_model::SpecifiedComponentValue::Number {
                    value,
                    serialization: token_serialization,
                },
                Token::Percentage { unit_value, .. } => {
                    stylo_cssom_model::SpecifiedComponentValue::Percentage {
                        value: unit_value * 100.0,
                        serialization: token_serialization,
                    }
                },
                Token::Dimension { value, unit, .. } => {
                    stylo_cssom_model::SpecifiedComponentValue::Dimension {
                        value,
                        unit: Arc::from(unit.as_ref()),
                        serialization: token_serialization,
                    }
                },
                Token::WhiteSpace(_) | Token::Comment(_) => continue,
                Token::Colon => stylo_cssom_model::SpecifiedComponentValue::Delimiter(':'),
                Token::Semicolon => stylo_cssom_model::SpecifiedComponentValue::Delimiter(';'),
                Token::Comma => stylo_cssom_model::SpecifiedComponentValue::Delimiter(','),
                Token::IncludeMatch => {
                    stylo_cssom_model::SpecifiedComponentValue::Operator(Arc::from("~="))
                },
                Token::DashMatch => {
                    stylo_cssom_model::SpecifiedComponentValue::Operator(Arc::from("|="))
                },
                Token::PrefixMatch => {
                    stylo_cssom_model::SpecifiedComponentValue::Operator(Arc::from("^="))
                },
                Token::SuffixMatch => {
                    stylo_cssom_model::SpecifiedComponentValue::Operator(Arc::from("$="))
                },
                Token::SubstringMatch => {
                    stylo_cssom_model::SpecifiedComponentValue::Operator(Arc::from("*="))
                },
                Token::CDO => {
                    stylo_cssom_model::SpecifiedComponentValue::Operator(Arc::from("<!--"))
                },
                Token::CDC => {
                    stylo_cssom_model::SpecifiedComponentValue::Operator(Arc::from("-->"))
                },
                Token::Function(name) => {
                    let arguments = parser
                        .parse_nested_block(|nested| {
                            Ok::<_, cssparser::ParseError<'_, ()>>(parse(nested, base_url))
                        })
                        .ok()??;
                    if name.eq_ignore_ascii_case("url")
                        && let [stylo_cssom_model::SpecifiedComponentValue::String(value)] =
                            arguments.as_slice()
                    {
                        stylo_cssom_model::SpecifiedComponentValue::Url {
                            value: value.clone(),
                            source: stylo_cssom_model::UrlSourceContext {
                                base_url: base_url.clone(),
                            },
                        }
                    } else {
                        stylo_cssom_model::SpecifiedComponentValue::Function {
                            name: Arc::from(name.as_ref()),
                            arguments: arguments.into_boxed_slice(),
                        }
                    }
                },
                Token::ParenthesisBlock | Token::SquareBracketBlock | Token::CurlyBracketBlock => {
                    let opening = match token {
                        Token::ParenthesisBlock => '(',
                        Token::SquareBracketBlock => '[',
                        Token::CurlyBracketBlock => '{',
                        _ => unreachable!(),
                    };
                    let nested = parser
                        .parse_nested_block(|nested| {
                            Ok::<_, cssparser::ParseError<'_, ()>>(parse(nested, base_url))
                        })
                        .ok()??;
                    stylo_cssom_model::SpecifiedComponentValue::Block {
                        opening,
                        values: nested.into_boxed_slice(),
                    }
                },
                Token::BadUrl(_)
                | Token::BadString(_)
                | Token::CloseParenthesis
                | Token::CloseSquareBracket
                | Token::CloseCurlyBracket => return None,
            };
            values.push(value);
        }
        Some(values)
    }

    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    parse(&mut parser, base_url).map(Vec::into_boxed_slice)
}

pub fn parse_inline_style_declarations(
    css: &str,
    base_url: Arc<str>,
) -> Arc<[stylo_cssom_model::SpecifiedDeclaration]> {
    let mut declarations = inline_native_declarations(css, &base_url);
    declarations.extend(parse_inline_compatibility_declarations(css, &base_url));
    declarations.sort_by_key(|(order, _)| *order);
    let mut winners: Vec<(usize, stylo_cssom_model::SpecifiedDeclaration)> = Vec::new();
    for (order, declaration) in declarations {
        if let Some(existing) = winners.iter_mut().find(|(_, candidate)| {
            compatibility::properties_match(&candidate.property, &declaration.property)
        }) {
            if existing.1.importance != stylo_cssom_model::Importance::Important
                || declaration.importance == stylo_cssom_model::Importance::Important
            {
                *existing = (order, declaration);
            }
        } else {
            winners.push((order, declaration));
        }
    }
    winners.sort_by_key(|(order, _)| *order);
    winners
        .into_iter()
        .map(|(_, declaration)| declaration)
        .collect::<Vec<_>>()
        .into()
}

fn inline_native_declarations(
    css: &str,
    base_url: &Arc<str>,
) -> Vec<(usize, stylo_cssom_model::SpecifiedDeclaration)> {
    let mut declarations = Vec::new();
    for (order, range) in crate::css_scan::split_top_level(css.as_bytes(), b';')
        .into_iter()
        .enumerate()
    {
        let authored = &css[range];
        let empty_custom_property = authored.split_once(':').is_some_and(|(property, value)| {
            property.trim().starts_with("--")
                && crate::css_scan::trim_ascii(value.as_bytes()).is_empty()
        });
        let block = if empty_custom_property {
            parse_inline_style_block(&format!("{authored};"))
        } else {
            parse_inline_style_block(authored)
        };
        let shorthand = authored
            .split_once(':')
            .map(|(property, _)| property.trim().to_ascii_lowercase())
            .map(|property| inline_style_cssom_backing_property(&property).to_owned())
            .and_then(|property| inline_style_cssom_property_schema(&property))
            .filter(|schema| inline_shorthand_serializes(schema));
        let shorthand_value = shorthand
            .and_then(|schema| inline_style_get_property_value(&block, schema.name))
            .and_then(|value| specified_style_value_from_css(&value, base_url));
        let mut parsed =
            specified_declarations_from_inline_style_block(&block, base_url.clone()).to_vec();
        for declaration in &mut parsed {
            if !declaration
                .shorthand_source
                .is_some_and(stylo_cssom_model::SpecifiedShorthandSource::has_pending_substitution)
            {
                declaration.shorthand_source = shorthand
                    .map(|schema| stylo_cssom_model::SpecifiedShorthandSource::Parsed(schema.id));
                declaration.shorthand_value.clone_from(&shorthand_value);
            }
        }
        declarations.extend(parsed.into_iter().map(|declaration| (order, declaration)));
    }
    declarations
}

pub fn specified_style_value_from_css(
    css: &str,
    base_url: &Arc<str>,
) -> Option<stylo_cssom_model::SpecifiedStyleValue> {
    Some(specified_style_value_from_components(
        parse_specified_component_values(css, base_url)?,
    ))
}

fn specified_style_value_from_components(
    components: Box<[stylo_cssom_model::SpecifiedComponentValue]>,
) -> stylo_cssom_model::SpecifiedStyleValue {
    use stylo_cssom_model::{CssWideKeyword, SpecifiedComponentValue, SpecifiedStyleValue};

    let keyword = if let [SpecifiedComponentValue::Ident(keyword)] = components.as_ref() {
        match keyword.to_ascii_lowercase().as_str() {
            "initial" => Some(CssWideKeyword::Initial),
            "inherit" => Some(CssWideKeyword::Inherit),
            "unset" => Some(CssWideKeyword::Unset),
            "revert" => Some(CssWideKeyword::Revert),
            "revert-layer" => Some(CssWideKeyword::RevertLayer),
            _ => None,
        }
    } else {
        None
    };
    keyword.map_or_else(
        || SpecifiedStyleValue::Components(components),
        SpecifiedStyleValue::CssWide,
    )
}

fn parse_inline_compatibility_declarations(
    css: &str,
    base_url: &Arc<str>,
) -> Vec<(usize, stylo_cssom_model::SpecifiedDeclaration)> {
    crate::css_scan::split_top_level(css.as_bytes(), b';')
        .into_iter()
        .enumerate()
        .flat_map(|(order, range)| {
            let declaration = &css[range];
            let Some(colon) = declaration.find(':') else {
                return Vec::new();
            };
            let authored_property = declaration[..colon].trim();
            let (value, importance) =
                split_inline_declaration_importance(declaration[colon + 1..].trim());
            parse_inline_compatibility_declaration(authored_property, value, importance, base_url)
                .into_iter()
                .flat_map(|declaration| {
                    compatibility::expand_declaration(declaration, base_url, false)
                })
                .map(|declaration| (order, declaration))
                .collect::<Vec<_>>()
        })
        .collect()
}

pub fn parse_inline_compatibility_declaration(
    authored_property: &str,
    value: &str,
    importance: stylo_cssom_model::Importance,
    base_url: &Arc<str>,
) -> Option<stylo_cssom_model::SpecifiedDeclaration> {
    let property = compatibility_declaration_property(authored_property, value)?;
    let url_data = url::Url::parse(base_url).ok()?.into();
    let namespaces = Namespaces::default();
    let context = declaration_parser_context(CssRuleType::Style, &url_data, &namespaces);
    parse_compatibility_declaration(property, value, importance, &context)
}

fn compatibility_declaration_property(
    authored_property: &str,
    value: &str,
) -> Option<stylo_cssom_model::InlineCompatibilityProperty> {
    inline_compatibility_property(authored_property)
        .or_else(|| {
            (authored_property.eq_ignore_ascii_case("display")
                && matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "-webkit-box" | "-webkit-inline-box"
                ))
            .then_some(stylo_cssom_model::InlineCompatibilityProperty::WebkitBoxDisplay)
        })
        .or_else(|| {
            (authored_property.eq_ignore_ascii_case("text-align")
                && matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "-moz-left"
                        | "-moz-center"
                        | "-moz-right"
                        | "-webkit-left"
                        | "-webkit-center"
                        | "-webkit-right"
                ))
            .then_some(stylo_cssom_model::InlineCompatibilityProperty::LegacyTextAlign)
        })
}

fn parse_compatibility_declaration(
    property: stylo_cssom_model::InlineCompatibilityProperty,
    value: &str,
    importance: stylo_cssom_model::Importance,
    context: &ParserContext<'_>,
) -> Option<stylo_cssom_model::SpecifiedDeclaration> {
    let trimmed = value.trim();
    match property {
        stylo_cssom_model::InlineCompatibilityProperty::FlowTolerance
            if trimmed.eq_ignore_ascii_case("auto") =>
        {
            return None;
        },
        stylo_cssom_model::InlineCompatibilityProperty::FlowTolerance
            if !matches!(trimmed.to_ascii_lowercase().as_str(), "normal" | "infinite") =>
        {
            let block = parse_inline_style_block(&format!("masonry-slack:{trimmed}"));
            if inline_style_get_property_value(&block, "masonry-slack").is_none() {
                return None;
            }
        },
        stylo_cssom_model::InlineCompatibilityProperty::GridLanesPack
            if !matches!(trimmed.to_ascii_lowercase().as_str(), "normal" | "dense") =>
        {
            return None;
        },
        _ => {},
    }
    let base_url = Arc::from(context.url_data.as_str());
    let components = parse_specified_component_values(trimmed, &base_url)?;
    if components.is_empty() {
        return None;
    }
    let value = if inline_compatibility_value_is_valid(property, &components) {
        specified_style_value_from_components(components)
    } else {
        return None;
    };
    Some(stylo_cssom_model::SpecifiedDeclaration {
        property: stylo_cssom_model::SpecifiedPropertyName::Compatibility(property),
        value,
        importance,
        shorthand_source: None,
        shorthand_value: None,
        typed_om_representation: None,
    })
}

pub fn parse_inline_style_property_declarations(
    property: &str,
    value: &str,
    priority: CssomDeclarationPriority,
    base_url: &Arc<str>,
) -> Option<Vec<stylo_cssom_model::SpecifiedDeclaration>> {
    if let Some(declaration) = parse_inline_compatibility_declaration(
        property,
        value,
        match priority {
            CssomDeclarationPriority::Normal => stylo_cssom_model::Importance::Normal,
            CssomDeclarationPriority::Important => stylo_cssom_model::Importance::Important,
        },
        base_url,
    ) {
        return Some(compatibility::expand_declaration(
            declaration,
            base_url,
            true,
        ));
    }
    let backing_property = inline_style_cssom_backing_property(property);
    let mut block = parse_inline_style_block("");
    if !inline_style_set_property(
        &mut block,
        DeclarationPropertyInput::new(property, value),
        priority,
    ) || inline_style_get_property_value(&block, backing_property).is_none()
    {
        return None;
    }
    let mut declarations =
        specified_declarations_from_inline_style_block(&block, base_url.clone()).to_vec();
    if let Some(shorthand) =
        stylo_cssom_model::property_schema(&backing_property.to_ascii_lowercase())
            .filter(|schema| inline_shorthand_serializes(schema))
    {
        let value = inline_style_get_property_value(&block, shorthand.name)
            .and_then(|value| specified_style_value_from_css(&value, base_url))?;
        for declaration in &mut declarations {
            declaration.shorthand_source = Some(
                if declaration.shorthand_source.is_some_and(
                    stylo_cssom_model::SpecifiedShorthandSource::has_pending_substitution,
                ) {
                    stylo_cssom_model::SpecifiedShorthandSource::CssomMutationPendingSubstitution(
                        shorthand.id,
                    )
                } else {
                    stylo_cssom_model::SpecifiedShorthandSource::CssomMutation(shorthand.id)
                },
            );
            declaration.shorthand_value = Some(value.clone());
        }
    }
    Some(declarations)
}

fn inline_shorthand_serializes(schema: &stylo_cssom_model::PropertySchemaRow) -> bool {
    schema.kind == stylo_cssom_model::PropertyKind::Shorthand
        && !matches!(
            schema.name,
            "page-break-before" | "page-break-after" | "page-break-inside"
        )
}

fn inline_compatibility_value_is_valid(
    property: stylo_cssom_model::InlineCompatibilityProperty,
    value: &[stylo_cssom_model::SpecifiedComponentValue],
) -> bool {
    use stylo_cssom_model::{
        InlineCompatibilityProperty as Property, SpecifiedComponentValue as Component,
    };

    let positive_integer = |value: f32| value >= 1.0 && value.fract() == 0.0;
    match (property, value) {
        (Property::Continue, [Component::Ident(value)]) => matches!(
            value.to_ascii_lowercase().as_str(),
            "auto"
                | "collapse"
                | "discard"
                | "inherit"
                | "initial"
                | "revert"
                | "revert-layer"
                | "unset"
        ),
        (Property::LineClamp, [Component::Ident(value)]) => {
            matches!(value.to_ascii_lowercase().as_str(), "none" | "auto")
        },
        (Property::LineClamp, [Component::Ident(value), Component::Ident(ellipsis)]) => {
            value.eq_ignore_ascii_case("auto") && ellipsis.eq_ignore_ascii_case("no-ellipsis")
        },
        (Property::LineClamp, [Component::String(_)]) => true,
        (Property::LineClamp, [Component::Number { value: lines, .. }]) => positive_integer(*lines),
        (
            Property::LineClamp,
            [
                Component::Number { value: lines, .. },
                Component::Ident(ellipsis),
            ],
        ) => {
            positive_integer(*lines)
                && matches!(
                    ellipsis.to_ascii_lowercase().as_str(),
                    "none" | "auto" | "no-ellipsis"
                )
        },
        (Property::LineClamp, [Component::Number { value: lines, .. }, Component::String(_)]) => {
            positive_integer(*lines)
        },
        (Property::WebkitLineClamp, [Component::Ident(value)]) => {
            value.eq_ignore_ascii_case("none")
        },
        (Property::WebkitLineClamp, [Component::Number { value: lines, .. }]) => {
            positive_integer(*lines)
        },
        (Property::FlowTolerance, _)
        | (Property::GridLanesPack, _)
        | (Property::LegacyTextAlign, _)
        | (Property::WebkitBoxDisplay, _) => true,
        _ => false,
    }
}

pub fn inline_compatibility_properties()
-> impl Iterator<Item = stylo_cssom_model::InlineCompatibilityProperty> {
    use stylo_cssom_model::InlineCompatibilityProperty as Property;

    [
        Property::FlowTolerance,
        Property::GridLanesPack,
        Property::Continue,
        Property::LineClamp,
        Property::WebkitLineClamp,
    ]
    .into_iter()
}

pub fn inline_compatibility_property(
    name: &str,
) -> Option<stylo_cssom_model::InlineCompatibilityProperty> {
    inline_compatibility_properties()
        .find(|property| property.css_name().eq_ignore_ascii_case(name))
}

pub fn split_inline_declaration_importance(value: &str) -> (&str, stylo_cssom_model::Importance) {
    for (bang, _) in value.rmatch_indices('!') {
        let mut input = ParserInput::new(&value[bang..]);
        let mut parser = Parser::new(&mut input);
        if parse_important(&mut parser).is_ok() && parser.is_exhausted() {
            return (&value[..bang], stylo_cssom_model::Importance::Important);
        }
    }
    (value, stylo_cssom_model::Importance::Normal)
}

pub fn stylo_inline_style_block(
    declarations: impl IntoIterator<Item = stylo_cssom_model::RuleDeclaration>,
    url_data: &UrlExtraData,
) -> InlineStyleBlock {
    let mut block = style::properties::declaration_block::PropertyDeclarationBlock::new();
    for declaration in declarations {
        if declaration.pending_substitution().is_some() {
            let (value, importance) = restore_pending_declaration(
                &declaration,
                CssomDeclarationContext::Style,
                &Namespaces::default(),
            )
            .expect("a projected pending longhand must retain its valid shorthand source");
            let _ = block.push(value, importance);
            continue;
        }
        let Ok(id) = PropertyId::parse_enabled_for_all_content(declaration.name()) else {
            continue;
        };
        let mut declarations = SourcePropertyDeclaration::default();
        if parse_one_declaration_into(
            &mut declarations,
            id,
            declaration.value(),
            Origin::Author,
            url_data,
            None,
            ParsingMode::DEFAULT,
            selectors::matching::QuirksMode::NoQuirks,
            CssRuleType::Style,
        )
        .is_ok()
        {
            let importance = if declaration.important() {
                Importance::Important
            } else {
                Importance::Normal
            };
            block.extend(declarations.drain(), importance);
        }
    }
    InlineStyleBlock(block)
}

pub fn into_stylo_property_declaration_block(
    block: InlineStyleBlock,
) -> style::properties::declaration_block::PropertyDeclarationBlock {
    block.0
}

pub fn specified_declarations_from_inline_style_block(
    block: &InlineStyleBlock,
    base_url: Arc<str>,
) -> Arc<[stylo_cssom_model::SpecifiedDeclaration]> {
    specified_declarations_from_native_block(&block.0, base_url)
}

pub fn specified_declarations_from_native_block(
    block: &style::properties::declaration_block::PropertyDeclarationBlock,
    base_url: Arc<str>,
) -> Arc<[stylo_cssom_model::SpecifiedDeclaration]> {
    block
        .declaration_importance_iter()
        .filter_map(|(declaration, importance)| {
            let has_pending_substitution =
                matches!(declaration, PropertyDeclaration::WithVariables(_));
            let name = declaration.id().name().into_owned();
            let property = if let Some(schema) = stylo_cssom_model::property_schema(&name) {
                stylo_cssom_model::SpecifiedPropertyName::Standard(schema.id)
            } else if name.starts_with("--") {
                stylo_cssom_model::SpecifiedPropertyName::Custom(Arc::from(name.as_str()))
            } else {
                return None;
            };
            let (serialized, shorthand_source) = match declaration {
                PropertyDeclaration::WithVariables(declaration) => (
                    declaration.value.variable_value().css.clone(),
                    declaration
                        .value
                        .from_shorthand()
                        .and_then(|shorthand| stylo_cssom_model::property_schema(shorthand.name())),
                ),
                _ => {
                    let mut value = String::new();
                    declaration.to_css(&mut value).ok()?;
                    (value, None)
                },
            };
            let trimmed = serialized.trim();
            let value = match trimmed.to_ascii_lowercase().as_str() {
                "initial" => stylo_cssom_model::SpecifiedStyleValue::CssWide(
                    stylo_cssom_model::CssWideKeyword::Initial,
                ),
                "inherit" => stylo_cssom_model::SpecifiedStyleValue::CssWide(
                    stylo_cssom_model::CssWideKeyword::Inherit,
                ),
                "unset" => stylo_cssom_model::SpecifiedStyleValue::CssWide(
                    stylo_cssom_model::CssWideKeyword::Unset,
                ),
                "revert" => stylo_cssom_model::SpecifiedStyleValue::CssWide(
                    stylo_cssom_model::CssWideKeyword::Revert,
                ),
                "revert-layer" => stylo_cssom_model::SpecifiedStyleValue::CssWide(
                    stylo_cssom_model::CssWideKeyword::RevertLayer,
                ),
                _ if name == "opacity" => trimmed
                    .strip_suffix('%')
                    .and_then(|percentage| percentage.trim().parse::<f32>().ok())
                    .map(|percentage| percentage / 100.0)
                    .or_else(|| trimmed.parse::<f32>().ok())
                    .and_then(stylo_cssom_model::Opacity::new)
                    .map_or_else(
                        || {
                            Some(stylo_cssom_model::SpecifiedStyleValue::Components(
                                parse_specified_component_values(trimmed, &base_url)?,
                            ))
                        },
                        |opacity| Some(stylo_cssom_model::SpecifiedStyleValue::Opacity(opacity)),
                    )?,
                _ if matches!(
                    property,
                    stylo_cssom_model::SpecifiedPropertyName::Custom(_)
                ) || has_pending_substitution =>
                {
                    stylo_cssom_model::SpecifiedStyleValue::TokenStream(Arc::from(trimmed))
                },
                _ => stylo_cssom_model::SpecifiedStyleValue::Components(
                    parse_specified_component_values(trimmed, &base_url)?,
                ),
            };
            Some(stylo_cssom_model::SpecifiedDeclaration {
                property,
                value,
                importance: if importance.important() {
                    stylo_cssom_model::Importance::Important
                } else {
                    stylo_cssom_model::Importance::Normal
                },
                shorthand_source: shorthand_source.map(|schema| {
                    stylo_cssom_model::SpecifiedShorthandSource::PendingSubstitution(schema.id)
                }),
                shorthand_value: shorthand_source
                    .and_then(|_| specified_style_value_from_css(trimmed, &base_url)),
                typed_om_representation: None,
            })
        })
        .collect::<Vec<_>>()
        .into()
}

pub fn specified_declarations_from_property_declaration(
    declaration: PropertyDeclaration,
    base_url: Arc<str>,
) -> Option<Vec<stylo_cssom_model::SpecifiedDeclaration>> {
    let mut block = style::properties::declaration_block::PropertyDeclarationBlock::new();
    let _ = block.push(declaration, Importance::Normal);
    let declarations =
        specified_declarations_from_inline_style_block(&InlineStyleBlock(block), base_url).to_vec();
    (!declarations.is_empty()).then_some(declarations)
}

#[must_use]
pub fn inline_style_cssom_backing_property(property: &str) -> &str {
    match property.to_ascii_lowercase().as_str() {
        "flow-tolerance" => "masonry-slack",
        "grid-gap" => "gap",
        "text-box-trim" => "leading-trim",
        "word-wrap" => "overflow-wrap",
        _ => property,
    }
}

#[must_use]
pub fn inline_style_cssom_property_schema(
    property: &str,
) -> Option<&'static stylo_cssom_model::PropertySchemaRow> {
    let property = inline_style_cssom_backing_property(property);
    PropertyId::parse_enabled_for_all_content(property)
        .ok()
        .and_then(|id| crate::property_schema::property_schema_for_id(&id))
        .or_else(|| stylo_cssom_model::property_schema(&property.to_ascii_lowercase()))
}

#[must_use]
pub fn inline_style_cssom_backing_value<'a>(property: &str, value: &'a str) -> Option<&'a str> {
    if !crate::css_scan::named_function_is_closed(value, b"progress") {
        return None;
    }
    if property.eq_ignore_ascii_case("text-box-trim") {
        let mut input = ParserInput::new(value);
        let mut parser = Parser::new(&mut input);
        let keyword = parser.expect_ident_cloned().ok();
        if parser.expect_exhausted().is_ok()
            && keyword.is_some_and(|keyword| {
                matches!(
                    keyword.to_ascii_lowercase().as_str(),
                    "both" | "end" | "normal" | "start"
                )
            })
        {
            return None;
        }
        return Some(value);
    }
    if !property.eq_ignore_ascii_case("flow-tolerance") {
        return Some(value);
    }
    let keyword = value.trim();
    if keyword.eq_ignore_ascii_case("normal") {
        Some("infinite")
    } else if keyword.eq_ignore_ascii_case("infinite") {
        Some("auto")
    } else if keyword.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(value)
    }
}

#[must_use]
pub fn inline_style_cssom_authored_value(property: &str, value: Option<String>) -> Option<String> {
    value.map(|value| {
        let keyword = match (property.to_ascii_lowercase().as_str(), value.as_str()) {
            ("flow-tolerance", "infinite") => Some("normal"),
            ("flow-tolerance", "auto") => Some("infinite"),
            ("text-box-trim", "normal") => Some("none"),
            ("text-box-trim", "start") => Some("trim-start"),
            ("text-box-trim", "end") => Some("trim-end"),
            ("text-box-trim", "both") => Some("trim-both"),
            _ => None,
        };
        match keyword {
            Some(keyword) => keyword.to_owned(),
            None => value,
        }
    })
}

pub fn parse_cssom_declaration_block(
    css: &str,
    context: CssomDeclarationContext,
) -> CssomDeclarationBlock {
    crate::context::initialise_required_servo_style_prefs();
    let url_data: UrlExtraData = ABOUT_BLANK.clone().into();
    CssomDeclarationBlock(parse_style_attribute(
        css,
        &url_data,
        None,
        selectors::matching::QuirksMode::NoQuirks,
        context.rule_type(),
    ))
}

pub fn inline_style_get_property_value(block: &InlineStyleBlock, property: &str) -> Option<String> {
    declaration_block_get_property_value(&block.0, property)
}

fn declaration_block_get_property_value(
    block: &style::properties::declaration_block::PropertyDeclarationBlock,
    property: &str,
) -> Option<String> {
    let adapter = inline_style_cssom_property_schema(property).map(|schema| schema.serializer);
    if adapter == Some(stylo_cssom_model::AdapterRoute::ContainerShorthand) {
        let name = declaration_block_get_property_value(block, "container-name")?;
        let container_type = declaration_block_get_property_value(block, "container-type")?;
        let name_id = PropertyId::parse_enabled_for_all_content("container-name").ok()?;
        let type_id = PropertyId::parse_enabled_for_all_content("container-type").ok()?;
        if block.property_priority(&name_id) != block.property_priority(&type_id) {
            return None;
        }
        if let ShorthandCssWideValue::Uniform(keyword) =
            ShorthandCssWideValue::from_longhands(&name, &container_type)?
        {
            return Some(keyword.to_str().to_owned());
        }
        return if container_type == "normal" {
            Some(name)
        } else {
            Some(format!("{name} / {container_type}"))
        };
    }
    if adapter == Some(stylo_cssom_model::AdapterRoute::PositionTryShorthand) {
        return position_try_shorthand_value(block);
    }
    let id = stylo_serialized_property_id(property)?;
    let present = matches!(&id, PropertyId::Custom(_))
        && id
            .as_shorthand()
            .err()
            .is_some_and(|declaration| block.contains(declaration));
    let mut out = String::new();
    block.property_value_to_css(&id, &mut out).ok()?;
    (!out.is_empty() || present).then_some(out)
}

enum ShorthandCssWideValue {
    Absent,
    Uniform(CSSWideKeyword),
}

impl ShorthandCssWideValue {
    fn from_longhands(first: &str, second: &str) -> Option<Self> {
        match (
            CSSWideKeyword::from_ident(first),
            CSSWideKeyword::from_ident(second),
        ) {
            (Err(()), Err(())) => Some(Self::Absent),
            (Ok(first), Ok(second)) if first == second => Some(Self::Uniform(first)),
            _ => None,
        }
    }
}

fn position_try_shorthand_value(
    block: &style::properties::declaration_block::PropertyDeclarationBlock,
) -> Option<String> {
    let order_id = PropertyId::parse_enabled_for_all_content("position-try-order").ok()?;
    let fallbacks_id = PropertyId::parse_enabled_for_all_content("position-try-fallbacks").ok()?;
    if block.property_priority(&order_id) != block.property_priority(&fallbacks_id) {
        return None;
    }
    let order = declaration_block_get_property_value(block, "position-try-order")?;
    let fallbacks = declaration_block_get_property_value(block, "position-try-fallbacks")?;
    if let ShorthandCssWideValue::Uniform(keyword) =
        ShorthandCssWideValue::from_longhands(&order, &fallbacks)?
    {
        Some(keyword.to_str().to_owned())
    } else if order == "normal" {
        Some(fallbacks)
    } else {
        Some(format!("{order} {fallbacks}"))
    }
}

fn stylo_serialized_property_id(property: &str) -> Option<PropertyId> {
    let id = PropertyId::parse_enabled_for_all_content(property).ok()?;
    (matches!(id, PropertyId::Custom(_))
        || crate::property_schema::property_schema_for_id(&id)?.serializer
            == stylo_cssom_model::AdapterRoute::Stylo)
        .then_some(id)
}

pub fn inline_style_property_is_important(
    block: &InlineStyleBlock,
    property: &str,
) -> Option<bool> {
    let property = inline_style_cssom_backing_property(property);
    declaration_block_property_is_important(&block.0, property)
}

fn declaration_block_property_is_important(
    block: &style::properties::declaration_block::PropertyDeclarationBlock,
    property: &str,
) -> Option<bool> {
    if let Some(schema) = inline_style_cssom_property_schema(property)
        .filter(|schema| schema.kind == stylo_cssom_model::PropertyKind::Shorthand)
    {
        return Some(schema.shorthand_expansion.iter().all(|longhand| {
            declaration_block_property_is_important(block, longhand) == Some(true)
        }));
    }
    let id = stylo_serialized_property_id(property)?;
    block
        .contains(id.as_shorthand().err()?)
        .then(|| block.property_priority(&id).important())
}

pub fn inline_style_set_property(
    block: &mut InlineStyleBlock,
    input: DeclarationPropertyInput<'_>,
    priority: CssomDeclarationPriority,
) -> bool {
    let DeclarationPropertyInput { property, value } = input;
    let Some(value) = inline_style_cssom_backing_value(property, value) else {
        return false;
    };
    let property = inline_style_cssom_backing_property(property);
    declaration_block_set_property(&mut block.0, property, value, priority, CssRuleType::Style)
}

pub fn cssom_declaration_set_property(
    block: &mut CssomDeclarationBlock,
    property: &str,
    value: &str,
    priority: CssomDeclarationPriority,
    context: CssomDeclarationContext,
) -> bool {
    declaration_block_set_property(&mut block.0, property, value, priority, context.rule_type())
}

pub fn declaration_block_shorthand_values(
    block: &style::properties::declaration_block::PropertyDeclarationBlock,
) -> Vec<stylo_cssom_model::RuleDeclaration> {
    (0..)
        .map_while(stylo_cssom_model::property_schema_at)
        .filter(|schema| schema.kind == stylo_cssom_model::PropertyKind::Shorthand)
        .filter_map(|schema| {
            let value = declaration_block_get_property_value(block, schema.name)?;
            Some(
                stylo_cssom_model::RuleDeclaration::new(schema.name, value)
                    .with_importance(declaration_block_property_is_important(block, schema.name)?),
            )
        })
        .collect()
}

pub fn cssom_declaration_merge(target: &mut CssomDeclarationBlock, source: &CssomDeclarationBlock) {
    for (declaration, importance) in source.0.declaration_importance_iter() {
        let _ = target.0.push(declaration.clone(), importance);
    }
}

pub fn mutate_rule_declaration_block(
    block: &stylo_cssom_model::RuleDeclarationBlock,
    property: &str,
    value: &str,
    priority: CssomDeclarationPriority,
    context: CssomDeclarationContext,
) -> Option<stylo_cssom_model::RuleDeclarationBlock> {
    let property = property.trim();
    if property.is_empty() {
        return None;
    }
    let mut parsed = CssomDeclarationBlock(stylo_rule_declaration_block(block, context)?);
    let namespaces = stylo_namespaces(block.namespaces());
    let url_data = ABOUT_BLANK.clone().into();
    if value.is_empty() {
        let _ = cssom_declaration_remove_property(&mut parsed, property);
    } else if !declaration_block_set_property_with_context(
        &mut parsed.0,
        property,
        value,
        priority,
        context.rule_type(),
        &url_data,
        &namespaces,
    ) {
        return None;
    }
    Some(
        rule_declaration_block_from_cssom(&parsed, block.domain())
            .with_namespaces(block.namespaces().clone()),
    )
}

pub fn stylo_namespaces(namespaces: &stylo_cssom_model::RuleNamespaceContext) -> Namespaces {
    Namespaces {
        default: namespaces.default_namespace().map(style::Namespace::from),
        prefixes: namespaces
            .prefixes()
            .map(|(prefix, namespace)| {
                (
                    style::Prefix::from(prefix),
                    style::Namespace::from(namespace),
                )
            })
            .collect(),
    }
}

pub fn stylo_rule_declaration_block(
    block: &stylo_cssom_model::RuleDeclarationBlock,
    context: CssomDeclarationContext,
) -> Option<style::properties::declaration_block::PropertyDeclarationBlock> {
    let mut parsed = parse_cssom_declaration_block("", context);
    let namespaces = stylo_namespaces(block.namespaces());
    for declaration in block.declarations() {
        let importance = if declaration.important() {
            CssomDeclarationPriority::Important
        } else {
            CssomDeclarationPriority::Normal
        };
        if declaration.pending_substitution().is_some() {
            let (retained, importance) =
                restore_pending_declaration(declaration, context, &namespaces)?;
            let _ = parsed.0.push(retained, importance);
        } else if !declaration_block_set_property_with_context(
            &mut parsed.0,
            declaration.name(),
            declaration.value(),
            importance,
            context.rule_type(),
            &ABOUT_BLANK.clone().into(),
            &namespaces,
        ) {
            return None;
        }
    }
    Some(parsed.0)
}

fn restore_pending_declaration(
    declaration: &stylo_cssom_model::RuleDeclaration,
    context: CssomDeclarationContext,
    namespaces: &Namespaces,
) -> Option<(PropertyDeclaration, Importance)> {
    let pending = declaration.pending_substitution()?;
    let url_data = url::Url::parse(pending.base_url()).ok()?.into();
    let mut source = style::properties::declaration_block::PropertyDeclarationBlock::new();
    let importance = if declaration.important() {
        CssomDeclarationPriority::Important
    } else {
        CssomDeclarationPriority::Normal
    };
    if !declaration_block_set_property_with_context(
        &mut source,
        pending.shorthand().schema().name,
        pending.tokens(),
        importance,
        context.rule_type(),
        &url_data,
        namespaces,
    ) {
        return None;
    }
    source
        .declaration_importance_iter()
        .find(|(candidate, _)| candidate.id().name() == declaration.name())
        .map(|(value, importance)| (value.clone(), importance))
}

pub fn rule_declaration_block_from_cssom(
    block: &CssomDeclarationBlock,
    domain: stylo_cssom_model::RuleDeclarationDomain,
) -> stylo_cssom_model::RuleDeclarationBlock {
    let declarations = block
        .0
        .declaration_importance_iter()
        .map(|(declaration, importance)| rule_declaration_from_stylo(declaration, importance))
        .collect::<Vec<_>>();
    let mut serialization = String::new();
    let _ = block.0.to_css(&mut serialization);
    stylo_cssom_model::RuleDeclarationBlock::new(domain, serialization, declarations)
        .with_shorthand_values(declaration_block_shorthand_values(&block.0))
}

pub fn rule_declaration_from_stylo(
    declaration: &PropertyDeclaration,
    importance: Importance,
) -> stylo_cssom_model::RuleDeclaration {
    let name = declaration.id().name().into_owned();
    if let PropertyDeclaration::WithVariables(value) = declaration
        && let Some(shorthand) = value.value.from_shorthand()
    {
        let schema = stylo_cssom_model::property_schema(shorthand.name())
            .expect("a parsed shorthand must have a model schema");
        let source = value.value.variable_value();
        return stylo_cssom_model::RuleDeclaration::from_pending_substitution(
            name,
            schema.id,
            source.css.as_str(),
            source.url_data.as_str(),
        )
        .expect("a parsed pending longhand must belong to its originating shorthand")
        .with_importance(importance.important());
    }
    let mut value = String::new();
    let _ = declaration.to_css(&mut value);
    stylo_cssom_model::RuleDeclaration::new(name, value).with_importance(importance.important())
}

pub fn parse_nested_rule_declarations(
    css: &str,
) -> Option<Box<[stylo_cssom_model::RuleDeclaration]>> {
    let block = parse_cssom_declaration_block(css, CssomDeclarationContext::Style);
    let block =
        rule_declaration_block_from_cssom(&block, stylo_cssom_model::RuleDeclarationDomain::Nested);
    (!block.declarations().is_empty()).then(|| block.declarations().to_vec().into_boxed_slice())
}

#[must_use]
pub fn mutate_style_rule_declaration(
    node: &stylo_cssom_model::RuleNode,
    input: DeclarationPropertyInput<'_>,
    priority: CssomDeclarationPriority,
) -> Option<stylo_cssom_model::RuleNode> {
    let block = ordinary_rule_declaration_block(node)?;
    let DeclarationPropertyInput { property, value } = input;
    let updated = mutate_rule_declaration_block(
        block,
        property,
        value,
        priority,
        CssomDeclarationContext::Style,
    )
    .or_else(|| {
        crate::value_contains_valid_symbols_function(property, value).then(|| {
            let important = matches!(priority, CssomDeclarationPriority::Important);
            let mut declarations = block
                .declarations()
                .iter()
                .filter(|declaration| !declaration.name().eq_ignore_ascii_case(property))
                .cloned()
                .collect::<Vec<_>>();
            declarations.push(
                stylo_cssom_model::RuleDeclaration::new(property, value).with_importance(important),
            );
            stylo_cssom_model::RuleDeclarationBlock::from_declarations(block.domain(), declarations)
                .with_namespaces(block.namespaces().clone())
        })
    })?;
    Some(node.clone().with_cssom_declaration_block(updated))
}

#[must_use]
pub fn set_style_rule_typed_om_property(
    node: &stylo_cssom_model::RuleNode,
    input: DeclarationPropertyInput<'_>,
) -> Option<stylo_cssom_model::RuleNode> {
    let removed = mutate_style_rule_declaration(
        node,
        DeclarationPropertyInput::new(input.property, ""),
        CssomDeclarationPriority::Normal,
    )?;
    mutate_style_rule_declaration(&removed, input, CssomDeclarationPriority::Normal)
}

fn declaration_block_set_property(
    block: &mut style::properties::declaration_block::PropertyDeclarationBlock,
    property: &str,
    value: &str,
    priority: CssomDeclarationPriority,
    rule_type: CssRuleType,
) -> bool {
    let url_data: UrlExtraData = ABOUT_BLANK.clone().into();
    declaration_block_set_property_with_context(
        block,
        property,
        value,
        priority,
        rule_type,
        &url_data,
        &Namespaces::default(),
    )
}

fn declaration_block_set_property_with_context(
    block: &mut style::properties::declaration_block::PropertyDeclarationBlock,
    property: &str,
    value: &str,
    priority: CssomDeclarationPriority,
    rule_type: CssRuleType,
    url_data: &UrlExtraData,
    namespaces: &Namespaces,
) -> bool {
    if matches!(rule_type, CssRuleType::Style) {
        match inline_style_cssom_property_schema(property).map(|schema| schema.parser) {
            Some(stylo_cssom_model::AdapterRoute::ContainerShorthand) => {
                return apply_parsed_shorthand(
                    block,
                    SpecifiedContainerShorthand::parse(value)
                        .and_then(SpecifiedContainerShorthand::declarations),
                    priority,
                );
            },
            Some(stylo_cssom_model::AdapterRoute::PositionTryShorthand) => {
                return apply_parsed_shorthand(
                    block,
                    SpecifiedPositionTryShorthand::parse(value)
                        .and_then(SpecifiedPositionTryShorthand::declarations),
                    priority,
                );
            },
            Some(
                stylo_cssom_model::AdapterRoute::Stylo
                | stylo_cssom_model::AdapterRoute::Unsupported,
            )
            | None => {},
        }
    }
    let Ok(id) = PropertyId::parse_enabled_for_all_content(property) else {
        return false;
    };
    if matches!(rule_type, CssRuleType::Style)
        && matches!(id, PropertyId::NonCustom(_))
        && crate::property_schema::property_schema_for_id(&id)
            .is_none_or(|row| row.parser != stylo_cssom_model::AdapterRoute::Stylo)
    {
        return false;
    }
    let mut decls = SourcePropertyDeclaration::default();
    let context = declaration_parser_context(rule_type, url_data, namespaces);
    let mut input = ParserInput::new(value);
    if Parser::new(&mut input)
        .parse_entirely(|parser| PropertyDeclaration::parse_into(&mut decls, id, &context, parser))
        .is_err()
    {
        return false;
    }
    declaration_block_apply_declarations(block, decls, priority)
}

pub fn declaration_parser_context<'a>(
    rule_type: CssRuleType,
    url_data: &'a UrlExtraData,
    namespaces: &'a Namespaces,
) -> ParserContext<'a> {
    ParserContext::new(
        Origin::Author,
        url_data,
        Some(rule_type),
        ParsingMode::DEFAULT,
        selectors::matching::QuirksMode::NoQuirks,
        std::borrow::Cow::Borrowed(namespaces),
        None,
        None,
    )
}

fn declaration_block_apply_declarations(
    block: &mut style::properties::declaration_block::PropertyDeclarationBlock,
    mut declarations: SourcePropertyDeclaration,
    priority: CssomDeclarationPriority,
) -> bool {
    let importance = priority.importance();
    let mut updates = SourcePropertyDeclarationUpdate::default();
    if block.prepare_for_update(&declarations, importance, &mut updates) {
        block.update(declarations.drain(), importance, &mut updates);
    }
    true
}

pub fn inline_style_remove_property(
    block: &mut InlineStyleBlock,
    property: &str,
) -> Option<String> {
    declaration_block_remove_property(&mut block.0, property)
}

pub fn cssom_declaration_remove_property(
    block: &mut CssomDeclarationBlock,
    property: &str,
) -> Option<String> {
    declaration_block_remove_property(&mut block.0, property)
}

fn declaration_block_remove_property(
    block: &mut style::properties::declaration_block::PropertyDeclarationBlock,
    property: &str,
) -> Option<String> {
    if let Some(schema) = inline_style_cssom_property_schema(property)
        .filter(|schema| schema.kind == stylo_cssom_model::PropertyKind::Shorthand)
    {
        let old = declaration_block_get_property_value(block, property);
        for longhand in schema.shorthand_expansion {
            declaration_block_remove_property(block, longhand);
        }
        return old;
    }
    let id = stylo_serialized_property_id(property)?;
    let first = block.first_declaration_to_remove(&id)?;
    let mut out = String::new();

    let had_value = block.property_value_to_css(&id, &mut out).is_ok() && !out.is_empty();
    block.remove_property(&id, first);
    if had_value { Some(out) } else { None }
}

pub fn inline_style_declarations_with_importance(
    block: &InlineStyleBlock,
) -> Vec<(String, String, bool)> {
    block
        .0
        .declaration_importance_iter()
        .map(|(decl, importance)| {
            let name = decl.id().name().into_owned();
            let mut value = String::new();

            let _ = decl.to_css(&mut value);
            (name, value, importance.important())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_style_backing_values_preserve_authored_property_grammar() {
        for value in ["both", "end", "normal", "start"] {
            assert_eq!(
                inline_style_cssom_backing_value("text-box-trim", value),
                None
            );
        }
        for value in [
            "none",
            "trim-both",
            "trim-end",
            "trim-start",
            "initial",
            "inherit",
            "unset",
            "revert",
            "var(--trim)",
        ] {
            assert_eq!(
                inline_style_cssom_backing_value("text-box-trim", value),
                Some(value)
            );
        }
        for value in [
            "progress(5%, 0deg, 8deg",
            "progress(5%, 0px, 10px",
            "calc(progress(5%, 0px, 10px",
        ] {
            assert_eq!(inline_style_cssom_backing_value("opacity", value), None);
        }
        for value in [
            "calc(progress(5%, 0px, 10px) * 100%)",
            "calc(progress(5%, 0px, 10px) * 100%",
            "var(--opacity)",
        ] {
            assert_eq!(
                inline_style_cssom_backing_value("opacity", value),
                Some(value)
            );
        }
        assert_eq!(
            inline_style_cssom_backing_value("list-style-image", "linear-gradient(red, blue"),
            Some("linear-gradient(red, blue")
        );
    }

    #[test]
    fn inline_compatibility_declarations_remain_typed_and_authored() {
        use stylo_cssom_model::{
            InlineCompatibilityProperty as Compat, SpecifiedPropertyName as Property,
        };

        let declarations = parse_inline_style_declarations(
            concat!(
                "flow-tolerance:normal!important;",
                "grid-lanes-pack:dense;grid-lanes-direction:column track-reverse;",
                "display:-webkit-box;-webkit-box-orient:vertical;",
                "view-transition-scope:all;view-transition-group:contain",
            ),
            "about:blank".into(),
        );
        let compatibility = declarations
            .iter()
            .filter_map(|declaration| match &declaration.property {
                Property::Compatibility(property) => Some((*property, declaration.importance)),
                Property::Standard(_) | Property::Custom(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            compatibility,
            vec![
                (
                    Compat::FlowTolerance,
                    stylo_cssom_model::Importance::Important
                ),
                (Compat::GridLanesPack, stylo_cssom_model::Importance::Normal),
                (
                    Compat::WebkitBoxDisplay,
                    stylo_cssom_model::Importance::Normal
                ),
            ]
        );
    }

    #[test]
    fn inline_compatibility_declarations_enforce_custom_grammars() {
        let base_url = std::sync::Arc::from("about:blank");
        for value in [
            "row",
            "column fill-reverse",
            "row track-reverse fill-reverse",
            "normal",
        ] {
            assert!(
                parse_inline_style_property_declarations(
                    "grid-lanes-direction",
                    value,
                    CssomDeclarationPriority::Normal,
                    &base_url,
                )
                .is_some(),
                "{value}"
            );
        }
        for value in [
            "auto",
            "normal track-reverse",
            "row fill-reverse fill-reverse",
        ] {
            assert!(
                parse_inline_style_property_declarations(
                    "grid-lanes-direction",
                    value,
                    CssomDeclarationPriority::Normal,
                    &base_url,
                )
                .is_none(),
                "{value}"
            );
        }
        for value in ["none", "nearest", "contain", "normal", "custom-ident"] {
            assert!(
                parse_inline_style_property_declarations(
                    "view-transition-group",
                    value,
                    CssomDeclarationPriority::Normal,
                    &base_url,
                )
                .is_some(),
                "{value}"
            );
        }
        for value in ["default", "foo none", "#fff", "12px"] {
            assert!(
                parse_inline_style_property_declarations(
                    "view-transition-group",
                    value,
                    CssomDeclarationPriority::Normal,
                    &base_url,
                )
                .is_none(),
                "{value}"
            );
        }
    }

    #[test]
    fn inline_compatibility_declarations_keep_cascade_source_order() {
        use stylo_cssom_model::{
            InlineCompatibilityProperty as Compat, SpecifiedPropertyName as Property,
        };

        for (css, name, value, importance) in [
            (
                "flow-tolerance:0;masonry-slack:20px",
                "masonry-slack",
                "20px",
                stylo_cssom_model::Importance::Normal,
            ),
            (
                "masonry-slack:20px;flow-tolerance:0",
                "flow-tolerance",
                "0",
                stylo_cssom_model::Importance::Normal,
            ),
            (
                "flow-tolerance:0!important;masonry-slack:20px",
                "flow-tolerance",
                "0",
                stylo_cssom_model::Importance::Important,
            ),
            (
                "masonry-slack:20px!important;flow-tolerance:0",
                "masonry-slack",
                "20px",
                stylo_cssom_model::Importance::Important,
            ),
        ] {
            let winners = parse_inline_style_declarations(css, "about:blank".into())
                .iter()
                .filter_map(|declaration| {
                    let name = match &declaration.property {
                        Property::Standard(property)
                            if property.schema().name == "masonry-slack" =>
                        {
                            Some("masonry-slack")
                        },
                        Property::Compatibility(Compat::FlowTolerance) => Some("flow-tolerance"),
                        Property::Standard(_)
                        | Property::Custom(_)
                        | Property::Compatibility(_) => None,
                    }?;
                    Some((
                        name,
                        crate::specified::serialize_projected_specified_style_value(
                            &declaration.value,
                        )
                        .into_css_text(),
                        declaration.importance,
                    ))
                })
                .collect::<Vec<_>>();
            assert_eq!(winners, [(name, value.to_owned(), importance)], "{css}");
        }
    }

    #[test]
    fn object_view_box_retains_a_basic_shape_rectangle() {
        let block = parse_inline_style_block("object-view-box: xywh(10px 20px 30px 40px)");
        assert_eq!(
            inline_style_get_property_value(&block, "object-view-box").as_deref(),
            Some("xywh(10px 20px 30px 40px)")
        );
    }

    #[test]
    fn standard_property_pending_substitution_retains_its_token_stream() {
        let declarations = parse_inline_style_declarations(
            "clip-path:shape(from 0 0,curve to 5% var(--s,1) with 5% 5%, vline to 100%)",
            "about:blank".into(),
        );

        assert!(matches!(
            &declarations[0].value,
            stylo_cssom_model::SpecifiedStyleValue::TokenStream(value)
                if value.as_ref() == "shape(from 0 0,curve to 5% var(--s,1) with 5% 5%, vline to 100%)"
        ));
    }

    #[test]
    fn inline_shorthand_retains_pending_substitution_classification() {
        let declarations =
            parse_inline_style_declarations("margin:var(--space)", "about:blank".into());

        assert_eq!(declarations.len(), 4);
        assert!(declarations.iter().all(|declaration| {
            declaration
                .shorthand_source
                .is_some_and(stylo_cssom_model::SpecifiedShorthandSource::has_pending_substitution)
                && declaration.shorthand_value.is_some()
        }));
    }

    #[test]
    fn parsed_block_retains_pending_longhands_after_shorthand_override() {
        for css in [
            "transition:var(--timing)",
            "transition:var(--timing);transition-delay:1s",
        ] {
            let block = parse_inline_style_block(css);
            let declarations =
                specified_declarations_from_inline_style_block(&block, "about:blank".into());
            let duration = declarations
                .iter()
                .find(|declaration| {
                    matches!(
                        declaration.property,
                        stylo_cssom_model::SpecifiedPropertyName::Standard(property)
                            if property.schema().name == "transition-duration"
                    )
                })
                .unwrap_or_else(|| panic!("pending transition-duration was lost: {css}"));
            assert!(
                matches!(&duration.value, stylo_cssom_model::SpecifiedStyleValue::TokenStream(value)
                    if value.as_ref() == "var(--timing)"),
                "{css}"
            );
            assert!(
                matches!(duration.shorthand_source,
                    Some(stylo_cssom_model::SpecifiedShorthandSource::PendingSubstitution(property))
                        if property.schema().name == "transition"),
                "{css}"
            );
            let mut serialization = String::new();
            block
                .0
                .property_value_to_css(
                    &PropertyId::parse_enabled_for_all_content("transition-duration").unwrap(),
                    &mut serialization,
                )
                .unwrap();
            assert!(serialization.is_empty(), "{css}");
        }
    }

    #[test]
    fn inline_empty_custom_property_retains_an_empty_token_stream() {
        let declarations = parse_inline_style_declarations("--empty: ", "about:blank".into());

        assert!(matches!(
            declarations.as_ref(),
            [stylo_cssom_model::SpecifiedDeclaration {
                property: stylo_cssom_model::SpecifiedPropertyName::Custom(name),
                value: stylo_cssom_model::SpecifiedStyleValue::TokenStream(value),
                ..
            }] if name.as_ref() == "--empty" && value.is_empty()
        ));
    }

    #[test]
    fn cssom_adapter_shorthands_reject_mixed_css_wide_values() {
        for (shorthand, first, second, ordinary) in [
            (
                "container",
                "container-name",
                "container-type",
                "inline-size",
            ),
            (
                "position-try",
                "position-try-order",
                "position-try-fallbacks",
                "none",
            ),
        ] {
            for (left, right, expected) in [
                ("initial", ordinary, None),
                ("initial", "inherit", None),
                ("normal", "inherit", None),
                ("initial", "initial", Some("initial")),
                ("revert", "revert", Some("revert")),
            ] {
                let block = parse_inline_style_block(&format!("{first}:{left};{second}:{right}"));
                assert_eq!(
                    inline_style_get_property_value(&block, shorthand).as_deref(),
                    expected,
                    "{shorthand}: {left}, {right}",
                );
            }
        }
    }

    #[test]
    fn inline_shorthand_priority_does_not_require_serializable_values() {
        for (shorthand, css) in [
            (
                "container",
                "container-name:initial!important;container-type:inline-size!important",
            ),
            (
                "position-try",
                "position-try-order:initial!important;position-try-fallbacks:none!important",
            ),
            (
                "margin",
                "margin:1px!important;margin-top:initial!important",
            ),
        ] {
            let block = parse_inline_style_block(css);
            assert_eq!(
                inline_style_property_is_important(&block, shorthand),
                Some(true),
                "{shorthand}"
            );
        }
    }

    #[test]
    fn position_try_removal_does_not_require_a_serializable_shorthand() {
        for css in [
            "position-try-order:normal;color:red",
            "position-try-fallbacks:none;color:red",
            "position-try-order:normal!important;position-try-fallbacks:none;color:red",
        ] {
            let mut block = parse_inline_style_block(css);
            assert_eq!(
                inline_style_remove_property(&mut block, "position-try"),
                None,
                "{css}"
            );
            assert_eq!(
                inline_style_get_property_value(&block, "position-try-order"),
                None,
                "{css}"
            );
            assert_eq!(
                inline_style_get_property_value(&block, "position-try-fallbacks"),
                None,
                "{css}"
            );
            assert_eq!(
                inline_style_get_property_value(&block, "color").as_deref(),
                Some("red"),
                "{css}"
            );
        }
    }

    #[test]
    fn container_shorthand_expands_transactionally_and_serialises_canonically() {
        let mut block =
            parse_inline_style_block("container-name: old; container-type: inline-size");
        assert!(inline_style_set_property(
            &mut block,
            DeclarationPropertyInput::new("container", "FOO/size"),
            CssomDeclarationPriority::Important,
        ));
        assert_eq!(
            inline_style_get_property_value(&block, "container").as_deref(),
            Some("FOO / size")
        );
        assert_eq!(
            inline_style_get_property_value(&block, "container-name").as_deref(),
            Some("FOO")
        );
        assert_eq!(
            inline_style_get_property_value(&block, "container-type").as_deref(),
            Some("size")
        );

        let before_invalid = crate::declaration_serialization::serialise_inline_style_block(&block);
        assert!(!inline_style_set_property(
            &mut block,
            DeclarationPropertyInput::new("container", "FOO / size inline-size"),
            CssomDeclarationPriority::Normal,
        ));
        assert_eq!(
            crate::declaration_serialization::serialise_inline_style_block(&block),
            before_invalid
        );
        assert_eq!(
            inline_style_remove_property(&mut block, "container").as_deref(),
            Some("FOO / size")
        );
        assert_eq!(
            inline_style_get_property_value(&block, "container-name"),
            None
        );
        assert_eq!(
            inline_style_get_property_value(&block, "container-type"),
            None
        );
    }

    #[test]
    fn container_shorthand_accepts_only_complete_standard_grammar() {
        for (specified, serialised) in [
            ("initial", "initial"),
            ("revert-layer", "revert-layer"),
            ("none / normal", "none"),
            ("inline-size", "inline-size"),
            ("inline-size / inline-size", "inline-size / inline-size"),
            ("size / size", "size / size"),
            ("foo bar / size", "foo bar / size"),
            ("  FOO  /size", "FOO / size"),
            ("normal / size", "normal / size"),
        ] {
            let mut block = parse_inline_style_block("");
            assert!(
                inline_style_set_property(
                    &mut block,
                    DeclarationPropertyInput::new("container", specified),
                    CssomDeclarationPriority::Normal,
                ),
                "{specified} must parse",
            );
            assert_eq!(
                inline_style_get_property_value(&block, "container").as_deref(),
                Some(serialised),
                "{specified} must use the canonical shorthand serialization",
            );
        }
        for invalid in [
            "none none",
            "none / inline-size normal",
            "none / inline-size block-size",
            "none, normal",
            "none / none",
            "10px / inline-size",
            "name / block-size",
            "none / size style",
        ] {
            let mut block = parse_inline_style_block("");
            assert!(
                !inline_style_set_property(
                    &mut block,
                    DeclarationPropertyInput::new("container", invalid),
                    CssomDeclarationPriority::Normal,
                ),
                "{invalid} must be rejected",
            );
            assert_eq!(inline_style_get_property_value(&block, "container"), None);
        }
    }

    #[test]
    fn radial_shape_position_preserves_specified_syntax() {
        let block = parse_inline_style_block("shape-outside: circle(1in at bottom 2in right 1in)");
        assert_eq!(
            inline_style_get_property_value(&block, "shape-outside").as_deref(),
            Some("circle(1in at right 1in bottom 2in)")
        );
        let invalid = parse_inline_style_block("shape-outside: circle(1in at 50% left)");
        assert_eq!(
            inline_style_get_property_value(&invalid, "shape-outside"),
            None
        );
    }

    #[test]
    fn rule_declaration_mutation_retains_typed_shorthand_values() {
        let block = parse_style_rule_declarations(DeclarationInput::new(
            "margin-top: 1px; margin-right: 2px; margin-bottom: 3px; margin-left: 4px",
        ));
        assert_eq!(block.shorthand_values()[0].name(), "margin");
        assert_eq!(block.shorthand_values()[0].value(), "1px 2px 3px 4px");

        let updated = mutate_rule_declaration_block(
            &block,
            "margin-left",
            "5px",
            CssomDeclarationPriority::Normal,
            CssomDeclarationContext::Style,
        )
        .expect("the longhand mutation must parse");
        let margin = updated
            .shorthand_values()
            .iter()
            .find(|declaration| declaration.name() == "margin")
            .expect("the shorthand must remain observable");
        assert_eq!(margin.value(), "1px 2px 3px 5px");
    }

    #[test]
    fn rule_mutation_retains_pending_shorthand_longhands() {
        let mut block =
            parse_style_rule_declarations(DeclarationInput::new("transition:var(--timing)"));
        for (property, value) in [
            ("transition-delay", "1s"),
            ("color", "red"),
            ("transition-property", ""),
            ("opacity", "0.5"),
        ] {
            block = mutate_rule_declaration_block(
                &block,
                property,
                value,
                CssomDeclarationPriority::Normal,
                CssomDeclarationContext::Style,
            )
            .expect("pending longhands must survive a declaration mutation");
            let duration = block
                .declarations()
                .iter()
                .find(|value| value.name() == "transition-duration")
                .unwrap();
            assert_eq!(duration.value(), "");
            let source = duration.pending_substitution().unwrap();
            assert_eq!(source.tokens(), "var(--timing)");
            assert_eq!(source.shorthand().schema().name, "transition");
        }
        assert_eq!(
            block
                .declarations()
                .iter()
                .find(|value| value.name() == "transition-delay")
                .map(stylo_cssom_model::RuleDeclaration::value),
            Some("1s")
        );
        assert!(
            block
                .declarations()
                .iter()
                .all(|value| value.name() != "transition-property")
        );
    }

    #[test]
    fn position_try_shorthand_uses_the_typed_anchor_grammar() {
        let mut block = parse_inline_style_block("");
        assert!(inline_style_set_property(
            &mut block,
            DeclarationPropertyInput::new(
                "position-try",
                "most-inline-size flip-inline flip-block, --fallback",
            ),
            CssomDeclarationPriority::Normal,
        ));

        assert_eq!(
            inline_style_get_property_value(&block, "position-try").as_deref(),
            Some("most-inline-size flip-inline flip-block, --fallback")
        );
        assert_eq!(
            inline_style_get_property_value(&block, "position-try-order").as_deref(),
            Some("most-inline-size")
        );
        assert_eq!(
            inline_style_get_property_value(&block, "position-try-fallbacks").as_deref(),
            Some("flip-inline flip-block, --fallback")
        );
    }
}
