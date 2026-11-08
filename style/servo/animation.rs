/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS transitions and animations.

// NOTE(emilio): This code isn't really executed in Gecko, but we don't want to
// compile it out so that people remember it exists.

use crate::context::{CascadeInputs, SharedStyleContext};
use crate::derives::*;
use crate::dom::{OpaqueNode, TDocument, TElement, TNode};
use crate::properties::animated_properties::{AnimationValue, AnimationValueMap};
use crate::properties::longhands::animation_composition::computed_value::single_value::T as AnimationComposition;
use crate::properties::longhands::animation_direction::computed_value::single_value::T as AnimationDirection;
use crate::properties::longhands::animation_fill_mode::computed_value::single_value::T as AnimationFillMode;
use crate::properties::longhands::animation_play_state::computed_value::single_value::T as AnimationPlayState;
use crate::properties::longhands::interpolate_size::computed_value::T as InterpolateSize;
use crate::properties::AnimationDeclarations;
use crate::properties::{
    ComputedValues, Importance, LonghandId, PropertyDeclarationBlock, PropertyDeclarationId,
    PropertyDeclarationIdSet,
};
use crate::rule_tree::CascadeLevel;
use crate::selector_parser::PseudoElement;
use crate::shared_lock::{Locked, SharedRwLock};
use crate::style_resolver::{PrimaryStyle, StyleResolverForElement};
use crate::stylesheets::keyframes_rule::{KeyframesAnimation, KeyframesStep, KeyframesStepValue};
use crate::stylesheets::layer_rule::LayerOrder;
use crate::values::animated::Procedure;
#[cfg(feature = "servo")]
use crate::values::animated::{
    box_shadow::{AnimatedBoxShadowList, COMPONENTS as SHADOW_COMPONENTS},
    Animate,
};
use crate::values::computed::TimingFunction;
use crate::values::generics::easing::BeforeFlag;
use crate::values::specified::TransitionBehavior;
use crate::Atom;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use servo_arc::Arc;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

/// The inheritance target used while resolving CSS animation keyframes.
///
/// Generated pseudos inherit from their originating element's primary style,
/// so representing them as an ordinary element cascade loses the correct
/// parent for relative and inherited keyframe values.
#[derive(Clone, Copy)]
pub enum AnimationCascadeTarget<'a> {
    /// Resolve keyframes for an ordinary element.
    Element,
    /// Resolve keyframes for a generated pseudo inheriting from its
    /// originating element.
    Pseudo {
        /// The generated pseudo whose style is being animated.
        pseudo: &'a PseudoElement,
        /// The originating element's primary style.
        primary_style: &'a PrimaryStyle,
    },
}

impl<'a> AnimationCascadeTarget<'a> {
    pub(crate) const fn pseudo(self) -> Option<&'a PseudoElement> {
        match self {
            Self::Element => None,
            Self::Pseudo { pseudo, .. } => Some(pseudo),
        }
    }
}

/// Represents an animation for a given property.
#[derive(Clone, Debug, MallocSizeOf)]
pub struct PropertyAnimation {
    /// The value we are animating from.
    from: AnimationValue,

    /// The value we are animating to.
    to: AnimationValue,

    /// The timing function of this `PropertyAnimation`.
    timing_function: TimingFunction,

    /// The duration of this `PropertyAnimation` in seconds.
    pub duration: f64,

    /// The `interpolate-size` that applies to this animation.
    interpolate_size: InterpolateSize,
    #[cfg(feature = "servo")]
    shadow: Option<(AnimatedBoxShadowList, AnimatedBoxShadowList)>,
}

impl PropertyAnimation {
    /// Returns the given property longhand id.
    pub fn property_id(&self) -> PropertyDeclarationId<'_> {
        debug_assert_eq!(self.from.id(), self.to.id());
        self.from.id()
    }

    fn animate(
        &self,
        from: &AnimationValue,
        to: &AnimationValue,
        procedure: Procedure,
    ) -> Result<AnimationValue, ()> {
        #[cfg(feature = "servo")]
        if let Some((ref left, ref right)) = self.shadow {
            let sample = left.animate(right, procedure)?;
            return sample
                .components()
                .into_iter()
                .find(|value| value.id() == from.id())
                .ok_or(());
        }
        from.animate_with_interpolate_size(to, procedure, self.interpolate_size)
    }

    /// The output of the timing function given the progress ration of this animation.
    fn timing_function_output(&self, progress: f64, before_flag: BeforeFlag) -> f64 {
        let epsilon = 1. / (200. * self.duration);
        self.timing_function
            .calculate_output(progress, before_flag, epsilon)
    }

    /// Update the given animation at a given point of progress.
    fn calculate_value(&self, progress: f64, before_flag: BeforeFlag) -> AnimationValue {
        let progress = self.timing_function_output(progress, before_flag);
        let procedure = Procedure::Interpolate { progress };
        self.animate(&self.from, &self.to, procedure)
            .unwrap_or_else(|()| {
                // Fall back to discrete interpolation
                if progress < 0.5 {
                    self.from.clone()
                } else {
                    self.to.clone()
                }
            })
    }
}

/// This structure represents the state of an animation.
#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub enum AnimationState {
    /// The animation has been created, but is not running yet. This state
    /// is also used when an animation is still in the first delay phase.
    Pending,
    /// This animation is currently running.
    Running,
    /// This animation is paused. The inner field is the percentage of progress
    /// when it was paused, from 0 to 1.
    Paused(f64),
    /// This animation has finished.
    Finished,
    /// This animation has been canceled.
    Canceled,
}

impl AnimationState {
    /// Whether or not this state requires its owning animation to be ticked.
    fn needs_to_be_ticked(&self) -> bool {
        *self == AnimationState::Running || *self == AnimationState::Pending
    }
}

enum IgnoreTransitions {
    Canceled,
    CanceledAndFinished,
}

/// This structure represents a keyframes animation current iteration state.
///
/// If the iteration count is infinite, there's no other state, otherwise we
/// have to keep track the current iteration and the max iteration count.
#[derive(Clone, Debug, MallocSizeOf)]
pub enum KeyframesIterationState {
    /// Infinite iterations with the current iteration count.
    Infinite(f64),
    /// Current and max iterations.
    Finite(f64, f64),
}

/// A temporary data structure used when calculating ComputedKeyframes for an
/// animation. This data structure is used to collapse information for steps
/// which may be spread across multiple keyframe declarations into a single
/// instance per `start_percentage`.
struct IntermediateComputedKeyframe {
    declarations: PropertyDeclarationBlock,
    timing_function: Option<TimingFunction>,
    composition: Option<AnimationComposition>,
    start_percentage: f32,
}

impl IntermediateComputedKeyframe {
    fn new(start_percentage: f32) -> Self {
        IntermediateComputedKeyframe {
            declarations: PropertyDeclarationBlock::new(),
            timing_function: None,
            composition: None,
            start_percentage,
        }
    }

    /// Walk through all keyframe declarations and combine all declarations with the
    /// same `start_percentage` into individual `IntermediateComputedKeyframe`s.
    fn generate_for_keyframes(
        animation: &KeyframesAnimation,
        context: &SharedStyleContext,
        base_style: &ComputedValues,
    ) -> Vec<Self> {
        let mut intermediate_steps: Vec<Self> = Vec::with_capacity(animation.steps.len());
        let mut current_step = IntermediateComputedKeyframe::new(0.);
        for step in animation.steps.iter() {
            let start_percentage = step.start_percentage.0;
            if start_percentage != current_step.start_percentage {
                let new_step = IntermediateComputedKeyframe::new(start_percentage);
                intermediate_steps.push(std::mem::replace(&mut current_step, new_step));
            }

            current_step.update_from_step(step, context, base_style);
        }
        intermediate_steps.push(current_step);

        // We should always have a first and a last step, even if these are just
        // generated by KeyframesStepValue::ComputedValues.
        debug_assert!(intermediate_steps.first().unwrap().start_percentage == 0.);
        debug_assert!(intermediate_steps.last().unwrap().start_percentage == 1.);

        intermediate_steps
    }

    fn update_from_step(
        &mut self,
        step: &KeyframesStep,
        context: &SharedStyleContext,
        base_style: &ComputedValues,
    ) {
        // Each keyframe declaration may optionally specify a timing function, falling
        // back to the one defined global for the animation.
        let guard = &context.guards.author;
        if let Some(timing_function) = step.get_animation_timing_function(&guard) {
            self.timing_function = Some(timing_function.to_computed_value_without_context());
        }
        if let Some(composition) = step.get_animation_composition(&guard) {
            self.composition = Some(composition);
        }

        let block = match step.value {
            KeyframesStepValue::ComputedValues => return,
            KeyframesStepValue::Declarations { ref block } => block,
        };

        // Filter out !important, non-animatable properties, and the
        // 'display' property (which is only animatable from SMIL).
        let guard = block.read_with(&guard);
        for declaration in guard.normal_declaration_iter() {
            if let PropertyDeclarationId::Longhand(id) = declaration.id() {
                if id == LonghandId::Display {
                    continue;
                }

                if !id.is_animatable() {
                    continue;
                }
            }

            self.declarations.push(
                declaration.to_physical(base_style.writing_mode),
                Importance::Normal,
            );
        }
    }

    fn resolve_style<E>(
        self,
        element: E,
        context: &SharedStyleContext,
        base_style: &Arc<ComputedValues>,
        cascade_target: AnimationCascadeTarget,
        resolver: &mut StyleResolverForElement<E>,
    ) -> Arc<ComputedValues>
    where
        E: TElement,
    {
        if !self.declarations.any_normal() {
            return base_style.clone();
        }

        let document = element.as_node().owner_doc();
        let locked_block = Arc::new(document.shared_lock().wrap(self.declarations));
        let mut important_rules_changed = false;
        let rule_node = base_style.rules().clone();
        let new_node = context.stylist.rule_tree().update_rule_at_level(
            CascadeLevel::Animations,
            LayerOrder::root(),
            Some(locked_block.borrow_arc()),
            &rule_node,
            &context.guards,
            &mut important_rules_changed,
        );

        if new_node.is_none() {
            return base_style.clone();
        }

        let inputs = CascadeInputs {
            rules: new_node,
            visited_rules: base_style.visited_rules().cloned(),
            flags: base_style.flags.for_cascade_inputs(),
        };
        match cascade_target {
            AnimationCascadeTarget::Element => {
                resolver
                    .cascade_style_and_visited_with_default_parents(inputs)
                    .0
            },
            AnimationCascadeTarget::Pseudo {
                pseudo,
                primary_style,
            } => {
                resolver
                    .cascade_style_and_visited_for_pseudo_with_default_parents(
                        inputs,
                        pseudo,
                        primary_style,
                    )
                    .0
            },
        }
    }
}

/// A single computed keyframe for a CSS Animation.
#[derive(Clone, MallocSizeOf)]
struct ComputedKeyframe {
    /// The timing function to use for transitions between this step
    /// and the next one.
    timing_function: TimingFunction,

    composition: AnimationComposition,

