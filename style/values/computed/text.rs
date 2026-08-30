/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed types for text properties.

use crate::derives::*;
use crate::values::animated::{Animate, Procedure};
use crate::values::computed::length::{Length, LengthPercentage};
use crate::values::generics::length::GenericLengthPercentageOrAuto;
use crate::values::generics::text::{
    GenericHyphenateLimitChars, GenericInitialLetter, GenericTextDecorationInset,
    GenericTextDecorationLength, GenericTextIndent, GenericTextSizeAdjust,
    GenericTextUnderlineOffset,
};
use crate::values::generics::NumberOrAuto;
use crate::values::specified::text as specified;
use crate::values::specified::text::{TextEmphasisFillMode, TextEmphasisShapeKeyword};
use crate::values::{computed::NonNegativePercentage, CSSFloat, CSSInteger};
use crate::Zero;
use std::fmt::{self, Write};
use style_traits::{CssString, CssWriter, ToCss, ToTyped, TypedValue};

pub use crate::values::specified::text::{
    HangingPunctuation, HyphenateCharacter, LineBreak, MozControlCharacterVisibility, OverflowWrap,
    RubyPosition, TextAlignLast, TextAutospace, TextDecorationLine, TextDecorationSkipInk,
    TextEmphasisPosition, TextJustify, TextOverflow, TextTransform, TextUnderlinePosition,
    WhiteSpaceTrim, WordBreak, WordSpaceTransform,
};

/// A computed value for the `initial-letter` property.
pub type InitialLetter = GenericInitialLetter<CSSFloat, CSSInteger>;

/// The computed value of `text-size-adjust`.
pub type TextSizeAdjust = GenericTextSizeAdjust<NonNegativePercentage>;

/// Implements type for `text-decoration-thickness` property.
pub type TextDecorationLength = GenericTextDecorationLength<LengthPercentage>;

impl Animate for TextDecorationLength {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match (self, other) {
            (Self::LengthPercentage(from), Self::LengthPercentage(to)) => Ok(
                Self::LengthPercentage(from.animate_as_percentage_dimension_mix(to, procedure)?),
            ),
            (Self::Auto, Self::Auto) => Ok(Self::Auto),
            (Self::FromFont, Self::FromFont) => Ok(Self::FromFont),
            (Self::LengthPercentage(_), Self::Auto | Self::FromFont)
            | (Self::Auto, Self::LengthPercentage(_) | Self::FromFont)
            | (Self::FromFont, Self::LengthPercentage(_) | Self::Auto) => Err(()),
        }
    }
}

/// Computed value for `text-underline-offset`.
pub type TextUnderlineOffset = GenericTextUnderlineOffset<LengthPercentage>;

impl Animate for TextUnderlineOffset {
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match (&self.0, &other.0) {
            (
                GenericLengthPercentageOrAuto::LengthPercentage(from),
                GenericLengthPercentageOrAuto::LengthPercentage(to),
            ) => Ok(Self(GenericLengthPercentageOrAuto::LengthPercentage(
                from.animate_as_percentage_dimension_mix(to, procedure)?,
            ))),
            (GenericLengthPercentageOrAuto::Auto, GenericLengthPercentageOrAuto::Auto) => {
                Ok(Self::auto())
            },
            (
                GenericLengthPercentageOrAuto::LengthPercentage(_),
                GenericLengthPercentageOrAuto::Auto,
            )
            | (
                GenericLengthPercentageOrAuto::Auto,
                GenericLengthPercentageOrAuto::LengthPercentage(_),
            ) => Err(()),
        }
    }
}

#[cfg(test)]
mod text_decoration_animation_tests {
    use super::*;
    use crate::values::computed::Percentage;

    #[test]
    fn mixed_text_decoration_thickness_retains_calculated_endpoint() {
        let from =
            TextDecorationLength::LengthPercentage(LengthPercentage::new_length(Length::new(16.0)));
        let to =
            TextDecorationLength::LengthPercentage(LengthPercentage::new_percent(Percentage(2.0)));

        let sampled = from
            .animate(&to, Procedure::Interpolate { progress: 0.0 })
            .expect("mixed text-decoration-thickness endpoints must interpolate");

        assert_eq!(sampled.to_css_string(), "calc(0% + 16px)");
    }

