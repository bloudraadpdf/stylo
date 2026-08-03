/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-pdf-comment*` and `-bd-pdf-link-border`.

use crate::derives::*;
use crate::values::computed::color::Color;
use crate::values::computed::length::NonNegativeLength;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_pdf_comment as specified;

pub use specified::{
    BdPdfCommentAuthor, BdPdfCommentDate, BdPdfCommentDateFormat, BdPdfCommentIcon,
    BdPdfCommentKind, BdPdfCommentOpen, BdPdfCommentPosition, BdPdfCommentState,
    BdPdfCommentStateModel, BdPdfCommentString, BdPdfCommentSubject,
};

/// Computed value of `-bd-pdf-comment-colour`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfCommentColour {
    /// `auto` — viewer default.
    Auto,
    /// `<color>` — explicit annotation `/C` array.
    Colour(Color),
}

impl BdPdfCommentColour {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToComputedValue for specified::BdPdfCommentColour {
    type ComputedValue = BdPdfCommentColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfCommentColour::Auto => BdPdfCommentColour::Auto,
            specified::BdPdfCommentColour::Colour(c) => {
                BdPdfCommentColour::Colour(c.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfCommentColour::Auto => specified::BdPdfCommentColour::Auto,
            BdPdfCommentColour::Colour(c) => {
                specified::BdPdfCommentColour::Colour(ToComputedValue::from_computed_value(c))
            },
        }
    }
}

/// Computed value of `-bd-pdf-link-border`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfLinkBorder {
    /// `none` — no border.
    None,
    /// `<length> <color>` — explicit border.
    Border {
        /// Border width (non-negative).
        width: NonNegativeLength,
        /// Border colour.
        colour: Color,
    },
}

impl BdPdfLinkBorder {
    /// `none` value.
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToComputedValue for specified::BdPdfLinkBorder {
    type ComputedValue = BdPdfLinkBorder;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfLinkBorder::None => BdPdfLinkBorder::None,
            specified::BdPdfLinkBorder::Border { width, colour } => BdPdfLinkBorder::Border {
                width: width.to_computed_value(ctx),
                colour: colour.to_computed_value(ctx),
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfLinkBorder::None => specified::BdPdfLinkBorder::None,
            BdPdfLinkBorder::Border { width, colour } => specified::BdPdfLinkBorder::Border {
                width: ToComputedValue::from_computed_value(width),
                colour: ToComputedValue::from_computed_value(colour),
            },
        }
    }
}

/// Computed value of `-bd-pdf-link-border-style`. Re-exports the
/// specified enum — the keyword set is identity-mapped.
pub use specified::BdPdfLinkBorderStyle;

/// Computed value of `-bd-pdf-link-area`. Re-exports the
/// specified enum — the keyword set is identity-mapped.
pub use specified::BdPdfLinkArea;

/// Computed value of `-bd-pdf-link-border-color`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfLinkBorderColor {
    /// `auto` — defer to the shorthand or viewer default.
    Auto,
    /// Explicit colour value.
    Colour(Color),
}

impl BdPdfLinkBorderColor {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToComputedValue for specified::BdPdfLinkBorderColor {
    type ComputedValue = BdPdfLinkBorderColor;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfLinkBorderColor::Auto => BdPdfLinkBorderColor::Auto,
            specified::BdPdfLinkBorderColor::Colour(c) => {
                BdPdfLinkBorderColor::Colour(c.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfLinkBorderColor::Auto => specified::BdPdfLinkBorderColor::Auto,
            BdPdfLinkBorderColor::Colour(c) => {
                specified::BdPdfLinkBorderColor::Colour(ToComputedValue::from_computed_value(c))
            },
        }
    }
}

/// Computed value of `-bd-pdf-link-border-width`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfLinkBorderWidth {
    /// `auto` — defer to the shorthand or `0` default.
    Auto,
    /// Explicit length value (non-negative).
    Length(NonNegativeLength),
}

impl BdPdfLinkBorderWidth {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToComputedValue for specified::BdPdfLinkBorderWidth {
    type ComputedValue = BdPdfLinkBorderWidth;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfLinkBorderWidth::Auto => BdPdfLinkBorderWidth::Auto,
            specified::BdPdfLinkBorderWidth::Length(l) => {
                BdPdfLinkBorderWidth::Length(l.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfLinkBorderWidth::Auto => specified::BdPdfLinkBorderWidth::Auto,
            BdPdfLinkBorderWidth::Length(l) => {
                specified::BdPdfLinkBorderWidth::Length(ToComputedValue::from_computed_value(l))
            },
        }
    }
}
