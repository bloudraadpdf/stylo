/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! A [`@container`][container] rule.
//!
//! [container]: https://drafts.csswg.org/css-contain-3/#container-rule

use crate::computed_value_flags::ComputedValueFlags;
use crate::derives::*;
use crate::dom::TElement;
use crate::logical_geometry::{LogicalSize, WritingMode};
use crate::parser::ParserContext;
use crate::properties::ComputedValues;
use crate::queries::feature::{AllowsRanges, Evaluator, FeatureFlags, QueryFeatureDescription};
use crate::queries::scroll_state::ContainerScrollState;
use crate::queries::values::Orientation;
use crate::queries::{FeatureType, QueryCondition};
use crate::shared_lock::{
    DeepCloneWithLock, Locked, SharedRwLock, SharedRwLockReadGuard, ToCssWithGuard,
};
use crate::stylesheets::{CssRules, CustomMediaEvaluator};
use crate::stylist::Stylist;
use crate::values::computed::{CSSPixelLength, ContainerType, Context, Ratio};
use crate::values::specified::ContainerName;
use app_units::Au;
use cssparser::{Parser, SourceLocation};
use euclid::default::Size2D;
#[cfg(feature = "gecko")]
use malloc_size_of::{MallocSizeOfOps, MallocUnconditionalShallowSizeOf};
use selectors::kleene_value::KleeneValue;
use servo_arc::Arc;
use std::fmt::{self, Write};
use style_traits::{CssStringWriter, CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// A container rule.
#[derive(Debug, ToShmem)]
pub struct ContainerRule {
    /// The non-empty container condition list.
    pub conditions: Arc<ContainerConditions>,
    /// The nested rules inside the block.
    pub rules: Arc<Locked<CssRules>>,
    /// The source position where this rule was found.
    pub source_location: SourceLocation,
}

impl ContainerRule {
    /// Returns every comma-separated condition in source order.
    pub fn conditions(&self) -> impl Iterator<Item = &ContainerCondition> {
        self.conditions.iter()
    }

    /// Returns the sole condition used by the legacy CSSOM attributes.
    pub fn single_condition(&self) -> Option<&ContainerCondition> {
        self.conditions.single()
    }

    /// Measure heap usage.
    #[cfg(feature = "gecko")]
    pub fn size_of(&self, guard: &SharedRwLockReadGuard, ops: &mut MallocSizeOfOps) -> usize {
        // Measurement of other fields may be added later.
        self.rules.unconditional_shallow_size_of(ops)
            + self.rules.read_with(guard).size_of(guard, ops)
    }
}

impl DeepCloneWithLock for ContainerRule {
    fn deep_clone_with_lock(&self, lock: &SharedRwLock, guard: &SharedRwLockReadGuard) -> Self {
        let rules = self.rules.read_with(guard);
        Self {
            conditions: self.conditions.clone(),
            rules: Arc::new(lock.wrap(rules.deep_clone_with_lock(lock, guard))),
            source_location: self.source_location.clone(),
        }
    }
}

impl ToCssWithGuard for ContainerRule {
    fn to_css(&self, guard: &SharedRwLockReadGuard, dest: &mut CssStringWriter) -> fmt::Result {
        dest.write_str("@container ")?;
        self.conditions.to_css(&mut CssWriter::new(dest))?;
        self.rules.read_with(guard).to_css_block(guard, dest)
    }
}

/// A non-empty comma-separated list of container conditions.
#[derive(Debug, ToShmem)]
pub struct ContainerConditions {
    first: ContainerCondition,
    rest: Box<[ContainerCondition]>,
}

impl ContainerConditions {
    /// Parse one or more comma-separated conditions.
    pub fn parse<'a>(
        context: &ParserContext,
        input: &mut Parser<'a, '_>,
    ) -> Result<Self, ParseError<'a>> {
        let mut conditions = input
            .parse_comma_separated(|input| ContainerCondition::parse(context, input))?
            .into_iter();
        let first = conditions
            .next()
            .ok_or_else(|| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))?;
        Ok(Self {
            first,
            rest: conditions.collect(),
        })
    }

    /// Iterate over the non-empty list in source order.
    pub fn iter(&self) -> impl Iterator<Item = &ContainerCondition> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }

    fn single(&self) -> Option<&ContainerCondition> {
        self.rest.is_empty().then_some(&self.first)
    }

    /// Match when any listed condition matches its selected container.
    pub fn matches<E>(
        &self,
        stylist: &Stylist,
        element: E,
        originating_element: Option<OriginatingElement>,
        invalidation_flags: &mut ComputedValueFlags,
    ) -> KleeneValue
    where
        E: TElement,
    {
        KleeneValue::any(self.iter(), |condition| {
            condition.matches(stylist, element, originating_element, invalidation_flags)
        })
    }
}

