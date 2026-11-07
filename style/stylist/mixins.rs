/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Tree-scoped mixin expansion and lexical private custom properties.

use super::*;

impl CascadeData {
    pub(super) fn collect_private_names(
        &self,
        rules: std::slice::Iter<CssRule>,
        device: &Device,
        quirks_mode: QuirksMode,
        guard: &SharedRwLockReadGuard,
        names: &mut std::collections::HashMap<Atom, Atom>,
        id: usize,
    ) {
        for rule in rules {
            if let CssRule::Mixin(ref rule) = *rule {
                if rule.prelude.kind != crate::stylesheets::MixinRuleKind::Private {
                    continue;
                }
                for declaration in &rule.prelude.parameters {
                    names.entry(declaration.name.clone()).or_insert_with(|| {
                        Atom::from(format!("\0private-{id}-{}", declaration.name))
                    });
                }
            }
            if matches!(rule, CssRule::Style(_) | CssRule::Function(_)) {
                continue;
            }
            let mut effective = false;
            if let Some(children) = crate::stylesheets::RulesIterator::<
                crate::stylesheets::AllRules,
                &CustomMediaMap,
            >::children(
                rule,
                device,
                quirks_mode,
                &self.custom_media,
                guard,
                &mut effective,
            ) {
                self.collect_private_names(children, device, quirks_mode, guard, names, id);
            }
        }
    }

    pub(super) fn mixin_block(
        &self,
        block: &Arc<crate::shared_lock::Locked<PropertyDeclarationBlock>>,
        guard: &SharedRwLockReadGuard,
    ) -> Arc<crate::shared_lock::Locked<PropertyDeclarationBlock>> {
        if self.mixin_names.is_empty() {
            return block.clone();
        }
        let mut result = PropertyDeclarationBlock::new();
        for local in &self.mixin_locals {
            let mut declaration = PropertyDeclaration::Custom(local.clone());
            declaration.rename_private_properties(&self.mixin_names);
            result.push(declaration, crate::properties::Importance::Normal);
        }
        for (declaration, importance) in block.read_with(guard).declaration_importance_iter() {
            let mut declaration = declaration.clone();
            declaration.rename_private_properties(&self.mixin_names);
            result.push(declaration, importance);
        }
        Arc::new(block.wrap_related(result))
    }

    fn mixin_layer_path(
        &mut self,
        parent: &LayerName,
        priority: &[usize],
        name: &LayerName,
    ) -> (LayerName, Vec<usize>) {
        let mut path = parent.clone();
        let mut result = priority.to_vec();
        for component in name.layer_names() {
            path.0.push(component.clone());
            let next = self.mixin_layer_order.len();
            let rank = *self.mixin_layer_order.entry(path.clone()).or_insert(next);
            result.push(rank);
        }
        (path, result)
    }

    pub(super) fn collect_mixins(
        &mut self,
        rules: std::slice::Iter<CssRule>,
        device: &Device,
        quirks_mode: QuirksMode,
        guard: &SharedRwLockReadGuard,
    ) {
        self.collect_mixins_in_layer(
            rules,
            device,
            quirks_mode,
            guard,
            &LayerName::new_empty(),
            &[],
        );
    }

    fn collect_mixins_in_layer(
        &mut self,
        rules: std::slice::Iter<CssRule>,
        device: &Device,
        quirks_mode: QuirksMode,
        guard: &SharedRwLockReadGuard,
        layer: &LayerName,
        priority: &[usize],
    ) {
        for rule in rules {
            if let CssRule::Mixin(ref mixin) = *rule {
                if mixin.prelude.kind == crate::stylesheets::MixinRuleKind::Definition {
                    if let Some(ref name) = mixin.prelude.name {
                        let mut rank = priority.to_vec();
                        rank.push(usize::MAX);
                        if self
                            .mixin_priorities
                            .get(&name.0)
                            .is_none_or(|old| rank >= *old)
                        {
                            self.mixin_priorities.insert(name.0.clone(), rank);
                            self.mixins.insert(name.0.clone(), mixin.clone());
                        }
                    }
                }
                continue;
            }
            if matches!(rule, CssRule::Style(_) | CssRule::Function(_)) {
                continue;
            }
            if let CssRule::LayerStatement(ref statement) = *rule {
                for name in statement.names.iter() {
                    self.mixin_layer_path(layer, priority, name);
                }
                continue;
            }
            let mut effective = false;
            if let Some(children) = EffectiveRulesIterator::<&CustomMediaMap>::children(
                rule,
                device,
                quirks_mode,
                &self.custom_media,
                guard,
                &mut effective,
            ) {
                let layer_name = match rule {
                    CssRule::LayerBlock(rule) => {
                        Some(rule.name.clone().unwrap_or_else(LayerName::new_anonymous))
                    },
                    CssRule::Import(rule) => match &rule.read_with(guard).layer {
                        crate::stylesheets::import_rule::ImportLayer::None => None,
                        crate::stylesheets::import_rule::ImportLayer::Anonymous => {
                            Some(LayerName::new_anonymous())
                        },
                        crate::stylesheets::import_rule::ImportLayer::Named(name) => {
                            Some(name.clone())
                        },
                    },
                    _ => None,
                };
                if let Some(name) = layer_name {
                    let (path, rank) = self.mixin_layer_path(layer, priority, &name);
                    self.collect_mixins_in_layer(
                        children,
                        device,
                        quirks_mode,
                        guard,
                        &path,
                        &rank,
                    );
                } else {
                    self.collect_mixins_in_layer(
                        children,
                        device,
                        quirks_mode,
                        guard,
                        layer,
                        priority,
                    );
                }
            }
        }
    }

