use servo_arc::Arc;
use style::{
    properties::PropertyDeclarationBlock,
    shared_lock::{Locked, SharedRwLock, SharedRwLockReadGuard},
    stylesheets::{
        CssRule, CssRules, DocumentStyleSheet, PositionTryRule, Stylesheet, StylesheetInDocument,
    },
};
use stylo_cssom_model::{
    RuleDeclarationBlock, RuleDeclarationDomain, RuleGrammar, RuleNode, RuleSourceStamp,
};

use crate::declaration_parser::{CssomDeclarationContext, stylo_rule_declaration_block};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationLoweringError {
    RuleTopology,
    RuleGrammar,
    DeclarationContext,
    DeclarationValue,
}

pub struct NativePositionTrySource {
    pub rule: Arc<Locked<PositionTryRule>>,
    pub source: RuleSourceStamp,
}

pub struct LoweredStylesheet {
    pub stylesheet: DocumentStyleSheet,
    pub position_try_sources: Vec<NativePositionTrySource>,
}

fn has_pending(block: &RuleDeclarationBlock) -> bool {
    block
        .declarations()
        .iter()
        .any(|declaration| declaration.pending_substitution().is_some())
}

fn requires_declaration_lowering(block: &RuleDeclarationBlock) -> bool {
    has_pending(block)
        || matches!(
            block.domain(),
            RuleDeclarationDomain::Style
                | RuleDeclarationDomain::Nested
                | RuleDeclarationDomain::PositionTry
                | RuleDeclarationDomain::Keyframe
                | RuleDeclarationDomain::Page
                | RuleDeclarationDomain::Margin
        ) && block.declarations().iter().any(|declaration| {
            declaration.source_base_url().is_some() || declaration.source_referrer().is_some()
        })
}

pub fn has_pending_rules(rules: &[RuleNode]) -> bool {
    rules.iter().any(|rule| {
        rule.payload().declaration_block().is_some_and(has_pending)
            || has_pending_rules(rule.payload().nested())
    })
}

pub fn requires_source_lowering(rules: &[RuleNode]) -> bool {
    rules.iter().any(|rule| {
        rule.grammar() == RuleGrammar::PositionTry
            || rule.payload().imported_scope()
            || rule
                .payload()
                .declaration_block()
                .is_some_and(requires_declaration_lowering)
            || requires_source_lowering(rule.payload().nested())
    })
}

fn serialize_component_values<'i>(
    input: &mut cssparser::Parser<'i, '_>,
    output: &mut String,
) -> Result<(), cssparser::ParseError<'i, ()>> {
    use cssparser::{ToCss, Token};
    while let Ok(token) = input.next_including_whitespace_and_comments().cloned() {
        token.to_css(output).expect("writing a String cannot fail");
        let closing = match token {
            Token::Function(_) | Token::ParenthesisBlock => ')',
            Token::SquareBracketBlock => ']',
            Token::CurlyBracketBlock => '}',
            _ => continue,
        };
        input.parse_nested_block(|nested| serialize_component_values(nested, output))?;
        output.push(closing);
    }
    Ok(())
}

fn split_rule_block(css: &str) -> Option<(&str, std::borrow::Cow<'_, str>)> {
    let mut input = crate::rule_parser::stylesheet_parser_input(css);
    let mut parser = cssparser::Parser::new(&mut input);
    parser
        .parse_until_before(cssparser::Delimiter::CurlyBracketBlock, |prelude| {
            while prelude.next_including_whitespace_and_comments().is_ok() {}
            Ok::<_, cssparser::ParseError<'_, ()>>(())
        })
        .ok()?;
    let start = parser.position().byte_index();
    if !matches!(
        parser.next_including_whitespace_and_comments().ok()?,
        cssparser::Token::CurlyBracketBlock
    ) {
        return None;
    }
    let body_start = parser.position().byte_index();
    let end = parser
        .parse_nested_block(|body| {
            while body.next_including_whitespace_and_comments().is_ok() {}
            Ok::<_, cssparser::ParseError<'_, ()>>(body.position().byte_index())
        })
        .ok()?;
    let body = &css[body_start..end];
    let body = if css.as_bytes().get(end) == Some(&b'}') {
        std::borrow::Cow::Borrowed(body)
    } else {
        // CSS Syntax closes blocks/functions/strings at EOF. Tokenize before
        // appending generated delimiters: an authored trailing escape must
        // consume EOF, not the newly appended closing brace.
        let mut input = crate::rule_parser::stylesheet_parser_input(body);
        let mut repaired = String::new();
        serialize_component_values(&mut cssparser::Parser::new(&mut input), &mut repaired).ok()?;
        std::borrow::Cow::Owned(repaired)
    };
    Some((&css[..start], body))
}

pub fn expand_view_transition_rules(
    source: &[RuleNode],
) -> Result<Vec<RuleNode>, DeclarationLoweringError> {
    let mut output = Vec::new();
    for rule in source {
        let css = rule.projection_serialization();
        if rule.grammar() == RuleGrammar::Style {
            let (selector, body) =
                split_rule_block(&css).ok_or(DeclarationLoweringError::RuleGrammar)?;
            let expansion = crate::view_transition_root_rewrite::expand_style_rule(selector, &body);
            for (index, selector) in expansion.source_selectors.iter().enumerate() {
                let projected = if index == 0 {
                    rule.clone()
                } else {
                    rule.clone()
                        .with_cssom_selector(selector.as_str())
                        .ok_or(DeclarationLoweringError::RuleGrammar)?
                };
                output
                    .push(projected.with_projection_serialization(format!("{selector}{{{body}}}")));
            }
            if !expansion.generated.is_empty() {
                let generated =
                    crate::authored_rules::ParsedStylesheet::parse(&expansion.generated)
                        .map_err(|_| DeclarationLoweringError::RuleGrammar)?;
                output.extend_from_slice(generated.rule_nodes());
            }
        } else if rule.payload().nested().is_empty() || rule.grammar() == RuleGrammar::Keyframes {
            output.push(rule.clone());
        } else {
            let nested = expand_view_transition_rules(rule.payload().nested())?;
            let css = crate::view_transition_root_rewrite::rewrite_view_transition_root(&css)
                .into_owned();
            output.push(rule.clone().with_projected_nested(nested, css));
        }
    }
    Ok(output)
}

fn native_declarations(
    source: &RuleDeclarationBlock,
) -> Result<PropertyDeclarationBlock, DeclarationLoweringError> {
    let context = match source.domain() {
        RuleDeclarationDomain::Style
        | RuleDeclarationDomain::Nested
        | RuleDeclarationDomain::PositionTry => CssomDeclarationContext::Style,
        RuleDeclarationDomain::Keyframe => CssomDeclarationContext::Keyframe,
        RuleDeclarationDomain::Page => CssomDeclarationContext::Page,
        RuleDeclarationDomain::Margin => CssomDeclarationContext::Margin,
        RuleDeclarationDomain::FontFaceDescriptor
        | RuleDeclarationDomain::FunctionDescriptors
        | RuleDeclarationDomain::Private => {
            return Err(DeclarationLoweringError::DeclarationContext);
        },
    };
    stylo_rule_declaration_block(source, context).ok_or(DeclarationLoweringError::DeclarationValue)
}