/// The originating element of a pseudo-element whose container queries are evaluated.
#[derive(Clone, Copy)]
pub struct OriginatingElement<'a> {
    /// The style of the originating element.
    pub style: &'a ComputedValues,
    /// Whether the originating element can be the container of a size query: not when the pseudo-element generates a
    /// box outside the box of the originating element (CSS Conditional 5 §5).
    pub size_container: bool,
}

impl ToCss for ContainerConditions {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        for (index, condition) in self.iter().enumerate() {
            if index != 0 {
                dest.write_str(", ")?;
            }
            condition.to_css(dest)?;
        }
        Ok(())
    }
}

/// A container condition and filter, combined.
#[derive(Debug, ToShmem)]
pub struct ContainerCondition {
    kind: ContainerConditionKind,
    flags: FeatureFlags,
}

#[derive(Debug, ToShmem)]
enum ContainerConditionKind {
    Name(ContainerName),
    Query(QueryCondition),
    NamedQuery {
        name: ContainerName,
        query: QueryCondition,
    },
}

impl ContainerCondition {
    /// Returns the container name filter.
    pub fn name(&self) -> Option<&ContainerName> {
        match &self.kind {
            ContainerConditionKind::Name(name)
            | ContainerConditionKind::NamedQuery { name, .. } => Some(name),
            ContainerConditionKind::Query(_) => None,
        }
    }

    /// Returns the optional query after the name filter.
    pub fn query(&self) -> Option<&QueryCondition> {
        match &self.kind {
            ContainerConditionKind::Query(query)
            | ContainerConditionKind::NamedQuery { query, .. } => Some(query),
            ContainerConditionKind::Name(_) => None,
        }
    }
}

impl ToCss for ContainerCondition {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match &self.kind {
            ContainerConditionKind::Name(name) => name.to_css(dest),
            ContainerConditionKind::Query(query) => query.to_css(dest),
            ContainerConditionKind::NamedQuery { name, query } => {
                name.to_css(dest)?;
                dest.write_char(' ')?;
                query.to_css(dest)
            },
        }
    }
}

/// The result of a successful container query lookup.
pub struct ContainerLookupResult<E> {
    /// The relevant container.
    pub element: E,
    /// The sizing / writing-mode information of the container.
    pub info: ContainerInfo,
    /// The style of the element.
    pub style: Arc<ComputedValues>,
}

fn container_type_axes(ty_: ContainerType, wm: WritingMode) -> FeatureFlags {
    if ty_.intersects(ContainerType::SIZE) {
        FeatureFlags::all_container_axes()
    } else if ty_.intersects(ContainerType::INLINE_SIZE) {
        let physical_axis = if wm.is_vertical() {
            FeatureFlags::CONTAINER_REQUIRES_HEIGHT_AXIS
        } else {
            FeatureFlags::CONTAINER_REQUIRES_WIDTH_AXIS
        };
        FeatureFlags::CONTAINER_REQUIRES_INLINE_AXIS | physical_axis
    } else {
        FeatureFlags::empty()
    }
}

enum TraversalResult<T> {
    InProgress,
    StopTraversal,
    Done(T),
}

fn traverse_container<E, F, R>(
    mut e: E,
    originating_element_style: Option<&ComputedValues>,
    evaluator: F,
) -> Option<(E, R)>
where
    E: TElement,
    F: Fn(E, Option<&ComputedValues>) -> TraversalResult<R>,
{
    if originating_element_style.is_some() {
        match evaluator(e, originating_element_style) {
            TraversalResult::InProgress => {},
            TraversalResult::StopTraversal => return None,
            TraversalResult::Done(result) => return Some((e, result)),
        }
    }
    while let Some(element) = e.traversal_parent() {
        match evaluator(element, None) {
            TraversalResult::InProgress => {},
            TraversalResult::StopTraversal => return None,
            TraversalResult::Done(result) => return Some((element, result)),
        }
        e = element;
    }

    None
}

