/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed types for box properties.

use crate::derives::*;
use crate::values::animated::{Animate, Procedure, ToAnimatedValue};
use crate::values::computed::length::{FiniteLength, LengthPercentage, NonNegativeLength};
use crate::values::computed::{Context, Integer, Number, ToComputedValue};
use crate::values::generics::box_::{
    GenericBaselineShift, GenericContainIntrinsicSize, GenericFloat, GenericLineClamp,
    GenericOverflowClipMargin, GenericPerspective, GenericSnapBlock, GenericSnapInline,
};
use crate::values::specified::box_ as specified;
use std::fmt;
use style_traits::{CssWriter, ToCss};

pub use crate::values::generics::box_::{SnapBlockAlignment, SnapInlineAlignment};
pub use crate::values::specified::box_::{
    AlignmentBaseline, Appearance, BaselineSource, BookmarkLevel, BookmarkState, BreakBetween,
    BreakWithin, Clear, Contain, ContainerName, ContainerType, ContentVisibility, Display,
    FloatDefer, FloatReference, FootnoteDisplay, FootnotePolicy, MarginBreak, MarginTrim, Overflow,
    OverflowAnchor, OverscrollBehavior, PositionProperty, ScrollSnapAlign, ScrollSnapAxis,
    ScrollSnapStop, ScrollSnapStrictness, ScrollSnapType, ScrollbarGutter, TouchAction, WillChange,
    WritingModeProperty,
};

/// A computed value for the `float` property.
pub type Float = GenericFloat<FiniteLength>;

/// Closed fold over the semantic operators in a computed `float-offset`.
///
/// Implementors receive values only through these operator callbacks; the
/// private Stylo calculation tree and its raw resolver never escape. Each CSS
/// math operator has a distinct method, preventing a downstream projection
/// from silently treating nonlinear functions as an affine expression.
pub trait FloatOffsetCalculationFold {
    /// The caller's folded output type.
    type Output;

    /// Fold one absolute-length leaf (in computed CSS pixels).
    fn length(&mut self, value: FloatOffsetCalculationScalar) -> Self::Output;
    /// Fold one percentage leaf (stored as a fraction, where `1` is 100%).
    fn percentage(&mut self, value: FloatOffsetCalculationScalar) -> Self::Output;
    /// Fold one dimensionless number leaf.
    fn number(&mut self, value: FloatOffsetCalculationScalar) -> Self::Output;
    /// Fold unary negation.
    fn negate(&mut self, value: Self::Output) -> Self::Output;
    /// Fold multiplicative inversion.
    fn invert(&mut self, value: Self::Output) -> Self::Output;
    /// Fold an n-ary sum.
    fn sum(&mut self, values: Vec<Self::Output>) -> Self::Output;
    /// Fold an n-ary product.
    fn product(&mut self, values: Vec<Self::Output>) -> Self::Output;
    /// Fold `min()`.
    fn min(&mut self, values: Vec<Self::Output>) -> Self::Output;
    /// Fold `max()`.
    fn max(&mut self, values: Vec<Self::Output>) -> Self::Output;
    /// Fold `clamp()`.
    fn clamp(&mut self, min: Self::Output, center: Self::Output, max: Self::Output)
        -> Self::Output;
    /// Fold `round(nearest, …)`.
    fn round_nearest(&mut self, value: Self::Output, step: Self::Output) -> Self::Output;
    /// Fold `round(up, …)`.
    fn round_up(&mut self, value: Self::Output, step: Self::Output) -> Self::Output;
    /// Fold `round(down, …)`.
    fn round_down(&mut self, value: Self::Output, step: Self::Output) -> Self::Output;
    /// Fold `round(to-zero, …)`.
    fn round_to_zero(&mut self, value: Self::Output, step: Self::Output) -> Self::Output;
    /// Fold `mod()`.
    fn modulo(&mut self, dividend: Self::Output, divisor: Self::Output) -> Self::Output;
    /// Fold `rem()`.
    fn remainder(&mut self, dividend: Self::Output, divisor: Self::Output) -> Self::Output;
    /// Fold `hypot()`.
    fn hypot(&mut self, values: Vec<Self::Output>) -> Self::Output;
    /// Fold `abs()`.
    fn abs(&mut self, value: Self::Output) -> Self::Output;
    /// Fold `sign()`.
    fn sign(&mut self, value: Self::Output) -> Self::Output;
}

/// A finite scalar leaf in a computed `float-offset` calculation.
///
/// Construction is private to Stylo. Downstream folds may recover the value,
/// knowing that it is finite (including a semantically significant `-0`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(transparent)]
pub struct FiniteFloatOffsetCalculationScalar(f32);

impl FiniteFloatOffsetCalculationScalar {
    /// Recover the finite scalar.
    #[inline]
    pub const fn get(self) -> f32 {
        self.0
    }
}