fn merge_native_pending(
    source: &RuleDeclarationBlock,
    projected: &PropertyDeclarationBlock,
) -> Result<PropertyDeclarationBlock, DeclarationLoweringError> {
    let restored = native_declarations(source)?;
    let mut retained_context =
        style::properties::declaration_block::PropertyDeclarationIdSet::default();
    for declaration in source.declarations().iter().filter(|declaration| {
        declaration.source_base_url().is_some() || declaration.source_referrer().is_some()
    }) {
        let property =
            style::properties::PropertyId::parse_enabled_for_all_content(declaration.name())
                .map_err(|()| DeclarationLoweringError::DeclarationValue)?;
        match property.as_shorthand() {
            Ok(shorthand) => {
                for longhand in shorthand.longhands() {
                    retained_context
                        .insert(style::properties::PropertyDeclarationId::Longhand(longhand));
                }
            },
            Err(id) => {
                retained_context.insert(id);
            },
        }
    }
    let mut result = PropertyDeclarationBlock::new();
    for (declaration, importance) in restored.declaration_importance_iter() {
        let projected_declaration = projected
            .declaration_importance_iter()
            .find(|(candidate, _)| candidate.id() == declaration.id());
        if let (
            style::properties::PropertyDeclaration::WithVariables(source),
            Some((style::properties::PropertyDeclaration::WithVariables(projected), importance)),
        ) = (declaration, projected_declaration)
            && source.value.from_shorthand().is_none()
            && retained_context.contains(declaration.id())
        {
            let mut projected = projected.clone();
            projected.value = Arc::new(
                projected
                    .value
                    .with_url_data(source.value.variable_value().url_data.clone()),
            );
            let _ = result.push(
                style::properties::PropertyDeclaration::WithVariables(projected),
                importance,
            );
            continue;
        }
        let selected = if (retained_context.contains(declaration.id())
            && projected_declaration.is_some()
            && !matches!(
                declaration,
                style::properties::PropertyDeclaration::WithVariables(_)
            ))
            || matches!(declaration, style::properties::PropertyDeclaration::WithVariables(value)
            if value.value.from_shorthand().is_some())
        {
            Some((declaration, importance))
        } else {
            projected_declaration
        };
        if let Some((declaration, importance)) = selected {
            let _ = result.push(declaration.clone(), importance);
        }
    }
    for (declaration, importance) in projected.declaration_importance_iter() {
        if !result.contains(declaration.id()) {
            let _ = result.push(declaration.clone(), importance);
        }
    }
    Ok(result)
}

fn merge_native_declarations(
    source: &RuleDeclarationBlock,
    projected: &PropertyDeclarationBlock,
) -> Result<PropertyDeclarationBlock, DeclarationLoweringError> {
    if requires_declaration_lowering(source) {
        merge_native_pending(source, projected)
    } else {
        Ok(projected.clone())
    }
}

fn native_block(
    rule: &CssRule,
    guard: &SharedRwLockReadGuard<'_>,
) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
    Some(match rule {
        CssRule::Style(rule) => rule.read_with(guard).block.clone(),
        CssRule::NestedDeclarations(rule) => rule.read_with(guard).block.clone(),
        CssRule::Page(rule) => rule.read_with(guard).block.clone(),
        CssRule::Margin(rule) => rule.block.clone(),
        CssRule::PositionTry(rule) => rule.read_with(guard).block.clone(),
        _ => return None,
    })
}

fn native_children(
    rule: &CssRule,
    guard: &SharedRwLockReadGuard<'_>,
) -> Option<Arc<Locked<CssRules>>> {
    Some(match rule {
        CssRule::Style(rule) => return rule.read_with(guard).rules.clone(),
        CssRule::Page(rule) => rule.read_with(guard).rules.clone(),
        CssRule::Media(rule) => rule.rules.clone(),
        CssRule::Supports(rule) => rule.rules.clone(),
        CssRule::Container(rule) => rule.rules.clone(),
        CssRule::LayerBlock(rule) => rule.rules.clone(),
        CssRule::Scope(rule) => rule.rules.clone(),
        CssRule::StartingStyle(rule) => rule.rules.clone(),
        CssRule::Document(rule) => rule.rules.clone(),
        CssRule::Navigation(rule) => rule.rules.clone(),
        CssRule::When(rule) => rule.rules.clone(),
        CssRule::Else(rule) => rule.rules.clone(),
        CssRule::Mixin(rule) => return rule.rules.clone(),
        CssRule::Function(rule) => rule.rules.clone(),
        _ => return None,
    })
}

fn parser_for_input<'a, 'i>(
    parser: style::stylesheets::TopLevelRuleParser<'a, '_>,
) -> Result<style::stylesheets::TopLevelRuleParser<'a, 'i>, DeclarationLoweringError> {
    if parser.declaration_parser_state.has_parsed_declarations() {
        return Err(DeclarationLoweringError::DeclarationContext);
    }
    // Drop transient declaration errors before the next rule input.
    Ok(style::stylesheets::TopLevelRuleParser {
        declaration_parser_state: Default::default(),
        ..parser
    })
}

#[derive(Default)]
struct SourceParseMetadata {
    namespaces: style::stylesheets::Namespaces,
    source_map_url: Option<String>,
    source_url: Option<String>,
}

