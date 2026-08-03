/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe baseline-grid / line-snap CSS surface (F8).
//!
//! CSS parse surface only — the layout pass that snaps line boxes
//! to a grid is tracked separately. See
//! `docs/audits/CSS-COVERAGE-AUDIT-2026-05-14/stylo-push-plan.md`
//! family 8 for source vendors (`-ro-line-grid`, `-ro-line-snap`,
//! `-prince-baseline-grid`, `-prince-line-stacking-strategy`).

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::length::Length;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value of `-bd-line-grid`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdLineGrid {
    #[default]
    None,
    MatchParent,
    Create,
}

/// Specified value of `-bd-line-snap`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdLineSnap {
    #[default]
    None,
    Baseline,
    Contain,
}

/// Specified value of `-bd-baseline-grid`.
///
/// Prince spelling for the explicit baseline grid step size.
/// `none | auto | <length>`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdBaselineGrid {
    /// `none` — no baseline grid.
    None,
    /// `auto` — derive grid step from `line-height`.
    Auto,
    /// `<length>` — explicit grid step.
    Length(Length),
}

impl BdBaselineGrid {
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

impl Parse for BdBaselineGrid {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        if input.try_parse(|i| i.expect_ident_matching("auto")).is_ok() {
            return Ok(Self::Auto);
        }
        Ok(Self::Length(Length::parse(context, input)?))
    }
}

/// Specified value of `-bd-line-stacking-strategy`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdLineStackingStrategy {
    #[default]
    InlineLineHeight,
    BlockLineHeight,
    MaxHeight,
    GridHeight,
}
