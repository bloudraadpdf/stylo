/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Rhythmic Sizing Module Level 1 longhands.
//!
//! Implements:
//!
//! - `block-step-size` — <https://drafts.csswg.org/css-rhythm-1/#block-step-size>
//! - `block-step-insert` — <https://drafts.csswg.org/css-rhythm-1/#block-step-insert>
//! - `block-step-align` — <https://drafts.csswg.org/css-rhythm-1/#block-step-align>
//! - `block-step-round` — <https://drafts.csswg.org/css-rhythm-1/#block-step-round>
//!
//! The shorthand `block-step` is wired in `style/properties/shorthands.rs`.
//!
//! All four properties cascade through the `box` style struct because
//! they affect block-axis box sizing rather than text run shaping.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::NonNegativeLength;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// Specified value of the `block-step-size` property
/// (<https://drafts.csswg.org/css-rhythm-1/#block-step-size>).
///
/// `none` (initial) — the box does not participate in block-step
/// sizing. `<length>` — the box's block-axis size is quantised to a
/// multiple of this length.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BlockStepSize {
    /// `none` — initial; opt-out of block-step sizing.
    None,
    /// `<length>` — quantum the block-axis size is rounded to.
    Length(NonNegativeLength),
}

impl BlockStepSize {
    /// Initial value (`none`).
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

impl ToCss for BlockStepSize {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Length(l) => l.to_css(dest),
        }
    }
}

impl Parse for BlockStepSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let length = NonNegativeLength::parse(context, input)?;
        Ok(Self::Length(length))
    }
}

/// Specified value of the `block-step-insert` property
/// (<https://drafts.csswg.org/css-rhythm-1/#block-step-insert>).
///
/// Selects which box edge the rounded extra space is added to.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BlockStepInsert {
    /// Extra space is added to the margin edges — the initial value.
    #[default]
    MarginBox,
    /// Extra space is added to the padding edges.
    PaddingBox,
    /// Extra space is added to the content edges.
    ContentBox,
}

/// Specified value of the `block-step-align` property
/// (<https://drafts.csswg.org/css-rhythm-1/#block-step-align>).
///
/// Selects how the block-axis content is aligned within the quantised
/// box.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BlockStepAlign {
    /// `auto` — UA decides; equivalent to `center` for most engines.
    #[default]
    Auto,
    /// Centre the content within the quantised block.
    Center,
    /// Align content to the block-start edge.
    Start,
    /// Align content to the block-end edge.
    End,
}

/// Specified value of the `block-step-round` property
/// (<https://drafts.csswg.org/css-rhythm-1/#block-step-round>).
///
/// Selects the rounding mode used when the box's intrinsic size is
/// quantised to a multiple of `block-step-size`.
#[repr(u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum BlockStepRound {
    /// Round up — the initial value; never crop intrinsic content.
    #[default]
    Up,
    /// Round down — may crop intrinsic content.
    Down,
    /// Round to the nearest multiple.
    Nearest,
}
