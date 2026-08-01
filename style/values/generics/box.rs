/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Generic types for box properties.

use crate::derives::*;
use crate::values::animated::ToAnimatedZero;
use crate::values::CustomIdent;
use crate::Zero;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    FromPrimitive,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
#[allow(missing_docs)]
pub enum BaselineShiftKeyword {
    /// Lower by the offset appropriate for subscripts of the parent’s box. The UA may use the
    /// parent’s font metrics to find this offset; otherwise it defaults to dropping by one
    /// fifth of the parent’s used font-size.
    Sub,
    /// Raise by the offset appropriate for superscripts of the parent’s box. The UA may use the
    /// parent’s font metrics to find this offset; otherwise it defaults to raising by one third
    /// of the parent’s used font-size.
    Super,
    /// Align the line-over edge of the aligned subtree with the line-over edge of the line box.
    Top,
    /// Align the center of the aligned subtree with the center of the line box.
    Center,
    /// Align the line-under edge of the aligned subtree with the line-under edge of the line box.
    Bottom,
}

/// A generic value for the `baseline-shift` property.
/// https://drafts.csswg.org/css-inline-3/#baseline-shift
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum GenericBaselineShift<LengthPercentage> {
    /// One of the baseline-shift keywords
    Keyword(BaselineShiftKeyword),
    /// Raise (positive value) or lower (negative value) by the specified length or specified percentage of the line-height.
    Length(LengthPercentage),
}

pub use self::GenericBaselineShift as BaselineShift;

impl<L: Zero> BaselineShift<L> {
    /// Returns the initial `0` value.
    #[inline]
    pub fn zero() -> Self {
        BaselineShift::Length(Zero::zero())
    }
}

impl<L> ToAnimatedZero for BaselineShift<L> {
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Err(())
    }
}

/// Snap alignment for CSS Page Floats `snap-block()`.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum SnapBlockAlignment {
    /// Snap toward the block-start edge.
    Start,
    /// Snap toward the block-end edge.
    End,
    /// Snap toward the nearest block edge.
    Near,
}

impl ToCss for SnapBlockAlignment {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str(match self {
            Self::Start => "start",
            Self::End => "end",
            Self::Near => "near",
        })
    }
}

/// Line-relative alignment accepted by `snap-inline()`.
///
/// CSS Page Floats 3 intentionally uses `left | right | near` here, not
/// `snap-block()`'s `start | end | near`. Keeping the vocabularies nominally
/// separate prevents a block-axis alignment from entering inline snapping.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum SnapInlineAlignment {
    /// Snap toward line-left.
    Left,
    /// Snap toward line-right.
    Right,
    /// Snap toward the nearer inline edge.
    Near,
}

impl ToCss for SnapInlineAlignment {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        dest.write_str(match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Near => "near",
        })
    }
}

/// Payload for `float: snap-block(...)` — CSS Page Floats 3 section 3.2.
///
/// Supports both the single-threshold form
/// `snap-block(<length>, <alignment>)` and the two-threshold form
/// `snap-block(<start-threshold> <end-threshold>, <alignment>)`.
/// The bare keyword, one-threshold function, and two-threshold function are
/// disjoint variants. End-without-start and alignment-without-threshold have
/// no representation.
#[derive(
    Clone,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
pub enum GenericSnapBlock<LengthPercentage> {
    /// Bare `snap-block`; the used threshold is `2em` and alignment is `near`.
    Default,
    /// Function form with one threshold shared by both edges.
    One {
        /// Shared start/end threshold.
        threshold: LengthPercentage,
        /// Optional authored alignment; absent means `near`.
        alignment: Option<SnapBlockAlignment>,
    },
    /// Function form with independent start/end thresholds.
    Two {
        /// Block-start threshold.
        start: LengthPercentage,
        /// Block-end threshold.
        end: LengthPercentage,
        /// Optional authored alignment; absent means `near`.
        alignment: Option<SnapBlockAlignment>,
    },
}

impl<LengthPercentage: ToCss> ToCss for GenericSnapBlock<LengthPercentage> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Default => dest.write_str("snap-block"),
            Self::One {
                threshold,
                alignment,
            } => {
                dest.write_str("snap-block(")?;
                threshold.to_css(dest)?;
                if let Some(alignment) = alignment {
                    dest.write_str(", ")?;
                    alignment.to_css(dest)?;
                }
                dest.write_char(')')
            },
            Self::Two {
                start,
                end,
                alignment,
            } => {
                dest.write_str("snap-block(")?;
                start.to_css(dest)?;
                dest.write_char(' ')?;
                end.to_css(dest)?;
                if let Some(alignment) = alignment {
                    dest.write_str(", ")?;
                    alignment.to_css(dest)?;
                }
                dest.write_char(')')
            },
        }
    }
}

