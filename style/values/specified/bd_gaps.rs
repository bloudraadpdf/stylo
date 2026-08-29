/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe smaller standard-CSS gap fillers (F21).
//!
//! Properties in this module are either Stylo upstream additions
//! that the fork ungated for non-Gecko engines (e.g. `overlay`) or
//! Prince proprietary surface admitted as a native `-bd-*` keyword
//! (`border-clip`). The CSS-standard names are kept where possible;
//! Prince aliases live in the moegoe-css compat translator.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::{self, Context, ToComputedValue};
use crate::values::generics::gap::{
    GapRuleList as GenericGapRuleList, GapRuleListItem, GapRuleRepeatCount,
};
use crate::values::specified::Integer;
use crate::values::specified::{BorderSideWidth, BorderStyle, Color};
use cssparser::Parser;
use style_traits::{ParseError, StyleParseErrorKind};

/// A specified list for a gap-decoration color, style, or width longhand.
pub type GapRuleList<Value> = GenericGapRuleList<Value, Integer>;

/// The specified value of `column-rule-color` and `row-rule-color`.
pub type GapRuleColorList = GapRuleList<Color>;
/// The specified value of `column-rule-style` and `row-rule-style`.
pub type GapRuleStyleList = GapRuleList<BorderStyle>;

/// A specified gap-rule width that computes with line-width snapping.
#[derive(
    Clone, Debug, MallocSizeOf, Parse, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped,
)]
#[repr(transparent)]
#[typed_value(derive_fields)]
pub struct GapRuleWidth(BorderSideWidth);

impl GapRuleWidth {
    /// Construct from the shared `<line-width>` parser representation.
    pub fn from_border_side_width(width: BorderSideWidth) -> Self {
        Self(width)
    }

    /// Return the shared `<line-width>` parser representation.
    pub fn as_border_side_width(&self) -> &BorderSideWidth {
        &self.0
    }

    /// Construct the initial `medium` width.
    pub fn medium() -> Self {
        Self(BorderSideWidth::medium())
    }
}

impl ToComputedValue for GapRuleWidth {
    type ComputedValue = computed::bd_gaps::GapRuleWidth;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        let width = self.0.to_computed_value(context);
        computed::bd_gaps::GapRuleWidth::new(
            width.0,
            app_units::Au(context.device().app_units_per_device_pixel()),
        )
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self(BorderSideWidth::from_computed_value(
            &computed::BorderSideWidth(computed.length()),
        ))
    }
}

/// The specified value of `column-rule-width` and `row-rule-width`.
pub type GapRuleWidthList = GapRuleList<GapRuleWidth>;

impl<Value: Parse> Parse for GapRuleList<Value> {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        parse_gap_rule_list_with(context, input, Value::parse)
    }
}

pub(crate) fn parse_gap_rule_list_with<'i, 't, Value>(
    context: &ParserContext,
    input: &mut Parser<'i, 't>,
    mut parse_value: impl for<'tt> FnMut(
        &ParserContext,
        &mut Parser<'i, 'tt>,
    ) -> Result<Value, ParseError<'i>>,
) -> Result<GapRuleList<Value>, ParseError<'i>> {
    let mut saw_auto_repeater = false;
    let items = input.parse_comma_separated(|input| {
        if input
            .try_parse(|input| input.expect_function_matching("repeat"))
            .is_err()
        {
            return parse_value(context, input).map(GapRuleListItem::Value);
        }

        let item = input.parse_nested_block(|input| {
            let count = if input
                .try_parse(|input| input.expect_ident_matching("auto"))
                .is_ok()
            {
                GapRuleRepeatCount::Auto
            } else {
                GapRuleRepeatCount::Number(Integer::parse_positive(context, input)?)
            };
            input.expect_comma()?;
            let values = input.parse_comma_separated(|input| parse_value(context, input))?;
            GapRuleListItem::repeat(count, values)
                .ok_or_else(|| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
        })?;

        if matches!(
            item,
            GapRuleListItem::Repeat {
                count: GapRuleRepeatCount::Auto,
                ..
            }
        ) {
            if saw_auto_repeater {
                return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
            }
            saw_auto_repeater = true;
        }
        Ok(item)
    })?;
    GenericGapRuleList::from_vec(items)
        .ok_or_else(|| input.new_custom_error(StyleParseErrorKind::UnspecifiedError))
}

macro_rules! gap_keyword {
    ($(#[$meta:meta])* pub enum $name:ident { $($body:tt)* }) => {
        $(#[$meta])*
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
        pub enum $name { $($body)* }
    };
}

gap_keyword! {
    /// Controls how gap decorations break at visible intersections.
    pub enum RuleBreak {
        None,
        #[default]
        Normal,
        Intersection,
    }
}

gap_keyword! {
    pub enum RuleVisibilityItems {
        All,
        Around,
        Between,
        #[default]
        Normal,
    }
}

gap_keyword! {
    /// Controls which gap-decoration axis paints above the other.
    pub enum RuleOverlap {
        #[default]
        RowOverColumn,
        ColumnOverRow,
    }
}

/// Specified value of `overlay` (F21.24).
///
/// CSS Position 4 — controls whether an element participates in
/// the top-layer overlay (popover / modal-dialog stack). `auto`
/// defers to the element's open/closed state; `none` (initial)
/// keeps it in normal flow. Animation-only-keyword spec preserved.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum Overlay {
    #[default]
    None,
    Auto,
}

/// Specified value of `-bd-border-clip` (Tier 5 §A.5.6).
///
/// Native counterpart to Prince's `border-clip`. Controls the geometry
/// used to join two adjacent border sides at a rounded corner when
/// `border-radius` is non-zero. CSS Backgrounds 3 §7.7 does not
/// prescribe a single shape for the join — Prince admits three:
///
/// - `square` (initial) — the corner is closed by a straight diagonal
///   miter from the outer-radius arc endpoint to the inner-radius arc
///   endpoint. Matches the default CSS Backgrounds 3 behaviour.
/// - `round` — the corner is closed by an arc that follows the inner
///   border radius, smoothing the colour/style transition.
/// - `bevel` — the corner is closed by a flat cut perpendicular to the
///   line bisecting the two adjacent sides.
///
/// The property only affects paint geometry when adjacent sides differ
/// in colour or style; uniform-side borders draw a single rounded ring
/// regardless of value.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BorderClip {
    /// Straight diagonal miter (CSS Backgrounds 3 default).
    #[default]
    Square,
    /// Arc following the inner border radius.
    Round,
    /// Flat cut perpendicular to the corner bisector.
    Bevel,
}

/// Specified value of `mask-border-mode` (F21.8).
///
/// Determines whether the mask-border source image is interpreted
/// via its luminance or alpha channel. Mirrors mask-mode but applies
/// to the mask-border family.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum MaskBorderMode {
    #[default]
    Alpha,
    Luminance,
}
