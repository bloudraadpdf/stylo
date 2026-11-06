/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS handling for the computed value of
//! [`position`][position] values.
//!
//! [position]: https://drafts.csswg.org/css-backgrounds-3/#position

use crate::derives::*;
use crate::logical_geometry::PhysicalSide;
use crate::values::computed::{
    Context, Integer, LengthPercentage, NonNegativeNumber, Percentage, ToComputedValue,
};
use crate::values::generics;
use crate::values::generics::position::TreeScoped;
use crate::values::generics::position::{
    AnchorSideKeyword, AspectRatio as GenericAspectRatio, GenericAnchorFunction, GenericAnchorSide,
    GenericInset, Position as GenericPosition, PositionComponent as GenericPositionComponent,
    PositionOrAuto as GenericPositionOrAuto, ZIndex as GenericZIndex,
};
use crate::values::specified::position::{AnchorNameIdent, AnchorScopeKeyword, PositionAnchorKeyword};
pub use crate::values::specified::position::{
    DashedIdentAndOrTryTactic, GridAutoFlow, GridLanesDirection,
    GridLanesDirectionAxis, GridTemplateAreas, MasonryAutoFlow, PositionArea,
    PositionAreaAxis, PositionAreaKeyword, PositionAreaType, PositionTryFallbacks,
    PositionTryFallbacksTryTactic, PositionTryFallbacksTryTacticKeyword, PositionTryOrder,
    PositionVisibility,
};
use crate::Zero;
use std::fmt::{self, Write};
use style_traits::values::specified::AllowedNumericType;
use style_traits::{CssWriter, ToCss};

/// The computed value of `anchor-name`.
pub type AnchorName = TreeScoped<AnchorNameIdent>;

/// The computed value of `anchor-scope`.
pub type AnchorScope = TreeScoped<AnchorScopeKeyword>;

/// The computed value of `position-anchor`.
pub type PositionAnchor = TreeScoped<PositionAnchorKeyword>;

/// The computed value of a CSS `<position>`
pub type Position = GenericPosition<HorizontalPosition, VerticalPosition>;

/// The computed value of an `auto | <position>`
pub type PositionOrAuto = GenericPositionOrAuto<Position>;

/// The computed value of a CSS horizontal position.
pub type HorizontalPosition = LengthPercentage;

/// The computed value of a CSS vertical position.
pub type VerticalPosition = LengthPercentage;

/// The computed value of anchor side.
pub type AnchorSide = GenericAnchorSide<Percentage>;

impl AnchorSide {
    /// Break down given anchor side into its equivalent keyword and percentage.
    pub fn keyword_and_percentage(&self) -> (AnchorSideKeyword, Percentage) {
        match self {
            Self::Percentage(p) => (AnchorSideKeyword::Start, *p),
            Self::Keyword(k) => {
                if matches!(k, AnchorSideKeyword::Center) {
                    (AnchorSideKeyword::Start, Percentage(0.5))
                } else {
                    (*k, Percentage::zero())
                }
            },
        }
    }
}

/// The computed value of an `anchor()` function.
pub type AnchorFunction = GenericAnchorFunction<Percentage, Inset>;

#[cfg(feature = "gecko")]
use crate::{
    gecko_bindings::structs::AnchorPosOffsetResolutionParams,
    values::{computed::Length, DashedIdent},
};

impl AnchorFunction {
    /// Resolve the anchor function with the given resolver. Returns `Err()` if no anchor is found.
    #[cfg(feature = "gecko")]
    pub fn resolve(
        anchor_name: &TreeScoped<DashedIdent>,
        anchor_side: &AnchorSide,
        prop_side: PhysicalSide,
        params: &AnchorPosOffsetResolutionParams,
    ) -> Result<Length, ()> {
        use crate::gecko_bindings::structs::Gecko_GetAnchorPosOffset;

        let (keyword, percentage) = anchor_side.keyword_and_percentage();
        let mut offset = Length::zero();
        let valid = unsafe {
            Gecko_GetAnchorPosOffset(
                params,
                anchor_name.value.0.as_ptr(),
                &anchor_name.scope,
                prop_side as u8,
                keyword as u8,
                percentage.0,
                &mut offset,
            )
        };

        if !valid {
            return Err(());
        }

        Ok(offset)
    }
}