pub use self::GenericSnapBlock as SnapBlock;

/// Payload for `float: snap-inline(...)` — CSS Page Floats 3 section 3.2.
///
/// Inline-axis analogue of `GenericSnapBlock`. The function form requires a
/// start threshold, accepts an optional independent end threshold, and uses
/// the distinct `left | right | near` alignment vocabulary. End-without-start
/// and alignment-without-threshold have no representation.
#[derive(
    Clone,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
pub enum GenericSnapInline<LengthPercentage> {
    /// Bare `snap-inline`; the used threshold is `2em` and alignment is `near`.
    Default,
    /// Function form with a required threshold.
    One {
        /// Authored inline threshold.
        threshold: LengthPercentage,
        /// Optional line-relative alignment; absent means `near`.
        alignment: Option<SnapInlineAlignment>,
    },
    /// Function form with independent line-start/line-end thresholds.
    Two {
        /// Line-start threshold.
        start: LengthPercentage,
        /// Line-end threshold.
        end: LengthPercentage,
        /// Optional line-relative alignment; absent means `near`.
        alignment: Option<SnapInlineAlignment>,
    },
}

impl<LengthPercentage: ToCss> ToCss for GenericSnapInline<LengthPercentage> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Default => dest.write_str("snap-inline"),
            Self::One {
                threshold,
                alignment,
            } => {
                dest.write_str("snap-inline(")?;
                threshold.to_css(dest)?;
                if let Some(alignment) = alignment {
                    dest.write_str(", ")?;
                    alignment.to_css(dest)?;
                }
                dest.write_char(')')
            },
            Self::Two {
                start,
                end,
                alignment,
            } => {
                dest.write_str("snap-inline(")?;
                start.to_css(dest)?;
                dest.write_char(' ')?;
                end.to_css(dest)?;
                if let Some(alignment) = alignment {
                    dest.write_str(", ")?;
                    alignment.to_css(dest)?;
                }
                dest.write_char(')')
            },
        }
    }
}

pub use self::GenericSnapInline as SnapInline;

/// A generic value for the `float` property.
#[derive(
    Clone,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum GenericFloat<LengthPercentage> {
    Left,
    Right,
    None,
    InlineStart,
    InlineEnd,
    /// CSS Page Floats 3 §3.2.3: logical float value resolving to the outer
    /// (spine-facing away) inline edge — `right` on recto pages, `left` on
    /// verso pages (LTR progression).
    Outside,
    /// CSS Page Floats 3 §3.2.3: logical float value resolving to the inner
    /// (spine-facing) inline edge — `left` on recto pages, `right` on verso
    /// pages (LTR progression).
    Inside,
    BlockStart,
    BlockEnd,
    Footnote,
    Top,
    Bottom,
    /// Bloudraad native fixed physical page-top float. Unlike the standards
    /// `top` value, pagination compatibility heuristics do not demote it to a
    /// natural-flow placement.
    BdTop,
    /// Bloudraad native fixed physical page-bottom float. Unlike the standards
    /// `bottom` value, pagination compatibility heuristics do not demote it to
    /// a natural-flow placement.
    BdBottom,
    TopUnlessRoom,
    BottomUnlessRoom,
    SnapBlock(GenericSnapBlock<LengthPercentage>),
    SnapInline(GenericSnapInline<LengthPercentage>),
}

impl<LengthPercentage> GenericFloat<LengthPercentage> {
    /// Returns true if `self` is not `None`.
    pub fn is_floating(&self) -> bool {
        !matches!(self, Self::None)
    }

    /// Returns true if this is a page float or footnote float.
    pub fn is_page_or_footnote_float(&self) -> bool {
        matches!(
            self,
            Self::BlockStart
                | Self::BlockEnd
                | Self::Footnote
                | Self::Top
                | Self::Bottom
                | Self::BdTop
                | Self::BdBottom
                | Self::TopUnlessRoom
                | Self::BottomUnlessRoom
                | Self::SnapBlock(..)
                | Self::SnapInline(..)
        )
    }
}

impl<LengthPercentage: ToCss> ToCss for GenericFloat<LengthPercentage> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Left => dest.write_str("left"),
            Self::Right => dest.write_str("right"),
            Self::None => dest.write_str("none"),
            Self::InlineStart => dest.write_str("inline-start"),
            Self::InlineEnd => dest.write_str("inline-end"),
            Self::Outside => dest.write_str("outside"),
            Self::Inside => dest.write_str("inside"),
            Self::BlockStart => dest.write_str("block-start"),
            Self::BlockEnd => dest.write_str("block-end"),
            Self::Footnote => dest.write_str("footnote"),
            Self::Top => dest.write_str("top"),
            Self::Bottom => dest.write_str("bottom"),
            Self::BdTop => dest.write_str("-bd-top"),
            Self::BdBottom => dest.write_str("-bd-bottom"),
            Self::TopUnlessRoom => dest.write_str("top-unless-room"),
            Self::BottomUnlessRoom => dest.write_str("bottom-unless-room"),
            Self::SnapBlock(snap_block) => snap_block.to_css(dest),
            Self::SnapInline(snap_inline) => snap_inline.to_css(dest),
        }
    }
}

