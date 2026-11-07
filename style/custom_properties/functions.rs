/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Evaluation of custom functions.
//!
//! <https://drafts.csswg.org/css-mixins-1/#evaluating-custom-functions>

use super::{
    Name, NonCustomReferences, Substitution, SubstitutionFunction, Substitutor, VariableValue,
    compute_value,
};
use crate::computed_value_flags::ComputedValueFlags;
use crate::dom::{TElement, TNode, TShadowRoot};
use crate::properties::CSSWideKeyword;
use crate::properties_and_values::registry::PropertyRegistrationData;
use crate::properties_and_values::rule::Inherits;
use crate::properties_and_values::syntax::data_type::DependentDataTypes;
use crate::rule_tree::CascadeLevel;
use crate::stylesheets::container_rule::{ContainerConditions, OriginatingElement};
use crate::stylesheets::function_rule::{FunctionDescriptor, FunctionDescriptorName, FunctionRule};
use crate::stylesheets::{Origin, UrlExtraData};
use crate::stylist::{CascadeData, Stylist};
use crate::values::DashedIdent;
use cssparser::{Parser, ParserInput};
use servo_arc::Arc;
use smallvec::SmallVec;

/// A `@function` rule, compiled for evaluation: static conditional rules are folded.
#[derive(Debug)]
pub struct CustomFunction {
    parameters: Box<[Parameter]>,
    result: PropertyRegistrationData,
    body: Box<[FunctionBodyItem]>,
}

#[derive(Debug)]
struct Parameter {
    name: Name,
    registration: PropertyRegistrationData,
    default: Option<Arc<VariableValue>>,
}

/// An item of a compiled function body.
#[derive(Debug)]
pub enum FunctionBodyItem {
    /// A local variable or the `result` descriptor.
    Descriptor(FunctionDescriptor),
    /// A `@container` rule, evaluated against the calling element.
    Container(Arc<ContainerConditions>, Box<[FunctionBodyItem]>),
}

fn registration(
    syntax: &crate::properties_and_values::syntax::Descriptor,
    inherits: Inherits,
) -> PropertyRegistrationData {
    PropertyRegistrationData {
        syntax: syntax.clone(),
        inherits,
        initial_value: None,
    }
}

impl CustomFunction {
    /// Compile a function from its rule and its folded body.
    pub fn new(rule: &FunctionRule, body: Vec<FunctionBodyItem>) -> Self {
        Self {
            parameters: rule
                .parameters
                .iter()
                .map(|parameter| Parameter {
                    name: parameter.name.clone(),
                    registration: registration(&parameter.syntax, Inherits::True),
                    default: parameter.default.clone(),
                })
                .collect(),
            result: registration(&rule.return_type, Inherits::False),
            body: body.into_boxed_slice(),
        }
    }

    fn parameter(&self, name: &Name) -> Option<usize> {
        self.parameters
            .iter()
            .position(|parameter| parameter.name == *name)
    }

    fn registration(&self, name: &Name) -> &PropertyRegistrationData {
        match self.parameter(name) {
            Some(index) => &self.parameters[index].registration,
            None => PropertyRegistrationData::unregistered(),
        }
    }

    /// Whether every evaluation binds `name`: a parameter, or a local outside `@container`.
    fn binds(&self, name: &Name) -> bool {
        self.parameter(name).is_some()
            || self.body.iter().any(|item| {
                matches!(
                    item,
                    FunctionBodyItem::Descriptor(FunctionDescriptor {
                        name: FunctionDescriptorName::Local(ref local),
                        ..
                    }) if local == name
                )
            })
    }

    fn values(&self) -> impl Iterator<Item = &VariableValue> {
        fn descriptors<'a>(items: &'a [FunctionBodyItem], values: &mut Vec<&'a VariableValue>) {
            for item in items {
                match *item {
                    FunctionBodyItem::Descriptor(ref descriptor) => values.push(&descriptor.value),
                    FunctionBodyItem::Container(_, ref items) => descriptors(items, values),
                }
            }
        }
        let mut values = self
            .parameters
            .iter()
            .filter_map(|parameter| parameter.default.as_deref())
            .collect::<Vec<_>>();
        descriptors(&self.body, &mut values);
        values.into_iter()
    }

    fn dependent_types(&self) -> DependentDataTypes {
        self.parameters
            .iter()
            .fold(self.result.syntax.dependent_types(), |types, parameter| {
                types | parameter.registration.syntax.dependent_types()
            })
    }
}

/// The trees, innermost first, whose custom functions a declaration sees.
///
/// <https://drafts.csswg.org/css-shadow-1/#css-tree-scoped-reference>
pub type FunctionScope<'a> = Vec<&'a CascadeData>;

/// The element-dependent part of custom function evaluation.
pub trait CallingElement<'a> {
    /// The function scope of a declaration at `level`.
    fn function_scope(&self, level: CascadeLevel) -> FunctionScope<'a>;

    /// Whether `conditions` match the element's containers.
    fn container_matches(
        &self,
        conditions: &ContainerConditions,
        flags: &mut ComputedValueFlags,
    ) -> bool;

    /// What layout resolved the anchor function `function` in `property` of the element to.
    fn resolve_anchor_function(
        &self,
        property: crate::values::computed::position::AnchorProperty,
        function: &crate::values::computed::length_percentage::CalcNode,
    ) -> Option<crate::values::computed::position::AnchorFunctionResolution>;
}

/// The function scope without an element: the document tree of the declaration's origin.
pub fn document_function_scope(stylist: &Stylist, level: CascadeLevel) -> FunctionScope<'_> {
    vec![stylist.cascade_data().borrow_for_origin(level.origin())]
}

/// A calling element and, for a pseudo-element, its originating element's style.
pub struct ElementCallSite<'a, E> {
    /// The stylist with the document's cascade data.
    pub stylist: &'a Stylist,
    /// The element, or the originating element of a pseudo-element.
    pub element: E,
    /// The originating element when styling a pseudo-element.
    pub originating_element: Option<OriginatingElement<'a>>,
}