    /// The starting percentage (a number between 0 and 1) which represents
    /// at what point in an animation iteration this step is.
    start_percentage: f32,

    /// The animation values to transition to and from when processing this
    /// keyframe animation step.
    values: Box<[AnimationValue]>,
}

impl ComputedKeyframe {
    fn composite(
        composition: AnimationComposition,
        underlying: &AnimationValue,
        value: &AnimationValue,
        interpolate_size: InterpolateSize,
    ) -> AnimationValue {
        let procedure = match composition {
            AnimationComposition::Replace => return value.clone(),
            AnimationComposition::Add => Procedure::Add,
            AnimationComposition::Accumulate => Procedure::Accumulate { count: 1 },
        };
        underlying
            .animate_with_interpolate_size(value, procedure, interpolate_size)
            .unwrap_or_else(|()| value.clone())
    }

    fn generate_for_keyframes<E>(
        element: E,
        animation: &KeyframesAnimation,
        context: &SharedStyleContext,
        base_style: &Arc<ComputedValues>,
        default_timing_function: TimingFunction,
        default_composition: AnimationComposition,
        cascade_target: AnimationCascadeTarget,
        resolver: &mut StyleResolverForElement<E>,
    ) -> Box<[Self]>
    where
        E: TElement,
    {
        let mut animating_properties = PropertyDeclarationIdSet::default();
        for property in animation.properties_changed.iter() {
            debug_assert!(property.is_animatable());
            animating_properties.insert(property.to_physical(base_style.writing_mode));
        }

        let animation_values_from_style: Vec<AnimationValue> = animating_properties
            .iter()
            .map(|property| {
                AnimationValue::from_computed_values(property, &**base_style)
                    .expect("Unexpected non-animatable property.")
            })
            .collect();

        let intermediate_steps =
            IntermediateComputedKeyframe::generate_for_keyframes(animation, context, base_style);

        let mut computed_steps: Vec<Self> = Vec::with_capacity(intermediate_steps.len());
        for (step_index, step) in intermediate_steps.into_iter().enumerate() {
            let start_percentage = step.start_percentage;
            let properties_changed_in_step = step.declarations.property_ids().clone();
            let step_timing_function = step.timing_function.clone();
            let composition = step.composition.unwrap_or(default_composition);
            let step_style =
                step.resolve_style(element, context, base_style, cascade_target, resolver);
            let timing_function =
                step_timing_function.unwrap_or_else(|| default_timing_function.clone());

            let values = {
                // If a value is not set in a property declaration we use the value from
                // the style for the first and last keyframe. For intermediate ones, we
                // use the value from the previous keyframe.
                //
                // TODO(mrobinson): According to the spec, we should use an interpolated
                // value for properties missing from keyframe declarations.
                let default_values = if start_percentage == 0. || start_percentage == 1.0 {
                    animation_values_from_style.as_slice()
                } else {
                    debug_assert!(step_index != 0);
                    &computed_steps[step_index - 1].values
                };

                // For each property that is animating, pull the value from the resolved
                // style for this step if it's in one of the declarations. Otherwise, we
                // use the default value from the set we calculated above.
                animating_properties
                    .iter()
                    .zip(default_values.iter())
                    .map(|(property_declaration, default_value)| {
                        if properties_changed_in_step.contains(property_declaration) {
                            AnimationValue::from_computed_values(property_declaration, &step_style)
                                .unwrap_or_else(|| default_value.clone())
                        } else {
                            default_value.clone()
                        }
                    })
                    .collect()
            };

            computed_steps.push(ComputedKeyframe {
                timing_function,
                composition,
                start_percentage,
                values,
            });
        }
        computed_steps.into_boxed_slice()
    }
}

/// A CSS Animation
#[derive(Clone, MallocSizeOf)]
pub struct Animation {
    /// The name of this animation as defined by the style.
    pub name: Atom,

    /// The properties that change in this animation.
    properties_changed: PropertyDeclarationIdSet,

    /// The computed style for each keyframe of this animation.
    computed_steps: Box<[ComputedKeyframe]>,

    /// The time this animation started at, which is the current value of the animation
    /// timeline when this animation was created plus any animation delay.
    pub started_at: f64,

    /// The duration of this animation.
    pub duration: f64,

    /// The delay of the animation.
    pub delay: f64,

    /// The `animation-fill-mode` property of this animation.
    pub fill_mode: AnimationFillMode,

    /// The current iteration state for the animation.
    pub iteration_state: KeyframesIterationState,

    /// Whether this animation is paused.
    pub state: AnimationState,

    /// The declared animation direction of this animation.
    pub direction: AnimationDirection,

    /// The current animation direction. This can only be `normal` or `reverse`.
    pub current_direction: AnimationDirection,

    /// The original cascade style, needed to compute the generated keyframes of
    /// the animation.
    #[ignore_malloc_size_of = "ComputedValues"]
    pub cascade_style: Arc<ComputedValues>,

    /// Whether or not this animation is new and or has already been tracked
    /// by the script thread.
    pub is_new: bool,
}

impl Animation {
    fn composited_keyframe_value(
        &self,
        keyframe: &ComputedKeyframe,
        value: &AnimationValue,
        map: &AnimationValueMap,
    ) -> AnimationValue {
        let property = value.id();
        let underlying = map
            .get(&property.to_owned())
            .cloned()
            .or_else(|| AnimationValue::from_computed_values(property, &self.cascade_style))
            .expect("an animated property must have an underlying value");
        ComputedKeyframe::composite(
            keyframe.composition,
            &underlying,
            value,
            self.interpolate_size(),
        )
    }

    #[cfg(feature = "servo")]
    fn composited_shadow(
        &self,
        keyframe: &ComputedKeyframe,
        map: &AnimationValueMap,
    ) -> Option<AnimatedBoxShadowList> {
        if !SHADOW_COMPONENTS.iter().all(|id| {
            keyframe
                .values
                .iter()
                .any(|value| value.id() == PropertyDeclarationId::Longhand(*id))
        }) {
            return None;
        }
        let keyframe_values = keyframe
            .values
            .iter()
            .map(|value| (value.id().to_owned(), value.clone()))
            .collect();
        let value = AnimatedBoxShadowList::from_computed_values(
            &self.cascade_style,
            Some(&keyframe_values),
        );
        let underlying =
            AnimatedBoxShadowList::from_computed_values(&self.cascade_style, Some(map));
        match keyframe.composition {
            AnimationComposition::Replace => Some(value),
            AnimationComposition::Add => {
                Some(underlying.animate(&value, Procedure::Add).unwrap_or(value))
            },
            AnimationComposition::Accumulate => Some(
                underlying
                    .animate(&value, Procedure::Accumulate { count: 1 })
                    .unwrap_or(value),
            ),
        }
    }

    /// CSS Values 5 §11.3: an animation uses the `interpolate-size` of its non-animated style.
    fn interpolate_size(&self) -> InterpolateSize {
        self.cascade_style
            .get_inherited_box()
            .clone_interpolate_size()
    }

    /// Given the current time, advances this animation to the next iteration,
    /// updates times, and then toggles the direction if appropriate. Otherwise
    /// does nothing. Returns true if this animation has iterated.
    pub fn iterate_if_necessary(&mut self, time: f64) -> bool {
        if !self.iteration_over(time) {
            return false;
        }

        // Only iterate animations that are currently running.
        if self.state != AnimationState::Running {
            return false;
        }

        if self.on_last_iteration() {
            return false;
        }

        self.iterate();
        true
    }

    fn iterate(&mut self) {
        debug_assert!(!self.on_last_iteration());

        if let KeyframesIterationState::Finite(ref mut current, max) = self.iteration_state {
            *current = (*current + 1.).min(max);
        }

        if let AnimationState::Paused(ref mut progress) = self.state {
            debug_assert!(*progress > 1.);
            *progress -= 1.;
        }

        // Update the next iteration direction if applicable.
        self.started_at += self.duration;
        match self.direction {
            AnimationDirection::Alternate | AnimationDirection::AlternateReverse => {
                self.current_direction = match self.current_direction {
                    AnimationDirection::Normal => AnimationDirection::Reverse,
                    AnimationDirection::Reverse => AnimationDirection::Normal,
                    _ => unreachable!(),
                };
            },
            _ => {},
        }
    }

    /// The number of whole iterations a negative `animation-delay` skips past,
    /// given how far beyond the first iteration it lands (`starting_progress`,
    /// measured in iterations) and how many iterations remain before the last
    /// one (`iterations_left`, infinite for an infinite count).
    ///
    /// This is the closed form of stepping one iteration at a time while the
    /// remaining progress exceeds a single iteration. Stepping cannot be used:
    /// the delay is author-controlled and unbounded, so `animation: 1s -1e16s
    /// infinite` asks for 1e16 steps, and beyond 2^53 a `-= 1.` is lost to
    /// floating point outright, so the loop never terminates at all.
    fn iterations_skipped_by_delay(starting_progress: f64, iterations_left: f64) -> f64 {
        // `max` and `min` both discard a NaN operand, so a non-finite progress
        // folds to "nothing to skip" rather than propagating into the state.
        let iterations = (starting_progress - 1.).ceil().max(0.).min(iterations_left);
        if iterations.is_finite() {
            iterations
        } else {
            0.
        }
    }

    /// The keyframes bracketing `directed_progress`, as `(from, to)` indices
    /// into ascending keyframe offsets. `to` is `None` when the progress lies
    /// past the last offset, which leaves nothing to interpolate towards.
    ///
    /// The offsets are walked in their authored order whichever way the
    /// iteration runs: Web Animations 1 § 4.9.1 folds the playback direction
    /// into the progress before the keyframes are consulted, so a reversed
    /// iteration retraces the forward curve rather than its reflection.
    fn bracketing_keyframes(
        mut start_percentages: impl Iterator<Item = f32>,
        directed_progress: f64,
    ) -> (usize, Option<usize>) {
        // Web Animations 1 section 5.3.1: below 1, the interval starts at the last keyframe whose offset
        // does not exceed the progress; from 1 on, it is the interval that ends at offset 1.
        let progress = directed_progress as f32;
        let to = start_percentages.position(|start| {
            if progress >= 1. {
                start >= 1.
            } else {
                progress < start
            }
        });
        (to.and_then(|index| index.checked_sub(1)).unwrap_or(0), to)
    }