impl ContainerCondition {
    /// Parse a container condition.
    pub fn parse<'a>(
        context: &ParserContext,
        input: &mut Parser<'a, '_>,
    ) -> Result<Self, ParseError<'a>> {
        let name = input
            .try_parse(|input| ContainerName::parse_for_query(context, input))
            .ok();
        let kind = if input.is_exhausted() {
            ContainerConditionKind::Name(
                name.ok_or_else(|| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))?,
            )
        } else {
            let query = QueryCondition::parse(context, input, FeatureType::Container)?;
            match name {
                Some(name) => ContainerConditionKind::NamedQuery { name, query },
                None => ContainerConditionKind::Query(query),
            }
        };
        let flags = match &kind {
            ContainerConditionKind::Name(_) => FeatureFlags::empty(),
            ContainerConditionKind::Query(query)
            | ContainerConditionKind::NamedQuery { query, .. } => query.cumulative_flags(),
        };
        Ok(Self { kind, flags })
    }

    fn valid_container_info<E>(
        &self,
        potential_container: E,
        originating_element_style: Option<&ComputedValues>,
    ) -> TraversalResult<ContainerLookupResult<E>>
    where
        E: TElement,
    {
        let data;
        let style = match originating_element_style {
            Some(s) => s,
            None => {
                data = match potential_container.borrow_data() {
                    Some(d) => d,
                    None => return TraversalResult::InProgress,
                };
                &**data.styles.primary()
            },
        };
        let wm = style.writing_mode;
        let box_style = style.get_box();

        // Filter by container-type.
        let container_type = box_style.clone_container_type();
        let available_axes = container_type_axes(container_type, wm);
        if !available_axes.contains(self.flags.container_axes()) {
            return TraversalResult::InProgress;
        }
        let scroll_state = self
            .flags
            .contains(FeatureFlags::CONTAINER_REQUIRES_SCROLL_STATE);
        if scroll_state && !container_type.intersects(ContainerType::SCROLL_STATE) {
            return TraversalResult::InProgress;
        }

        // Filter by container-name.
        if let Some(name) = self.name() {
            let container_name = box_style.clone_container_name();
            for filter_name in name.0.iter() {
                if !container_name.0.contains(filter_name) {
                    return TraversalResult::InProgress;
                }
            }
        }

        let size = potential_container.query_container_size(&box_style.clone_display());
        let scroll_state = if scroll_state {
            potential_container.query_container_scroll_state()
        } else {
            ContainerScrollState::default()
        };
        let style = style.to_arc();
        TraversalResult::Done(ContainerLookupResult {
            element: potential_container,
            info: ContainerInfo {
                size,
                wm,
                scroll_state,
            },
            style,
        })
    }

    /// Performs container lookup for a given element.
    pub fn find_container<E>(
        &self,
        e: E,
        originating_element: Option<OriginatingElement>,
    ) -> Option<ContainerLookupResult<E>>
    where
        E: TElement,
    {
        let originating_element_style = originating_element
            .filter(|originating| {
                originating.size_container || self.flags.container_axes().is_empty()
            })
            .map(|originating| originating.style);
        match traverse_container(
            e,
            originating_element_style,
            |element, originating_element_style| {
                self.valid_container_info(element, originating_element_style)
            },
        ) {
            Some((_, result)) => Some(result),
            None => None,
        }
    }

    /// Tries to match a container query condition for a given element.
    ///
    /// A query with `<general-enclosed>` selects no query container, so it is unknown.
    pub fn matches<E>(
        &self,
        stylist: &Stylist,
        element: E,
        originating_element: Option<OriginatingElement>,
        invalidation_flags: &mut ComputedValueFlags,
    ) -> KleeneValue
    where
        E: TElement,
    {
        if self.flags.contains(FeatureFlags::GENERAL_ENCLOSED) {
            return KleeneValue::Unknown;
        }
        let result = self.find_container(element, originating_element);
        let Some(condition) = self.query() else {
            return KleeneValue::from(result.is_some());
        };
        // Set up the lookup for the container in question, as the condition may be using container
        // query lengths.
        let size_query_container_lookup = ContainerSizeQuery::for_option_element(
            result.as_ref().map(|r| r.element),
            /* known_parent_style = */ None,
            /* is_pseudo = */ false,
        );
        Context::for_container_query_evaluation(
            stylist.device(),
            Some(stylist),
            result.as_ref(),
            size_query_container_lookup,
            |context| {
                let matches = condition.matches(context, &mut CustomMediaEvaluator::none());
                if context
                    .style()
                    .flags()
                    .contains(ComputedValueFlags::USES_VIEWPORT_UNITS)
                {
                    // TODO(emilio): Might need something similar to improve
                    // invalidation of font relative container-query lengths.
                    invalidation_flags
                        .insert(ComputedValueFlags::USES_VIEWPORT_UNITS_ON_CONTAINER_QUERIES);
                }
                matches
            },
        )
    }
}