fn lower_rule_list(
    source: &[RuleNode],
    contents: &style::stylesheets::StylesheetContents,
    lock: &SharedRwLock,
    nesting: style::parser::NestingContext,
    position_try_sources: &mut Vec<RuleSourceStamp>,
    namespaces: &style::stylesheets::Namespaces,
    mut source_metadata: Option<&mut SourceParseMetadata>,
) -> Result<Vec<CssRule>, DeclarationLoweringError> {
    use style::{
        parser::ParserContext,
        stylesheets::{AllowImportRules, State, TopLevelRuleParser},
    };
    let mut context = ParserContext::new(
        contents.origin,
        &contents.url_data,
        None,
        style_traits::ParsingMode::DEFAULT,
        contents.quirks_mode,
        std::borrow::Cow::Borrowed(namespaces),
        None,
        source_metadata.as_ref().map(|_| &contents.use_counters),
    );
    context.nesting_context = nesting;
    let mut parser_state = TopLevelRuleParser {
        shared_lock: lock,
        loader: None,
        context,
        state: if nesting.rule_types.is_empty() {
            State::Start
        } else {
            State::Body
        },
        dom_error: None,
        insert_rule_context: None,
        allow_import_rules: AllowImportRules::Yes,
        wants_first_declaration_block: false,
        first_declaration_block: Default::default(),
        declaration_parser_state: Default::default(),
        function_declarations: None,
        error_reporting_state: Default::default(),
        rules: Vec::new(),
    };
    for source in source {
        let css = source.projection_serialization();
        let mut parser = parser_for_input(parser_state)?;
        'source_rule: {
            if source.grammar() == RuleGrammar::FunctionDeclarations {
                use style::stylesheets::function_rule::{FunctionDescriptor, FunctionDescriptors};
                let block = source
                    .payload()
                    .declaration_block()
                    .ok_or(DeclarationLoweringError::DeclarationContext)?;
                let mut descriptors = FunctionDescriptors::default();
                for declaration in block.declarations() {
                    let retained_url = declaration
                        .source_base_url()
                        .and_then(|url| url::Url::parse(url).ok())
                        .map(|url| {
                            style::stylesheets::UrlExtraData::from(url).with_referrer_source(
                                declaration
                                    .source_referrer()
                                    .and_then(|url| url::Url::parse(url).ok()),
                            )
                        });
                    let mut context = parser.context.clone();
                    if let Some(url) = &retained_url {
                        context.url_data = url;
                    }
                    let mut input =
                        crate::rule_parser::stylesheet_parser_input(declaration.value());
                    let descriptor = FunctionDescriptor::parse(
                        &context,
                        declaration.name().into(),
                        &mut cssparser::Parser::new(&mut input),
                    )
                    .map_err(|_| DeclarationLoweringError::DeclarationValue)?;
                    descriptors.push(descriptor);
                }
                parser.rules.push(CssRule::FunctionDeclarations(Arc::new(
                    style::stylesheets::FunctionDeclarationsRule {
                        descriptors,
                        source_location: cssparser::SourceLocation { line: 0, column: 1 },
                    },
                )));
                break 'source_rule;
            }
            if source.grammar() == RuleGrammar::NestedDeclarations {
                let source = source
                    .payload()
                    .declaration_block()
                    .ok_or(DeclarationLoweringError::DeclarationContext)?;
                let mut input = cssparser::ParserInput::new(&css);
                let mut input = cssparser::Parser::new(&mut input);
                let projected =
                    style::properties::declaration_block::parse_property_declaration_list(
                        &parser.context,
                        &mut input,
                        &[],
                    );
                let block = Arc::new(lock.wrap(merge_native_declarations(source, &projected)?));
                parser
                    .rules
                    .push(CssRule::NestedDeclarations(Arc::new(lock.wrap(
                        style::stylesheets::NestedDeclarationsRule {
                            block,
                            source_location: cssparser::SourceLocation { line: 0, column: 1 },
                        },
                    ))));
                break 'source_rule;
            }
            let first = parser.rules.len();
            let mut input = crate::rule_parser::stylesheet_parser_input(&css);
            let mut input = cssparser::Parser::new(&mut input);
            for result in cssparser::StyleSheetParser::new(&mut input, &mut parser) {
                if result.is_err() {
                    continue;
                }
            }
            if let Some(metadata) = source_metadata.as_deref_mut() {
                if let Some(url) = input.current_source_map_url() {
                    metadata.source_map_url = Some(url.to_owned());
                }
                if let Some(url) = input.current_source_url() {
                    metadata.source_url = Some(url.to_owned());
                }
            }
            let emitted = &parser.rules[first..];
            if !requires_source_lowering(std::slice::from_ref(source)) || emitted.is_empty() {
                break 'source_rule;
            }
            let [native] = emitted else {
                return Err(DeclarationLoweringError::RuleTopology);
            };
            lower_rule(
                source,
                native,
                contents,
                lock,
                nesting,
                position_try_sources,
                source_metadata
                    .as_ref()
                    .map_or(namespaces, |_| parser.context.namespaces.as_ref()),
            )?;
            if source.payload().imported_scope() {
                let CssRule::Scope(scope) = &parser.rules[first] else {
                    return Err(DeclarationLoweringError::RuleGrammar);
                };
                parser.rules[first] = CssRule::Scope(Arc::new(style::stylesheets::ScopeRule {
                    bounds: scope.bounds.clone(),
                    rules: scope.rules.clone(),
                    source_location: scope.source_location.clone(),
                    imported: true,
                }));
            }
        }
        parser_state = parser_for_input(parser)?;
    }
    if let Some(metadata) = source_metadata {
        metadata.namespaces = parser_state.context.namespaces.into_owned();
    }
    Ok(parser_state.rules)
}

fn lower_rule(
    source: &RuleNode,
    native: &CssRule,
    contents: &style::stylesheets::StylesheetContents,
    lock: &SharedRwLock,
    mut nesting: style::parser::NestingContext,
    position_try_sources: &mut Vec<RuleSourceStamp>,
    namespaces: &style::stylesheets::Namespaces,
) -> Result<(), DeclarationLoweringError> {
    if source.grammar() != crate::rule_parser::stylo_rule_grammar(native) {
        return Err(DeclarationLoweringError::RuleGrammar);
    }
    if let CssRule::Page(page) = native
        && requires_source_lowering(source.payload().nested())
    {
        let (destination, declarations) = {
            let guard = lock.read();
            let page = page.read_with(&guard);
            let mut declarations = page.block.read_with(&guard).clone();
            for child in &page.rules.read_with(&guard).0 {
                if let CssRule::NestedDeclarations(child) = child {
                    for (declaration, importance) in child
                        .read_with(&guard)
                        .block
                        .read_with(&guard)
                        .declaration_importance_iter()
                    {
                        let _ = declarations.push(declaration.clone(), importance);
                    }
                }
            }
            (page.block.clone(), declarations)
        };
        *destination.write_with(&mut lock.write()) = declarations;
    }
    if matches!(native, CssRule::PositionTry(_)) {
        position_try_sources.push(
            source
                .payload()
                .source_stamp()
                .ok_or(DeclarationLoweringError::RuleTopology)?,
        );
    }
    if let Some(block) = source
        .payload()
        .declaration_block()
        .filter(|block| requires_declaration_lowering(block))
    {
        let destination = native_block(native, &lock.read())
            .ok_or(DeclarationLoweringError::DeclarationContext)?;
        let value = merge_native_declarations(block, destination.read_with(&lock.read()))?;
        *destination.write_with(&mut lock.write()) = value;
    }
    let children = source.payload().nested();
    if !source.payload().imported_scope() && !requires_source_lowering(children) {
        return Ok(());
    }
    if let CssRule::Keyframes(rule) = native {
        let keyframe_context = (namespaces.default != contents.namespaces.default
            || namespaces.prefixes != contents.namespaces.prefixes)
            .then(|| {
                let mut context = style::stylesheets::StylesheetContents::from_str(
                    "",
                    contents.url_data.clone(),
                    contents.origin,
                    lock,
                    None,
                    None,
                    contents.quirks_mode,
                    style::stylesheets::AllowImportRules::Yes,
                    None,
                );
                Arc::get_mut(&mut context)
                    .expect("new keyframe context is unique")
                    .namespaces = namespaces.clone();
                context
            });
        let contents = keyframe_context.as_deref().unwrap_or(contents);
        let mut frames = Vec::with_capacity(children.len());
        for source in children {
            let css = source.projection_serialization();
            let frame = style::stylesheets::keyframes_rule::Keyframe::parse(&css, contents, lock)
                .map_err(|_| DeclarationLoweringError::RuleGrammar)?;
            if let Some(block) = source
                .payload()
                .declaration_block()
                .filter(|block| requires_declaration_lowering(block))
            {
                let destination = frame.read_with(&lock.read()).block.clone();
                let value = merge_native_declarations(block, destination.read_with(&lock.read()))?;
                *destination.write_with(&mut lock.write()) = value;
            }
            frames.push(frame);
        }
        rule.write_with(&mut lock.write()).keyframes = frames;
    } else {
        if source.payload().imported_scope() {
            nesting = style::parser::NestingContext::new_from_rule(None);
        } else {
            nesting.save(native.rule_type());
        }
        let rules = lower_rule_list(
            children,
            contents,
            lock,
            nesting,
            position_try_sources,
            namespaces,
            None,
        )?;
        let destination = native_children(native, &lock.read());
        if let Some(destination) = destination {
            destination.write_with(&mut lock.write()).0 = rules;
        } else if let CssRule::Style(rule) = native {
            rule.write_with(&mut lock.write()).rules = Some(Arc::new(lock.wrap(CssRules(rules))));
        } else {
            return Err(DeclarationLoweringError::DeclarationContext);
        }
    }
    Ok(())
}