    /// Fast-forwards a newly-created animation to the iteration that a negative
    /// `animation-delay` starts it in, given how far past its first iteration
    /// that delay lands (`starting_progress`, measured in iterations).
    fn advance_to_starting_progress(&mut self, starting_progress: f64) {
        // Stepping stops as soon as it reaches the last iteration, so a finite
        // count bounds the skip independently of how large the delay is.
        let iterations_left = match self.iteration_state {
            KeyframesIterationState::Finite(current, max) => (max - 1. - current).ceil().max(0.),
            KeyframesIterationState::Infinite(_) => f64::INFINITY,
        };

        let iterations = Self::iterations_skipped_by_delay(starting_progress, iterations_left);
        if iterations == 0. {
            return;
        }

        if let KeyframesIterationState::Finite(ref mut current, max) = self.iteration_state {
            *current = (*current + iterations).min(max);
        }

        if let AnimationState::Paused(ref mut progress) = self.state {
            *progress -= iterations;
        }

        self.started_at += iterations * self.duration;

        // An alternating animation flips direction once per iteration, so only
        // the parity of the skipped run can change where it ends up.
        if iterations % 2. == 1. {
            match self.direction {
                AnimationDirection::Alternate | AnimationDirection::AlternateReverse => {
                    self.current_direction = match self.current_direction {
                        AnimationDirection::Normal => AnimationDirection::Reverse,
                        AnimationDirection::Reverse => AnimationDirection::Normal,
                        _ => unreachable!(),
                    };
                },
                _ => {},
            }
        }
    }

    /// A number (> 0 and <= 1) which represents the fraction of a full iteration
    /// that the current iteration of the animation lasts. This will be less than 1
    /// if the current iteration is the fractional remainder of a non-integral
    /// iteration count.
    pub fn current_iteration_end_progress(&self) -> f64 {
        match self.iteration_state {
            KeyframesIterationState::Finite(current, max) => (max - current).min(1.),
            KeyframesIterationState::Infinite(_) => 1.,
        }
    }

    /// The duration of the current iteration of this animation which may be less
    /// than the animation duration if it has a non-integral iteration count.
    pub fn current_iteration_duration(&self) -> f64 {
        self.current_iteration_end_progress() * self.duration
    }

    /// Whether or not the current iteration is over. Note that this method assumes that
    /// the animation is still running.
    fn iteration_over(&self, time: f64) -> bool {
        time > (self.started_at + self.current_iteration_duration())
    }

    /// Assuming this animation is running, whether or not it is on the last iteration.
    fn on_last_iteration(&self) -> bool {
        match self.iteration_state {
            KeyframesIterationState::Finite(current, max) => current >= (max - 1.),
            KeyframesIterationState::Infinite(_) => false,
        }
    }

    /// How far this animation has run at `time`, measured in iterations. A zero
    /// iteration duration gives a zero active duration, so the animation steps
    /// from the phase before its interval to the phase after it at its start
    /// (Web Animations 1 section 4.8.3).
    fn elapsed_iterations(&self, time: f64) -> f64 {
        if self.duration == 0. {
            return if time < self.started_at {
                f64::NEG_INFINITY
            } else {
                f64::INFINITY
            };
        }
        (time - self.started_at) / self.duration
    }

    /// Whether or not this animation has finished at the provided time. This does
    /// not take into account canceling i.e. when an animation or transition is
    /// canceled due to changes in the style.
    pub fn has_ended(&self, time: f64) -> bool {
        if !self.on_last_iteration() {
            return false;
        }

        let progress = match self.state {
            AnimationState::Finished => return true,
            AnimationState::Paused(progress) => progress,
            AnimationState::Running | AnimationState::Pending => self.elapsed_iterations(time),
            AnimationState::Canceled => return false,
        };

        progress >= self.current_iteration_end_progress()
    }

    /// Updates the appropiate state from other animation.
    ///
    /// This happens when an animation is re-submitted to layout, presumably
    /// because of an state change.
    ///
    /// There are some bits of state we can't just replace, over all taking in
    /// account times, so here's that logic.
    pub fn update_from_other(&mut self, other: &Self, now: f64) {
        use self::AnimationState::*;

        debug!(
            "KeyframesAnimationState::update_from_other({:?}, {:?})",
            self, other
        );

        // Preserve playback time, not the old active-interval boundary. CSS
        // Animations 1 requires changed delays to apply from the original start.
        let old_started_at = self.started_at;
        let old_delay = self.delay;
        let old_duration = self.duration;
        let old_direction = self.current_direction;
        let old_state = self.state.clone();
        let old_iteration_state = self.iteration_state.clone();

        *self = other.clone();

        let delay_change = self.delay - old_delay;
        self.started_at = old_started_at + delay_change;
        self.current_direction = old_direction;

        // Don't update the iteration count, just the iteration limit.
        // TODO: see how changing the limit affects rendering in other browsers.
        // We might need to keep the iteration count even when it's infinite.
        match (&mut self.iteration_state, old_iteration_state) {
            (
                &mut KeyframesIterationState::Finite(ref mut iters, _),
                KeyframesIterationState::Finite(old_iters, _),
            ) => *iters = old_iters,
            _ => {},
        }

        // Don't pause or restart animations that should remain finished.
        // We call mem::replace because `has_ended(...)` looks at `Animation::state`.
        let new_state = std::mem::replace(&mut self.state, Running);
        if old_state == Finished && self.has_ended(now) {
            self.state = Finished;
        } else {
            self.state = new_state;
        }

        // If we're unpausing the animation, fake the start time so we seem to
        // restore it.
        //
        // If the animation keeps paused, keep the old value.
        //
        // If we're pausing the animation, compute the progress value.
        match (&mut self.state, &old_state) {
            (&mut Pending, &Paused(progress)) => {
                self.started_at = now - (old_duration * progress - delay_change);
            },
            (&mut Paused(ref mut new), &Paused(old)) => {
                *new = if self.duration == 0.0 {
                    0.0
                } else {
                    (old_duration * old - delay_change) / self.duration
                };
            },
            (&mut Paused(ref mut progress), &Running) => {
                *progress = if self.duration == 0.0 {
                    0.0
                } else {
                    (now - self.started_at) / self.duration
                };
            },
            _ => {},
        }

        // Try to detect when we should skip straight to the running phase to
        // avoid sending multiple animationstart events.
        if self.state == Pending && self.started_at <= now && old_state != Pending {
            self.state = Running;
        }
    }

    /// The directed progress and easing phase when this animation contributes a value.
    pub fn directed_progress_at(&self, now: f64) -> Option<(f64, BeforeFlag)> {
        let total_progress = match self.state {
            AnimationState::Running | AnimationState::Pending | AnimationState::Finished => {
                self.elapsed_iterations(now)
            }
            AnimationState::Paused(progress) => progress,
            AnimationState::Canceled => return None,
        };

        let before_phase = total_progress < 0.;
        if before_phase && self.fill_mode != AnimationFillMode::Backwards && self.fill_mode != AnimationFillMode::Both {
            return None;
        }
        let after_phase = self.has_ended(now);
        if after_phase && self.fill_mode != AnimationFillMode::Forwards && self.fill_mode != AnimationFillMode::Both {
            return None;
        }
        let total_progress = total_progress.min(self.current_iteration_end_progress()).max(0.0);

        // Reversing an iteration mirrors the progress and nothing else
        // (Web Animations 1 § 4.9.1 "Calculating the directed progress"). The
        // keyframes are then walked, bracketed, and eased in ordinary offset
        // order, so a reversed iteration retraces the very curve the forward
        // one drew rather than its reflection.
        let directed = match self.current_direction {
            AnimationDirection::Normal => total_progress,
            AnimationDirection::Reverse => 1. - total_progress,
            _ => unreachable!(),
        };
        let before_flag = match self.current_direction {
            AnimationDirection::Normal if before_phase => BeforeFlag::Set,
            AnimationDirection::Reverse if after_phase => BeforeFlag::Set,
            _ => BeforeFlag::Unset,
        };
        Some((directed, before_flag))
    }

    /// Fill in an `AnimationValueMap` with values calculated from this animation at
    /// the given time value.
    fn get_property_declaration_at_time(&self, now: f64, map: &mut AnimationValueMap) {
        debug_assert!(!self.computed_steps.is_empty());
        let Some((directed_progress, before_flag)) = self.directed_progress_at(now) else {
            return;
        };

        // Get the indices of the previous (from) keyframe and the next (to) keyframe.
        let (prev_keyframe_index, next_keyframe_index) = Self::bracketing_keyframes(
            self.computed_steps.iter().map(|step| step.start_percentage),
            directed_progress,
        );

        debug!(
            "Animation::get_property_declaration_at_time: keyframe from {:?} to {:?}",
            prev_keyframe_index, next_keyframe_index
        );

        let prev_keyframe = &self.computed_steps[prev_keyframe_index];
        let next_keyframe = match next_keyframe_index {
            Some(index) => &self.computed_steps[index],
            None => return,
        };

        // If we only need to take into account one keyframe, then exit early
        // in order to avoid doing more work.
        let mut add_declarations_to_map = |keyframe: &ComputedKeyframe| {
            #[cfg(feature = "servo")]
            let shadow = self.composited_shadow(keyframe, map);
            #[cfg(feature = "servo")]
            if let Some(ref shadow) = shadow {
                for value in shadow.components() {
                    map.insert(value.id().to_owned(), value);
                }
            }
            for value in keyframe.values.iter() {
                #[cfg(feature = "servo")]
                if shadow.is_some()
                    && matches!(value.id(), PropertyDeclarationId::Longhand(id) if SHADOW_COMPONENTS.contains(&id))
                {
                    continue;
                }
                let value = self.composited_keyframe_value(keyframe, value, map);
                map.insert(value.id().to_owned(), value.clone());
            }
        };
        if directed_progress >= 1.0 {
            add_declarations_to_map(&next_keyframe);
            return;
        }

        let percentage_between_keyframes =
            (next_keyframe.start_percentage - prev_keyframe.start_percentage).abs() as f64;
        let duration_between_keyframes = percentage_between_keyframes * self.duration;
        let progress_between_keyframes =
            (directed_progress - prev_keyframe.start_percentage as f64) / percentage_between_keyframes;

        #[cfg(feature = "servo")]
        let shadow_sampled = if let (Some(from), Some(to)) = (
            self.composited_shadow(prev_keyframe, map),
            self.composited_shadow(next_keyframe, map),
        ) {
            let epsilon = 1.0 / (200.0 * duration_between_keyframes);
            let progress =
                prev_keyframe
                    .timing_function
                    .calculate_output(progress_between_keyframes, before_flag, epsilon);
            let sample = from
                .animate(&to, Procedure::Interpolate { progress })
                .unwrap_or_else(|_| if progress < 0.5 { from } else { to });
            for value in sample.components() {
                map.insert(value.id().to_owned(), value);
            }
            true
        } else {
            false
        };

        for (from, to) in prev_keyframe.values.iter().zip(next_keyframe.values.iter()) {
            #[cfg(feature = "servo")]
            if shadow_sampled
                && matches!(from.id(), PropertyDeclarationId::Longhand(id) if SHADOW_COMPONENTS.contains(&id))
            {
                continue;
            }
            let from = self.composited_keyframe_value(prev_keyframe, from, map);
            let to = self.composited_keyframe_value(next_keyframe, to, map);
            let animation = PropertyAnimation {
                from,
                to,
                timing_function: prev_keyframe.timing_function.clone(),
                duration: duration_between_keyframes as f64,
                interpolate_size: self.interpolate_size(),
                #[cfg(feature = "servo")]
                shadow: None,
            };

            let value = animation.calculate_value(progress_between_keyframes, before_flag);
            map.insert(value.id().to_owned(), value);
        }
    }
}