/// Exact numeric state of a leaf in a computed `float-offset` calculation.
///
/// CSS Values 4 permits infinities and NaN inside calculations. They remain
/// semantic math values here instead of crossing the crate boundary as raw
/// nonfinite floats that could be mistaken for geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FloatOffsetCalculationScalar {
    /// A finite scalar, with private construction.
    Finite(FiniteFloatOffsetCalculationScalar),
    /// Positive infinity.
    PositiveInfinity,
    /// Negative infinity.
    NegativeInfinity,
    /// Not-a-number.
    NaN,
}

impl FloatOffsetCalculationScalar {
    #[inline]
    pub(crate) fn from_css_number(value: f32) -> Self {
        if value.is_nan() {
            Self::NaN
        } else if value == f32::INFINITY {
            Self::PositiveInfinity
        } else if value == f32::NEG_INFINITY {
            Self::NegativeInfinity
        } else {
            debug_assert!(value.is_finite());
            Self::Finite(FiniteFloatOffsetCalculationScalar(value))
        }
    }
}

/// A computed `float-offset` construct whose used value requires layout
/// context that the closed calculation fold cannot supply.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnsupportedFloatOffsetCalculation {
    /// An `anchor()` function requires anchor-positioning context.
    Anchor,
    /// An `anchor-size()` function requires anchor-positioning context.
    AnchorSize,
}

/// A semantics-preserving computed `float-offset`.
///
/// The calculation tree is private and cannot be unpacked or resolved to a raw
/// length. Every public resolution returns [`FiniteLength`], applying CSS
/// Values 4 top-level censorship after the exact calculation has been
/// evaluated for the caller's percentage basis.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(transparent)]
pub struct FloatOffset(LengthPercentage);

impl FloatOffset {
    /// The initial zero offset.
    #[inline]
    pub fn zero() -> Self {
        use crate::Zero;
        Self(LengthPercentage::zero())
    }

    /// Resolve against a percentage basis and censor the used value into the
    /// finite computed-length domain.
    #[inline]
    pub fn resolve_finite(&self, percentage_basis: FiniteLength) -> FiniteLength {
        FiniteLength::new_censored(self.0.resolve(percentage_basis.into_length()))
    }

    /// Project this offset through a closed semantics-preserving fold.
    ///
    /// A typed error is returned for unresolved anchor functions, which
    /// require an element/layout context unavailable at the computed-style
    /// boundary. No raw calculation node or uncensored geometry is exposed.
    #[inline]
    pub fn fold_calculation<F: FloatOffsetCalculationFold>(
        &self,
        fold: &mut F,
    ) -> Result<F::Output, UnsupportedFloatOffsetCalculation> {
        self.0.fold_float_offset_calculation(fold)
    }
}

impl ToComputedValue for specified::FloatOffset {
    type ComputedValue = FloatOffset;

    #[inline]
    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        FloatOffset(self.0.to_computed_float_offset_value(context))
    }

    #[inline]
    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::FloatOffset(ToComputedValue::from_computed_value(&computed.0))
    }
}

/// A computed payload for `float: snap-block(...)`.
pub type SnapBlock = GenericSnapBlock<FiniteLength>;

/// A computed payload for `float: snap-inline(...)`.
pub type SnapInline = GenericSnapInline<FiniteLength>;

/// A computed value for the `baseline-shift` property.
pub type BaselineShift = GenericBaselineShift<LengthPercentage>;

/// A computed value for the `overflow-clip-margin` property.
pub type OverflowClipMargin = GenericOverflowClipMargin<NonNegativeLength>;

/// A computed value for the `contain-intrinsic-size` property.
pub type ContainIntrinsicSize = GenericContainIntrinsicSize<NonNegativeLength>;

impl ContainIntrinsicSize {
    /// Converts contain-intrinsic-size to auto style.
    pub fn add_auto_if_needed(&self) -> Option<Self> {
        Some(match *self {
            Self::None => Self::AutoNone,
            Self::Length(ref l) => Self::AutoLength(*l),
            Self::AutoNone | Self::AutoLength(..) => return None,
        })
    }
}

/// A computed value for the `line-clamp` property.
pub type LineClamp = GenericLineClamp<Integer>;

impl LineClamp {
    /// Returns the line count when this value is not `none`.
    ///
    /// The returned type cannot represent zero or a negative count.
    #[inline]
    pub fn lines(self) -> Option<crate::values::computed::overflow_4::PositiveLineCount> {
        if self.is_none() {
            None
        } else {
            Some(crate::values::computed::overflow_4::PositiveLineCount::from_legacy(&self))
        }
    }
}

impl Animate for LineClamp {
    #[inline]
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        if self.is_none() != other.is_none() {
            return Err(());
        }
        if self.is_none() {
            return Ok(Self::none());
        }
        let value = self.value().animate(other.value(), procedure)?.max(1);
        Ok(Self::from_positive(
            crate::values::generics::GreaterThanOrEqualToOne(value),
        ))
    }
}

/// A computed value for the `perspective` property.
pub type Perspective = GenericPerspective<NonNegativeLength>;