fn replacement_rule_context(
    stylesheet: &DocumentStyleSheet,
    lock: &SharedRwLock,
    private_lock: &SharedRwLock,
) -> Arc<style::stylesheets::StylesheetContents> {
    let guard = lock.read();
    let original = stylesheet.contents(&guard);
    let mut contents = style::stylesheets::StylesheetContents::from_str(
        "",
        original.url_data.clone(),
        original.origin,
        private_lock,
        None,
        None,
        original.quirks_mode,
        style::stylesheets::AllowImportRules::Yes,
        None,
    );
    let target = Arc::get_mut(&mut contents).expect("new lowering context is unique");
    target.namespaces = original.namespaces.clone();
    target.source_map_url = original.source_map_url.clone();
    target.source_url = original.source_url.clone();
    target.use_counters = original.use_counters.clone();
    contents
}

/// Constructs typed source rules without parsing a whole-sheet serialization first.
///
/// `metadata` must have no rules. Its origin, URL data, quirks, media, disabled
/// state and scope are retained, while the source parser supplies namespaces,
/// source directives and use counters. The caller lock may already be read-held.
/// Callers preserving the original non-contextual rule source locations should
/// retain `lower_pending_declarations` for rules not requiring source lowering.
pub fn lower_source_rules(
    rules: &[RuleNode],
    metadata: DocumentStyleSheet,
    lock: &SharedRwLock,
) -> Result<LoweredStylesheet, DeclarationLoweringError> {
    let guard = lock.read();
    if !metadata.contents(&guard).rules(&guard).is_empty() {
        return Err(DeclarationLoweringError::RuleTopology);
    }
    drop(guard);
    lower_stylesheet_rules(rules, metadata, lock, true)
}

pub fn lower_pending_declarations(
    rules: &[RuleNode],
    stylesheet: DocumentStyleSheet,
    lock: &SharedRwLock,
) -> Result<LoweredStylesheet, DeclarationLoweringError> {
    if !requires_source_lowering(rules) {
        return Ok(LoweredStylesheet {
            stylesheet,
            position_try_sources: Vec::new(),
        });
    }
    lower_stylesheet_rules(rules, stylesheet, lock, false)
}

fn lower_stylesheet_rules(
    rules: &[RuleNode],
    stylesheet: DocumentStyleSheet,
    lock: &SharedRwLock,
    parse_metadata: bool,
) -> Result<LoweredStylesheet, DeclarationLoweringError> {
    let private_lock = SharedRwLock::new();
    let mut contents = replacement_rule_context(&stylesheet, lock, &private_lock);
    let media = stylesheet.0.media.clone();
    let disabled = stylesheet.0.disabled();
    let scope = stylesheet.1;
    drop(stylesheet);
    let mut source_stamps = Vec::new();
    let mut parsed = SourceParseMetadata::default();
    let rules = lower_rule_list(
        rules,
        &contents,
        &private_lock,
        style::parser::NestingContext::new_from_rule(None),
        &mut source_stamps,
        &contents.namespaces,
        parse_metadata.then_some(&mut parsed),
    )?;
    if parse_metadata {
        let target = Arc::get_mut(&mut contents).expect("new lowering context is unique");
        target.namespaces = parsed.namespaces;
        if parsed.source_map_url.is_some() {
            target.source_map_url = parsed.source_map_url;
        }
        if parsed.source_url.is_some() {
            target.source_url = parsed.source_url;
        }
    }
    contents.rules.write_with(&mut private_lock.write()).0 = rules;
    let guard = private_lock.read();
    let contents = contents.deep_clone(lock, None, &guard);
    fn collect_position_try_rules(
        rules: &[CssRule],
        guard: &SharedRwLockReadGuard<'_>,
        output: &mut Vec<Arc<Locked<PositionTryRule>>>,
    ) {
        for rule in rules {
            if let CssRule::PositionTry(rule) = rule {
                output.push(rule.clone());
            } else if let Some(children) = native_children(rule, guard) {
                collect_position_try_rules(&children.read_with(guard).0, guard, output);
            }
        }
    }
    let mut native_rules = Vec::new();
    let final_guard = lock.read();
    collect_position_try_rules(
        &contents.rules.read_with(&final_guard).0,
        &final_guard,
        &mut native_rules,
    );
    drop(final_guard);
    if native_rules.len() != source_stamps.len() {
        return Err(DeclarationLoweringError::RuleTopology);
    }
    let position_try_sources = native_rules
        .into_iter()
        .zip(source_stamps)
        .map(|(rule, source)| NativePositionTrySource { rule, source })
        .collect();
    let stylesheet = DocumentStyleSheet(
        Arc::new(Stylesheet {
            contents: lock.wrap(contents),
            shared_lock: lock.clone(),
            media,
            disabled: std::sync::atomic::AtomicBool::new(disabled),
        }),
        scope,
    );
    Ok(LoweredStylesheet {
        stylesheet,
        position_try_sources,
    })
}

#[cfg(test)]
mod tests {
    use style::shared_lock::{DeepCloneWithLock, ToCssWithGuard};
    use style::stylesheets::StylesheetInDocument;

    fn source_metadata(
        lock: &style::shared_lock::SharedRwLock,
        css: &str,
        origin: style::stylesheets::Origin,
    ) -> style::stylesheets::DocumentStyleSheet {
        use style::stylesheets::{AllowImportRules, DocumentStyleSheet, Stylesheet, UrlExtraData};
        let sheet = Stylesheet::from_str(
            css,
            UrlExtraData::from(url::Url::parse("https://source.test/path/style.css").unwrap())
                .with_referrer_source(Some(
                    url::Url::parse("https://response.test/styles.css").unwrap(),
                )),
            origin,
            servo_arc::Arc::new(
                lock.wrap(
                    crate::media_list::CssomMediaList::parse("print and (min-width: 1px)")
                        .parsed()
                        .clone(),
                ),
            ),
            lock.clone(),
            None,
            None,
            selectors::matching::QuirksMode::Quirks,
            AllowImportRules::Yes,
        );
        sheet.set_disabled(true);
        DocumentStyleSheet(
            servo_arc::Arc::new(sheet),
            Some(style::stylesheets::scope_rule::ImplicitScopeRoot::DocumentElement),
        )
    }

    fn assert_native_source_data(
        before: &[style::stylesheets::CssRule],
        after: &[style::stylesheets::CssRule],
        guard: &style::shared_lock::SharedRwLockReadGuard<'_>,
    ) {
        use style::stylesheets::CssRule;
        assert_eq!(before.len(), after.len());
        for (before, after) in before.iter().zip(after) {
            assert_eq!(before.rule_type(), after.rule_type());
            if let (CssRule::Style(before), CssRule::Style(after)) = (before, after) {
                let before = before.read_with(guard);
                let after = after.read_with(guard);
                assert_eq!(before.selectors, after.selectors);
                assert_eq!(before.source_location, after.source_location);
            }
            if let (CssRule::Keyframes(before), CssRule::Keyframes(after)) = (before, after) {
                let before = before.read_with(guard);
                let after = after.read_with(guard);
                assert_eq!(before.keyframes.len(), after.keyframes.len());
                for (before, after) in before.keyframes.iter().zip(&after.keyframes) {
                    let before = before.read_with(guard);
                    let after = after.read_with(guard);
                    assert_eq!(before.selector, after.selector);
                    assert_eq!(before.source_location, after.source_location);
                    assert_eq!(
                        before
                            .block
                            .read_with(guard)
                            .declaration_importance_iter()
                            .collect::<Vec<_>>(),
                        after
                            .block
                            .read_with(guard)
                            .declaration_importance_iter()
                            .collect::<Vec<_>>()
                    );
                }
            }
            if let (CssRule::Scope(before), CssRule::Scope(after)) = (before, after) {
                assert_eq!(before.imported, after.imported);
            }
            match (
                super::native_block(before, guard),
                super::native_block(after, guard),
            ) {
                (Some(before), Some(after)) => {
                    assert_eq!(
                        before
                            .read_with(guard)
                            .declaration_importance_iter()
                            .collect::<Vec<_>>(),
                        after
                            .read_with(guard)
                            .declaration_importance_iter()
                            .collect::<Vec<_>>()
                    );
                },
                (None, None) => {},
                _ => panic!("native declaration block topology differs"),
            }
            match (
                super::native_children(before, guard),
                super::native_children(after, guard),
            ) {
                (Some(before), Some(after)) => assert_native_source_data(
                    &before.read_with(guard).0,
                    &after.read_with(guard).0,
                    guard,
                ),
                (None, None) => {},
                _ => panic!("native child topology differs"),
            }
        }
    }