impl fmt::Debug for Animation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Animation")
            .field("name", &self.name)
            .field("started_at", &self.started_at)
            .field("duration", &self.duration)
            .field("delay", &self.delay)
            .field("iteration_state", &self.iteration_state)
            .field("state", &self.state)
            .field("direction", &self.direction)
            .field("current_direction", &self.current_direction)
            .field("cascade_style", &())
            .finish()
    }
}

/// The identity of a CSS transition, retained across clones and timeline seeks.
#[derive(Clone, Copy, Debug, Eq, Hash, MallocSizeOf, PartialEq)]
pub struct TransitionId(u64);

/// A CSS Transition
#[derive(Clone, Debug, MallocSizeOf)]
pub struct Transition {
    id: TransitionId,

    /// The start time of this transition, which is the current value of the animation
    /// timeline when this transition was created plus any animation delay.
    pub start_time: f64,

    /// The delay used for this transition.
    pub delay: f64,

    /// The internal style `PropertyAnimation` for this transition.
    pub property_animation: PropertyAnimation,

    /// The state of this transition.
    pub state: AnimationState,

    /// Whether or not this transition is new and or has already been tracked
    /// by the script thread.
    pub is_new: bool,

    /// If this `Transition` has been replaced by a new one this field is
    /// used to help produce better reversed transitions.
    pub reversing_adjusted_start_value: AnimationValue,
    #[cfg(feature = "servo")]
    reversing_adjusted_shadow: Option<AnimatedBoxShadowList>,

    /// If this `Transition` has been replaced by a new one this field is
    /// used to help produce better reversed transitions.
    pub reversing_shortening_factor: f64,
}

impl Transition {
    /// Returns the identity of this transition object.
    pub fn id(&self) -> TransitionId {
        self.id
    }

    fn new(
        start_time: f64,
        delay: f64,
        duration: f64,
        from: AnimationValue,
        to: AnimationValue,
        timing_function: &TimingFunction,
        interpolate_size: InterpolateSize,
        #[cfg(feature = "servo")] shadow: Option<(AnimatedBoxShadowList, AnimatedBoxShadowList)>,
    ) -> Self {
        #[cfg(feature = "servo")]
        let reversing_adjusted_shadow = shadow.as_ref().map(|(from, _)| from.clone());
        let property_animation = PropertyAnimation {
            from: from.clone(),
            to,
            timing_function: timing_function.clone(),
            duration,
            interpolate_size,
            #[cfg(feature = "servo")]
            shadow,
        };
        static NEXT_TRANSITION_ID: AtomicU64 = AtomicU64::new(0);
        let id = NEXT_TRANSITION_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("transition identifiers exhausted");
        Self {
            id: TransitionId(id),
            start_time,
            delay,
            property_animation,
            state: AnimationState::Pending,
            is_new: true,
            reversing_adjusted_start_value: from,
            #[cfg(feature = "servo")]
            reversing_adjusted_shadow,
            reversing_shortening_factor: 1.0,
        }
    }

    fn end_value_matches(
        &self,
        value: &AnimationValue,
        #[cfg(feature = "servo")] shadow: Option<&AnimatedBoxShadowList>,
    ) -> bool {
        #[cfg(feature = "servo")]
        if self.property_animation.shadow.is_some() || shadow.is_some() {
            return self.property_animation.shadow.as_ref().map(|(_, end)| end) == shadow;
        }
        self.property_animation.to == *value
    }

    fn reversing_start_matches(
        &self,
        value: &AnimationValue,
        #[cfg(feature = "servo")] shadow: Option<&AnimatedBoxShadowList>,
    ) -> bool {
        #[cfg(feature = "servo")]
        if self.reversing_adjusted_shadow.is_some() || shadow.is_some() {
            return self.reversing_adjusted_shadow.as_ref() == shadow;
        }
        self.reversing_adjusted_start_value == *value
    }

    #[cfg(feature = "servo")]
    fn current_shadow(&self, time: f64) -> Option<AnimatedBoxShadowList> {
        let (from, to) = self.property_animation.shadow.as_ref()?;
        let progress = (time - self.start_time) / self.property_animation.duration;
        let eased = self
            .property_animation
            .timing_function_output(progress.clamp(0.0, 1.0), Self::before_flag(progress));
        Some(
            from.animate(to, Procedure::Interpolate { progress: eased })
                .unwrap_or_else(|_| {
                    if eased < 0.5 {
                        from.clone()
                    } else {
                        to.clone()
                    }
                }),
        )
    }

    fn update_for_possibly_reversed_transition(
        &mut self,
        replaced_transition: &Transition,
        delay: f64,
        now: f64,
    ) {
        // If we reach here, we need to calculate a reversed transition according to
        // https://drafts.csswg.org/css-transitions/#starting
        //
        //  "...if the reversing-adjusted start value of the running transition
        //  is the same as the value of the property in the after-change style (see
        //  the section on reversing of transitions for why these case exists),
        //  implementations must cancel the running transition and start
        //  a new transition..."
        if !replaced_transition.reversing_start_matches(
            &self.property_animation.to,
            #[cfg(feature = "servo")]
            self.property_animation.shadow.as_ref().map(|(_, to)| to),
        ) {
            return;
        }

        // "* reversing-adjusted start value is the end value of the running transition"
        let replaced_animation = &replaced_transition.property_animation;
        self.reversing_adjusted_start_value = replaced_animation.to.clone();
        #[cfg(feature = "servo")]
        {
            self.reversing_adjusted_shadow =
                replaced_animation.shadow.as_ref().map(|(_, to)| to.clone());
        }

        // "* reversing shortening factor is the absolute value, clamped to the
        //    range [0, 1], of the sum of:
        //    1. the output of the timing function of the old transition at the
        //      time of the style change event, times the reversing shortening
        //      factor of the old transition
        //    2.  1 minus the reversing shortening factor of the old transition."
        let elapsed_progress = (now - replaced_transition.start_time)
            / (replaced_transition.property_animation.duration);
        let timing_function_output = replaced_animation.timing_function_output(
            elapsed_progress.clamp(0.0, 1.0),
            Transition::before_flag(elapsed_progress),
        );
        let old_reversing_shortening_factor = replaced_transition.reversing_shortening_factor;
        self.reversing_shortening_factor = ((timing_function_output
            * old_reversing_shortening_factor)
            + (1.0 - old_reversing_shortening_factor))
            .abs()
            .min(1.0)
            .max(0.0);

        // "* start time is the time of the style change event plus:
        //    1. if the matching transition delay is nonnegative, the matching
        //       transition delay, or.
        //    2. if the matching transition delay is negative, the product of the new
        //       transition’s reversing shortening factor and the matching transition delay,"
        self.start_time = if delay >= 0. {
            now + delay
        } else {
            now + (self.reversing_shortening_factor * delay)
        };

        // "* end time is the start time plus the product of the matching transition
        //    duration and the new transition’s reversing shortening factor,"
        self.property_animation.duration *= self.reversing_shortening_factor;

        // "* start value is the current value of the property in the running transition,
        //  * end value is the value of the property in the after-change style,"
        let procedure = Procedure::Interpolate {
            progress: timing_function_output,
        };
        #[cfg(feature = "servo")]
        if let (Some((left, right)), Some((new_start, _))) = (
            &replaced_animation.shadow,
            &mut self.property_animation.shadow,
        ) {
            if let Ok(sample) = left.animate(right, procedure) {
                *new_start = sample;
            }
        }
        match replaced_animation.animate(
            &replaced_animation.from,
            &replaced_animation.to,
            procedure,
        ) {
            Ok(new_start) => self.property_animation.from = new_start,
            Err(..) => {},
        }
    }

    /// Whether or not this animation has ended at the provided time. This does
    /// not take into account canceling i.e. when an animation or transition is
    /// canceled due to changes in the style.
    pub fn has_ended(&self, time: f64) -> bool {
        time >= self.start_time + (self.property_animation.duration)
    }

    /// Update the given animation at a given point of progress.
    pub fn calculate_value(&self, time: f64) -> AnimationValue {
        let progress = (time - self.start_time) / (self.property_animation.duration);
        self.property_animation
            .calculate_value(progress.clamp(0.0, 1.0), Self::before_flag(progress))
    }

    /// A transition always runs forwards, so its before flag is set before its active interval (Web Animations 1
    /// section 4.10).
    fn before_flag(progress: f64) -> BeforeFlag {
        if progress < 0. {
            BeforeFlag::Set
        } else {
            BeforeFlag::Unset
        }
    }
}

/// Holds the animation state for a particular element.
#[derive(Debug, Default, MallocSizeOf)]
pub struct ElementAnimationSet {
    /// The animations for this element.
    pub animations: Vec<Animation>,

    /// The transitions for this element.
    pub transitions: Vec<Transition>,

    /// Whether or not this ElementAnimationSet has had animations or transitions
    /// which have been added, removed, or had their state changed.
    pub dirty: bool,
}

impl ElementAnimationSet {
    fn update_animation_list(&mut self, new_animations: Vec<Animation>, now: f64) {
        self.dirty |= !self.animations.is_empty() || !new_animations.is_empty();
        let mut unmatched = std::mem::take(&mut self.animations);
        for mut new_animation in new_animations.into_iter().rev() {
            if let Some(index) = unmatched.iter().rposition(|existing| {
                existing.state != AnimationState::Canceled && existing.name == new_animation.name
            }) {
                let mut existing = unmatched.remove(index);
                existing.update_from_other(&new_animation, now);
                new_animation = existing;
            }
            self.animations.push(new_animation);
        }
        self.animations.reverse();
        for mut animation in unmatched {
            animation.state = AnimationState::Canceled;
            self.animations.push(animation);
        }
    }

    /// Cancel all animations in this `ElementAnimationSet`. This is typically called
    /// when the element has been removed from the DOM.
    pub fn cancel_all_animations(&mut self) {
        self.dirty = !self.animations.is_empty();
        for animation in self.animations.iter_mut() {
            animation.state = AnimationState::Canceled;
        }
        self.cancel_active_transitions();
    }

    fn cancel_active_transitions(&mut self) {
        for transition in self.transitions.iter_mut() {
            if transition.state != AnimationState::Finished {
                self.dirty = true;
                transition.state = AnimationState::Canceled;
            }
        }
    }

    /// Apply all active animations.
    pub fn apply_active_animations(
        &self,
        context: &SharedStyleContext,
        style: &mut Arc<ComputedValues>,
    ) {
        let now = context.current_time_for_animations;
        let mutable_style = Arc::make_mut(style);
        if let Some(map) = self.get_value_map_for_active_animations(now) {
            for value in map.values() {
                value.set_in_style_for_servo(mutable_style, &context.stylist);
            }
        }

        if let Some(map) = self.get_value_map_for_transitions(now, IgnoreTransitions::Canceled) {
            for value in map.values() {
                value.set_in_style_for_servo(mutable_style, &context.stylist);
            }
        }
    }