/// A computed value for the `resize` property.
#[allow(missing_docs)]
#[cfg_attr(feature = "servo", derive(Deserialize, Serialize))]
#[derive(
    Clone, Copy, Debug, Eq, Hash, MallocSizeOf, Parse, PartialEq, ToCss, ToResolvedValue, ToTyped,
)]
#[repr(u8)]
pub enum Resize {
    None,
    Both,
    Horizontal,
    Vertical,
}

impl ToComputedValue for specified::Resize {
    type ComputedValue = Resize;

    #[inline]
    fn to_computed_value(&self, context: &Context) -> Resize {
        let is_vertical = context.style().writing_mode.is_vertical();
        match self {
            specified::Resize::Inline => {
                context
                    .rule_cache_conditions
                    .borrow_mut()
                    .set_writing_mode_dependency(context.builder.writing_mode);
                if is_vertical {
                    Resize::Vertical
                } else {
                    Resize::Horizontal
                }
            },
            specified::Resize::Block => {
                context
                    .rule_cache_conditions
                    .borrow_mut()
                    .set_writing_mode_dependency(context.builder.writing_mode);
                if is_vertical {
                    Resize::Horizontal
                } else {
                    Resize::Vertical
                }
            },
            specified::Resize::None => Resize::None,
            specified::Resize::Both => Resize::Both,
            specified::Resize::Horizontal => Resize::Horizontal,
            specified::Resize::Vertical => Resize::Vertical,
        }
    }

    #[inline]
    fn from_computed_value(computed: &Resize) -> specified::Resize {
        match computed {
            Resize::None => specified::Resize::None,
            Resize::Both => specified::Resize::Both,
            Resize::Horizontal => specified::Resize::Horizontal,
            Resize::Vertical => specified::Resize::Vertical,
        }
    }
}

/// The computed `zoom` property value.
#[derive(
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    PartialOrd,
    ToResolvedValue,
    ToTyped,
)]
#[cfg_attr(feature = "servo", derive(Deserialize, Serialize))]
#[repr(C)]
pub struct Zoom(f32);

impl ToComputedValue for specified::Zoom {
    type ComputedValue = Zoom;

    #[inline]
    fn to_computed_value(&self, _: &Context) -> Self::ComputedValue {
        let n = match *self {
            Self::Normal => return Zoom::ONE,
            Self::Document => return Zoom::DOCUMENT,
            Self::Value(ref n) => n.0.to_number().get(),
        };
        if n == 0.0 {
            // For legacy reasons, zoom: 0 (and 0%) computes to 1. ¯\_(ツ)_/¯
            return Zoom::ONE;
        }
        Zoom(n)
    }

    #[inline]
    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self::new_number(computed.value())
    }
}

impl ToCss for Zoom {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: fmt::Write,
    {
        use std::fmt::Write;
        if *self == Self::DOCUMENT {
            return dest.write_str("document");
        }
        self.value().to_css(dest)
    }
}

impl ToAnimatedValue for Zoom {
    type AnimatedValue = Number;

    #[inline]
    fn to_animated_value(self, _: &crate::values::animated::Context) -> Self::AnimatedValue {
        self.value()
    }

    #[inline]
    fn from_animated_value(animated: Self::AnimatedValue) -> Self {
        Zoom(animated.max(0.0))
    }
}

impl Zoom {
    /// The value 1. This is by far the most common value.
    pub const ONE: Zoom = Zoom(1.0);

    /// The `document` value. This can appear in the computed zoom property value, but not in the
    /// `effective_zoom` field.
    pub const DOCUMENT: Zoom = Zoom(0.0);

    /// Returns whether we're the number 1.
    #[inline]
    pub fn is_one(self) -> bool {
        self == Self::ONE
    }

    /// Returns whether we're the `document` keyword.
    #[inline]
    pub fn is_document(self) -> bool {
        self == Self::DOCUMENT
    }

    /// Returns the inverse of our value.
    #[inline]
    pub fn inverted(&self) -> Option<Self> {
        if self.0 == 0.0 {
            return None;
        }
        Some(Self(1. / self.0))
    }

    /// Returns the value as a float.
    #[inline]
    pub fn value(&self) -> f32 {
        self.0
    }

    /// Computes the effective zoom for a given new zoom value in rhs.
    pub fn compute_effective(self, specified: Self) -> Self {
        if specified == Self::DOCUMENT {
            return Self::ONE;
        }
        if self == Self::ONE {
            return specified;
        }
        if specified == Self::ONE {
            return self;
        }
        Zoom(self.0 * specified.0)
    }

    /// Returns the zoomed value.
    #[inline]
    pub fn zoom(self, value: f32) -> f32 {
        if self == Self::ONE {
            return value;
        }
        value * self.value()
    }

    /// Returns the un-zoomed value.
    #[inline]
    pub fn unzoom(self, value: f32) -> f32 {
        // Avoid division by zero if our effective zoom computation ends up being zero.
        if self == Self::ONE || self.0 == 0.0 {
            return value;
        }
        value / self.value()
    }
}