/// Perform the adjustment of a given value for a given try tactic, as per:
/// https://drafts.csswg.org/css-anchor-position-1/#swap-due-to-a-try-tactic
pub(crate) trait TryTacticAdjustment {
    /// Performs the adjustments necessary given an old side we're relative to, and a new side
    /// we're relative to.
    fn try_tactic_adjustment(&mut self, old_side: PhysicalSide, new_side: PhysicalSide);
}

impl<T: TryTacticAdjustment> TryTacticAdjustment for Box<T> {
    fn try_tactic_adjustment(&mut self, old_side: PhysicalSide, new_side: PhysicalSide) {
        (**self).try_tactic_adjustment(old_side, new_side);
    }
}

impl<T: TryTacticAdjustment> TryTacticAdjustment for generics::NonNegative<T> {
    fn try_tactic_adjustment(&mut self, old_side: PhysicalSide, new_side: PhysicalSide) {
        self.0.try_tactic_adjustment(old_side, new_side);
    }
}

impl<Percentage: TryTacticAdjustment> TryTacticAdjustment for GenericAnchorSide<Percentage> {
    fn try_tactic_adjustment(&mut self, old_side: PhysicalSide, new_side: PhysicalSide) {
        match self {
            Self::Percentage(p) => p.try_tactic_adjustment(old_side, new_side),
            Self::Keyword(side) => side.try_tactic_adjustment(old_side, new_side),
        }
    }
}

impl<Percentage: TryTacticAdjustment, Fallback: TryTacticAdjustment> TryTacticAdjustment
    for GenericAnchorFunction<Percentage, Fallback>
{
    fn try_tactic_adjustment(&mut self, old_side: PhysicalSide, new_side: PhysicalSide) {
        self.side.try_tactic_adjustment(old_side, new_side);
        if let Some(fallback) = self.fallback.as_mut() {
            fallback.try_tactic_adjustment(old_side, new_side);
        }
    }
}

/// A computed type for `inset` properties.
pub type Inset = GenericInset<Percentage, LengthPercentage>;

impl crate::values::computed::length_percentage::ToAnchorCalcNode for Inset {
    fn to_anchor_calc_node(
        &self,
    ) -> Result<crate::values::computed::length_percentage::CalcNode, ()> {
        use crate::values::computed::length_percentage::{
            anchor_size_calc_node, fallback_calc_node, CalcLengthPercentageLeaf, CalcNode,
        };
        match self {
            Self::Auto => Err(()),
            Self::LengthPercentage(lp) | Self::AnchorContainingCalcFunction(lp) => Ok(lp.to_calc_node()),
            Self::AnchorFunction(function) => Ok(CalcNode::Anchor(Box::new(GenericAnchorFunction {
                target_element: function.target_element.clone(),
                side: match function.side {
                    GenericAnchorSide::Keyword(keyword) => GenericAnchorSide::Keyword(keyword),
                    GenericAnchorSide::Percentage(percentage) => GenericAnchorSide::Percentage(Box::new(
                        CalcNode::Leaf(CalcLengthPercentageLeaf::Percentage(percentage)),
                    )),
                },
                fallback: fallback_calc_node(function.fallback.as_ref())?,
            }))),
            Self::AnchorSizeFunction(function) => anchor_size_calc_node(&**function),
        }
    }
}

/// The property of an anchor function that layout resolves, with its physical side or axis (CSS Anchor Positioning 1
/// §3.2.1, §5.1.1, style & layout interleaving).
#[derive(Clone, Copy, Debug)]
pub enum AnchorProperty {
    /// An inset property.
    Inset(PhysicalSide),
    /// `width`, `height`, `min-width` or `min-height`.
    Size(crate::logical_geometry::PhysicalAxis),
    /// `max-width` or `max-height`.
    MaxSize(crate::logical_geometry::PhysicalAxis),
    /// A margin property.
    Margin(PhysicalSide),
}

