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
            || rule
                .payload()
                .declaration_block()
                .is_some_and(requires_declaration_lowering)
            || requires_source_lowering(rule.payload().nested())
    })
}

fn split_rule_block(css: &str) -> Option<(&str, &str)> {
    let mut input = cssparser::ParserInput::new(css);
    let mut parser = cssparser::Parser::new(&mut input);
    loop {
        let start = parser.position().byte_index();
        if matches!(
            parser.next_including_whitespace_and_comments().ok()?,
            cssparser::Token::CurlyBracketBlock
        ) {
            let body = css[parser.position().byte_index()..]
                .trim_end()
                .strip_suffix('}')?;
            return Some((&css[..start], body));
        }
    }
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
            let expansion = crate::view_transition_root_rewrite::expand_style_rule(selector, body);
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
        RuleDeclarationDomain::FontFaceDescriptor => {
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
    let mut result = PropertyDeclarationBlock::new();
    for (source, importance) in restored.declaration_importance_iter() {
        let selected = if matches!(source, style::properties::PropertyDeclaration::WithVariables(value)
            if value.value.from_shorthand().is_some())
        {
            Some((source, importance))
        } else {
            projected
                .declaration_importance_iter()
                .find(|(candidate, _)| candidate.id() == source.id())
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
    if has_pending(source) {
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
        CssRule::When(rule) => rule.rules.clone(),
        CssRule::Else(rule) => rule.rules.clone(),
        _ => return None,
    })
}

fn lower_rule_list(
    source: &[RuleNode],
    contents: &style::stylesheets::StylesheetContents,
    lock: &SharedRwLock,
    nesting: style::parser::NestingContext,
    position_try_sources: &mut Vec<RuleSourceStamp>,
) -> Result<Vec<CssRule>, DeclarationLoweringError> {
    use style::{
        parser::ParserContext,
        stylesheets::{AllowImportRules, State, TopLevelRuleParser},
    };
    let serialized = source
        .iter()
        .map(RuleNode::projection_serialization)
        .collect::<Vec<_>>();
    let mut context = ParserContext::new(
        contents.origin,
        &contents.url_data,
        None,
        style_traits::ParsingMode::DEFAULT,
        contents.quirks_mode,
        std::borrow::Cow::Borrowed(&contents.namespaces),
        None,
        None,
    );
    context.nesting_context = nesting;
    let mut parser = TopLevelRuleParser {
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
        error_reporting_state: Default::default(),
        rules: Vec::new(),
    };
    for (source, css) in source.iter().zip(&serialized) {
        if source.grammar() == RuleGrammar::NestedDeclarations {
            let source = source
                .payload()
                .declaration_block()
                .ok_or(DeclarationLoweringError::DeclarationContext)?;
            let mut input = cssparser::ParserInput::new(css);
            let mut input = cssparser::Parser::new(&mut input);
            let projected = style::properties::declaration_block::parse_property_declaration_list(
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
            continue;
        }
        let first = parser.rules.len();
        let mut input = crate::rule_parser::stylesheet_parser_input(css);
        let mut input = cssparser::Parser::new(&mut input);
        for result in cssparser::StyleSheetParser::new(&mut input, &mut parser) {
            if result.is_err() {
                continue;
            }
        }
        let emitted = &parser.rules[first..];
        if !requires_source_lowering(std::slice::from_ref(source)) || emitted.is_empty() {
            continue;
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
        )?;
    }
    Ok(parser.rules)
}

fn lower_rule(
    source: &RuleNode,
    native: &CssRule,
    contents: &style::stylesheets::StylesheetContents,
    lock: &SharedRwLock,
    mut nesting: style::parser::NestingContext,
    position_try_sources: &mut Vec<RuleSourceStamp>,
) -> Result<(), DeclarationLoweringError> {
    if source.grammar() != crate::rule_parser::stylo_rule_grammar(native) {
        return Err(DeclarationLoweringError::RuleGrammar);
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
    if !requires_source_lowering(children) {
        return Ok(());
    }
    if let CssRule::Keyframes(rule) = native {
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
        nesting.save(native.rule_type());
        let rules = lower_rule_list(children, contents, lock, nesting, position_try_sources)?;
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
    let private_lock = SharedRwLock::new();
    let contents = {
        let guard = lock.read();
        stylesheet
            .contents(&guard)
            .deep_clone(&private_lock, None, &guard)
    };
    let mut source_stamps = Vec::new();
    let rules = lower_rule_list(
        rules,
        &contents,
        &private_lock,
        style::parser::NestingContext::new_from_rule(None),
        &mut source_stamps,
    )?;
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
    let stylesheet = DocumentStyleSheet(Arc::new(Stylesheet {
        contents: lock.wrap(contents),
        shared_lock: lock.clone(),
        media: stylesheet.0.media.clone(),
        disabled: std::sync::atomic::AtomicBool::new(stylesheet.0.disabled()),
    }));
    Ok(LoweredStylesheet {
        stylesheet,
        position_try_sources,
    })
}

#[cfg(test)]
mod tests {
    use style::shared_lock::{DeepCloneWithLock, ToCssWithGuard};

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