    #[test]
    fn typed_source_constructor_matches_full_parse_lowering() {
        use style::stylesheets::Origin;
        crate::context::initialise_required_servo_style_prefs();
        let cases = [
            "@namespace url(urn:default);@namespace s url(urn:svg);s|a{color:var(--c);background:url(icon.png)}@position-try --fallback{left:3px}",
            "@layer first;@import url(other.css);@namespace s url(urn:svg);@layer first{@media all{s|a{color:var(--c);background:url(icon.png)}}}@supports(display:block){p{margin:var(--m)}}@position-try --fallback{top:2px}",
            "p{color:var(--c)}@namespace late url(urn:late);@unknown nope;p{color:blue;invalid-property:foo}@position-try --fallback{left:4px}",
            "@font-face{font-family:Test;src:url(font.woff)}@keyframes pulse{from{color:var(--c)}to{color:blue}}@page{margin:var(--m);@top-left{content:var(--label)}}p{& .child{color:var(--c)}}@position-try --fallback{top:5px}",
            "@namespace s url(urn:attributes);@keyframes namespaced{from{color:attr(s|colour type(<color>),red);--name:attr(s|label type(*),fallback);margin:var(--m)}to{color:blue}}@position-try --fallback{top:6px}",
        ];
        for origin in [Origin::UserAgent, Origin::User, Origin::Author] {
            for css in cases {
                let parsed = crate::authored_rules::ParsedStylesheet::parse(css).unwrap();
                let base = crate::CssStylesheetBaseUrl::from_absolute(
                    url::Url::parse("https://source.test/path/style.css").unwrap(),
                );
                let mut rules = crate::authored_rules::project_stylesheet_pending_contexts(
                    parsed.rule_nodes(),
                    &base,
                    None,
                );
                if let Some(first) = rules.first_mut() {
                    *first = first.clone().with_projection_serialization(format!(
                        "{}/*# sourceMappingURL=first.map */",
                        first.projection_serialization()
                    ));
                }
                if let Some(last) = rules.last_mut() {
                    *last = last.clone().with_projection_serialization(format!(
                        "{}/*# sourceMappingURL=last.map *//*# sourceURL=authored.css */",
                        last.projection_serialization()
                    ));
                }
                rules.push(stylo_cssom_model::RuleNode::imported_scope(
                    &stylo_cssom_model::RuleImportScope::Explicit("(.card)".into()),
                    [stylo_cssom_model::RuleNode::style(".card", "color:green")],
                ));
                let root = stylo_cssom_model::InternalStylesheetRoot::new(
                    stylo_cssom_model::StyleOrigin::Author,
                    rules,
                );
                let lock = style::shared_lock::SharedRwLock::new();
                let old = super::lower_pending_declarations(
                    root.rules(),
                    source_metadata(&lock, &root.projection_serialization(), origin),
                    &lock,
                )
                .unwrap();
                // This read guard is also held by the real shadow stylesheet caller.
                let guard = lock.read();
                let metadata = source_metadata(&lock, "", origin);
                let alias = metadata.clone();
                let new = super::lower_source_rules(root.rules(), metadata, &lock).unwrap();
                assert!(alias.contents(&guard).rules(&guard).is_empty());
                assert!(servo_arc::Arc::ptr_eq(
                    &new.stylesheet.0.media,
                    &alias.0.media
                ));
                let before = old.stylesheet.contents(&guard);
                let after = new.stylesheet.contents(&guard);
                assert_eq!(
                    before
                        .rules(&guard)
                        .iter()
                        .map(|rule| rule.to_css_string(&guard))
                        .collect::<Vec<_>>(),
                    after
                        .rules(&guard)
                        .iter()
                        .map(|rule| rule.to_css_string(&guard))
                        .collect::<Vec<_>>(),
                    "{origin:?}: {css}"
                );
                assert_native_source_data(&before.rules(&guard), &after.rules(&guard), &guard);
                assert_eq!(before.namespaces.default, after.namespaces.default);
                assert_eq!(before.namespaces.prefixes, after.namespaces.prefixes);
                assert_eq!(before.source_map_url.as_deref(), Some("last.map"));
                assert_eq!(before.source_url.as_deref(), Some("authored.css"));
                assert_eq!(before.source_map_url, after.source_map_url);
                assert_eq!(before.source_url, after.source_url);
                assert_eq!(
                    format!("{:?}", before.use_counters),
                    format!("{:?}", after.use_counters)
                );
                assert_eq!(before.url_data, after.url_data);
                assert_eq!(before.origin, after.origin);
                assert_eq!(before.quirks_mode, after.quirks_mode);
                assert_eq!(old.stylesheet.0.disabled(), new.stylesheet.0.disabled());
                assert_eq!(old.stylesheet.1, new.stylesheet.1);
                assert_eq!(
                    old.position_try_sources
                        .iter()
                        .map(|source| source.source)
                        .collect::<Vec<_>>(),
                    new.position_try_sources
                        .iter()
                        .map(|source| source.source)
                        .collect::<Vec<_>>()
                );
                for source in new.position_try_sources {
                    let _ = source.rule.read_with(&guard);
                }
            }
        }
    }

    #[test]
    fn noncontextual_source_keeps_existing_fast_return_and_locations() {
        let parsed =
            crate::authored_rules::ParsedStylesheet::parse("p{color:red}\nspan{color:blue}")
                .unwrap();
        assert!(!super::requires_source_lowering(parsed.rule_nodes()));
        let lock = style::shared_lock::SharedRwLock::new();
        let original = source_metadata(
            &lock,
            "p{color:red}\nspan{color:blue}",
            style::stylesheets::Origin::Author,
        );
        let alias = original.clone();
        // The concrete consumer selects the new constructor only for contextual roots.
        let lowered =
            super::lower_pending_declarations(parsed.rule_nodes(), original, &lock).unwrap();
        assert!(servo_arc::Arc::ptr_eq(&lowered.stylesheet.0, &alias.0));
        let guard = lock.read();
        let style::stylesheets::CssRule::Style(second) =
            &lowered.stylesheet.contents(&guard).rules(&guard)[1]
        else {
            panic!("the second style remains native");
        };
        assert_eq!(second.read_with(&guard).source_location.line, 1);
    }

    #[test]
    fn typed_source_constructor_rejects_nonempty_metadata() {
        let lock = style::shared_lock::SharedRwLock::new();
        let original = source_metadata(&lock, "p{color:red}", style::stylesheets::Origin::Author);
        let alias = original.clone();
        assert!(matches!(
            super::lower_source_rules(&[], original, &lock),
            Err(super::DeclarationLoweringError::RuleTopology)
        ));
        let guard = lock.read();
        assert_eq!(alias.contents(&guard).rules(&guard).len(), 1);
    }