/// What layout resolved an anchor function to.
#[derive(Clone, Copy, Debug)]
pub enum AnchorFunctionResolution {
    /// The length of a resolvable anchor function.
    Length(crate::values::computed::Length),
    /// The function is not resolvable: it takes its fallback, or makes its declaration invalid at computed-value time
    /// (§3.2.1, §5.1.1).
    Invalid,
}

/// Why the anchor functions of a value stay unsubstituted.
enum AnchorSubstitutionFailure {
    /// Layout has not resolved the element.
    Unresolved,
    /// An anchor function without a fallback is not resolvable.
    Invalid,
}

/// Replaces each anchor function of `node` by the length that `resolve` gives, or else by its fallback.
fn substitute_anchor_functions(
    node: &mut crate::values::computed::length_percentage::CalcNode,
    resolve: &mut dyn FnMut(
        &crate::values::computed::length_percentage::CalcNode,
    ) -> Option<AnchorFunctionResolution>,
) -> Result<(), AnchorSubstitutionFailure> {
    use crate::values::computed::length_percentage::{CalcLengthPercentageLeaf, CalcNode};
    let mut outcome = Ok(());
    node.visit_depth_first(|node| {
        if outcome.is_err() {
            return;
        }
        let fallback = match node {
            CalcNode::Anchor(function) => function.fallback.as_ref().map(|fallback| (**fallback).clone()),
            CalcNode::AnchorSize(function) => function.fallback.as_ref().map(|fallback| (**fallback).clone()),
            _ => return,
        };
        outcome = match resolve(node) {
            None => Err(AnchorSubstitutionFailure::Unresolved),
            Some(AnchorFunctionResolution::Length(length)) => {
                *node = CalcNode::Leaf(CalcLengthPercentageLeaf::Length(length));
                Ok(())
            },
            Some(AnchorFunctionResolution::Invalid) => match fallback {
                Some(mut fallback) => substitute_anchor_functions(&mut fallback, &mut *resolve)
                    .map(|()| *node = fallback),
                None => Err(AnchorSubstitutionFailure::Invalid),
            },
        };
    });
    outcome
}

/// The value with its anchor functions substituted: `Ok(None)` when it is invalid at computed-value time, `Err` when
/// layout has not resolved the element.
fn resolve_anchor_functions<T: crate::values::computed::length_percentage::ToAnchorCalcNode>(
    value: &T,
    property: AnchorProperty,
    context: &Context,
    clamping_mode: style_traits::values::specified::AllowedNumericType,
) -> Result<Option<LengthPercentage>, ()> {
    let mut node = value.to_anchor_calc_node()?;
    match substitute_anchor_functions(&mut node, &mut |function| {
        context.resolve_anchor_function(property, function)
    }) {
        Ok(()) => Ok(Some(LengthPercentage::new_calc(node, clamping_mode))),
        Err(AnchorSubstitutionFailure::Invalid) => Ok(None),
        Err(AnchorSubstitutionFailure::Unresolved) => Err(()),
    }
}

/// A computed value that style & layout interleaving replaced, as the cascade computed it.
#[derive(Clone, Debug)]
pub enum AnchorBase {
    /// An inset property.
    Inset(Inset),
    /// `width`, `height`, `min-width` or `min-height`.
    Size(crate::values::computed::Size),
    /// `max-width` or `max-height`.
    MaxSize(crate::values::computed::MaxSize),
    /// A margin property.
    Margin(crate::values::computed::Margin),
}

/// A replaced computed value with its property.
#[derive(Clone, Debug)]
pub struct AnchorBaseValue {
    /// The physical longhand.
    pub property: crate::properties::LonghandId,
    /// The value before the replacement.
    pub value: AnchorBase,
}