impl<'a, E: TElement + 'a> ElementCallSite<'a, E> {
    fn tree(&self, level: CascadeLevel) -> Option<<E::ConcreteNode as TNode>::ConcreteShadowRoot> {
        let steps = match level {
            CascadeLevel::AuthorNormal {
                shadow_cascade_order,
            } => shadow_cascade_order.steps(),
            CascadeLevel::AuthorImportant {
                shadow_cascade_order,
            } => -shadow_cascade_order.steps(),
            _ => 0,
        };
        if steps < 0 {
            let mut slots = SmallVec::<[E; 4]>::new();
            let mut slot = self.element.assigned_slot();
            while let Some(assigned) = slot {
                slots.push(assigned);
                slot = assigned.assigned_slot();
            }
            return match slots.get(usize::from(steps.unsigned_abs()) - 1) {
                Some(slot) => slot.containing_shadow(),
                None => self.element.shadow_root(),
            };
        }
        let mut tree = self.element.containing_shadow();
        for _ in 0..steps {
            tree = tree.and_then(|root| root.host().containing_shadow());
        }
        tree
    }
}

impl<'a, E: TElement + 'a> CallingElement<'a> for ElementCallSite<'a, E> {
    fn function_scope(&self, level: CascadeLevel) -> FunctionScope<'a> {
        if level.origin() != Origin::Author {
            return document_function_scope(self.stylist, level);
        }
        let mut scope = FunctionScope::new();
        let mut tree = self.tree(level);
        while let Some(root) = tree {
            scope.extend(root.style_data());
            tree = root.host().containing_shadow();
        }
        scope.push(
            self.stylist
                .cascade_data()
                .borrow_for_origin(Origin::Author),
        );
        scope
    }

    fn container_matches(
        &self,
        conditions: &ContainerConditions,
        flags: &mut ComputedValueFlags,
    ) -> bool {
        conditions
            .matches(self.stylist, self.element, self.originating_element, flags)
            .to_bool(/* unknown = */ false)
    }

    fn resolve_anchor_function(
        &self,
        property: crate::values::computed::position::AnchorProperty,
        function: &crate::values::computed::length_percentage::CalcNode,
    ) -> Option<crate::values::computed::position::AnchorFunctionResolution> {
        // A pseudo-element's own box is not the box that layout resolved for its originating element.
        self.originating_element
            .is_none()
            .then(|| self.element.resolve_anchor_function(property, function))
            .flatten()
    }
}

fn lookup<'a>(
    scope: &FunctionScope<'a>,
    offset: usize,
    name: &DashedIdent,
) -> Option<(&'a CustomFunction, usize)> {
    scope[offset..]
        .iter()
        .enumerate()
        .find_map(|(index, data)| Some((&**data.custom_function(&name.0)?, offset + index)))
}

fn calls(value: &VariableValue) -> impl Iterator<Item = &DashedIdent> {
    value
        .references
        .refs
        .iter()
        .filter_map(|reference| match reference.function {
            SubstitutionFunction::Function { ref name, .. } => Some(name),
            _ => None,
        })
}

/// What a value's custom function calls may read beyond their arguments.
#[derive(Default)]
pub(super) struct FunctionDependencies {
    /// Custom properties of the calling element.
    pub names: SmallVec<[Name; 4]>,
    /// Font-relative units in the reachable bodies.
    pub non_custom_references: NonCustomReferences,
    /// The types of the reachable parameters and results.
    pub dependent_types: DependentDataTypes,
    /// Whether a reachable body uses `attr()`.
    pub any_attr: bool,
}

impl FunctionDependencies {
    /// Collect the dependencies of the calls in `value`, made from the innermost tree of `scope`.
    pub fn new(value: &VariableValue, scope: &FunctionScope) -> Self {
        let mut dependencies = Self::default();
        let mut visiting = Vec::new();
        for name in calls(value) {
            let Some((function, offset)) = lookup(scope, 0, name) else {
                continue;
            };
            for name in dependencies.collect(function, scope, offset, &mut visiting) {
                if !dependencies.names.contains(&name) {
                    dependencies.names.push(name);
                }
            }
        }
        dependencies
    }

    fn collect<'a>(
        &mut self,
        function: &'a CustomFunction,
        scope: &FunctionScope<'a>,
        offset: usize,
        visiting: &mut Vec<*const CustomFunction>,
    ) -> Vec<Name> {
        if visiting.contains(&(function as *const _)) {
            return Vec::new();
        }
        visiting.push(function);
        self.dependent_types |= function.dependent_types();
        let mut free = Vec::new();
        let mut inherited = Vec::new();
        for value in function.values() {
            let conditions = super::conditionals::condition_values(value);
            for value in std::iter::once(value).chain(conditions.iter()) {
                free.extend(super::conditionals::condition_names(value));
                self.non_custom_references |= value.references.non_custom_references;
                self.any_attr |= value.references.any_attr;
                free.extend(
                    value
                        .references
                        .refs
                        .iter()
                        .filter_map(|reference| reference.function.variable_name().cloned()),
                );
                for reference in &value.references.refs {
                    if let SubstitutionFunction::Inherit(ref arguments) = reference.function {
                        if let Some(argument) = arguments.first() {
                            let mut input =
                                ParserInput::new(&value.css[argument.start..argument.end]);
                            let name = Parser::new(&mut input).parse_entirely(|input| {
                                let name = input.expect_ident()?;
                                super::parse_name(name)
                                    .map(crate::Atom::from)
                                    .map_err(|()| input.new_error_for_next_token::<()>())
                            });
                            if let Ok(name) = name {
                                inherited.push(name);
                            }
                        }
                    }
                }
                for name in calls(value) {
                    if let Some((called, offset)) = lookup(scope, offset, name) {
                        free.extend(self.collect(called, scope, offset, visiting));
                    }
                }
            }
        }
        visiting.pop();
        free.retain(|name| !function.binds(name));
        free.extend(inherited);
        free
    }
}