    #[test]
    fn mixed_text_underline_offset_retains_calculated_endpoint() {
        let from: TextUnderlineOffset = GenericTextUnderlineOffset::length_percentage(
            LengthPercentage::new_percent(Percentage(1.0)),
        );
        let to: TextUnderlineOffset = GenericTextUnderlineOffset::length_percentage(
            LengthPercentage::new_length(Length::new(32.0)),
        );

        let sampled = from
            .animate(&to, Procedure::Interpolate { progress: 1.0 })
            .expect("mixed text-underline-offset endpoints must interpolate");

        assert_eq!(sampled.to_css_string(), "calc(0% + 32px)");
    }
}

/// Implements type for `text-decoration-inset` property.
pub type TextDecorationInset = GenericTextDecorationInset<Length>;

/// The computed value of `text-align`.
pub type TextAlign = specified::TextAlignKeyword;

/// The computed value of `text-indent`.
pub type TextIndent = GenericTextIndent<LengthPercentage>;

/// A computed value for the `hyphenate-character` property.
pub type HyphenateLimitChars = GenericHyphenateLimitChars<CSSInteger>;

impl HyphenateLimitChars {
    /// Return the `auto` value, which has all three component values as `auto`.
    #[inline]
    pub fn auto() -> Self {
        Self {
            total_word_length: NumberOrAuto::Auto,
            pre_hyphen_length: NumberOrAuto::Auto,
            post_hyphen_length: NumberOrAuto::Auto,
        }
    }
}

/// A computed value for the `letter-spacing` property.
#[repr(transparent)]
#[derive(
    Animate,
    Clone,
    ComputeSquaredDistance,
    Copy,
    Debug,
    MallocSizeOf,
    PartialEq,
    ToAnimatedValue,
    ToAnimatedZero,
    ToResolvedValue,
)]
pub struct GenericLetterSpacing<L>(pub L);
/// This is generic just to make the #[derive()] code do the right thing for lengths.
pub type LetterSpacing = GenericLetterSpacing<LengthPercentage>;

impl LetterSpacing {
    /// Return the `normal` computed value, which is just zero.
    #[inline]
    pub fn normal() -> Self {
        Self(LengthPercentage::zero())
    }
}

impl ToCss for LetterSpacing {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        // https://drafts.csswg.org/css-text/#propdef-letter-spacing
        //
        // For legacy reasons, a computed letter-spacing of zero yields a
        // resolved value (getComputedStyle() return value) of normal.
        if self.0.is_zero() {
            return dest.write_str("normal");
        }
        self.0.to_css(dest)
    }
}

impl ToTyped for LetterSpacing {
    // XXX The specification does not currently define how this property should
    // be reified into Typed OM. The current behavior follows existing WPT
    // coverage (letter-spacing.html). We may file a spec issue once more data
    // is collected to update the Property-specific Rules section to align with
    // observed test expectations.
    fn to_typed(&self) -> Option<TypedValue> {
        if self.0.is_zero() {
            return Some(TypedValue::Keyword(CssString::from("normal")));
        }
        // XXX According to the test, should return TypedValue::Numeric with
        // unit "px" or "percent" once that variant is available. Tracked in
        // bug 1990419.
        None
    }
}

/// A computed value for the `word-spacing` property.
pub type WordSpacing = LengthPercentage;

impl WordSpacing {
    /// Return the `normal` computed value, which is just zero.
    #[inline]
    pub fn normal() -> Self {
        LengthPercentage::zero()
    }
}

/// A computed value for the `text-combine-upright` property.
#[derive(Clone, Copy, Debug, Eq, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum TextCombineUpright {
    /// `none`
    None,
    /// `all`
    All,
    /// `digits <integer [2,4]>`
    Digits(CSSInteger),
}

impl TextCombineUpright {
    /// Return the initial value.
    #[inline]
    pub fn get_initial_value() -> Self {
        Self::None
    }
}

impl ToCss for TextCombineUpright {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::None => dest.write_str("none"),
            Self::All => dest.write_str("all"),
            Self::Digits(digits) => {
                dest.write_str("digits ")?;
                write!(dest, "{digits}")
            },
        }
    }
}

/// Computed value for the text-emphasis-style property
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[allow(missing_docs)]
#[repr(C, u8)]
pub enum TextEmphasisStyle {
    /// [ <fill> || <shape> ]
    Keyword {
        #[css(skip_if = "TextEmphasisFillMode::is_filled")]
        fill: TextEmphasisFillMode,
        shape: TextEmphasisShapeKeyword,
    },
    /// `none`
    None,
    /// `<string>` (of which only the first grapheme cluster will be used).
    String(crate::OwnedStr),
}