/// What style & layout interleaving replaced in a style: the base that layout positions from.
#[derive(Clone, Debug, Default)]
pub struct AnchorLayoutBase {
    /// The values whose anchor functions layout resolved, as the cascade computed them.
    pub values: Vec<AnchorBaseValue>,
    /// The style before the position option applied.
    pub option_base: Option<servo_arc::Arc<crate::properties::ComputedValues>>,
}

impl AnchorLayoutBase {
    /// The value that the cascade computed for `property` before interleaving replaced it.
    pub fn value(&self, property: crate::properties::LonghandId) -> Option<&AnchorBase> {
        self.values
            .iter()
            .find(|value| value.property == property)
            .map(|value| &value.value)
    }

    pub(crate) fn into_present(self) -> Option<Self> {
        (!self.values.is_empty() || self.option_base.is_some()).then_some(self)
    }
}

/// A computed value that may hold anchor functions, which layout resolves through the element.
pub trait ResolveAnchors: Sized {
    /// The value with its anchor functions replaced by the length that layout resolved for them in `property`.
    /// The cascade keeps the replaced value as the base for layout.
    fn resolve_anchors(self, property: crate::properties::LonghandId, context: &mut Context) -> Self;
}

fn physical_side(property: crate::properties::LonghandId) -> Option<PhysicalSide> {
    use crate::properties::LonghandId;
    Some(match property {
        LonghandId::Top | LonghandId::MarginTop => PhysicalSide::Top,
        LonghandId::Right | LonghandId::MarginRight => PhysicalSide::Right,
        LonghandId::Bottom | LonghandId::MarginBottom => PhysicalSide::Bottom,
        LonghandId::Left | LonghandId::MarginLeft => PhysicalSide::Left,
        _ => return None,
    })
}

fn physical_axis(property: crate::properties::LonghandId) -> Option<crate::logical_geometry::PhysicalAxis> {
    use crate::logical_geometry::PhysicalAxis;
    use crate::properties::LonghandId;
    Some(match property {
        LonghandId::Width | LonghandId::MinWidth | LonghandId::MaxWidth => PhysicalAxis::Horizontal,
        LonghandId::Height | LonghandId::MinHeight | LonghandId::MaxHeight => PhysicalAxis::Vertical,
        _ => return None,
    })
}

/// Records `base` as the value that interleaving replaced by `resolved` in `property`.
fn replaced<T>(
    context: &mut Context,
    property: crate::properties::LonghandId,
    base: AnchorBase,
    resolved: T,
) -> T {
    context
        .builder
        .anchor_layout_base
        .values
        .push(AnchorBaseValue { property, value: base });
    resolved
}

impl ResolveAnchors for Inset {
    fn resolve_anchors(self, property: crate::properties::LonghandId, context: &mut Context) -> Self {
        if matches!(self, Self::Auto | Self::LengthPercentage(_)) {
            return self;
        }
        let Some(side) = physical_side(property) else {
            return self;
        };
        match resolve_anchor_functions(&self, AnchorProperty::Inset(side), context, AllowedNumericType::All) {
            Ok(resolved) => replaced(
                context,
                property,
                AnchorBase::Inset(self),
                resolved.map_or(Self::Auto, Self::LengthPercentage),
            ),
            Err(()) => self,
        }
    }
}

impl ResolveAnchors for crate::values::computed::Size {
    fn resolve_anchors(self, property: crate::properties::LonghandId, context: &mut Context) -> Self {
        if !matches!(self, Self::AnchorSizeFunction(_) | Self::AnchorContainingCalcFunction(_)) {
            return self;
        }
        let Some(axis) = physical_axis(property) else {
            return self;
        };
        match resolve_anchor_functions(&self, AnchorProperty::Size(axis), context, AllowedNumericType::NonNegative) {
            Ok(resolved) => replaced(
                context,
                property,
                AnchorBase::Size(self),
                resolved.map_or(Self::Auto, |length| Self::LengthPercentage(generics::NonNegative(length))),
            ),
            Err(()) => self,
        }
    }
}