/// Information needed to evaluate an individual container query.
#[derive(Copy, Clone)]
pub struct ContainerInfo {
    size: Size2D<Option<Au>>,
    wm: WritingMode,
    scroll_state: ContainerScrollState,
}

impl ContainerInfo {
    fn size(&self) -> Option<Size2D<Au>> {
        Some(Size2D::new(self.size.width?, self.size.height?))
    }

    pub(crate) fn writing_mode(&self) -> WritingMode {
        self.wm
    }

    pub(crate) fn scroll_state(&self) -> ContainerScrollState {
        self.scroll_state
    }
}

fn eval_width(context: &Context) -> Option<CSSPixelLength> {
    let info = context.container_info.as_ref()?;
    Some(CSSPixelLength::new(info.size.width?.to_f32_px()))
}

fn eval_height(context: &Context) -> Option<CSSPixelLength> {
    let info = context.container_info.as_ref()?;
    Some(CSSPixelLength::new(info.size.height?.to_f32_px()))
}

fn eval_inline_size(context: &Context) -> Option<CSSPixelLength> {
    let info = context.container_info.as_ref()?;
    Some(CSSPixelLength::new(
        LogicalSize::from_physical(info.wm, info.size)
            .inline?
            .to_f32_px(),
    ))
}

fn eval_block_size(context: &Context) -> Option<CSSPixelLength> {
    let info = context.container_info.as_ref()?;
    Some(CSSPixelLength::new(
        LogicalSize::from_physical(info.wm, info.size)
            .block?
            .to_f32_px(),
    ))
}

fn eval_aspect_ratio(context: &Context) -> Option<Ratio> {
    let info = context.container_info.as_ref()?;
    Some(Ratio::new(
        info.size.width?.0 as f32,
        info.size.height?.0 as f32,
    ))
}

fn eval_orientation(context: &Context, value: Option<Orientation>) -> KleeneValue {
    let size = match context.container_info.as_ref().and_then(|info| info.size()) {
        Some(size) => size,
        None => return KleeneValue::Unknown,
    };
    KleeneValue::from(Orientation::eval(size, value))
}

/// https://drafts.csswg.org/css-contain-3/#container-features
///
/// TODO: Support style queries, perhaps.
pub static CONTAINER_FEATURES: [QueryFeatureDescription; 6] = [
    feature!(
        atom!("width"),
        AllowsRanges::Yes,
        Evaluator::OptionalLength(eval_width),
        FeatureFlags::CONTAINER_REQUIRES_WIDTH_AXIS,
    ),
    feature!(
        atom!("height"),
        AllowsRanges::Yes,
        Evaluator::OptionalLength(eval_height),
        FeatureFlags::CONTAINER_REQUIRES_HEIGHT_AXIS,
    ),
    feature!(
        atom!("inline-size"),
        AllowsRanges::Yes,
        Evaluator::OptionalLength(eval_inline_size),
        FeatureFlags::CONTAINER_REQUIRES_INLINE_AXIS,
    ),
    feature!(
        atom!("block-size"),
        AllowsRanges::Yes,
        Evaluator::OptionalLength(eval_block_size),
        FeatureFlags::CONTAINER_REQUIRES_BLOCK_AXIS,
    ),
    feature!(
        atom!("aspect-ratio"),
        AllowsRanges::Yes,
        Evaluator::OptionalNumberRatio(eval_aspect_ratio),
        // XXX from_bits_truncate is const, but the pipe operator isn't, so this
        // works around it.
        FeatureFlags::from_bits_truncate(
            FeatureFlags::CONTAINER_REQUIRES_BLOCK_AXIS.bits()
                | FeatureFlags::CONTAINER_REQUIRES_INLINE_AXIS.bits()
        ),
    ),
    feature!(
        atom!("orientation"),
        AllowsRanges::No,
        keyword_evaluator!(eval_orientation, Orientation),
        FeatureFlags::from_bits_truncate(
            FeatureFlags::CONTAINER_REQUIRES_BLOCK_AXIS.bits()
                | FeatureFlags::CONTAINER_REQUIRES_INLINE_AXIS.bits()
        ),
    ),
];