    /// Clear all canceled animations and transitions from this `ElementAnimationSet`.
    pub fn clear_canceled_animations(&mut self) {
        self.animations
            .retain(|animation| animation.state != AnimationState::Canceled);
        self.transitions
            .retain(|animation| animation.state != AnimationState::Canceled);
    }

    /// Whether this `ElementAnimationSet` is empty, which means it doesn't
    /// hold any animations in any state.
    pub fn is_empty(&self) -> bool {
        self.animations.is_empty() && self.transitions.is_empty()
    }

    /// Whether or not this state needs animation ticks for its transitions
    /// or animations.
    pub fn needs_animation_ticks(&self) -> bool {
        self.animations
            .iter()
            .any(|animation| animation.state.needs_to_be_ticked())
            || self
                .transitions
                .iter()
                .any(|transition| transition.state.needs_to_be_ticked())
    }

    /// The number of running animations and transitions for this `ElementAnimationSet`.
    pub fn running_animation_and_transition_count(&self) -> usize {
        self.animations
            .iter()
            .filter(|animation| animation.state.needs_to_be_ticked())
            .count()
            + self
                .transitions
                .iter()
                .filter(|transition| transition.state.needs_to_be_ticked())
                .count()
    }

    /// If this `ElementAnimationSet` has any any active animations.
    pub fn has_active_animation(&self) -> bool {
        self.animations
            .iter()
            .any(|animation| animation.state != AnimationState::Canceled)
    }

    /// If this `ElementAnimationSet` has any any active transitions.
    pub fn has_active_transition(&self) -> bool {
        self.transitions
            .iter()
            .any(|transition| transition.state != AnimationState::Canceled)
    }

    /// Update our animations given a new style, canceling or starting new animations
    /// when appropriate.
    pub fn update_animations_for_new_style<E>(
        &mut self,
        element: E,
        context: &SharedStyleContext,
        new_style: &Arc<ComputedValues>,
        cascade_target: AnimationCascadeTarget,
        resolver: &mut StyleResolverForElement<E>,
    ) where
        E: TElement,
    {
        maybe_start_animations(
            element,
            &context,
            &new_style,
            cascade_target,
            self,
            resolver,
        );
    }

    /// Update our transitions given a new style, canceling or starting new animations
    /// when appropriate.
    pub fn update_transitions_for_new_style(
        &mut self,
        might_need_transitions_update: bool,
        context: &SharedStyleContext,
        old_style: Option<&Arc<ComputedValues>>,
        after_change_style: &Arc<ComputedValues>,
        host_animated: &crate::properties::LonghandIdSet,
    ) {
        // If this is the first style, we don't trigger any transitions and we assume
        // there were no previously triggered transitions.
        let mut before_change_style = match old_style {
            Some(old_style) => Arc::clone(old_style),
            None => return,
        };

        // If the style of this element is display:none, then cancel all active transitions.
        if after_change_style.get_box().clone_display().is_none() {
            self.cancel_active_transitions();
            return;
        }

        if !might_need_transitions_update {
            return;
        }

        // We convert old values into `before-change-style` here.
        if self.has_active_transition() || self.has_active_animation() {
            self.apply_active_animations(context, &mut before_change_style);
        }

        let transitioning_properties = start_transitions_if_applicable(
            context,
            &before_change_style,
            after_change_style,
            self,
            host_animated,
        );

        // Cancel any non-finished transitions that have properties which no
        // longer transition.
        //
        // Step 3 in https://drafts.csswg.org/css-transitions/#starting:
        // > If the element has a running transition or completed transition for
        // > the property, and there is not a matching transition-property value,
        // > then implementations must cancel the running transition or remove the
        // > completed transition from the set of completed transitions.
        //
        // TODO: This is happening here as opposed to in
        // `start_transition_if_applicable` as an optimization, but maybe this
        // code should be reworked to be more like the specification.
        for transition in self.transitions.iter_mut() {
            if transition.state == AnimationState::Finished
                || transition.state == AnimationState::Canceled
            {
                continue;
            }
            if transitioning_properties.contains(transition.property_animation.property_id()) {
                continue;
            }
            transition.state = AnimationState::Canceled;
            self.dirty = true;
        }
    }

    fn start_transition_if_applicable(
        &mut self,
        context: &SharedStyleContext,
        property_declaration_id: &PropertyDeclarationId,
        index: usize,
        old_style: &ComputedValues,
        new_style: &Arc<ComputedValues>,
    ) {
        let style = new_style.get_ui();
        let interpolate_size = new_style.get_inherited_box().clone_interpolate_size();
        #[cfg(not(feature = "servo"))]
        let interpolable = |from: &AnimationValue, to: &AnimationValue| {
            from.animate_with_interpolate_size(
                to,
                Procedure::Interpolate { progress: 0.5 },
                interpolate_size,
            )
            .is_ok()
        };
        let allow_discrete =
            style.transition_behavior_mod(index) == TransitionBehavior::AllowDiscrete;

        // FIXME(emilio): Handle the case where old_style and new_style's writing mode differ.
        let Some(from) = AnimationValue::from_computed_values(*property_declaration_id, old_style)
        else {
            return;
        };
        let Some(to) = AnimationValue::from_computed_values(*property_declaration_id, new_style)
        else {
            return;
        };

        #[cfg(feature = "servo")]
        let shadow = {
            use crate::values::computed::TransitionProperty;
            let coordinated = matches!(style.transition_property_at(index), TransitionProperty::NonCustom(id)
                if matches!(id.longhand_or_shorthand(), Err(crate::properties::ShorthandId::All | crate::properties::ShorthandId::BoxShadow)));
            match property_declaration_id {
                PropertyDeclarationId::Longhand(id)
                    if coordinated && SHADOW_COMPONENTS.contains(id) =>
                {
                    Some((
                        AnimatedBoxShadowList::from_computed_values(old_style, None),
                        AnimatedBoxShadowList::from_computed_values(new_style, None),
                    ))
                },
                _ => None,
            }
        };
        #[cfg(feature = "servo")]
        let (from, to) = if let Some((ref left, ref right)) = shadow {
            let component = |list: &AnimatedBoxShadowList| {
                list.components()
                    .into_iter()
                    .find(|value| value.id() == *property_declaration_id)
                    .unwrap()
            };
            (component(left), component(right))
        } else {
            (from, to)
        };
        #[cfg(feature = "servo")]
        let interpolable = |from: &AnimationValue, to: &AnimationValue| {
            shadow.as_ref().map_or_else(
                || {
                    from.animate_with_interpolate_size(
                        to,
                        Procedure::Interpolate { progress: 0.5 },
                        interpolate_size,
                    )
                    .is_ok()
                },
                |(left, right)| {
                    left.animate(right, Procedure::Interpolate { progress: 0.5 })
                        .is_ok()
                },
            )
        };
        let timing_function = style.transition_timing_function_mod(index);
        let duration = style.transition_duration_mod(index).seconds() as f64;
        let delay = style.transition_delay_mod(index).seconds() as f64;
        let now = context.current_time_for_animations;
        let transitionable = property_declaration_id.is_animatable()
            && (allow_discrete || !property_declaration_id.is_discrete_animatable())
            && (allow_discrete
                || match property_declaration_id {
                    PropertyDeclarationId::Custom(_) => from.interpolable_with(&to),
                    _ => interpolable(&from, &to),
                });

        // CSS Transitions §3: only a running or completed transition matches.
        let mut existing_transition = self.transitions.iter_mut().find(|transition| {
            transition.state != AnimationState::Canceled
                && transition.property_animation.property_id() == *property_declaration_id
        });

        // Step 1:
        // > If all of the following are true:
        // >  - the element does not have a running transition for the property,
        // >  - the before-change style is different from the after-change style
        // >    for that property, and the values for the property are
        // >    transitionable,
        // >  - the element does not have a completed transition for the property
        // >    or the end value of the completed transition is different from the
        // >    after-change style for the property,
        // >  - there is a matching transition-property value, and
        // >  - the combined duration is greater than 0s,
        //
        // This function is only run if there is a matching transition-property
        // value, so that check is skipped here.
        let has_running_transition = existing_transition.as_ref().is_some_and(|transition| {
            transition.state != AnimationState::Finished
                && transition.state != AnimationState::Canceled
        });
        let no_completed_transition_or_end_values_differ =
            existing_transition.as_ref().is_none_or(|transition| {
                transition.state != AnimationState::Finished
                    || !transition.end_value_matches(
                        &to,
                        #[cfg(feature = "servo")]
                        shadow.as_ref().map(|(_, to)| to),
                    )
            });
        if !has_running_transition
            && from != to
            && transitionable
            && no_completed_transition_or_end_values_differ
            && (duration + delay > 0.0)
        {
            // > then implementations must remove the completed transition (if
            // > present) from the set of completed transitions and start a
            // > transition whose:
            // >
            // > - start time is the time of the style change event plus the matching transition delay,
            // > - end time is the start time plus the matching transition duration,
            // > - start value is the value of the transitioning property in the before-change style,
            // > - end value is the value of the transitioning property in the after-change style,
            // > - reversing-adjusted start value is the same as the start value, and
            // > - reversing shortening factor is 1.
            self.transitions.push(Transition::new(
                now + delay, /* start_time */
                delay,
                duration,
                from,
                to,
                &timing_function,
                interpolate_size,
                #[cfg(feature = "servo")]
                shadow.clone(),
            ));
            self.dirty = true;
            return;
        }

        // > Step 2: Otherwise, if the element has a completed transition for the
        // > property and the end value of the completed transition is different
        // > from the after-change style for the property, then implementations
        // > must remove the completed transition from the set of completed
        // > transitions.
        //
        // All completed transitions will be cleared from the `AnimationSet` in
        // `process_animations_for_style in `matching.rs`.

        // > Step 3: If the element has a running transition or completed
        // > transition for the property, and there is not a matching
        // > transition-property value, then implementations must cancel the
        // > running transition or remove the completed transition from the set
        // > of completed transitions.
        //
        // - All completed transitions will be cleared cleared from the `AnimationSet` in
        //   `process_animations_for_style in `matching.rs`.
        // - Transitions for properties that don't have a matching transition-property
        //   value will be canceled in `Self::update_transitions_for_new_style`. In addition,
        //   this method is only called for properties that do ahave a matching
        //   transition-property value.

        let Some(existing_transition) = existing_transition.as_mut() else {
            return;
        };

        // > Step 4: If the element has a running transition for the property,
        // > there is a matching transition-property value, and the end value of
        // > the running transition is not equal to the value of the property in
        // > the after-change style, then:
        if has_running_transition
            && !existing_transition.end_value_matches(
                &to,
                #[cfg(feature = "servo")]
                shadow.as_ref().map(|(_, to)| to),
            )
        {
            // > Step 4.1: If the current value of the property in the running transition is
            // > equal to the value of the property in the after-change style, or
            // > if these two values are not transitionable, then implementations
            // > must cancel the running transition.
            let current_value = existing_transition.calculate_value(now);
            let transitionable_from_current_value =
                transitionable && (allow_discrete || interpolable(&current_value, &to));
            #[cfg(feature = "servo")]
            let current_shadow = existing_transition.current_shadow(now);
            let current_matches = current_value == to;
            #[cfg(feature = "servo")]
            let current_matches = if current_shadow.is_some() || shadow.is_some() {
                current_shadow.as_ref() == shadow.as_ref().map(|(_, to)| to)
            } else {
                current_matches
            };
            #[cfg(feature = "servo")]
            let transitionable_from_current_value = match (current_shadow.as_ref(), shadow.as_ref())
            {
                (Some(current), Some((_, target))) => {
                    allow_discrete
                        || current
                            .animate(target, Procedure::Interpolate { progress: 0.5 })
                            .is_ok()
                },
                _ => transitionable_from_current_value,
            };
            if current_matches || !transitionable_from_current_value {
                existing_transition.state = AnimationState::Canceled;
                self.dirty = true;
                return;
            }

            // > Step 4.2: Otherwise, if the combined duration is less than or
            // > equal to 0s, or if the current value of the property in the
            // > running transition is not transitionable with the value of the
            // > property in the after-change style, then implementations must
            // > cancel the running transition.
            if duration + delay <= 0.0 {
                existing_transition.state = AnimationState::Canceled;
                self.dirty = true;
                return;
            }

            // > Step 4.3: Otherwise, if the reversing-adjusted start value of the
            // > running transition is the same as the value of the property in
            // > the after-change style (see the section on reversing of
            // > transitions for why these case exists), implementations must
            // > cancel the running transition and start a new transition whose:
            #[cfg(feature = "servo")]
            let shadow = shadow.map(|(from, to)| (current_shadow.unwrap_or(from), to));
            if existing_transition.reversing_start_matches(
                &to,
                #[cfg(feature = "servo")]
                shadow.as_ref().map(|(_, to)| to),
            ) {
                existing_transition.state = AnimationState::Canceled;

                let mut transition = Transition::new(
                    now + delay, /* start_time */
                    delay,
                    duration,
                    from,
                    to,
                    &timing_function,
                    interpolate_size,
                    #[cfg(feature = "servo")]
                    shadow.clone(),
                );

                // This function takes care of applying all of the modifications to the transition
                // after "whose:" above.
                transition.update_for_possibly_reversed_transition(
                    &existing_transition,
                    delay,
                    now,
                );

                self.transitions.push(transition);
                self.dirty = true;
                return;
            }

            // > Step 4.4: Otherwise, implementations must cancel the running
            // > transition and start a new transition whose:
            // >  - start time is the time of the style change event plus the matching transition delay,
            // >  - end time is the start time plus the matching transition duration,
            // >  - start value is the current value of the property in the running transition,
            // >  - end value is the value of the property in the after-change style,
            // >  - reversing-adjusted start value is the same as the start value, and
            // >  - reversing shortening factor is 1.
            existing_transition.state = AnimationState::Canceled;
            self.transitions.push(Transition::new(
                now + delay, /* start_time */
                delay,
                duration,
                current_value,
                to,
                &timing_function,
                interpolate_size,
                #[cfg(feature = "servo")]
                shadow.clone(),
            ));
            self.dirty = true;
        }
    }