impl ResolveAnchors for crate::values::computed::MaxSize {
    fn resolve_anchors(self, property: crate::properties::LonghandId, context: &mut Context) -> Self {
        if !matches!(self, Self::AnchorSizeFunction(_) | Self::AnchorContainingCalcFunction(_)) {
            return self;
        }
        let Some(axis) = physical_axis(property) else {
            return self;
        };
        match resolve_anchor_functions(&self, AnchorProperty::MaxSize(axis), context, AllowedNumericType::NonNegative)
        {
            Ok(resolved) => replaced(
                context,
                property,
                AnchorBase::MaxSize(self),
                resolved.map_or(Self::None, |length| Self::LengthPercentage(generics::NonNegative(length))),
            ),
            Err(()) => self,
        }
    }
}

impl ResolveAnchors for crate::values::computed::Margin {
    fn resolve_anchors(self, property: crate::properties::LonghandId, context: &mut Context) -> Self {
        if !matches!(self, Self::AnchorSizeFunction(_) | Self::AnchorContainingCalcFunction(_)) {
            return self;
        }
        let Some(side) = physical_side(property) else {
            return self;
        };
        match resolve_anchor_functions(&self, AnchorProperty::Margin(side), context, AllowedNumericType::All) {
            Ok(resolved) => replaced(
                context,
                property,
                AnchorBase::Margin(self),
                Self::LengthPercentage(resolved.unwrap_or_else(LengthPercentage::zero)),
            ),
            Err(()) => self,
        }
    }
}

impl crate::values::animated::Animate for Inset {
    fn animate(
        &self,
        other: &Self,
        procedure: crate::values::animated::Procedure,
    ) -> Result<Self, ()> {
        Ok(match (self, other) {
            (Self::Auto, Self::Auto) => Self::Auto,
            (Self::LengthPercentage(one), Self::LengthPercentage(other)) => {
                Self::LengthPercentage(one.animate(other, procedure)?)
            },
            _ => Self::AnchorContainingCalcFunction(LengthPercentage::mix_anchor_values(
                self,
                other,
                procedure,
                style_traits::values::specified::AllowedNumericType::All,
            )?),
        })
    }
}
impl TryTacticAdjustment for Inset {
    // https://drafts.csswg.org/css-anchor-position-1/#swap-due-to-a-try-tactic:
    //
    //     For inset properties, change the specified side in anchor() functions to maintain the
    //     same relative relationship to the new direction that they had to the old.
    //
    //     If a <percentage> is used, and directions are opposing, change it to 100% minus the
    //     original percentage.
    fn try_tactic_adjustment(&mut self, old_side: PhysicalSide, new_side: PhysicalSide) {
        match self {
            Self::Auto => {},
            Self::AnchorContainingCalcFunction(lp) | Self::LengthPercentage(lp) => {
                lp.try_tactic_adjustment(old_side, new_side)
            },
            Self::AnchorFunction(anchor) => anchor.try_tactic_adjustment(old_side, new_side),
            Self::AnchorSizeFunction(anchor) => anchor.try_tactic_adjustment(old_side, new_side),
        }
    }
}

impl Position {
    /// `50% 50%`
    #[inline]
    pub fn center() -> Self {
        Self::new(
            LengthPercentage::new_percent(Percentage(0.5)),
            LengthPercentage::new_percent(Percentage(0.5)),
        )
    }

    /// `0% 0%`
    #[inline]
    pub fn zero() -> Self {
        Self::new(LengthPercentage::zero(), LengthPercentage::zero())
    }
}

impl ToCss for Position {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        self.horizontal.to_css(dest)?;
        dest.write_char(' ')?;
        self.vertical.to_css(dest)
    }
}

impl GenericPositionComponent for LengthPercentage {
    fn is_center(&self) -> bool {
        match self.to_percentage() {
            Some(Percentage(per)) => per == 0.5,
            _ => false,
        }
    }
}