/// Result of a container size query, signifying the hypothetical containment boundary in terms of physical axes.
/// Defined by up to two size containers. Queries on logical axes are resolved with respect to the querying
/// element's writing mode.
#[derive(Copy, Clone, Default)]
pub struct ContainerSizeQueryResult {
    width: Option<Au>,
    height: Option<Au>,
}

impl ContainerSizeQueryResult {
    fn get_viewport_size(context: &Context) -> Size2D<Au> {
        use crate::values::specified::ViewportVariant;
        context.viewport_size_for_viewport_unit_resolution(ViewportVariant::Small)
    }

    fn get_logical_viewport_size(context: &Context) -> LogicalSize<Au> {
        LogicalSize::from_physical(
            context.builder.writing_mode,
            Self::get_viewport_size(context),
        )
    }

    /// Get the inline-size of the query container.
    pub fn get_container_inline_size(&self, context: &Context) -> Au {
        if context.builder.writing_mode.is_horizontal() {
            if let Some(w) = self.width {
                return w;
            }
        } else {
            if let Some(h) = self.height {
                return h;
            }
        }
        Self::get_logical_viewport_size(context).inline
    }

    /// Get the block-size of the query container.
    pub fn get_container_block_size(&self, context: &Context) -> Au {
        if context.builder.writing_mode.is_horizontal() {
            self.get_container_height(context)
        } else {
            self.get_container_width(context)
        }
    }

    /// Get the width of the query container.
    pub fn get_container_width(&self, context: &Context) -> Au {
        if let Some(w) = self.width {
            return w;
        }
        Self::get_viewport_size(context).width
    }

    /// Get the height of the query container.
    pub fn get_container_height(&self, context: &Context) -> Au {
        if let Some(h) = self.height {
            return h;
        }
        Self::get_viewport_size(context).height
    }

    // Merge the result of a subsequent lookup, preferring the initial result.
    fn merge(self, new_result: Self) -> Self {
        let mut result = self;
        if let Some(width) = new_result.width {
            result.width.get_or_insert(width);
        }
        if let Some(height) = new_result.height {
            result.height.get_or_insert(height);
        }
        result
    }

    fn is_complete(&self) -> bool {
        self.width.is_some() && self.height.is_some()
    }
}

/// Unevaluated lazy container size query.
pub enum ContainerSizeQuery<'a> {
    /// Query prior to evaluation.
    NotEvaluated(Box<dyn Fn() -> ContainerSizeQueryResult + 'a>),
    /// Cached evaluated result.
    Evaluated(ContainerSizeQueryResult),
}