pub use self::GenericFloat as Float;

/// https://drafts.csswg.org/css-sizing-4/#intrinsic-size-override
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToAnimatedValue,
    ToAnimatedZero,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[value_info(other_values = "auto")]
#[repr(C, u8)]
pub enum GenericContainIntrinsicSize<L> {
    /// The keyword `none`.
    None,
    /// The keywords 'auto none',
    AutoNone,
    /// A non-negative length.
    Length(L),
    /// "auto <Length>"
    AutoLength(L),
}

pub use self::GenericContainIntrinsicSize as ContainIntrinsicSize;

impl<L: ToCss> ToCss for ContainIntrinsicSize<L> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match *self {
            Self::None => dest.write_str("none"),
            Self::AutoNone => dest.write_str("auto none"),
            Self::Length(ref l) => l.to_css(dest),
            Self::AutoLength(ref l) => {
                dest.write_str("auto ")?;
                l.to_css(dest)
            },
        }
    }
}

/// Note that we only implement -webkit-line-clamp as a single, longhand
/// property for now, but the spec defines line-clamp as a shorthand for
/// separate max-lines, block-ellipsis, and continue properties.
///
/// https://drafts.csswg.org/css-overflow-3/#line-clamp
#[derive(
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToAnimatedValue,
    ToAnimatedZero,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(transparent)]
#[value_info(other_values = "none")]
pub struct GenericLineClamp<I>(pub I);

pub use self::GenericLineClamp as LineClamp;

impl<I: Zero> LineClamp<I> {
    /// Returns the `none` value.
    pub fn none() -> Self {
        Self(crate::Zero::zero())
    }

    /// Returns whether we're the `none` value.
    pub fn is_none(&self) -> bool {
        self.0.is_zero()
    }
}

impl<I: Zero + ToCss> ToCss for LineClamp<I> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if self.is_none() {
            return dest.write_str("none");
        }
        self.0.to_css(dest)
    }
}

/// A generic value for the `perspective` property.
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToAnimatedZero,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
#[typed_value(derive_fields)]
pub enum GenericPerspective<NonNegativeLength> {
    /// A non-negative length.
    Length(NonNegativeLength),
    /// The keyword `none`.
    None,
}

pub use self::GenericPerspective as Perspective;

impl<L> Perspective<L> {
    /// Returns `none`.
    #[inline]
    pub fn none() -> Self {
        Perspective::None
    }
}

#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum PositionProperty {
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
    #[css(function = "running")]
    Running(CustomIdent),
}

impl PositionProperty {
    /// Is the box absolutely positioned?
    pub fn is_absolutely_positioned(self) -> bool {
        matches!(self, Self::Absolute | Self::Fixed)
    }

    /// Returns the `running(name)` identifier when present.
    pub fn running_name(&self) -> Option<&CustomIdent> {
        match self {
            Self::Running(name) => Some(name),
            _ => None,
        }
    }
}

/// https://drafts.csswg.org/css-overflow-4/#overflow-clip-margin's <visual-box>. Note that the
/// spec has special behavior for the omitted keyword, but that's rather odd, see:
/// https://github.com/w3c/csswg-drafts/issues/13185
#[allow(missing_docs)]
#[derive(
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    Parse,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum OverflowClipMarginBox {
    ContentBox,
    PaddingBox,
    BorderBox,
}

/// https://drafts.csswg.org/css-overflow-4/#overflow-clip-margin
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToAnimatedZero,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C)]
pub struct GenericOverflowClipMargin<L> {
    /// The offset of the clip.
    pub offset: L,
    /// The box that we're clipping to.
    #[animation(constant)]
    pub visual_box: OverflowClipMarginBox,
}

pub use self::GenericOverflowClipMargin as OverflowClipMargin;

impl<L: Zero> GenericOverflowClipMargin<L> {
    /// Returns the `none` value.
    pub fn zero() -> Self {
        Self {
            offset: Zero::zero(),
            visual_box: OverflowClipMarginBox::PaddingBox,
        }
    }
}

impl<L: Zero + ToCss> ToCss for OverflowClipMargin<L> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if self.visual_box == OverflowClipMarginBox::PaddingBox {
            return self.offset.to_css(dest);
        }
        self.visual_box.to_css(dest)?;
        if !self.offset.is_zero() {
            dest.write_char(' ')?;
            self.offset.to_css(dest)?;
        }
        Ok(())
    }
}