#[inline]
fn block_or_inline_to_inferred(keyword: PositionAreaKeyword) -> PositionAreaKeyword {
    if matches!(
        keyword.axis(),
        PositionAreaAxis::Block | PositionAreaAxis::Inline
    ) {
        keyword.with_axis(PositionAreaAxis::Inferred)
    } else {
        keyword
    }
}

#[inline]
fn inferred_to_block(keyword: PositionAreaKeyword) -> PositionAreaKeyword {
    keyword.with_inferred_axis(PositionAreaAxis::Block)
}

#[inline]
fn inferred_to_inline(keyword: PositionAreaKeyword) -> PositionAreaKeyword {
    keyword.with_inferred_axis(PositionAreaAxis::Inline)
}

// This exists because the spec currently says that further simplifications
// should be made to the computed value. That's confusing though, and probably
// all these simplifications should be wrapped up into the simplifications that
// we make to the specified value. I.e. all this should happen in
// PositionArea::parse_internal().
// See also https://github.com/w3c/csswg-drafts/issues/12759
impl ToComputedValue for PositionArea {
    type ComputedValue = Self;

    fn to_computed_value(&self, _context: &Context) -> Self {
        let mut computed = self.clone();
        let pair_type = self.get_type();
        if pair_type == PositionAreaType::Logical || pair_type == PositionAreaType::SelfLogical {
            if computed.second != PositionAreaKeyword::None {
                computed.first = block_or_inline_to_inferred(computed.first);
                computed.second = block_or_inline_to_inferred(computed.second);
            }
        } else if pair_type == PositionAreaType::Inferred
            || pair_type == PositionAreaType::SelfInferred
        {
            if computed.second == PositionAreaKeyword::SpanAll {
                // We remove the superfluous span-all, converting the inferred
                // keyword to a logical keyword to avoid ambiguity, per spec.
                computed.first = inferred_to_block(computed.first);
                computed.second = PositionAreaKeyword::None;
            } else if computed.first == PositionAreaKeyword::SpanAll {
                computed.first = computed.second;
                computed.first = inferred_to_inline(computed.first);
                computed.second = PositionAreaKeyword::None;
            }
        }

        if computed.first == computed.second {
            computed.second = PositionAreaKeyword::None;
        }
        computed
    }

    fn from_computed_value(computed: &Self) -> Self {
        computed.clone()
    }
}

/// A computed value for the `z-index` property.
pub type ZIndex = GenericZIndex<Integer>;

/// A computed value for the `aspect-ratio` property.
pub type AspectRatio = GenericAspectRatio<NonNegativeNumber>;

/// Computed value of `masonry-slack` (CSS Grid 3 §3.8).
///
/// Mirrors the specified-side enum but carries a computed
/// `NonNegativeLengthPercentage` for the explicit-length case. See
/// `crate::values::specified::position::MasonrySlack` for the
/// authoring-side grammar and rationale.
///
/// `ToShmem` is intentionally omitted — computed `LengthPercentage`
/// is a tagged union without a `ToShmem` impl and the computed value
/// never enters shareable-memory storage (only the specified value
/// does, and that lives on the specified-side `MasonrySlack`).
#[derive(
    Clone,
    ComputeSquaredDistance,
    Debug,
    MallocSizeOf,
    PartialEq,
    ToAnimatedZero,
    ToCss,
    ToResolvedValue,
    ToTyped,
)]
#[repr(C, u8)]
pub enum MasonrySlack {
    /// `infinite` — no slack constraint; initial value.
    Infinite,
    /// `auto` — UA-defined slack heuristic.
    Auto,
    /// `<length-percentage>` — explicit slack budget.
    LengthPercentage(crate::values::computed::length::NonNegativeLengthPercentage),
}