    pub(super) fn add_mixin_rule<S>(
        &mut self,
        rule: &crate::stylesheets::MixinRule,
        device: &Device,
        quirks_mode: QuirksMode,
        stylesheet: &S,
        sheet_index: usize,
        guard: &SharedRwLockReadGuard,
        rebuild_kind: SheetRebuildKind,
        containing_rule_state: &mut ContainingRuleState,
        mut precomputed_pseudo_element_decls: Option<&mut PrecomputedPseudoElementDeclarations>,
        mut difference: Option<&mut CascadeDataDifference>,
    ) -> Result<(), AllocErr>
    where
        S: StylesheetInDocument + 'static,
    {
        use crate::stylesheets::MixinRuleKind;
        match rule.prelude.kind {
            MixinRuleKind::Definition => {},
            MixinRuleKind::Private => {
                if let Some(ref rules) = rule.rules {
                    if let Some(selectors) = containing_rule_state
                        .ancestor_selector_lists
                        .last()
                        .cloned()
                    {
                        let mut block = PropertyDeclarationBlock::new();
                        for declaration in &rule.prelude.parameters {
                            if let Some(ref value) = declaration.default {
                                let mut declaration = PropertyDeclaration::Custom(
                                    crate::properties::CustomDeclaration {
                                        name: declaration.name.clone(),
                                        value: crate::properties::CustomDeclarationValue::Unparsed(
                                            value.clone(),
                                        ),
                                    },
                                );
                                declaration.rename_private_properties(&self.mixin_names);
                                block.push(declaration, crate::properties::Importance::Normal);
                            }
                        }
                        let block = Arc::new(rules.wrap_related(block));
                        self.add_styles(
                            &selectors,
                            &block,
                            None,
                            containing_rule_state,
                            None,
                            guard,
                            SheetRebuildKind::CascadeOnly,
                            precomputed_pseudo_element_decls.as_deref_mut(),
                            quirks_mode,
                            None,
                        )?;
                    }
                    self.add_rule_list(
                        rules.read_with(guard).0.iter(),
                        device,
                        quirks_mode,
                        stylesheet,
                        sheet_index,
                        guard,
                        rebuild_kind,
                        containing_rule_state,
                        precomputed_pseudo_element_decls.as_deref_mut(),
                        difference.as_deref_mut(),
                    )?;
                }
            },
            MixinRuleKind::Apply => {
                if let Some(ref name) = rule.prelude.name {
                    if !self.mixin_stack.contains(&name.0) {
                        if let Some(definition) = self.mixins.get(&name.0).cloned() {
                            use crate::properties::{
                                CSSWideKeyword, CustomDeclaration, CustomDeclarationValue,
                                Importance,
                            };
                            use crate::stylesheets::function_rule::FunctionDescriptorName;
                            let arguments = rule
                                .prelude
                                .invocation
                                .as_ref()
                                .and_then(|value| value.function_arguments())
                                .unwrap_or_default();
                            if arguments.len() > definition.prelude.parameters.len() {
                                return Ok(());
                            }
                            let saved_names = self.mixin_names.clone();
                            let saved_locals = self.mixin_locals.clone();
                            static NEXT_MIXIN: std::sync::atomic::AtomicUsize =
                                std::sync::atomic::AtomicUsize::new(1);
                            let id = NEXT_MIXIN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let mut parameters = PropertyDeclarationBlock::new();
                            for (index, parameter) in
                                definition.prelude.parameters.iter().enumerate()
                            {
                                let private =
                                    Atom::from(format!("\0mixin-{id}-{}", parameter.name));
                                let value = if let Some(source) =
                                    arguments.get(index).or(parameter.default.as_ref())
                                {
                                    let function_name =
                                        Atom::from(format!("\0mixin-{id}-argument-{index}"));
                                    let mut function_parameters =
                                        definition.prelude.parameters.clone();
                                    for parameter in &mut function_parameters {
                                        if let Some(ref mut default) = parameter.default {
                                            let mut value = (**default).clone();
                                            value.rename_private_references(&saved_names);
                                            *default = Arc::new(value);
                                        }
                                    }
                                    let function_rule = crate::stylesheets::FunctionRule {
                                    name: crate::values::DashedIdent(function_name.clone()),
                                    parameters: function_parameters,
                                    return_type: crate::properties_and_values::syntax::Descriptor::universal(),
                                    rules: definition.rules.as_ref().unwrap().clone(),
                                    source_location: definition.source_location,
                                };
                                    let result = crate::custom_properties::VariableValue::private_variable_reference(parameter.name.clone(), &source.url_data);
                                    let body = vec![FunctionBodyItem::Descriptor(
                                        crate::stylesheets::function_rule::FunctionDescriptor {
                                            name: FunctionDescriptorName::Result,
                                            value: Arc::new(result),
                                        },
                                    )];
                                    self.custom_functions.try_insert(
                                        function_name.clone(),
                                        Arc::new(CustomFunction::new(&function_rule, body)),
                                        LayerId::root(),
                                    )?;
                                    let mut value = crate::custom_properties::VariableValue::private_function_call(function_name, &arguments, &source.url_data).unwrap();
                                    value.rename_private_references(&saved_names);
                                    CustomDeclarationValue::Unparsed(Arc::new(value))
                                } else {
                                    CustomDeclarationValue::CSSWideKeyword(CSSWideKeyword::Initial)
                                };
                                parameters.push(
                                    PropertyDeclaration::Custom(CustomDeclaration {
                                        name: private.clone(),
                                        value,
                                    }),
                                    Importance::Normal,
                                );
                                self.mixin_names.insert(parameter.name.clone(), private);
                            }
                            if let Some(ref rules) = definition.rules {
                                if let Some(selectors) = containing_rule_state
                                    .ancestor_selector_lists
                                    .last()
                                    .cloned()
                                {
                                    let block = Arc::new(rules.wrap_related(parameters));
                                    self.add_styles(
                                        &selectors,
                                        &block,
                                        None,
                                        containing_rule_state,
                                        None,
                                        guard,
                                        SheetRebuildKind::CascadeOnly,
                                        precomputed_pseudo_element_decls.as_deref_mut(),
                                        quirks_mode,
                                        None,
                                    )?;
                                }
                                let mut locals = self.function_body(
                                    rules.read_with(guard).0.iter(),
                                    device,
                                    quirks_mode,
                                    guard,
                                    rebuild_kind,
                                );
                                fn collect_local_values(
                                    items: &[FunctionBodyItem],
                                    result: &mut std::collections::HashMap<
                                        Atom,
                                        Arc<crate::custom_properties::VariableValue>,
                                    >,
                                ) {
                                    for item in items {
                                        match item {
                                            FunctionBodyItem::Descriptor(descriptor) => {
                                                if let FunctionDescriptorName::Local(ref name) =
                                                    descriptor.name
                                                {
                                                    result.insert(
                                                        name.clone(),
                                                        descriptor.value.clone(),
                                                    );
                                                }
                                            },
                                            FunctionBodyItem::Container(_, children) => {
                                                collect_local_values(children, result)
                                            },
                                        }
                                    }
                                }
                                fn rename_outer_references(
                                    items: &mut [FunctionBodyItem],
                                    names: &std::collections::HashMap<Atom, Atom>,
                                ) {
                                    for item in items {
                                        match item {
                                            FunctionBodyItem::Descriptor(descriptor) => {
                                                let mut value = (*descriptor.value).clone();
                                                value.rename_private_references(names);
                                                descriptor.value = Arc::new(value);
                                            },
                                            FunctionBodyItem::Container(_, children) => {
                                                rename_outer_references(children, names)
                                            },
                                        }
                                    }
                                }
                                let mut local_values = std::collections::HashMap::new();
                                collect_local_values(&locals, &mut local_values);
                                let mut outer_names = saved_names.clone();
                                for parameter in &definition.prelude.parameters {
                                    outer_names.remove(&parameter.name);
                                }
                                for local in local_values.keys() {
                                    outer_names.remove(local);
                                }
                                rename_outer_references(&mut locals, &outer_names);
                                let parameter_names = self.mixin_names.clone();
                                for (local, source) in local_values {
                                    let function_name =
                                        Atom::from(format!("\0mixin-{id}-local-function-{local}"));
                                    let arguments: Vec<_> = definition.prelude.parameters.iter().map(|parameter| Arc::new(crate::custom_properties::VariableValue::private_variable_reference(parameter_names[&parameter.name].clone(), &source.url_data))).collect();
                                    let function_rule = crate::stylesheets::FunctionRule { name: crate::values::DashedIdent(function_name.clone()), parameters: definition.prelude.parameters.clone(), return_type: crate::properties_and_values::syntax::Descriptor::universal(), rules: rules.clone(), source_location: definition.source_location };
                                    let mut body = locals.clone();
                                    body.push(FunctionBodyItem::Descriptor(crate::stylesheets::function_rule::FunctionDescriptor { name: FunctionDescriptorName::Result, value: Arc::new(crate::custom_properties::VariableValue::private_variable_reference(local.clone(), &source.url_data)) }));
                                    self.custom_functions.try_insert(
                                        function_name.clone(),
                                        Arc::new(CustomFunction::new(&function_rule, body)),
                                        LayerId::root(),
                                    )?;
                                    let value = crate::custom_properties::VariableValue::private_function_call(function_name, &arguments, &source.url_data).unwrap();
                                    self.mixin_names.insert(
                                        local.clone(),
                                        Atom::from(format!("\0mixin-{id}-local-{local}")),
                                    );
                                    self.mixin_locals.push(CustomDeclaration {
                                        name: self.mixin_names[&local].clone(),
                                        value: CustomDeclarationValue::Unparsed(Arc::new(value)),
                                    });
                                }
                            }
                            self.mixin_stack.push(name.0.clone());
                            self.mixin_contents.push(rule.rules.clone());
                            self.mixin_call_environments
                                .push((saved_names.clone(), saved_locals.clone()));
                            if let Some(ref rules) = definition.rules {
                                self.add_rule_list(
                                    rules.read_with(guard).0.iter(),
                                    device,
                                    quirks_mode,
                                    stylesheet,
                                    sheet_index,
                                    guard,
                                    rebuild_kind,
                                    containing_rule_state,
                                    precomputed_pseudo_element_decls.as_deref_mut(),
                                    difference.as_deref_mut(),
                                )?;
                            }
                            self.mixin_contents.pop();
                            self.mixin_call_environments.pop();
                            self.mixin_stack.pop();
                            self.mixin_names = saved_names;
                            self.mixin_locals = saved_locals;
                        }
                    }
                }
            },
            MixinRuleKind::Result | MixinRuleKind::Contents => {
                let previous = if rule.prelude.kind == MixinRuleKind::Contents
                    && self.mixin_contents.last().is_some_and(Option::is_some)
                {
                    self.mixin_call_environments
                        .last()
                        .cloned()
                        .map(|(names, locals)| {
                            (
                                std::mem::replace(&mut self.mixin_names, names),
                                std::mem::replace(&mut self.mixin_locals, locals),
                            )
                        })
                } else {
                    None
                };
                let rules = if rule.prelude.kind == MixinRuleKind::Contents {
                    self.mixin_contents
                        .last()
                        .and_then(|contents| contents.clone())
                        .or_else(|| rule.rules.clone())
                } else {
                    rule.rules.clone()
                };
                if let Some(rules) = rules {
                    self.add_rule_list(
                        rules.read_with(guard).0.iter(),
                        device,
                        quirks_mode,
                        stylesheet,
                        sheet_index,
                        guard,
                        rebuild_kind,
                        containing_rule_state,
                        precomputed_pseudo_element_decls.as_deref_mut(),
                        difference.as_deref_mut(),
                    )?;
                }
                if let Some((names, locals)) = previous {
                    self.mixin_names = names;
                    self.mixin_locals = locals;
                }
            },
        }
        Ok(())
    }
}