#[derive(Clone)]
enum Local<'a> {
    Pending(&'a VariableValue),
    InProgress(usize),
    Done(Option<VariableValue>),
}

struct Frame<'a> {
    function: &'a CustomFunction,
    /// The definition tree: an index into the function scope.
    offset: usize,
    caller: Option<usize>,
    /// Resolved parameters; during argument resolution, only the earlier ones.
    parameters: Vec<Option<VariableValue>>,
    /// The locals of the evaluated body, once argument resolution is over.
    locals: Option<Vec<(&'a Name, Local<'a>)>>,
}

/// A guarded substitution context: a function evaluation, or a local's resolution.
///
/// <https://drafts.csswg.org/css-values-5/#substitution-context>
struct GuardedContext<'a> {
    function: Option<&'a CustomFunction>,
    attribute: Option<crate::values::specified::AttrName>,
    cyclic: bool,
}

/// The custom function evaluations of one substitution.
pub(super) struct FunctionCalls<'a> {
    level: Option<CascadeLevel>,
    scope: Option<FunctionScope<'a>>,
    frames: Vec<Frame<'a>>,
    stack: Vec<GuardedContext<'a>>,
}

impl<'a> FunctionCalls<'a> {
    /// Calls made by a declaration at `level`; `None` where the value has no calls.
    pub fn new(level: Option<CascadeLevel>) -> Self {
        Self {
            level,
            scope: None,
            frames: Vec::new(),
            stack: Vec::new(),
        }
    }

    fn guard(&mut self, function: Option<&'a CustomFunction>) -> usize {
        self.stack.push(GuardedContext {
            function,
            attribute: None,
            cyclic: false,
        });
        self.stack.len() - 1
    }

    /// Ends a guard; returns whether its context became cyclic.
    pub(super) fn release(&mut self, entry: usize) -> bool {
        debug_assert_eq!(entry, self.stack.len() - 1);
        self.stack.pop().is_some_and(|context| context.cyclic)
    }

    fn mark_cyclic(&mut self, entry: usize) {
        for context in &mut self.stack[entry..] {
            context.cyclic = true;
        }
    }

    pub(super) fn guard_attribute(
        &mut self,
        name: &crate::values::specified::AttrName,
    ) -> Result<usize, ()> {
        if let Some(entry) = self
            .stack
            .iter()
            .position(|context| context.attribute.as_ref() == Some(name))
        {
            self.mark_cyclic(entry);
            return Err(());
        }
        let entry = self.guard(None);
        self.stack[entry].attribute = Some(name.clone());
        Ok(entry)
    }
}

pub(super) fn keyword(css: &str) -> Option<CSSWideKeyword> {
    let mut input = ParserInput::new(css);
    Parser::new(&mut input)
        .parse_entirely(|input| {
            CSSWideKeyword::parse(input).map_err(|()| input.new_error_for_next_token::<()>())
        })
        .ok()
}

fn owned(substitution: Substitution, url_data: &UrlExtraData) -> VariableValue {
    let mut value = VariableValue::new(
        substitution.css.into_owned(),
        url_data,
        substitution.first_token_type,
        substitution.last_token_type,
    );
    value.attr_tainted = substitution.attr_tainted;
    value
}

impl<'a, 'b, 't> Substitutor<'a, 'b, 't> {
    pub(super) fn style_query(
        &mut self,
        frame: Option<usize>,
        name: &Name,
        value: Option<&VariableValue>,
    ) -> bool {
        let mut binding = frame;
        while let Some(index) = binding {
            let current = &self.calls.frames[index];
            if current.function.parameter(name).is_some()
                || current
                    .locals
                    .as_ref()
                    .is_some_and(|locals| locals.iter().any(|(local, _)| *local == name))
            {
                break;
            }
            binding = current.caller;
        }
        let registration = match binding {
            Some(index) => self.calls.frames[index].function.registration(name).clone(),
            None => self.stylist.get_custom_property_registration(name).clone(),
        };
        let initial = match binding {
            Some(index) => self.calls.frames[index]
                .function
                .parameter(name)
                .and_then(|parameter| self.calls.frames[index].parameters[parameter].clone()),
            None => registration
                .initial_value
                .as_ref()
                .map(|value| (**value).clone()),
        };
        let current = self
            .variable(frame, name)
            .map(|value| value.css.into_owned());
        let Some(value) = value else {
            return current.as_deref()
                != registration
                    .initial_value
                    .as_ref()
                    .map(|value| value.css.as_str());
        };
        let queried = match keyword(&value.css) {
            Some(CSSWideKeyword::Initial) => initial,
            Some(CSSWideKeyword::Inherit) => self
                .inherit_function(frame, name)
                .map(|inherited| owned(inherited, &value.url_data)),
            Some(CSSWideKeyword::Unset) if frame.is_some() => None,
            Some(CSSWideKeyword::Unset) if registration.inherits() => self
                .inherit_function(frame, name)
                .map(|inherited| owned(inherited, &value.url_data)),
            Some(CSSWideKeyword::Unset) => initial,
            Some(_) => return false,
            None => self
                .substitute_value(value, frame)
                .ok()
                .and_then(|value_result| self.typed(&registration, value_result, &value.url_data)),
        };
        current.as_deref() == queried.as_ref().map(|value| value.css.as_str())
    }