impl crate::values::animated::Animate for MasonrySlack {
    fn animate(
        &self,
        other: &Self,
        procedure: crate::values::animated::Procedure,
    ) -> Result<Self, ()> {
        match (self, other) {
            (Self::LengthPercentage(from), Self::LengthPercentage(to)) => {
                Ok(Self::LengthPercentage(
                    crate::values::generics::NonNegative(from.0.animate(&to.0, procedure)?),
                ))
            },
            (Self::Infinite, Self::Infinite) => Ok(Self::Infinite),
            (Self::Auto, Self::Auto) => Ok(Self::Auto),
            _ => Err(()),
        }
    }
}

impl crate::values::animated::ToAnimatedValue for MasonrySlack {
    type AnimatedValue = Self;

    fn to_animated_value(self, _: &crate::values::animated::Context) -> Self {
        self
    }

    fn from_animated_value(animated: Self) -> Self {
        match animated {
            Self::LengthPercentage(value) => Self::LengthPercentage(
                crate::values::computed::length::NonNegativeLengthPercentage::from_animated_value(
                    value,
                ),
            ),
            keyword => keyword,
        }
    }
}

impl MasonrySlack {
    /// Initial value (`infinite`).
    #[inline]
    pub fn initial() -> Self {
        Self::Infinite
    }
}

#[cfg(test)]
mod masonry_slack_tests {
    use super::MasonrySlack;
    use crate::values::animated::{Animate, Procedure};
    use crate::values::computed::{Length, LengthPercentage, Percentage};
    use crate::values::generics::NonNegative;

    #[test]
    fn masonry_slack_interpolates_numeric_values_but_not_keywords() {
        let length = |value| {
            MasonrySlack::LengthPercentage(NonNegative(LengthPercentage::new_length(Length::new(
                value,
            ))))
        };
        let percentage = |value| {
            MasonrySlack::LengthPercentage(NonNegative(LengthPercentage::new_percent(Percentage(
                value,
            ))))
        };
        let sample = Procedure::Interpolate { progress: 0.25 };
        assert_eq!(
            length(10.0).animate(&length(50.0), sample),
            Ok(length(20.0))
        );
        assert_eq!(
            percentage(0.1).animate(&percentage(0.5), sample),
            Ok(percentage(0.2))
        );
        assert!(length(10.0)
            .animate(&MasonrySlack::Infinite, sample)
            .is_err());
        assert!(MasonrySlack::Auto
            .animate(&MasonrySlack::Infinite, sample)
            .is_err());
    }

    #[test]
    fn masonry_slack_animation_property_recognises_fit_tolerance() {
        use crate::properties::PropertyId;
        use crate::test_support::{pref_lock, BoolPrefGuard};
        let _lock = pref_lock().lock().unwrap();
        let _pref = BoolPrefGuard::set("layout.css.grid-template-masonry-value.enabled", true);
        let alias = PropertyId::parse_enabled_for_all_content("fit-tolerance").unwrap();
        let backing = PropertyId::parse_enabled_for_all_content("masonry-slack").unwrap();
        assert_eq!(alias.as_shorthand(), backing.as_shorthand());
        assert!(alias.is_animatable());
    }

    #[test]
    fn masonry_slack_clamps_extrapolation_and_keeps_mixed_percentage_endpoints() {
        use crate::values::animated::ToAnimatedValue;
        use style_traits::ToCss;
        let length = |value| {
            MasonrySlack::LengthPercentage(NonNegative(LengthPercentage::new_length(Length::new(
                value,
            ))))
        };
        let sampled = length(10.0)
            .animate(&length(50.0), Procedure::Interpolate { progress: -1.0 })
            .unwrap();
        assert_eq!(MasonrySlack::from_animated_value(sampled), length(0.0));
        let percentage = MasonrySlack::LengthPercentage(NonNegative(
            LengthPercentage::new_percent(Percentage(0.5)),
        ));
        let sampled = length(10.0)
            .animate(&percentage, Procedure::Interpolate { progress: 0.0 })
            .unwrap();
        assert_eq!(sampled.to_css_string(), "calc(0% + 10px)");
    }
}