    /// Generate a `AnimationValueMap` for this `ElementAnimationSet`'s
    /// transitions, ignoring those specified by the `ignore_transitions`
    /// argument.
    fn get_value_map_for_transitions(
        &self,
        now: f64,
        ignore_transitions: IgnoreTransitions,
    ) -> Option<AnimationValueMap> {
        if !self.has_active_transition() {
            return None;
        }

        let mut map =
            AnimationValueMap::with_capacity_and_hasher(self.transitions.len(), Default::default());
        for transition in &self.transitions {
            match ignore_transitions {
                IgnoreTransitions::Canceled => {
                    if transition.state == AnimationState::Canceled {
                        continue;
                    }
                },
                IgnoreTransitions::CanceledAndFinished => {
                    if transition.state == AnimationState::Canceled
                        || transition.state == AnimationState::Finished
                    {
                        continue;
                    }
                },
            }

            let value = transition.calculate_value(now);
            map.insert(value.id().to_owned(), value);
        }

        Some(map)
    }

    /// Generate a `AnimationValueMap` for this `ElementAnimationSet`'s
    /// active animations at the given time value.
    pub fn get_value_map_for_active_animations(&self, now: f64) -> Option<AnimationValueMap> {
        if !self.has_active_animation() {
            return None;
        }

        let mut map = Default::default();
        for animation in &self.animations {
            animation.get_property_declaration_at_time(now, &mut map);
        }

        Some(map)
    }
}

#[derive(Clone, Debug, Eq, Hash, MallocSizeOf, PartialEq)]
/// A key that is used to identify nodes in the `DocumentAnimationSet`.
pub struct AnimationSetKey {
    /// The node for this `AnimationSetKey`.
    pub node: OpaqueNode,
    /// The pseudo element for this `AnimationSetKey`. If `None` this key will
    /// refer to the main content for its node.
    pub pseudo_element: Option<PseudoElement>,
}

impl AnimationSetKey {
    /// Create a new key given a node and optional pseudo element.
    pub fn new(node: OpaqueNode, pseudo_element: Option<PseudoElement>) -> Self {
        AnimationSetKey {
            node,
            pseudo_element,
        }
    }

    /// Create a new key for the main content of this node.
    pub fn new_for_non_pseudo(node: OpaqueNode) -> Self {
        AnimationSetKey {
            node,
            pseudo_element: None,
        }
    }

    /// Create a new key for given node and pseudo element.
    pub fn new_for_pseudo(node: OpaqueNode, pseudo_element: PseudoElement) -> Self {
        AnimationSetKey {
            node,
            pseudo_element: Some(pseudo_element),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, MallocSizeOf)]
enum AnimationSamplingMode {
    #[default]
    Enabled,
    SuppressedForBaseStyle,
}

#[derive(Clone, Debug, Default, MallocSizeOf)]
/// A set of animations for a document.
pub struct DocumentAnimationSet {
    /// The `ElementAnimationSet`s that this set contains.
    #[ignore_malloc_size_of = "Arc is hard"]
    pub sets: Arc<RwLock<FxHashMap<AnimationSetKey, ElementAnimationSet>>>,

    sampling_mode: AnimationSamplingMode,
}

impl DocumentAnimationSet {
    /// Return a shared animation set that updates retained animation state but
    /// omits sampled declarations while calculating animation-free base style.
    pub fn for_base_style_recalculation(&self) -> Self {
        Self {
            sets: self.sets.clone(),
            sampling_mode: AnimationSamplingMode::SuppressedForBaseStyle,
        }
    }

    /// Whether this view calculates animation-free base style.
    pub fn recalculates_base_style(&self) -> bool {
        matches!(
            self.sampling_mode,
            AnimationSamplingMode::SuppressedForBaseStyle
        )
    }

    /// Return whether or not the provided node has active CSS animations.
    pub fn has_active_animations(&self, key: &AnimationSetKey) -> bool {
        self.sets
            .read()
            .get(key)
            .map_or(false, |set| set.has_active_animation())
    }

    /// Return whether or not the provided node has active CSS transitions.
    pub fn has_active_transitions(&self, key: &AnimationSetKey) -> bool {
        self.sets
            .read()
            .get(key)
            .map_or(false, |set| set.has_active_transition())
    }

    /// Return a locked PropertyDeclarationBlock with animation values for the given
    /// key and time.
    pub fn get_animation_declarations(
        &self,
        key: &AnimationSetKey,
        time: f64,
        shared_lock: &SharedRwLock,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        if matches!(
            self.sampling_mode,
            AnimationSamplingMode::SuppressedForBaseStyle
        ) {
            return None;
        }
        self.sets
            .read()
            .get(key)
            .and_then(|set| set.get_value_map_for_active_animations(time))
            .map(|map| {
                let block = PropertyDeclarationBlock::from_animation_value_map(&map);
                Arc::new(shared_lock.wrap(block))
            })
    }

    /// Return a locked PropertyDeclarationBlock with transition values for the given
    /// key and time.
    pub fn get_transition_declarations(
        &self,
        key: &AnimationSetKey,
        time: f64,
        shared_lock: &SharedRwLock,
    ) -> Option<Arc<Locked<PropertyDeclarationBlock>>> {
        if matches!(
            self.sampling_mode,
            AnimationSamplingMode::SuppressedForBaseStyle
        ) {
            return None;
        }
        self.sets
            .read()
            .get(key)
            .and_then(|set| {
                set.get_value_map_for_transitions(time, IgnoreTransitions::CanceledAndFinished)
            })
            .map(|map| {
                let block = PropertyDeclarationBlock::from_animation_value_map(&map);
                Arc::new(shared_lock.wrap(block))
            })
    }

    /// Get all the animation declarations for the given key, returning an empty
    /// `AnimationDeclarations` if there are no animations.
    pub fn get_all_declarations(
        &self,
        key: &AnimationSetKey,
        time: f64,
        shared_lock: &SharedRwLock,
    ) -> AnimationDeclarations {
        if matches!(
            self.sampling_mode,
            AnimationSamplingMode::SuppressedForBaseStyle
        ) {
            return AnimationDeclarations::default();
        }
        let sets = self.sets.read();
        let set = match sets.get(key) {
            Some(set) => set,
            None => return Default::default(),
        };

        let animations = set.get_value_map_for_active_animations(time).map(|map| {
            let block = PropertyDeclarationBlock::from_animation_value_map(&map);
            Arc::new(shared_lock.wrap(block))
        });
        let transitions = set
            .get_value_map_for_transitions(time, IgnoreTransitions::CanceledAndFinished)
            .map(|map| {
                let block = PropertyDeclarationBlock::from_animation_value_map(&map);
                Arc::new(shared_lock.wrap(block))
            });
        AnimationDeclarations {
            animations,
            transitions,
        }
    }

