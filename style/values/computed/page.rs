/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed @page at-rule properties and named-page style properties

use crate::derives::*;
use crate::values::computed::length::{Length, NonNegativeLength};
use crate::values::computed::{Context, ToComputedValue};
use crate::values::generics;
use crate::values::generics::size::Size2D;

use crate::values::specified::page as specified;
pub use generics::page::GenericPageSize;
pub use generics::page::PageMarks;
pub use generics::page::PageOrientation;
pub use generics::page::PageSizeOrientation;
pub use generics::page::PaperSize;
pub use specified::PageName;

/// Per-side computed bleed lengths (top, right, bottom, left).
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BleedSides {
    /// Top edge.
    pub top: Length,
    /// Right edge.
    pub right: Length,
    /// Bottom edge.
    pub bottom: Length,
    /// Left edge.
    pub left: Length,
}

/// Computed value of the `bleed` page descriptor.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum Bleed {
    /// `auto`
    Auto,
    /// Non-negative computed length (applied to all four edges).
    Length(Length),
    /// Per-side computed lengths.
    Sides(BleedSides),
}

impl Bleed {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether this is the `auto` value.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl style_traits::ToCss for Bleed {
    fn to_css<W>(&self, dest: &mut style_traits::CssWriter<W>) -> std::fmt::Result
    where
        W: std::fmt::Write,
    {
        use std::fmt::Write as _;
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Length(l) => l.to_css(dest),
            Self::Sides(BleedSides {
                top,
                right,
                bottom,
                left,
            }) => {
                top.to_css(dest)?;
                dest.write_char(' ')?;
                right.to_css(dest)?;
                if bottom != top || left != right {
                    dest.write_char(' ')?;
                    bottom.to_css(dest)?;
                    if left != right {
                        dest.write_char(' ')?;
                        left.to_css(dest)?;
                    }
                }
                Ok(())
            },
        }
    }
}

impl ToComputedValue for specified::Bleed {
    type ComputedValue = Bleed;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::Bleed::Auto => Bleed::Auto,
            specified::Bleed::Length(length) => {
                Bleed::Length(length.to_computed_value(ctx).clamp_to_non_negative())
            },
            specified::Bleed::Sides(s) => Bleed::Sides(BleedSides {
                top: s.top.to_computed_value(ctx).clamp_to_non_negative(),
                right: s.right.to_computed_value(ctx).clamp_to_non_negative(),
                bottom: s.bottom.to_computed_value(ctx).clamp_to_non_negative(),
                left: s.left.to_computed_value(ctx).clamp_to_non_negative(),
            }),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            Bleed::Auto => specified::Bleed::Auto,
            Bleed::Length(length) => {
                specified::Bleed::Length(ToComputedValue::from_computed_value(length))
            },
            Bleed::Sides(s) => specified::Bleed::Sides(specified::BleedSides {
                top: ToComputedValue::from_computed_value(&s.top),
                right: ToComputedValue::from_computed_value(&s.right),
                bottom: ToComputedValue::from_computed_value(&s.bottom),
                left: ToComputedValue::from_computed_value(&s.left),
            }),
        }
    }
}

/// Computed value of the @page size descriptor
///
/// The spec says that the computed value should be the same as the specified
/// value but with all absolute units, but it's not currently possibly observe
/// the computed value of page-size.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum PageSize {
    /// Specified size, paper size, or paper size and orientation.
    Size(Size2D<NonNegativeLength>),
    /// `landscape` or `portrait` value, no specified size.
    Orientation(PageSizeOrientation),
    /// `auto` value
    Auto,
}

impl ToComputedValue for specified::PageSize {
    type ComputedValue = PageSize;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match &*self {
            Self::Size(s) => PageSize::Size(s.to_computed_value(ctx)),
            Self::PaperSize(p, PageSizeOrientation::Landscape) => PageSize::Size(Size2D {
                width: p.long_edge().to_computed_value(ctx),
                height: p.short_edge().to_computed_value(ctx),
            }),
            Self::PaperSize(p, PageSizeOrientation::Portrait) => PageSize::Size(Size2D {
                width: p.short_edge().to_computed_value(ctx),
                height: p.long_edge().to_computed_value(ctx),
            }),
            Self::Orientation(o) => PageSize::Orientation(*o),
            Self::Auto => PageSize::Auto,
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match *computed {
            PageSize::Size(s) => Self::Size(ToComputedValue::from_computed_value(&s)),
            PageSize::Orientation(o) => Self::Orientation(o),
            PageSize::Auto => Self::Auto,
        }
    }
}

impl PageSize {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        PageSize::Auto
    }

    /// Whether this is the `auto` value.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(*self, PageSize::Auto)
    }
}