impl<'a> ContainerSizeQuery<'a> {
    fn evaluate_potential_size_container<E>(
        e: E,
        originating_element_style: Option<&ComputedValues>,
    ) -> TraversalResult<ContainerSizeQueryResult>
    where
        E: TElement,
    {
        let data;
        let style = match originating_element_style {
            Some(s) => s,
            None => {
                data = match e.borrow_data() {
                    Some(d) => d,
                    None => return TraversalResult::InProgress,
                };
                &**data.styles.primary()
            },
        };
        if !style
            .flags
            .contains(ComputedValueFlags::SELF_OR_ANCESTOR_HAS_SIZE_CONTAINER_TYPE)
        {
            // We know we won't find a size container.
            return TraversalResult::StopTraversal;
        }

        let wm = style.writing_mode;
        let box_style = style.get_box();

        let container_type = box_style.clone_container_type();
        let size = e.query_container_size(&box_style.clone_display());
        if container_type.intersects(ContainerType::SIZE) {
            TraversalResult::Done(ContainerSizeQueryResult {
                width: size.width,
                height: size.height,
            })
        } else if container_type.intersects(ContainerType::INLINE_SIZE) {
            if wm.is_horizontal() {
                TraversalResult::Done(ContainerSizeQueryResult {
                    width: size.width,
                    height: None,
                })
            } else {
                TraversalResult::Done(ContainerSizeQueryResult {
                    width: None,
                    height: size.height,
                })
            }
        } else {
            TraversalResult::InProgress
        }
    }

    /// Find the query container size for a given element. Meant to be used as a callback for new().
    fn lookup<E>(
        element: E,
        originating_element_style: Option<&ComputedValues>,
    ) -> ContainerSizeQueryResult
    where
        E: TElement + 'a,
    {
        match traverse_container(
            element,
            originating_element_style,
            |e, originating_element_style| {
                Self::evaluate_potential_size_container(e, originating_element_style)
            },
        ) {
            Some((container, result)) => {
                if result.is_complete() {
                    result
                } else {
                    // Traverse up from the found size container to see if we can get a complete containment.
                    result.merge(Self::lookup(container, None))
                }
            },
            None => ContainerSizeQueryResult::default(),
        }
    }

    /// Create a new instance of the container size query for given element, with a deferred lookup callback.
    pub fn for_element<E>(
        element: E,
        known_parent_style: Option<&'a ComputedValues>,
        is_pseudo: bool,
    ) -> Self
    where
        E: TElement + 'a,
    {
        let parent;
        let data;
        let parent_style = match known_parent_style {
            Some(s) => Some(s),
            None => {
                // No need to bother if we're the top element.
                parent = match element.traversal_parent() {
                    Some(parent) => parent,
                    None => return Self::none(),
                };
                data = parent.borrow_data();
                data.as_ref().map(|data| &**data.styles.primary())
            },
        };

        // If there's no style, such as being `display: none` or so, we still want to show a
        // correct computed value, so give it a try.
        let should_traverse = parent_style.map_or(true, |s| {
            s.flags
                .contains(ComputedValueFlags::SELF_OR_ANCESTOR_HAS_SIZE_CONTAINER_TYPE)
        });
        if !should_traverse {
            return Self::none();
        }
        return Self::NotEvaluated(Box::new(move || {
            Self::lookup(element, if is_pseudo { known_parent_style } else { None })
        }));
    }

    /// Create a new instance, but with optional element.
    pub fn for_option_element<E>(
        element: Option<E>,
        known_parent_style: Option<&'a ComputedValues>,
        is_pseudo: bool,
    ) -> Self
    where
        E: TElement + 'a,
    {
        if let Some(e) = element {
            Self::for_element(e, known_parent_style, is_pseudo)
        } else {
            Self::none()
        }
    }

    /// Create a query that evaluates to empty, for cases where container size query is not required.
    pub fn none() -> Self {
        ContainerSizeQuery::Evaluated(ContainerSizeQueryResult::default())
    }

