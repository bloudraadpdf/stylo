/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed types for box properties.

use crate::derives::*;
use crate::values::animated::{Animate, Procedure, ToAnimatedValue};
use crate::values::computed::length::{FiniteLength, Length, LengthPercentage, NonNegativeLength};
use crate::values::computed::{Context, Integer, Number, ToComputedValue};
use crate::values::generics::box_::{
    GenericBaselineShift, GenericContainIntrinsicSize, GenericFloat, GenericLineClamp,
    GenericOverflowClipMargin, GenericPerspective, GenericSnapBlock, GenericSnapInline,
};
use crate::values::specified::box_ as specified;
use crate::values::resolved::{Context as ResolvedContext, ToResolvedValue};
use std::fmt::{self, Write};
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

/// A computed `float-offset` represented solely by two bounded finite
/// endpoints. The raw calculation tree cannot inhabit computed style, so NaN
/// and infinity have no representation after this boundary.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToTyped)]
#[repr(C)]
pub struct FloatOffset {
    at_zero_basis: FiniteLength,
    at_hundred_px_basis: FiniteLength,
}

impl FloatOffset {
    /// The initial zero offset.
    #[inline]
    pub fn zero() -> Self {
        Self {
            at_zero_basis: FiniteLength::new_censored(Length::new(0.0)),
            at_hundred_px_basis: FiniteLength::new_censored(Length::new(0.0)),
        }
    }

    /// Resolve against a percentage basis and censor the used value into the
    /// finite computed-length domain.
    #[inline]
    pub fn resolve_finite(&self, percentage_basis: FiniteLength) -> FiniteLength {
        let length = self.at_zero_basis.px() as f64;
        let percentage_points =
            self.at_hundred_px_basis.px() as f64 - self.at_zero_basis.px() as f64;
        FiniteLength::from_f64_censored(
            length + percentage_basis.px() as f64 * percentage_points / 100.0,
        )
    }

    /// Resolve the affine value at a zero basis. Together with
    /// [`Self::at_hundred_px_basis`], this recovers the length and percentage
    /// components without exposing the raw calculation tree.
    #[inline]
    pub fn at_zero_basis(&self) -> FiniteLength {
        self.at_zero_basis
    }

    /// Resolve the affine value at a 100 CSS-pixel basis.
    #[inline]
    pub fn at_hundred_px_basis(&self) -> FiniteLength {
        self.at_hundred_px_basis
    }
}

impl ToCss for FloatOffset {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        let length = self.at_zero_basis;
        let percentage_points = self.at_hundred_px_basis.px() - length.px();
        if percentage_points == 0.0 {
            return length.to_css(dest);
        }
        if length.px() == 0.0 {
            percentage_points.to_css(dest)?;
            return dest.write_char('%');
        }
        dest.write_str("calc(")?;
        length.to_css(dest)?;
        if percentage_points >= 0.0 {
            dest.write_str(" + ")?;
        } else {
            dest.write_str(" - ")?;
        }
        percentage_points.abs().to_css(dest)?;
        dest.write_str("%)")
    }
}

impl ToResolvedValue for FloatOffset {
    type ResolvedValue = Self;

    fn to_resolved_value(self, context: &ResolvedContext) -> Self::ResolvedValue {
        let percentage_points =
            self.at_hundred_px_basis.px() as f64 - self.at_zero_basis.px() as f64;
        let at_zero_basis = self.at_zero_basis.to_resolved_value(context);
        Self {
            at_zero_basis,
            at_hundred_px_basis: FiniteLength::from_f64_censored(
                at_zero_basis.px() as f64 + percentage_points,
            ),
        }
    }

    #[inline]
    fn from_resolved_value(value: Self::ResolvedValue) -> Self {
        value
    }
}

impl ToComputedValue for specified::FloatOffset {
    type ComputedValue = FloatOffset;

    #[inline]
    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        let computed = self.0.to_computed_value(context);
        FloatOffset {
            at_zero_basis: FiniteLength::new_censored(computed.resolve(Length::new(0.0))),
            at_hundred_px_basis: FiniteLength::new_censored(
                computed.resolve(Length::new(100.0)),
            ),
        }
    }

    #[inline]
    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        let percentage_points =
            computed.at_hundred_px_basis.px() - computed.at_zero_basis.px();
        let value = LengthPercentage::new_length(computed.at_zero_basis.into_length());
        if percentage_points == 0.0 {
            return specified::FloatOffset(ToComputedValue::from_computed_value(&value));
        }
        let value = LengthPercentage::new_calc(
            crate::values::computed::length_percentage::CalcNode::Sum(
                vec![
                    crate::values::computed::length_percentage::CalcNode::Leaf(
                        crate::values::computed::length_percentage::CalcLengthPercentageLeaf::Length(
                            computed.at_zero_basis.into_length(),
                        ),
                    ),
                    crate::values::computed::length_percentage::CalcNode::Leaf(
                        crate::values::computed::length_percentage::CalcLengthPercentageLeaf::Percentage(
                            crate::values::computed::Percentage(percentage_points / 100.0),
                        ),
                    ),
                ]
                .into(),
            ),
            style_traits::values::specified::AllowedNumericType::All,
        );
        specified::FloatOffset(ToComputedValue::from_computed_value(&value))
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