    pub(super) fn inherit_function(
        &mut self,
        frame: Option<usize>,
        name: &Name,
    ) -> Option<Substitution<'a>> {
        if let Some(frame) = frame {
            return self.variable(self.calls.frames[frame].caller, name);
        }
        let registration = self.stylist.get_custom_property_registration(name);
        self.computed_context
            .inherited_custom_properties()
            .get(registration, name)
            .map(|value| Substitution::from_value(value.to_variable_value()))
    }

    /// The value of `var(name)` seen from `frame`, or from the element.
    pub(super) fn frame_variable(
        &mut self,
        mut frame: Option<usize>,
        name: &Name,
    ) -> Option<Option<VariableValue>> {
        while let Some(index) = frame {
            let current = &self.calls.frames[index];
            if let Some(local) = current
                .locals
                .as_ref()
                .and_then(|locals| locals.iter().position(|(local, _)| *local == name))
            {
                return Some(self.local(index, local));
            }
            if let Some(parameter) = current.function.parameter(name) {
                return Some(current.parameters.get(parameter).cloned().flatten());
            }
            frame = current.caller;
        }
        None
    }

    fn typed(
        &self,
        registration: &PropertyRegistrationData,
        value: Substitution,
        url_data: &UrlExtraData,
    ) -> Option<VariableValue> {
        if registration.syntax.is_universal() {
            return Some(owned(value, url_data));
        }
        let tainted = !value.attr_tainted.is_empty();
        let computed = compute_value(&value.css, url_data, registration, self.computed_context)
            .ok()?
            .to_variable_value();
        let computed = Substitution::from_value(computed);
        Some(owned(
            if tainted {
                computed.attr_tainted()
            } else {
                computed
            },
            url_data,
        ))
    }

    /// The value of `name` on the caller, typed by this frame's registration.
    fn inherited(
        &mut self,
        frame: usize,
        name: &Name,
        url_data: &UrlExtraData,
    ) -> Option<VariableValue> {
        let caller = self.calls.frames[frame].caller;
        let value = self.variable(caller, name)?;
        let function = self.calls.frames[frame].function;
        self.typed(function.registration(name), value, url_data)
    }

    /// A declared parameter or local value after substitution, with CSS-wide keywords resolved.
    fn declared(
        &mut self,
        frame: usize,
        name: &Name,
        value: Substitution,
        url_data: &UrlExtraData,
        initial: impl FnOnce(&Self) -> Option<VariableValue>,
    ) -> Option<VariableValue> {
        match keyword(&value.css) {
            Some(CSSWideKeyword::Initial) => initial(self),
            Some(CSSWideKeyword::Inherit) => self.inherited(frame, name, url_data),
            Some(_) => None,
            None => {
                let function = self.calls.frames[frame].function;
                self.typed(function.registration(name), value, url_data)
            },
        }
    }

    fn local_state(&mut self, frame: usize, index: usize) -> &mut (&'a Name, Local<'a>) {
        &mut self.calls.frames[frame]
            .locals
            .as_mut()
            .expect("locals exist once arguments resolve")[index]
    }

    fn local(&mut self, frame: usize, index: usize) -> Option<VariableValue> {
        let (name, state) = self.local_state(frame, index).clone();
        let declared = match state {
            Local::Done(value) => return value,
            Local::InProgress(entry) => {
                self.calls.mark_cyclic(entry);
                return None;
            },
            Local::Pending(declared) => declared,
        };
        let entry = self.calls.guard(None);
        self.local_state(frame, index).1 = Local::InProgress(entry);
        let value = self
            .substitute_value(declared, Some(frame))
            .ok()
            .and_then(|value| {
                self.declared(frame, name, value, &declared.url_data, |this| {
                    let current = &this.calls.frames[frame];
                    current
                        .function
                        .parameter(name)
                        .and_then(|parameter| current.parameters[parameter].clone())
                })
            });
        let value = if self.calls.release(entry) {
            None
        } else {
            value
        };
        self.local_state(frame, index).1 = Local::Done(value.clone());
        value
    }

    fn body(
        &self,
        items: &'a [FunctionBodyItem],
        locals: &mut Vec<(&'a Name, Local<'a>)>,
        result: &mut Option<&'a VariableValue>,
    ) {
        for item in items {
            match *item {
                FunctionBodyItem::Descriptor(FunctionDescriptor {
                    name: FunctionDescriptorName::Local(ref name),
                    ref value,
                }) => {
                    locals.retain(|(local, _)| *local != name);
                    locals.push((name, Local::Pending(value)));
                },
                FunctionBodyItem::Descriptor(FunctionDescriptor {
                    name: FunctionDescriptorName::Result,
                    ref value,
                }) => *result = Some(value),
                FunctionBodyItem::Container(ref conditions, ref items) => {
                    if self.computed_context.function_container_matches(conditions) {
                        self.body(items, locals, result);
                    }
                },
            }
        }
    }

    /// Evaluate `--name(arguments)` called from `caller`.
    ///
    /// <https://drafts.csswg.org/css-mixins-1/#evaluate-a-custom-function>
    pub(super) fn call(
        &mut self,
        caller: Option<usize>,
        name: &DashedIdent,
        arguments: Vec<Option<Substitution<'static>>>,
        url_data: &UrlExtraData,
    ) -> Option<Substitution<'a>> {
        let level = self.calls.level.unwrap_or(self.computed_context.scope);
        let computed_context = self.computed_context;
        let scope = self
            .calls
            .scope
            .get_or_insert_with(|| computed_context.function_scope(level));
        let offset = caller.map_or(0, |frame| self.calls.frames[frame].offset);
        let (function, offset) = lookup(scope, offset, name)?;
        computed_context
            .rule_cache_conditions
            .borrow_mut()
            .set_uncacheable();
        if arguments.len() > function.parameters.len()
            || function.parameters[arguments.len()..]
                .iter()
                .any(|parameter| parameter.default.is_none())
        {
            return None;
        }
        if let Some(entry) = self.calls.stack.iter().position(|context| {
            context
                .function
                .is_some_and(|other| std::ptr::eq(other, function))
        }) {
            self.calls.mark_cyclic(entry);
            return None;
        }
        let entry = self.calls.guard(Some(function));
        self.calls.frames.push(Frame {
            function,
            offset,
            caller,
            parameters: Vec::with_capacity(function.parameters.len()),
            locals: None,
        });
        let frame = self.calls.frames.len() - 1;
        let mut arguments = arguments.into_iter();
        for parameter in function.parameters.iter() {
            let initial = |_: &Self| None;
            let value = arguments
                .next()
                .flatten()
                .and_then(|argument| {
                    self.declared(frame, &parameter.name, argument, url_data, initial)
                })
                .or_else(|| {
                    let default = parameter.default.as_deref()?;
                    let value = self
                        .substitute_value(default, Some(frame))
                        .ok()?
                        .into_owned();
                    self.declared(frame, &parameter.name, value, &default.url_data, initial)
                });
            self.calls.frames[frame].parameters.push(value);
        }
        let mut locals = Vec::new();
        let mut result = None;
        self.body(&function.body, &mut locals, &mut result);
        let count = locals.len();
        self.calls.frames[frame].locals = Some(locals);
        for index in 0..count {
            self.local(frame, index);
        }
        let value = result.and_then(|declared| {
            let value = self.substitute_value(declared, Some(frame)).ok()?;
            if function.result.syntax.is_universal() {
                return Some(owned(value, &declared.url_data));
            }
            self.typed(&function.result, value, &declared.url_data)
        });
        self.calls.frames.pop();
        if self.calls.release(entry) {
            return None;
        }
        value.map(Substitution::from_value)
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::super::{AttrTaintedRange, DeferredCustomProperties, VariableValue};
    use super::super::{CustomPropertiesBuilder, DeferFontRelativeCustomPropertyResolution};
    use crate::Atom;
    use crate::applicable_declarations::CascadePriority;
    use crate::context::QuirksMode;
    use crate::dom::{AttributeProvider, AttributeTracker, ExpandedAttributeName};
    use crate::properties::{PropertyDeclaration, StyleBuilder};
    use crate::rule_cache::RuleCacheConditions;
    use crate::rule_tree::CascadeLevel;
    use crate::shared_lock::StylesheetGuards;
    use crate::stylesheets::container_rule::ContainerSizeQuery;
    use crate::stylesheets::layer_rule::LayerOrder;
    use crate::stylesheets::{CssRule, DocumentStyleSheet, StylesheetInDocument};
    use crate::stylist::Stylist;
    use crate::test_support::{BoolPrefGuard, pref_lock};
    use crate::test_support::{parse_stylesheet, test_device};
    use crate::values::computed::Context;
    use servo_arc::Arc;

    struct Attributes(&'static [(&'static str, &'static str)]);

    impl AttributeProvider for Attributes {
        fn get_attr(&self, attr: &ExpandedAttributeName) -> Option<String> {
            self.0
                .iter()
                .find(|(name, _)| *name == &*attr.local_name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    /// The `--actual` and `--expected` values of the last style rule, as in
    /// WPT's `css/css-mixins/resources/utils.js`.
    fn actual_and_expected(css: &str) -> (Option<String>, Option<String>) {
        let [actual, expected] = cascade(css, &Attributes(&[]), ["actual", "expected"]);
        (
            actual.map(|value| value.css),
            expected.map(|value| value.css),
        )
    }

    /// The computed values of `names` from the custom properties of the last style rule.
    fn cascade<const N: usize>(
        css: &str,
        attributes: &Attributes,
        names: [&str; N],
    ) -> [Option<VariableValue>; N] {
        cascade_with(
            css,
            attributes,
            names,
            DeferFontRelativeCustomPropertyResolution::No,
        )
        .0
    }

    fn cascade_with<const N: usize>(
        css: &str,
        attributes: &Attributes,
        names: [&str; N],
        defer: DeferFontRelativeCustomPropertyResolution,
    ) -> ([Option<VariableValue>; N], Option<DeferredCustomProperties>) {
        let _guard = pref_lock().lock().unwrap();
        let _attr_pref = BoolPrefGuard::set("layout.css.attr.enabled", true);
        let _style_pref = BoolPrefGuard::set("layout.css.style-queries.enabled", true);
        let sheet = Arc::new(parse_stylesheet(css));
        let mut stylist = Stylist::new(test_device(), QuirksMode::NoQuirks);
        let guard = sheet.shared_lock.read();
        stylist.append_stylesheet(DocumentStyleSheet(sheet.clone(), None), &guard);
        stylist.flush(&StylesheetGuards::same(&guard));
        let block = sheet
            .contents(&guard)
            .rules(&guard)
            .iter()
            .rev()
            .find_map(|rule| match *rule {
                CssRule::Style(ref rule) => Some(rule.read_with(&guard).block.clone()),
                _ => None,
            })
            .expect("a target rule");
        let block = block.read_with(&guard);
        let mut conditions = RuleCacheConditions::default();
        let mut context = Context::new(
            StyleBuilder::new(stylist.device(), Some(&stylist), None, None, None, false),
            QuirksMode::NoQuirks,
            &mut conditions,
            ContainerSizeQuery::none(),
        );
        let mut tracker = AttributeTracker::new(attributes);
        let mut builder = CustomPropertiesBuilder::new(&stylist, &mut context);
        for (declaration, _) in block.declaration_importance_iter() {
            if let PropertyDeclaration::Custom(ref declaration) = *declaration {
                let priority = CascadePriority::new(
                    CascadeLevel::same_tree_author_normal(),
                    LayerOrder::root(),
                );
                builder.cascade(declaration, priority, 0, &mut tracker);
            }
        }
        let deferred = builder.build(defer, &mut tracker);
        let values = names.map(|name| {
            context
                .builder
                .custom_properties
                .inherited
                .get(&Atom::from(name))
                .map(|value| value.to_variable_value())
        });
        (values, deferred)
    }

    fn assert_templates(templates: &[(&str, &str)]) {
        let failures = templates
            .iter()
            .filter_map(|&(name, css)| {
                let (actual, expected) = actual_and_expected(css);
                (actual != expected).then(|| format!("{name}: {actual:?} != {expected:?}"))
            })
            .collect::<Vec<_>>();
        assert!(failures.is_empty(), "{failures:#?}");
    }

    #[test]
    fn results_parameters_and_defaults() {
        assert_templates(&[
            (
                "literal result",
                "@function --f() { result: 12px; } #t { --actual: --f(); --expected: 12px; }",
            ),
            (
                "typed return computes",
                "@function --f() returns <length> { result: calc(12px + 1px); } \
                 #t { --actual: --f(); --expected: 13px; }",
            ),
            (
                "typed return mismatch",
                "@function --f() returns <length> { result: 12s; } #t { --actual: --f(); }",
            ),
            (
                "missing result",
                "@function --f() { } #t { --actual: --f(); }",
            ),
            (
                "empty result",
                "@function --f() { result:; } #t { --actual: --f(); --expected:; }",
            ),
            (
                "later result wins",
                "@function --f() { result: 12px; result: 24px; } \
                 #t { --actual: --f(); --expected: 24px; }",
            ),
            (
                "nested call",
                "@function --f() { result: --g(); } @function --g() { result: 12px; } \
                 #t { --actual: --f(); --expected: 12px; }",
            ),
            (
                "multiple parameters",
                "@function --f(--x, --y, --z) { result: var(--x) var(--y) var(--z); } \
                 #t { --actual: --f(100px, auto, red); --expected: 100px auto red; }",
            ),
            (
                "typed parameters compute",
                "@function --f(--x <length>, --y <angle>, --z <time>) \
                 { result: var(--x) var(--y) var(--z); } \
                 #t { --actual: --f(calc(100px + 1px), 1turn, 1000ms); \
                 --expected: 101px 360deg 1s; }",
            ),
            (
                "universal parameter keeps calc()",
                "@function --f(--x type(*)) { result: var(--x); } \
                 #t { --actual: --f(calc(100px + 1px)); --expected: calc(100px + 1px); }",
            ),
            (
                "var() in argument resolves at the call site",
                "@function --f(--x) { --one: FAIL; result: var(--x); } \
                 #t { --one: 1px; --actual: --f(calc(100px + var(--one))); \
                 --expected: calc(100px + 1px); }",
            ),
            (
                "invalid argument triggers fallback",
                "@function --f(--x <length>) { result: var(--x, PASS); } \
                 #t { --x: FAIL; --actual: --f(red); --expected: PASS; }",
            ),
            (
                "defaults",
                "@function --f(--x, --y <length>: 2px, --z <length>: 3px) \
                 { result: var(--x) var(--y) var(--z); } \
                 #t { --actual: --f(1px, 5px); --expected: 1px 5px 3px; }",
            ),
            (
                "invalid arguments are defaulted",
                "@function --f(--x <number>: 1, --y <number>, --z <number>: 3) \
                 { result: var(--x) var(--y) var(--z); } \
                 #t { --actual: --f(red, 2, var(--unknown)); --expected: 1 2 3; }",
            ),
            (
                "default sees an earlier parameter, not a local",
                "@function --f(--x, --y: var(--x)) { --x: 17px; result: var(--x) var(--y); } \
                 #t { --x: FAIL; --y: FAIL; --actual: --f(5px); --expected: 17px 5px; }",
            ),
            (
                "typed default with reference",
                "@function --f(--x: 5px, --y <length>: calc(var(--x) + 1px)) \
                 { result: var(--x) var(--y); } \
                 #t { --x: FAIL; --actual: --f(); --expected: 5px 6px; }",
            ),
            (
                "default referencing a later parameter is invalid (WPT over spec)",
                "@function --f(--a: var(--b), --b: 3px) { result: var(--a, PASS); } \
                 #t { --b: FAIL; --actual: --f(); --expected: PASS; }",
            ),
            (
                "missing argument without default invalidates the call (WPT over spec)",
                "@function --f(--x, --y, --z) { result: 10px; } #t { --actual: --f(1, 2); }",
            ),
            (
                "too many arguments",
                "@function --f(--x) { result: 10px; } #t { --actual: --f(1, 2); }",
            ),
            (
                "{}-wrapped list argument",
                "@function --f(--x, --y) { result: var(--x) | var(--y); } \
                 #t { --actual: --f({1px, 2px}, 3px); --expected: 1px, 2px | 3px; }",
            ),
        ]);
    }

    #[test]
    fn conditional_values_use_local_registrations_and_substitution() {
        assert_templates(&[
            (
                "local value",
                "@function --f() { --x: 3px; result: if(style(--x: 3px): PASS; else: FAIL); } \
                 #t { --x: 5px; --actual: --f(); --expected: PASS; }",
            ),
            (
                "queried value",
                "@function --f() { --x: 3px; result: if(style(--y: var(--x)): PASS; else: FAIL); } \
                 #t { --y: 3px; --actual: --f(); --expected: PASS; }",
            ),
            (
                "condition dependencies resolve first",
                "@function --f() { result: if(style(--y: var(--x)): PASS; else: FAIL); } \
                 #t { --actual: --f(); --x: var(--source); --source: 3px; --y: 3px; --expected: PASS; }",
            ),
            (
                "typed parameter",
                "@function --f(--c <color>) { result: if(style(--c: red): PASS; else: FAIL); } \
                 #t { --actual: --f(#f00); --expected: PASS; }",
            ),
            (
                "boolean query of an argument",
                "@function --f(--x) { result: if(style(--x): PASS; else: FAIL); } \
                 #t { --x: var(--x); --actual: --f(3px); --expected: PASS; }",
            ),
            (
                "typed caller parameter",
                "@function --f(--c <color>) { result: --g(); } \
                 @function --g() { result: if(style(--c: red): PASS; else: FAIL); } \
                 #t { --actual: --f(#f00); --expected: PASS; }",
            ),
            (
                "initial argument",
                "@function --f(--c: green) { result: if(style(--c: initial): PASS; else: FAIL); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "invalid query value",
                "@function --f() { result: if(style(--c: initial): PASS; else: FAIL); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "cyclic query",
                "@function --f() { --x: if(style(--x): FAIL; else: FAIL); result: var(--x, PASS); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "unused branch does not cycle",
                "@function --f() { --x: if(else: PASS; else: var(--x)); result: var(--x); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "supports and media",
                "@function --f() { result: if(supports(color: green) and media(width > 0px): PASS; else: FAIL); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
        ]);
    }

    #[test]
    fn inherit_reads_the_calling_frame() {
        assert_templates(&[
            (
                "element",
                "@function --f() { --x: FAIL; result: inherit(--x); } \
                 #t { --x: PASS; --actual: --f(); --expected: PASS; }",
            ),
            (
                "function",
                "@function --f() { --x: PASS; result: --g(); } \
                 @function --g() { --x: FAIL; result: inherit(--x); } \
                 #t { --x: FAIL; --actual: --f(); --expected: PASS; }",
            ),
            (
                "invalid parent value",
                "@function --f() { --x: var(--missing); result: --g(); } \
                 @function --g() { --x: FAIL; result: inherit(--x, PASS); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "cyclic parent value",
                "@function --f() { --x: var(--x); result: --g(); } \
                 @function --g() { --x: FAIL; result: inherit(--x, PASS); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "typed parent value",
                "@function --f(--x <length>) { result: --g(); } \
                 @function --g() { --x: 14px; result: inherit(--x); } \
                 #t { --actual: --f(42.0px); --expected: 42px; }",
            ),
        ]);
    }

    #[test]
    fn locals_and_dynamic_scope() {
        assert_templates(&[
            (
                "locals cascade",
                "@function --f() { --x: 10px; --y: var(--x); result: var(--y); --x: 20px; } \
                 #t { --actual: --f(); --expected: 20px; }",
            ),
            (
                "local does not leak",
                "@function --f() { --x: 10px; result: 1px; } \
                 #t { --x: 20px; --actual: --f() var(--x); --expected: 1px 20px; }",
            ),
            (
                "custom properties are visible",
                "@function --f() { result: var(--x); } \
                 #t { --x: 10px; --actual: --f(); --expected: 10px; }",
            ),
            (
                "caller locals and arguments are visible",
                "@function --f(--y) { --x: PASS; result: --g(); } \
                 @function --g() { result: var(--x) var(--y); } \
                 #t { --x: FAIL; --y: FAIL; --actual: --f(OK); --expected: PASS OK; }",
            ),
            (
                "same function, different scopes",
                "@function --one() { --x: 1; result: --f(); } \
                 @function --two() { --x: 2; result: --f(); } \
                 @function --f() { result: var(--x); } \
                 #t { --x: 0; --actual: --one() --two() --f(); --expected: 1 2 0; }",
            ),
            (
                "inner call sees resolved outer locals",
                "@function --a() { --x: --b(); --y: var(--px); result: var(--x); } \
                 @function --b() { result: var(--y, FAIL); } \
                 #t { --px: 10px; --actual: --a(); --expected: 10px; }",
            ),
            (
                "outer typed argument",
                "@function --f(--l <length>: 10.00px) { result: --g(); } \
                 @function --g() { result: var(--l); } \
                 #t { --actual: --f(); --expected: 10px; }",
            ),
            (
                "initial gives the argument",
                "@function --f(--x: FAIL1) { --x: FAIL2; --x: initial; result: var(--x); } \
                 #t { --actual: --f(PASS); --expected: PASS; }",
            ),
            (
                "initial via fallback",
                "@function --f(--x: PASS) { --x: var(--unknown, initial); result: var(--x); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "inherit gives the caller value",
                "@function --f(--x: FAIL1) { --x: inherit; result: var(--x); } \
                 @function --g(--x) { --x: PASS; result: --f(FAIL3); } \
                 #t { --actual: --g(FAIL4); --expected: PASS; }",
            ),
            (
                "default with inherit",
                "@function --f(--x: inherit) { result: var(--x); } \
                 #t { --x: PASS1; --actual: --f() --f(PASS2); --expected: PASS1 PASS2; }",
            ),
            (
                "other keywords are invalid in locals",
                "@function --f() { --x: unset; --y: revert-layer; \
                 result: var(--x, PASS) var(--y, PASS); } \
                 #t { --actual: --f(); --expected: PASS PASS; }",
            ),
            (
                "result keyword is left unresolved",
                "@function --f() { result: initial; } \
                 #t { --tmp: --f(); --actual: var(--tmp, PASS); --expected: PASS; }",
            ),
            (
                "typed result rejects a keyword",
                "@function --f() returns <length> { result: initial; } \
                 #t { --tmp: --f(); --actual: var(--tmp, PASS); --expected: PASS; }",
            ),
            (
                "conditional bodies",
                "@function --f() { --x: FAIL; @supports (color: green) { \
                 @media (width > 0px) { --x: PASS; } } result: var(--x); \
                 @supports (not (color: green)) { result: FAIL; } } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "stronger layer wins",
                "@layer theme, base; @layer base { @function --f() { result: 10px; } } \
                 @layer theme { @function --f() { result: 20px; } } \
                 #t { --actual: --f(); --expected: 10px; }",
            ),
            (
                "unlayered wins",
                "@function --f() { result: 3px; } @layer { @function --f() { result: 1px; } } \
                 #t { --actual: --f(); --expected: 3px; }",
            ),
        ]);
    }

    #[test]
    fn cycles() {
        assert_templates(&[
            (
                "local self-cycle",
                "@function --f() { --x: var(--x); result: var(--x, PASS); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "local shadows a cyclic property",
                "@function --f() { --x: var(--y, PASS); result: var(--x); } \
                 #t { --x: var(--y); --y: var(--x); --actual: --f(); --expected: PASS; }",
            ),
            (
                "function self-cycle",
                "@function --f() { result: --f(); } \
                 #t { --tmp: --f(); --actual: var(--tmp, PASS); --expected: PASS; }",
            ),
            (
                "cycle through an unused local",
                "@function --f() { --unused: --f(); result: FAIL; } \
                 #t { --tmp: --f(); --actual: var(--tmp, PASS); --expected: PASS; }",
            ),
            (
                "cycle through a custom property",
                "@function --f() { result: var(--global); } \
                 #t { --global: --f(); --tmp: --f(); --actual: var(--tmp, PASS); \
                 --expected: PASS; }",
            ),
            (
                "cycle through a local in another function",
                "@function --f() { --a: --g(); result: var(--a, PASS); } \
                 @function --g() { result: var(--a); } \
                 #t { --actual: --f(); --expected: PASS; }",
            ),
            (
                "cyclic default",
                "@function --f(--x, --y: --f(13px)) { result: 10px; } \
                 #t { --tmp: --f(42px); --actual: var(--tmp, PASS); --expected: PASS; }",
            ),
            (
                "parameters bind, so properties do not cycle",
                "@function --baz(--x) { --y: 10px; result: calc(var(--x) + var(--y)); } \
                 #t { --x: --baz(1px); --y: --baz(2px); --actual: var(--x) var(--y); \
                 --expected: calc(1px + 10px) calc(2px + 10px); }",
            ),
            (
                "an argument calling the same function is not a cycle",
                "@function --bump(--v) returns <integer> { result: calc(var(--v) + 1); } \
                 #t { --late: --bump(var(--early)); --early: --bump(1); \
                 --actual: var(--late); --expected: 3; }",
            ),
            (
                "an argument reading its own property is a cycle",
                "@function --bump(--v) returns <integer> { result: calc(var(--v) + 1); } \
                 #t { --cyclic: --bump(var(--cyclic)); --actual: var(--cyclic); }",
            ),
        ]);
    }

    #[test]
    fn attribute_values_substitute_in_the_calling_frame() {
        let attributes = Attributes(&[("data-x", "var(--x)"), ("data-f", "--f()")]);
        for (name, css, expected) in [
            (
                "local",
                "@function --f() { --x: PASS; result: attr(data-x type(*)); } \
                 #t { --x: FAIL; --actual: --f(); }",
                "PASS",
            ),
            (
                "parameter",
                "@function --f(--x) { result: attr(data-x type(*)); } \
                 #t { --x: FAIL; --actual: --f(PASS); }",
                "PASS",
            ),
            (
                "typed",
                "@function --f() returns <length> { --x: calc(10px + 2px); \
                 result: attr(data-x type(<length>)); } #t { --actual: --f(); }",
                "12px",
            ),
            (
                "local cycle",
                "@function --f() { --x: attr(data-x type(*)); \
                 --y: attr(data-x type(*), PASS); result: var(--y, PASS); } \
                 #t { --actual: --f(); }",
                "PASS",
            ),
            (
                "function cycle",
                "@function --f() { --local: --g(); result: var(--local); } \
                 @function --g() { result: attr(data-f type(*)); } \
                 #t { --tmp: --f(); --actual: var(--tmp, PASS); }",
                "PASS",
            ),
        ] {
            let [actual] = cascade(css, &attributes, ["actual"]);
            assert_eq!(
                actual.map(|value| value.css).as_deref(),
                Some(expected),
                "{name}"
            );
        }
    }

    #[test]
    fn attr_results_stay_tainted_through_calls() {
        let attributes = Attributes(&[("data-cat", "url(cat.png)")]);
        for (name, css) in [
            (
                "result",
                "@function --f() { result: attr(data-cat type(*)); }",
            ),
            (
                "typed result",
                "@function --f() returns <url> { result: attr(data-cat type(*)); }",
            ),
            (
                "local",
                "@function --f() { --l: attr(data-cat type(*)); result: var(--l); }",
            ),
            ("argument", "@function --f(--a) { result: var(--a); }"),
            (
                "default",
                "@function --f(--a: attr(data-cat type(*))) { result: var(--a); }",
            ),
            (
                "caller frame",
                "@function --f() { --x: attr(data-cat type(*)); result: --g(); } \
                 @function --g() { result: var(--x); }",
            ),
            (
                "inherit",
                "@function --f() { --x: attr(data-cat type(*)); result: --g(); } \
                 @function --g() { --x: inherit; result: var(--x); }",
            ),
        ] {
            let call = if name == "argument" {
                "--f(attr(data-cat type(*)))"
            } else {
                "--f()"
            };
            let css = format!("{css} #t {{ --actual: a {call} b; }}");
            let [actual] = cascade(&css, &attributes, ["actual"]);
            let actual = actual.unwrap_or_else(|| panic!("{name}: invalid"));
            let start = actual.css.find("url").expect("the url is substituted");
            assert_eq!(
                actual.attr_tainted.as_slice(),
                [AttrTaintedRange {
                    start,
                    end: actual.css.len() - 2
                }],
                "{name}: {}",
                actual.css
            );
        }
        let [actual] = cascade(
            "@function --f() { result: url(cat.png); } #t { --actual: --f(); }",
            &attributes,
            ["actual"],
        );
        assert!(actual.unwrap().attr_tainted.is_empty());
    }

    #[test]
    fn font_relative_results_defer_registered_properties() {
        for (name, function, deferred) in [
            ("untyped result", "@function --f() { result: 1em; }", true),
            (
                "typed result",
                "@function --f() returns <length> { result: 1em; }",
                true,
            ),
            (
                "no font-relative units",
                "@function --f() { result: 1px; }",
                false,
            ),
        ] {
            let css = format!(
                "@property --actual {{ syntax: '<length>'; inherits: true; initial-value: 0px; }} \
                 {function} #t {{ --actual: --f(); }}"
            );
            let (_, values) = cascade_with(
                &css,
                &Attributes(&[]),
                [],
                DeferFontRelativeCustomPropertyResolution::Yes,
            );
            let actual =
                values.is_some_and(|values| values.values.get(&Atom::from("actual")).is_some());
            assert_eq!(actual, deferred, "{name}");
        }
    }
}