    /// Get the result of the container size query, doing the lookup if called for the first time.
    pub fn get(&mut self) -> ContainerSizeQueryResult {
        match self {
            Self::NotEvaluated(lookup) => {
                *self = Self::Evaluated((lookup)());
                match self {
                    Self::Evaluated(info) => *info,
                    _ => unreachable!("Just evaluated but not set?"),
                }
            },
            Self::Evaluated(info) => *info,
        }
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use crate::test_support::{author_parser_context, pref_lock, test_url_data, BoolPrefGuard};
    use cssparser::ParserInput;

    fn parse(query: &str) -> ContainerConditions {
        let _lock = pref_lock().lock().unwrap();
        let _style_queries = BoolPrefGuard::set("layout.css.style-queries.enabled", true);
        let _scroll_state = BoolPrefGuard::set("layout.css.scroll-state.enabled", true);
        let url_data = test_url_data();
        let context = author_parser_context(&url_data);
        let mut input = ParserInput::new(query);
        Parser::new(&mut input)
            .parse_entirely(|input| ContainerConditions::parse(&context, input))
            .unwrap()
    }

    #[test]
    fn style_queries_serialize_nested_conditions_and_ranges() {
        for (query, expected) in [
            (
                "style(  ( --FOO: BAR) OR ( prop: val  ) )",
                "style((--FOO: BAR) or ( prop: val  ))",
            ),
            ("style(( (  (( (--foo )) )  )))", "style((((((--foo))))))"),
            ("style((not ((--foo: BAR))))", "style((not ((--foo: BAR))))"),
            (
                "style(100px  > --\\{foo >10px)",
                "style(100px > --\\{foo > 10px)",
            ),
            ("style(--foo: bar !important)", "style(--foo: bar)"),
            (
                "style(--foo < --bar <= --baz)",
                "style(--foo < --bar <= --baz)",
            ),
            (
                "style(10px <= calc(10em + 20em))",
                "style(10px <= calc(10em + 20em))",
            ),
        ] {
            assert_eq!(parse(query).to_css_string(), expected);
        }
    }

    #[test]
    fn invalid_style_features_are_general_enclosed() {
        for query in [
            "style(--foo: bar;)",
            "style(style(--foo: bar))",
            "style(10px < 10em > 10)",
            "style( < 10em)",
            "style(10px < 10em !)",
            "style(10px ! < 10em)",
            "(width > 0px) or (foo: bar)",
        ] {
            let conditions = parse(query);
            let condition = conditions.iter().next().unwrap();
            assert!(
                condition.flags.contains(FeatureFlags::GENERAL_ENCLOSED),
                "{query}"
            );
        }
    }

    #[test]
    fn scroll_state_queries_serialize_bare_features_and_conditions() {
        for (query, expected) in [
            (
                "scroll-state(        snapped:inline)",
                "scroll-state(snapped: inline)",
            ),
            ("scroll-STate(snapped:    )", "scroll-STate(snapped:    )"),
            ("scroll-STate(snapped)", "scroll-state(snapped)"),
            (
                "scroll-state(  ( snapped: INLINE) OR ( SNAPPED: BlOCK  ) )",
                "scroll-state((snapped: inline) or (snapped: block))",
            ),
            (
                "scroll-state((scrollable: left))",
                "scroll-state((scrollable: left))",
            ),
            (
                "scroll-state(not ((stuck: top) and (stuck: bottom)))",
                "scroll-state(not ((stuck: top) and (stuck: bottom)))",
            ),
        ] {
            assert_eq!(parse(query).to_css_string(), expected);
        }
    }

    #[test]
    fn scroll_state_features_require_a_scroll_state_container() {
        let conditions = parse("scroll-state(scrolled: inline-end)");
        let condition = conditions.iter().next().unwrap();
        assert!(condition
            .flags
            .contains(FeatureFlags::CONTAINER_REQUIRES_SCROLL_STATE));
        for query in [
            "scroll-state(stuck: auto)",
            "scroll-state(style(stuck: top))",
            "style(scroll-state(stuck: top))",
            "scroll-state(--foo)",
        ] {
            let conditions = parse(query);
            let condition = conditions.iter().next().unwrap();
            assert!(
                condition.flags.contains(FeatureFlags::GENERAL_ENCLOSED),
                "{query}"
            );
        }
    }

    #[test]
    fn size_features_keep_substitution_functions_for_the_query_container() {
        for (query, expected) in [
            ("(width > var(--x))", "(width > var(--x))"),
            (
                "(var(--a, 1px) < width <= calc(var(--b) * 2))",
                "(var(--a, 1px) < width <= calc(var(--b) * 2))",
            ),
        ] {
            let conditions = parse(query);
            let condition = conditions.iter().next().unwrap();
            assert!(
                !condition.flags.contains(FeatureFlags::GENERAL_ENCLOSED),
                "{query}"
            );
            assert_eq!(conditions.to_css_string(), expected);
        }
        let conditions = parse("(width > foo)");
        let condition = conditions.iter().next().unwrap();
        assert!(condition.flags.contains(FeatureFlags::GENERAL_ENCLOSED));
    }
}