    #[test]
    fn replacement_context_omits_rules_and_preserves_metadata_and_aliases() {
        use servo_arc::Arc;
        use style::{
            shared_lock::{SharedRwLock, ToCssWithGuard},
            stylesheets::{
                AllowImportRules, CssRule, DocumentStyleSheet, Origin, Stylesheet,
                StylesheetContents, StylesheetInDocument, UrlExtraData,
                scope_rule::ImplicitScopeRoot,
            },
            use_counters::CustomUseCounter,
        };

        fn assert_metadata(actual: &StylesheetContents, expected: &StylesheetContents) {
            assert_eq!(actual.origin, expected.origin);
            assert_eq!(actual.url_data, expected.url_data);
            assert_eq!(actual.quirks_mode, expected.quirks_mode);
            assert_eq!(actual.namespaces.default, expected.namespaces.default);
            assert_eq!(actual.namespaces.prefixes, expected.namespaces.prefixes);
            assert_eq!(actual.source_map_url, expected.source_map_url);
            assert_eq!(actual.source_url, expected.source_url);
            assert!(
                actual
                    .use_counters
                    .custom
                    .recorded(CustomUseCounter::MaybeHasFullBaseUriDependency,)
            );
        }

        crate::context::initialise_required_servo_style_prefs();
        let css = concat!(
            "@namespace url(urn:default);",
            "@namespace s url(urn:svg);",
            "s|a{background-image:url(icon.png);color:red}",
            "@position-try --fallback{left:3px}",
        );
        let parsed = crate::authored_rules::ParsedStylesheet::parse(css).unwrap();
        let base = crate::CssStylesheetBaseUrl::from_absolute(
            url::Url::parse("https://styles.test/directory/style.css").unwrap(),
        );
        let source = crate::authored_rules::project_stylesheet_pending_contexts(
            parsed.rule_nodes(),
            &base,
            None,
        );
        let source = stylo_cssom_model::InternalStylesheetRoot::new(
            stylo_cssom_model::StyleOrigin::Author,
            source,
        );
        assert!(super::requires_source_lowering(source.rules()));
        let lock = SharedRwLock::new();
        let private_lock = SharedRwLock::new();
        let native = Stylesheet::from_str(
            css,
            UrlExtraData::from(url::Url::parse(base.as_str()).unwrap()).with_referrer_source(Some(
                url::Url::parse("https://response.test/style.css").unwrap(),
            )),
            Origin::Author,
            Arc::new(lock.wrap(style::media_queries::MediaList::empty())),
            lock.clone(),
            None,
            None,
            selectors::matching::QuirksMode::Quirks,
            AllowImportRules::Yes,
        );
        {
            let mut guard = lock.write();
            let contents = Arc::get_mut(native.contents.write_with(&mut guard)).unwrap();
            contents.source_map_url = Some("source.css.map".into());
            contents.source_url = Some("authored.css".into());
            contents
                .use_counters
                .custom
                .record(CustomUseCounter::MaybeHasFullBaseUriDependency);
        }
        native.set_disabled(true);
        let original =
            DocumentStyleSheet(Arc::new(native), Some(ImplicitScopeRoot::DocumentElement));
        let alias = original.clone();
        let (original_rules, original_style, original_css) = {
            let guard = lock.read();
            let contents = original.contents(&guard);
            assert_eq!(
                contents.namespaces.default.as_ref().unwrap().to_string(),
                "urn:default"
            );
            assert!(
                contents
                    .namespaces
                    .prefixes
                    .iter()
                    .any(|(prefix, namespace)| {
                        prefix.to_string() == "s" && namespace.to_string() == "urn:svg"
                    })
            );
            let CssRule::Style(rule) = &contents.rules(&guard)[2] else {
                panic!("the third original rule is the namespaced style");
            };
            (
                contents.rules.clone(),
                rule.clone(),
                contents
                    .rules(&guard)
                    .iter()
                    .map(|rule| rule.to_css_string(&guard))
                    .collect::<Vec<_>>(),
            )
        };
        let scratch = super::replacement_rule_context(&original, &lock, &private_lock);
        {
            let private_guard = private_lock.read();
            let original_guard = lock.read();
            assert!(
                scratch.rules(&private_guard).is_empty(),
                "lowering must not clone the rule tree it replaces"
            );
            assert_metadata(&scratch, original.contents(&original_guard));
        }
        drop(scratch);
        let lowered = super::lower_pending_declarations(source.rules(), original, &lock).unwrap();
        let guard = lock.read();
        let before = alias.contents(&guard);
        let after = lowered.stylesheet.contents(&guard);
        assert_metadata(after, before);
        assert!(Arc::ptr_eq(&before.rules, &original_rules));
        let CssRule::Style(rule) = &before.rules(&guard)[2] else {
            panic!("the original style alias remains unchanged");
        };
        assert!(Arc::ptr_eq(rule, &original_style));
        assert_eq!(
            before
                .rules(&guard)
                .iter()
                .map(|rule| rule.to_css_string(&guard))
                .collect::<Vec<_>>(),
            original_css
        );
        assert!(Arc::ptr_eq(&lowered.stylesheet.0.media, &alias.0.media));
        assert_eq!(lowered.stylesheet.0.disabled(), alias.0.disabled());
        assert_eq!(lowered.stylesheet.1, alias.1);
        let CssRule::Style(rule) = &after.rules(&guard)[2] else {
            panic!("the lowered namespaced style uses the caller lock");
        };
        assert_eq!(
            rule.read_with(&guard).selectors,
            original_style.read_with(&guard).selectors
        );
        assert_eq!(
            rule.read_with(&guard).to_css_string(&guard),
            original_style.read_with(&guard).to_css_string(&guard)
        );
        assert_eq!(lowered.position_try_sources.len(), 1);
    }

    #[test]
    fn initial_stylesheet_owner_is_released_before_contextual_lowering() {
        use std::sync::{
            Arc as StdArc,
            atomic::{AtomicBool, Ordering},
        };
        use stylo_cssom_model::{NativeDeclarationBacking, RetainedNativeDeclarations};

        #[derive(Debug)]
        struct InitialOwner {
            stylesheet: servo_arc::Arc<style::stylesheets::Stylesheet>,
            checked: StdArc<AtomicBool>,
        }
        impl NativeDeclarationBacking for InitialOwner {
            fn as_any(&self) -> &dyn std::any::Any {
                assert!(
                    self.stylesheet.is_unique(),
                    "the initial stylesheet owner must drop before declaration lowering"
                );
                self.checked.store(true, Ordering::Relaxed);
                self
            }
            fn equivalent(&self, other: &dyn NativeDeclarationBacking) -> bool {
                other.as_any().downcast_ref::<Self>().is_some_and(|other| {
                    servo_arc::Arc::ptr_eq(&self.stylesheet, &other.stylesheet)
                })
            }
        }
        crate::context::initialise_required_servo_style_prefs();
        let css = "p{background-image:url(icon.png);color:red}";
        let parsed = crate::authored_rules::ParsedStylesheet::parse(css).unwrap();
        let base = crate::CssStylesheetBaseUrl::from_absolute(
            url::Url::parse("https://styles.test/style.css").unwrap(),
        );
        let mut source = crate::authored_rules::project_stylesheet_pending_contexts(
            parsed.rule_nodes(),
            &base,
            None,
        );
        let lock = style::shared_lock::SharedRwLock::new();
        let native = servo_arc::Arc::new(style::stylesheets::Stylesheet::from_str(
            css,
            style::stylesheets::UrlExtraData::from(url::Url::parse(base.as_str()).unwrap()),
            style::stylesheets::Origin::Author,
            servo_arc::Arc::new(lock.wrap(style::media_queries::MediaList::empty())),
            lock.clone(),
            None,
            None,
            selectors::matching::QuirksMode::NoQuirks,
            style::stylesheets::AllowImportRules::Yes,
        ));
        let checked = StdArc::new(AtomicBool::new(false));
        let block = source[0]
            .payload()
            .declaration_block()
            .unwrap()
            .clone()
            .with_native_backing(Some(RetainedNativeDeclarations(StdArc::new(
                InitialOwner {
                    stylesheet: native.clone(),
                    checked: checked.clone(),
                },
            ))));
        source[0] = source[0].clone().with_declaration_block(block);
        let lowered = super::lower_pending_declarations(
            &source,
            style::stylesheets::DocumentStyleSheet(native, None),
            &lock,
        )
        .unwrap();
        assert!(checked.load(Ordering::Relaxed));
        let guard = lock.read();
        let [style::stylesheets::CssRule::Style(rule)] =
            lowered.stylesheet.contents(&guard).rules(&guard)
        else {
            panic!("the contextual style is preserved");
        };
        assert!(
            rule.read_with(&guard)
                .to_css_string(&guard)
                .contains("color: red")
        );
    }