    /// Cancel all animations for set at the given key.
    pub fn cancel_all_animations_for_key(&self, key: &AnimationSetKey) {
        if let Some(set) = self.sets.write().get_mut(key) {
            set.cancel_all_animations();
        }
    }
}

/// Kick off any new transitions for this node and return all of the properties that are
/// transitioning. This is at the end of calculating style for a single node.
pub fn start_transitions_if_applicable(
    context: &SharedStyleContext,
    old_style: &ComputedValues,
    new_style: &Arc<ComputedValues>,
    animation_state: &mut ElementAnimationSet,
    host_animated: &crate::properties::LonghandIdSet,
) -> PropertyDeclarationIdSet {
    // See <https://www.w3.org/TR/css-transitions-1/#transitions>
    // "If a property is specified multiple times in the value of transition-property
    // (either on its own, via a shorthand that contains it, or via the all value),
    // then the transition that starts uses the duration, delay, and timing function
    // at the index corresponding to the last item in the value of transition-property
    // that calls for animating that property."
    // See Example 3 of <https://www.w3.org/TR/css-transitions-1/#transitions>
    //
    // Reversing the transition order here means that transitions defined later in the list
    // have preference, in accordance with the specification.
    //
    // TODO: It would be better to be able to do this without having to allocate an array.
    // We should restructure the code or make `transition_properties()` return a reversible
    // iterator in order to avoid the allocation.
    let mut transition_properties = new_style.transition_properties().collect::<Vec<_>>();
    transition_properties.reverse();

    let mut properties_that_transition = PropertyDeclarationIdSet::default();
    for transition in transition_properties {
        let physical_property = transition
            .property
            .as_borrowed()
            .to_physical(new_style.writing_mode);
        if properties_that_transition.contains(physical_property) {
            continue;
        }

        properties_that_transition.insert(physical_property);
        if matches!(physical_property, PropertyDeclarationId::Longhand(id) if host_animated.contains(id))
        {
            continue;
        }
        animation_state.start_transition_if_applicable(
            context,
            &physical_property,
            transition.index,
            old_style,
            new_style,
        );
    }

    properties_that_transition
}

/// Triggers animations for a given node looking at the animation property
/// values.
pub fn maybe_start_animations<E>(
    element: E,
    context: &SharedStyleContext,
    new_style: &Arc<ComputedValues>,
    cascade_target: AnimationCascadeTarget,
    animation_state: &mut ElementAnimationSet,
    resolver: &mut StyleResolverForElement<E>,
) where
    E: TElement,
{
    let style = new_style.get_ui();
    let mut new_animations = Vec::new();
    for (i, name) in style.animation_name_iter().enumerate() {
        let name = match name.as_atom() {
            Some(atom) => atom,
            None => continue,
        };

        debug!("maybe_start_animations: name={}", name);
        // Web Animations 1 section 4.8.3 gives a zero iteration duration a zero
        // active duration, so the animation still fills with the value of the
        // phase it holds.
        let duration = style.animation_duration_mod(i).seconds() as f64;

        let keyframe_animation = match context.stylist.lookup_keyframes(name, element) {
            Some(animation) => animation,
            None => continue,
        };

        debug!("maybe_start_animations: animation {} found", name);

        // If this animation doesn't have any keyframe, we can just continue
        // without submitting it to the compositor, since both the first and
        // the second keyframes would be synthetised from the computed
        // values.
        if keyframe_animation.steps.is_empty() {
            continue;
        }

        // NB: This delay may be negative, meaning that the animation may be created
        // in a state where we have advanced one or more iterations or even that the
        // animation begins in a finished state.
        let delay = style.animation_delay_mod(i).seconds();

        let iteration_count = style.animation_iteration_count_mod(i);
        let iteration_state = if iteration_count.0.is_infinite() {
            KeyframesIterationState::Infinite(0.0)
        } else {
            KeyframesIterationState::Finite(0.0, iteration_count.0 as f64)
        };

        let animation_direction = style.animation_direction_mod(i);

        let initial_direction = match animation_direction {
            AnimationDirection::Normal | AnimationDirection::Alternate => {
                AnimationDirection::Normal
            },
            AnimationDirection::Reverse | AnimationDirection::AlternateReverse => {
                AnimationDirection::Reverse
            },
        };

        let now = context.current_time_for_animations;
        let started_at = now + delay as f64;
        let starting_progress = (now - started_at) / duration;
        let state = match style.animation_play_state_mod(i) {
            AnimationPlayState::Paused => AnimationState::Paused(starting_progress),
            AnimationPlayState::Running => AnimationState::Pending,
        };

        let computed_steps = ComputedKeyframe::generate_for_keyframes(
            element,
            &keyframe_animation,
            context,
            new_style,
            style.animation_timing_function_mod(i),
            style.animation_composition_mod(i),
            cascade_target,
            resolver,
        );

        let mut new_animation = Animation {
            name: name.clone(),
            properties_changed: keyframe_animation.properties_changed.clone(),
            computed_steps,
            started_at,
            duration,
            fill_mode: style.animation_fill_mode_mod(i),
            delay: delay as f64,
            iteration_state,
            state,
            direction: animation_direction,
            current_direction: initial_direction,
            cascade_style: new_style.clone(),
            is_new: true,
        };

        // If we started with a negative delay, make sure we iterate the animation if
        // the delay moves us past the first iteration.
        new_animation.advance_to_starting_progress(starting_progress);

        new_animations.push(new_animation);
    }
    animation_state.update_animation_list(new_animations, context.current_time_for_animations);
}

#[cfg(test)]
mod tests {
    use super::{
        Animation, AnimationComposition, AnimationDirection, AnimationFillMode, AnimationState,
        ComputedKeyframe, ElementAnimationSet, InterpolateSize, KeyframesIterationState,
    };
    use crate::properties::animated_properties::AnimationValue;
    use crate::properties::style_structs::Font;
    use crate::properties::ComputedValues;
    use crate::values::animated::{Animate, Procedure};
    use crate::Atom;

    #[cfg(feature = "servo")]
    #[test]
    fn shadow_transition_lifecycle_compares_the_whole_list_when_offset_is_unchanged() {
        let stylist = crate::stylist::Stylist::new(crate::test_support::test_device(), selectors::matching::QuirksMode::NoQuirks);
        use crate::properties::longhands;
        use crate::values::animated::box_shadow::AnimatedBoxShadowList;
        use crate::values::computed::{BoxShadowOffset, BoxShadowPosition, Length};
        let mut start = ComputedValues::initial_values_with_font_override(Font::initial_values());
        AnimationValue::BoxShadowOffset(longhands::box_shadow_offset::computed_value::OwnedList(
            [BoxShadowOffset::Offset(
                Length::new(10.0),
                Length::new(10.0),
            )]
            .into_iter()
            .collect(),
        ))
        .set_in_style_for_servo(servo_arc::Arc::make_mut(&mut start), &stylist);
        let mut end = start.clone();
        AnimationValue::BoxShadowOffset(longhands::box_shadow_offset::computed_value::OwnedList(
            [BoxShadowOffset::Offset(
                Length::new(20.0),
                Length::new(20.0),
            )]
            .into_iter()
            .collect(),
        ))
        .set_in_style_for_servo(servo_arc::Arc::make_mut(&mut end), &stylist);
        let mut changed_inset = end.clone();
        AnimationValue::BoxShadowPosition(
            longhands::box_shadow_position::computed_value::OwnedList(
                [BoxShadowPosition::Inset].into_iter().collect(),
            ),
        )
        .set_in_style_for_servo(servo_arc::Arc::make_mut(&mut changed_inset), &stylist);
        let from = AnimatedBoxShadowList::from_computed_values(&start, None);
        let to = AnimatedBoxShadowList::from_computed_values(&end, None);
        let inset = AnimatedBoxShadowList::from_computed_values(&changed_inset, None);
        let offset = |value: &AnimatedBoxShadowList| value.components().into_iter().nth(1).unwrap();
        let transition = super::Transition::new(
            0.0,
            0.0,
            1.0,
            offset(&from),
            offset(&to),
            &super::TimingFunction::Keyword(crate::values::generics::easing::TimingKeyword::Linear),
            InterpolateSize::NumericOnly,
            Some((from.clone(), to.clone())),
        );
        assert_eq!(offset(&to), offset(&inset));
        assert!(transition.end_value_matches(&offset(&to), Some(&to)));
        assert!(!transition.end_value_matches(&offset(&inset), Some(&inset)));
        assert!(transition
            .current_shadow(0.5)
            .unwrap()
            .animate(&inset, Procedure::Interpolate { progress: 0.5 })
            .is_err());
        assert!(transition.reversing_start_matches(&offset(&from), Some(&from)));
        let mut changed_start = start.clone();
        AnimationValue::BoxShadowPosition(
            longhands::box_shadow_position::computed_value::OwnedList(
                [BoxShadowPosition::Inset].into_iter().collect(),
            ),
        )
        .set_in_style_for_servo(servo_arc::Arc::make_mut(&mut changed_start), &stylist);
        let false_reverse = AnimatedBoxShadowList::from_computed_values(&changed_start, None);
        assert_eq!(offset(&from), offset(&false_reverse));
        assert!(!transition.reversing_start_matches(&offset(&false_reverse), Some(&false_reverse)));
    }

    #[cfg(feature = "servo")]
    #[test]
    fn coordinated_shadow_transition_samples_transparent_padding_for_every_component() {
        let stylist = crate::stylist::Stylist::new(crate::test_support::test_device(), selectors::matching::QuirksMode::NoQuirks);
        use crate::properties::{longhands, LonghandId, PropertyDeclarationId};
        use crate::values::animated::box_shadow::AnimatedBoxShadowList;
        use crate::values::computed::{BoxShadowOffset, Length};
        let initial = ComputedValues::initial_values_with_font_override(Font::initial_values());
        let mut target = initial.clone();
        AnimationValue::BoxShadowOffset(longhands::box_shadow_offset::computed_value::OwnedList(
            [BoxShadowOffset::Offset(
                Length::new(20.0),
                Length::new(40.0),
            )]
            .into_iter()
            .collect(),
        ))
        .set_in_style_for_servo(servo_arc::Arc::make_mut(&mut target), &stylist);
        let from = AnimatedBoxShadowList::from_computed_values(&initial, None);
        let to = AnimatedBoxShadowList::from_computed_values(&target, None);
        let expected = from
            .animate(&to, Procedure::Interpolate { progress: 0.25 })
            .unwrap();
        for ((from_value, to_value), expected) in from
            .components()
            .into_iter()
            .zip(to.components())
            .zip(expected.components())
        {
            let animation = super::PropertyAnimation {
                from: from_value,
                to: to_value,
                timing_function: super::TimingFunction::Keyword(
                    crate::values::generics::easing::TimingKeyword::Linear,
                ),
                duration: 1.0,
                interpolate_size: InterpolateSize::NumericOnly,
                shadow: Some((from.clone(), to.clone())),
            };
            assert_eq!(
                animation.calculate_value(0.25, super::BeforeFlag::Unset),
                expected
            );
        }
        let values = expected.components();
        let color = values
            .iter()
            .find(|value| value.id() == PropertyDeclarationId::Longhand(LonghandId::BoxShadowColor))
            .unwrap();
        assert_ne!(color, &to.components()[0]);
    }

    #[test]
    fn transition_identity_survives_cloning_and_seeking_but_not_replacement() {
        let create = || {
            super::Transition::new(
                0.,
                0.,
                1.,
                AnimationValue::Opacity(0.),
                AnimationValue::Opacity(1.),
                &super::TimingFunction::Keyword(
                    crate::values::generics::easing::TimingKeyword::Linear,
                ),
                InterpolateSize::NumericOnly,
                #[cfg(feature = "servo")]
                None,
            )
        };
        let original = create();
        let mut sampled = original.clone();
        sampled.start_time = 2.;
        assert_eq!(sampled.id(), original.id());
        assert_ne!(create().id(), original.id());
    }

    fn pending_animation() -> Animation {
        Animation {
            name: Atom::from("short"),
            properties_changed: Default::default(),
            computed_steps: Box::default(),
            started_at: 0.,
            duration: 0.001,
            delay: 0.,
            fill_mode: AnimationFillMode::None,
            iteration_state: KeyframesIterationState::Finite(0., 1.),
            state: AnimationState::Pending,
            direction: AnimationDirection::Normal,
            current_direction: AnimationDirection::Normal,
            cascade_style: ComputedValues::initial_values_with_font_override(Font::initial_values()),
            is_new: false,
        }
    }