    fn lower_contextual_stylesheet(
        css: &str,
    ) -> Result<
        (style::shared_lock::SharedRwLock, super::LoweredStylesheet),
        super::DeclarationLoweringError,
    > {
        crate::context::initialise_required_servo_style_prefs();
        let parsed = crate::authored_rules::ParsedStylesheet::parse(css).unwrap();
        let base = crate::CssStylesheetBaseUrl::from_absolute(
            url::Url::parse("https://styles.test/style.css").unwrap(),
        );
        let source = crate::authored_rules::project_stylesheet_pending_contexts(
            parsed.rule_nodes(),
            &base,
            None,
        );
        let source = super::expand_view_transition_rules(&source)?;
        let projected = source
            .iter()
            .map(stylo_cssom_model::RuleNode::projection_serialization)
            .collect::<Vec<_>>()
            .join("\n");
        let lock = style::shared_lock::SharedRwLock::new();
        let native = style::stylesheets::Stylesheet::from_str(
            &projected,
            style::stylesheets::UrlExtraData::from(
                url::Url::parse("https://styles.test/style.css").unwrap(),
            ),
            style::stylesheets::Origin::Author,
            servo_arc::Arc::new(lock.wrap(style::media_queries::MediaList::empty())),
            lock.clone(),
            None,
            None,
            selectors::matching::QuirksMode::NoQuirks,
            style::stylesheets::AllowImportRules::Yes,
        );
        let lowered = super::lower_pending_declarations(
            &source,
            style::stylesheets::DocumentStyleSheet(servo_arc::Arc::new(native), None),
            &lock,
        )?;
        Ok((lock, lowered))
    }

    #[test]
    fn view_transition_expansion_retains_css_eof_token_recovery() {
        fn assert_component_tokens_equal<'i>(
            left: &mut cssparser::Parser<'i, '_>,
            right: &mut cssparser::Parser<'i, '_>,
        ) -> Result<(), cssparser::ParseError<'i, ()>> {
            loop {
                match (
                    left.next_including_whitespace_and_comments().cloned(),
                    right.next_including_whitespace_and_comments().cloned(),
                ) {
                    (Err(_), Err(_)) => return Ok(()),
                    (Ok(left_token), Ok(right_token)) => {
                        assert_eq!(left_token, right_token);
                        if matches!(
                            left_token,
                            cssparser::Token::Function(_)
                                | cssparser::Token::ParenthesisBlock
                                | cssparser::Token::SquareBracketBlock
                                | cssparser::Token::CurlyBracketBlock
                        ) {
                            left.parse_nested_block(|left| {
                                right.parse_nested_block(|right| {
                                    assert_component_tokens_equal(left, right)
                                })
                            })?;
                        }
                    },
                    tokens => panic!("component token counts differ: {tokens:?}"),
                }
            }
        }

        crate::context::initialise_required_servo_style_prefs();
        for css in [
            r"p{--x:foo\",
            r"p{--x:1foo\",
            r"p{--x:url(foo\",
            r#"p{--x:"foo\"#,
            "p{--a:green;color:var(--a",
            "p{background:url(\"green.png",
            "p{background:url(green.png",
            "::view-transition-group(card){transform:translateX(var(--x",
        ] {
            let parsed = crate::authored_rules::ParsedStylesheet::parse(css).unwrap();
            let expanded = super::expand_view_transition_rules(parsed.rule_nodes()).expect(css);
            let reparsed = crate::authored_rules::ParsedStylesheet::parse(
                &expanded[0].projection_serialization(),
            )
            .unwrap();
            let original = parsed.rule_nodes()[0]
                .payload()
                .declaration_block()
                .unwrap();
            let repaired = reparsed.rule_nodes()[0]
                .payload()
                .declaration_block()
                .unwrap();
            assert_eq!(
                original.declarations().len(),
                repaired.declarations().len(),
                "{css}"
            );
            for (original, repaired) in original.declarations().iter().zip(repaired.declarations())
            {
                assert_eq!(original.name(), repaired.name(), "{css}");
                assert_eq!(original.important(), repaired.important(), "{css}");
                // Escaped U+FFFD and literal U+FFFD are the same token value.
                // Compare decoded component tokens, including nested blocks,
                // rather than requiring the same escape spelling after EOF repair.
                let values = match (
                    original.pending_substitution(),
                    repaired.pending_substitution(),
                ) {
                    (Some(original), Some(repaired)) => {
                        assert_eq!(original.shorthand(), repaired.shorthand(), "{css}");
                        assert_eq!(original.base_url(), repaired.base_url(), "{css}");
                        (original.tokens(), repaired.tokens())
                    },
                    (None, None) => (original.value(), repaired.value()),
                    _ => panic!("pending substitution classification changed: {css}"),
                };
                let mut left = cssparser::ParserInput::new(values.0);
                let mut right = cssparser::ParserInput::new(values.1);
                assert_component_tokens_equal(
                    &mut cssparser::Parser::new(&mut left),
                    &mut cssparser::Parser::new(&mut right),
                )
                .expect(css);
            }
            lower_contextual_stylesheet(css).expect(css);
        }
    }

    #[test]
    fn eof_recovery_preserves_exact_custom_property_cssom_serialization() {
        crate::context::initialise_required_servo_style_prefs();
        for (css, expected) in [
            (r"p{--x:foo\", "foo�"),
            (r"p{--x:1foo\", "1foo�"),
            (r"p{--x:url(foo\", "url(foo�)"),
            (r#"p{--x:"foo\"#, "\"foo\""),
        ] {
            let parsed = crate::authored_rules::ParsedStylesheet::parse(css).unwrap();
            assert_eq!(
                parsed.rule_nodes()[0]
                    .payload()
                    .declaration_block()
                    .unwrap()
                    .declarations()[0]
                    .value(),
                expected,
                "{css}"
            );
            let expanded = super::expand_view_transition_rules(parsed.rule_nodes()).expect(css);
            let reparsed = crate::authored_rules::ParsedStylesheet::parse(
                &expanded[0].projection_serialization(),
            )
            .unwrap();
            assert_eq!(
                reparsed.rule_nodes()[0]
                    .payload()
                    .declaration_block()
                    .unwrap()
                    .declarations()[0]
                    .value(),
                expected,
                "{css}"
            );
            lower_contextual_stylesheet(css).expect(css);
        }
    }

    #[test]
    fn contextual_mixin_lowering_keeps_result_blocks_and_local_descriptors() {
        for (css, locals) in [
            (
                "@mixin --colour(--c){@result{color:var(--c)}}p{@apply --colour(green)}",
                false,
            ),
            (
                "@mixin --colour(){--c:green;@result{color:var(--c)}}p{@apply --colour}",
                true,
            ),
            (
                "@mixin --colour(){--c:green;@result{@media (min-width:0px){color:var(--c)}}}p{@apply --colour}",
                true,
            ),
        ] {
            let (lock, lowered) = lower_contextual_stylesheet(css).expect(css);
            let guard = lock.read();
            let style::stylesheets::CssRule::Mixin(mixin) =
                &lowered.stylesheet.contents(&guard).rules(&guard)[0]
            else {
                panic!("mixin definition was lost");
            };
            let children = mixin.rules.as_ref().unwrap().read_with(&guard);
            assert_eq!(
                children.0.iter().any(|rule| matches!(
                    rule,
                    style::stylesheets::CssRule::FunctionDeclarations(_)
                )),
                locals,
                "{css}"
            );
            assert!(
                children
                    .0
                    .iter()
                    .any(|rule| matches!(rule, style::stylesheets::CssRule::Mixin(_))),
                "result block was lost: {css}"
            );
        }
    }

    #[test]
    fn contextual_page_background_lowering_keeps_page_shorthand_components() {
        crate::context::initialise_required_servo_style_prefs();
        let css = "@page{size:600px 300px;margin:20px;border:20px solid blue;padding:20px;background-image:url(cat.png);background-repeat:no-repeat}";
        let parsed = crate::authored_rules::ParsedStylesheet::parse(css).unwrap();
        let block = parsed.rule_nodes()[0]
            .payload()
            .declaration_block()
            .unwrap();
        for declaration in block.declarations() {
            let single = stylo_cssom_model::RuleDeclarationBlock::from_declarations(
                block.domain(),
                vec![declaration.clone()],
            );
            assert!(
                super::native_declarations(&single).is_ok(),
                "canonical page component did not restore: {}: {}",
                declaration.name(),
                declaration.value()
            );
        }
        lower_contextual_stylesheet(css)
            .expect("the page box background and border must lower in page context");
        let forbidden = stylo_cssom_model::RuleDeclarationBlock::from_declarations(
            stylo_cssom_model::RuleDeclarationDomain::Page,
            vec![stylo_cssom_model::RuleDeclaration::new(
                "transform",
                "scale(1)",
            )],
        );
        assert!(
            super::native_declarations(&forbidden).is_err(),
            "unrelated element-only properties remain invalid in page context"
        );
        let mut author = crate::declaration_parser::parse_cssom_declaration_block(
            "",
            crate::declaration_parser::CssomDeclarationContext::Page,
        );
        assert!(
            !crate::declaration_parser::cssom_declaration_set_property(
                &mut author,
                "border-image-source",
                "url(cat.png)",
                crate::declaration_parser::CssomDeclarationPriority::Normal,
                crate::declaration_parser::CssomDeclarationContext::Page
            ),
            "restoring a shorthand reset must not widen author grammar"
        );
    }

    #[test]
    fn contextual_lowering_keeps_a_fully_parsed_compatibility_value() {
        crate::context::initialise_required_servo_style_prefs();
        let authored = crate::authored_rules::ParsedStylesheet::parse(
            "a::after{content:target-counter(attr(href),attr(data-counter -ro-ident))}",
        )
        .unwrap();
        let base = crate::CssStylesheetBaseUrl::from_absolute(
            url::Url::parse("https://styles.test/style.css").unwrap(),
        );
        let source = crate::authored_rules::project_stylesheet_pending_contexts(
            authored.rule_nodes(),
            &base,
            None,
        );
        let projected = crate::authored_rules::ParsedStylesheet::parse(
            "a::after{content:target-counter(attr(href),attr(data-counter -bd-ident))}",
        )
        .unwrap();
        let native = super::native_declarations(
            projected.rule_nodes()[0]
                .payload()
                .declaration_block()
                .unwrap(),
        )
        .unwrap();
        let lowered =
            super::merge_native_pending(source[0].payload().declaration_block().unwrap(), &native)
                .unwrap();
        assert!(matches!(
            lowered.declaration_importance_iter().next(),
            Some((style::properties::PropertyDeclaration::Content(_), _))
        ));
    }

    #[test]
    fn imported_scope_keeps_top_level_selector_parsing() {
        crate::context::initialise_required_servo_style_prefs();
        let source = stylo_cssom_model::RuleNode::imported_scope(
            &stylo_cssom_model::RuleImportScope::Explicit("(.card)".into()),
            [stylo_cssom_model::RuleNode::style(".card", "color: green")],
        );
        let lock = style::shared_lock::SharedRwLock::new();
        let parsed = style::stylesheets::Stylesheet::from_str(
            &source.projection_serialization(),
            style::stylesheets::UrlExtraData::from(crate::context::ABOUT_BLANK.clone()),
            style::stylesheets::Origin::Author,
            servo_arc::Arc::new(lock.wrap(style::media_queries::MediaList::empty())),
            lock.clone(),
            None,
            None,
            selectors::matching::QuirksMode::NoQuirks,
            style::stylesheets::AllowImportRules::Yes,
        );
        let lowered = super::lower_pending_declarations(
            &[source],
            style::stylesheets::DocumentStyleSheet(servo_arc::Arc::new(parsed), None),
            &lock,
        )
        .expect("the import scope should lower without nesting its style selectors");
        let guard = lock.read();
        let [style::stylesheets::CssRule::Scope(scope)] =
            lowered.stylesheet.contents(&guard).rules(&guard)
        else {
            panic!("the imported rules must be scoped");
        };
        assert!(scope.imported);
        let [style::stylesheets::CssRule::Style(rule)] = scope.rules.read_with(&guard).0.as_slice()
        else {
            panic!("the imported style rule must be retained");
        };
        assert!(
            rule.read_with(&guard)
                .selectors
                .slice()
                .iter()
                .all(|selector| {
                    !selector.iter().any(|component| {
                        matches!(component, selectors::parser::Component::ImplicitScope)
                    })
                })
        );
    }

    #[test]
    fn view_transition_expansion_keeps_functional_pseudo_class_arguments() {
        let source = crate::authored_rules::ParsedStylesheet::parse(
            ".e:nth-child(3){color:red} .f:not(.g){color:red}",
        )
        .expect("the sheet parses");
        let expanded = super::expand_view_transition_rules(source.rule_nodes())
            .expect("the style rules expand");
        let selectors: Vec<_> = expanded
            .iter()
            .map(|rule| rule.projection_serialization())
            .collect();
        assert_eq!(
            selectors,
            [".e:nth-child(3){color:red}", ".f:not(.g){color:red}"]
        );
    }

    #[test]
    fn native_nested_declarations_clone_into_the_destination_lock() {
        let source_lock = style::shared_lock::SharedRwLock::new();
        let destination_lock = style::shared_lock::SharedRwLock::new();
        let native = style::stylesheets::CssRule::NestedDeclarations(servo_arc::Arc::new(
            source_lock.wrap(style::stylesheets::NestedDeclarationsRule {
                block: servo_arc::Arc::new(
                    source_lock.wrap(style::properties::PropertyDeclarationBlock::new()),
                ),
                source_location: cssparser::SourceLocation { line: 0, column: 1 },
            }),
        ));
        let cloned = native.deep_clone_with_lock(&destination_lock, &source_lock.read());
        assert_eq!(cloned.to_css_string(&destination_lock.read()), "");
    }
}