    fn named_animation(name: &str, started_at: f64, duration: f64) -> Animation {
        Animation {
            name: Atom::from(name),
            started_at,
            duration,
            ..pending_animation()
        }
    }

    #[test]
    fn changing_animation_delay_preserves_playback_time_and_moves_active_progress() {
        let mut existing = named_animation("colorize", 10.0, 100000.0);
        existing.state = AnimationState::Running;
        let mut updated = named_animation("colorize", 20.0 - 50000.0, 100000.0);
        updated.delay = -50000.0;
        existing.update_from_other(&updated, 20.0);
        assert_eq!(existing.started_at, 10.0 - 50000.0);
        assert_eq!(existing.state, AnimationState::Running);
        assert_eq!((20.0 - existing.started_at) / existing.duration, 0.5001);
        updated.delay = 100.0;
        existing.update_from_other(&updated, 20.0);
        assert_eq!(existing.started_at, 110.0);
    }

    #[test]
    fn changing_delay_on_a_paused_animation_adjusts_its_held_active_time() {
        let mut existing = named_animation("paused", 10.0, 100.0);
        existing.state = AnimationState::Paused(0.25);
        let mut updated = named_animation("paused", 1000.0, 100.0);
        updated.delay = -50.0;
        updated.state = AnimationState::Paused(0.0);
        existing.update_from_other(&updated, 1000.0);
        assert_eq!(existing.state, AnimationState::Paused(0.75));
        updated.state = AnimationState::Pending;
        existing.update_from_other(&updated, 2000.0);
        assert_eq!(existing.started_at, 1925.0);
    }

    #[test]
    fn animation_name_repetition_creates_distinct_effects() {
        let mut set = ElementAnimationSet::default();
        set.update_animation_list(
            vec![
                named_animation("repeat", 10., 0.),
                named_animation("repeat", 10., 2.),
            ],
            10.,
        );
        assert_eq!(set.animations.len(), 2);
        assert_eq!(set.animations[0].duration, 0.);
        assert_eq!(set.animations[1].duration, 2.);
    }

    #[test]
    fn animation_name_repetition_matches_old_effects_from_the_end() {
        let mut set = ElementAnimationSet {
            animations: vec![named_animation("repeat", 1., 4.)],
            ..Default::default()
        };
        set.update_animation_list(
            vec![
                named_animation("repeat", 10., 3.),
                named_animation("repeat", 10., 2.),
            ],
            10.,
        );
        assert_eq!(set.animations.len(), 2);
        assert_eq!(set.animations[0].started_at, 10.);
        assert_eq!(set.animations[1].started_at, 1.);
        assert_eq!(set.animations[0].duration, 3.);
        assert_eq!(set.animations[1].duration, 2.);
    }

    #[test]
    fn animation_name_matching_keeps_order_and_cancels_unmatched_effects() {
        let mut set = ElementAnimationSet {
            animations: vec![
                named_animation("repeat", 1., 4.),
                named_animation("other", 2., 4.),
                named_animation("repeat", 3., 4.),
            ],
            ..Default::default()
        };
        set.update_animation_list(
            vec![
                named_animation("other", 10., 2.),
                named_animation("repeat", 10., 0.),
            ],
            10.,
        );
        assert_eq!(set.animations.len(), 3);
        assert_eq!(set.animations[0].name, Atom::from("other"));
        assert_eq!(set.animations[0].started_at, 2.);
        assert_eq!(set.animations[1].started_at, 3.);
        assert_eq!(set.animations[1].duration, 0.);
        assert_eq!(set.animations[2].started_at, 1.);
        assert_eq!(set.animations[2].state, AnimationState::Canceled);
        set.update_animation_list(Vec::new(), 11.);
        assert!(set
            .animations
            .iter()
            .all(|animation| animation.state == AnimationState::Canceled));
    }

    #[test]
    fn pending_animation_ends_at_its_active_interval_boundary() {
        let animation = pending_animation();

        assert!(!animation.has_ended(0.));
        assert!(animation.has_ended(0.05));
    }

    /// Web Animations 1 section 4.8.3: an animation whose iteration duration is
    /// zero holds the phase after its active interval from its start.
    #[test]
    fn a_zero_duration_animation_ends_where_it_starts() {
        let mut animation = pending_animation();
        animation.duration = 0.;

        assert!(!animation.has_ended(-1.));
        assert!(animation.has_ended(0.));
        assert!(animation.has_ended(1.));
    }

    #[test]
    fn composites_keyframe_values_with_the_underlying_value() {
        let underlying = AnimationValue::Opacity(0.2);
        let value = AnimationValue::Opacity(0.3);

        assert_eq!(
            ComputedKeyframe::composite(
                AnimationComposition::Replace,
                &underlying,
                &value,
                InterpolateSize::NumericOnly,
            ),
            value,
        );
        assert_eq!(
            ComputedKeyframe::composite(
                AnimationComposition::Add,
                &underlying,
                &value,
                InterpolateSize::NumericOnly,
            ),
            AnimationValue::Opacity(0.5),
        );
        assert_eq!(
            ComputedKeyframe::composite(
                AnimationComposition::Accumulate,
                &underlying,
                &value,
                InterpolateSize::NumericOnly,
            ),
            AnimationValue::Opacity(0.5),
        );
    }

    #[test]
    fn positive_fragment_line_counts_interpolate_as_integers() {
        let interpolate = Procedure::Interpolate { progress: 0.3 };

        assert_eq!(
            AnimationValue::Orphans(10)
                .animate(&AnimationValue::Orphans(20), interpolate)
                .expect("orphans values must interpolate"),
            AnimationValue::Orphans(13),
        );
        assert_eq!(
            AnimationValue::Widows(10)
                .animate(&AnimationValue::Widows(20), interpolate)
                .expect("widows values must interpolate"),
            AnimationValue::Widows(13),
        );
    }

    /// The stepping loop that `iterations_skipped_by_delay` replaced, kept as
    /// the oracle. `max` is `None` for an infinite iteration count.
    fn stepped_reference(starting_progress: f64, max: Option<f64>) -> f64 {
        let mut progress = starting_progress;
        let mut current = 0f64;
        // `Animation::on_last_iteration()`: `current >= max - 1` for a finite
        // count, always false for an infinite one.
        while progress > 1. && !max.is_some_and(|max| current >= max - 1.) {
            current += 1.;
            progress -= 1.;
        }
        current
    }

    fn skipped(starting_progress: f64, max: Option<f64>) -> f64 {
        let iterations_left = match max {
            Some(max) => (max - 1.).ceil().max(0.),
            None => f64::INFINITY,
        };
        Animation::iterations_skipped_by_delay(starting_progress, iterations_left)
    }

    #[test]
    fn matches_stepping_over_tractable_inputs() {
        let progresses = [
            0., 0.5, 1., 1.000_001, 1.5, 2., 2.5, 3., 3.5, 7., 7.25, 16., 33.75,
        ];
        let counts = [
            None,
            Some(1.),
            Some(1.5),
            Some(2.),
            Some(3.),
            Some(3.5),
            Some(8.),
            Some(64.),
        ];

        for progress in progresses {
            for max in counts {
                assert_eq!(
                    skipped(progress, max),
                    stepped_reference(progress, max),
                    "progress {progress}, iteration count {max:?}",
                );
            }
        }
    }

    #[test]
    fn resolves_unbounded_delay_in_closed_form() {
        // `animation: 1s -1e16s infinite`, which the stepping loop could never
        // finish: past 2^53 its `progress -= 1.` is a no-op.
        let iterations = skipped(1e16, None);
        assert!(
            iterations.is_finite() && iterations > 0.,
            "expected a finite skip, got {iterations}",
        );
        assert!(
            iterations <= 1e16,
            "skip {iterations} may not exceed the progress it came from",
        );
    }

    #[test]
    fn caps_an_unbounded_delay_at_a_finite_iteration_count() {
        // A finite count bounds the skip however large the delay is; stepping
        // stopped on the last iteration for the same reason.
        assert_eq!(skipped(1e16, Some(4.)), 3.);
        assert_eq!(skipped(1e16, Some(1.)), 0.);
    }

    #[test]
    fn folds_an_unbounded_progress_against_an_infinite_count() {
        // Stepping had no answer here at all - it would run forever - so there
        // is no oracle to match, only a defensible fold.
        assert_eq!(skipped(f64::INFINITY, None), 0.);
        assert_eq!(skipped(f64::NAN, None), 0.);
        assert_eq!(skipped(f64::NEG_INFINITY, None), 0.);
    }

    #[test]
    fn caps_a_non_finite_progress_at_a_finite_iteration_count() {
        // Stepping does terminate once a count bounds it, so the oracle still
        // applies however degenerate the progress is.
        for progress in [f64::INFINITY, f64::NAN, f64::NEG_INFINITY] {
            assert_eq!(
                skipped(progress, Some(4.)),
                stepped_reference(progress, Some(4.)),
                "progress {progress}",
            );
        }
    }

    /// Ascending keyframe offsets for `from { } 50% { } to { }`.
    const OFFSETS: [f32; 3] = [0., 0.5, 1.];

    fn bracketing(directed_progress: f64) -> (usize, Option<usize>) {
        Animation::bracketing_keyframes(OFFSETS.into_iter(), directed_progress)
    }

    #[test]
    fn brackets_a_progress_between_two_keyframes() {
        assert_eq!(bracketing(0.25), (0, Some(1)));
        assert_eq!(bracketing(0.75), (1, Some(2)));
    }

    #[test]
    fn brackets_a_progress_sitting_on_a_keyframe() {
        // Web Animations 1 section 5.3.1: the interval starts at the keyframe the progress lands on.
        assert_eq!(bracketing(0.5), (1, Some(2)));
    }

    #[test]
    fn brackets_the_ends_of_the_animation() {
        assert_eq!(bracketing(0.), (0, Some(1)));
        assert_eq!(bracketing(1.), (1, Some(2)));
    }

    #[test]
    fn a_step_easing_applies_at_the_start_of_its_interval() {
        use crate::values::computed::easing::ComputedTimingFunction;
        use crate::values::generics::easing::{BeforeFlag, StepPosition};

        let animation = super::PropertyAnimation {
            from: AnimationValue::Opacity(1.),
            to: AnimationValue::Opacity(0.5),
            timing_function: ComputedTimingFunction::Steps(1, StepPosition::JumpStart),
            duration: 1.,
            interpolate_size: InterpolateSize::NumericOnly,
            #[cfg(feature = "servo")]
            shadow: None,
        };
        assert_eq!(
            animation.calculate_value(0., BeforeFlag::Unset),
            AnimationValue::Opacity(0.5)
        );
        assert_eq!(
            animation.calculate_value(0., BeforeFlag::Set),
            AnimationValue::Opacity(1.)
        );
    }
}
