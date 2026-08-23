/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified types for CSS values related to backgrounds.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::background::BackgroundSize as GenericBackgroundSize;
use crate::values::specified::length::{
    NonNegativeLengthPercentage, NonNegativeLengthPercentageOrAuto,
};
use cssparser::{match_ignore_ascii_case, Parser};
use selectors::parser::SelectorParseErrorKind;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// A specified `background-clip` layer.
#[cfg_attr(feature = "servo", derive(serde::Deserialize, Hash, serde::Serialize))]
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    FromPrimitive,
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
#[allow(missing_docs)]
pub enum BackgroundClip {
    BorderBox,
    PaddingBox,
    ContentBox,
    Text,
    #[cfg(feature = "servo")]
    BorderArea,
    #[cfg(feature = "servo")]
    #[css(keyword = "border-area text")]
    BorderAreaText,
}

impl Parse for BackgroundClip {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        Self::parse_value(input)
    }
}

impl BackgroundClip {
    fn parse_value<'i, 't>(input: &mut Parser<'i, 't>) -> Result<Self, ParseError<'i>> {
        let ident = input.expect_ident_cloned()?;
        #[cfg(feature = "servo")]
        match_ignore_ascii_case! { &ident,
            "text" if input.try_parse(|input| input.expect_ident_matching("border-area")).is_ok() => {
                return Ok(Self::BorderAreaText);
            },
            "border-area" => {
                return Ok(if input.try_parse(|input| input.expect_ident_matching("text")).is_ok() {
                    Self::BorderAreaText
                } else {
                    Self::BorderArea
                });
            },
            _ => {},
        }
        Ok(match_ignore_ascii_case! { &ident,
            "border-box" => Self::BorderBox,
            "padding-box" => Self::PaddingBox,
            "content-box" => Self::ContentBox,
            "text" => Self::Text,
            _ => return Err(input.new_custom_error(SelectorParseErrorKind::UnexpectedIdent(ident))),
        })
    }
}

/// A specified value for the `background-size` property.
pub type BackgroundSize = GenericBackgroundSize<NonNegativeLengthPercentage>;

impl Parse for BackgroundSize {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if let Ok(width) = input.try_parse(|i| NonNegativeLengthPercentageOrAuto::parse(context, i))
        {
            let height = input
                .try_parse(|i| NonNegativeLengthPercentageOrAuto::parse(context, i))
                .unwrap_or(NonNegativeLengthPercentageOrAuto::auto());
            return Ok(GenericBackgroundSize::ExplicitSize { width, height });
        }
        Ok(try_match_ident_ignore_ascii_case! { input,
            "cover" => GenericBackgroundSize::Cover,
            "contain" => GenericBackgroundSize::Contain,
        })
    }
}

/// One of the keywords for `background-repeat`.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
#[allow(missing_docs)]
#[value_info(other_values = "repeat-x,repeat-y")]
pub enum BackgroundRepeatKeyword {
    Repeat,
    Space,
    Round,
    NoRepeat,
}

/// The value of the `background-repeat` property, with `repeat-x` / `repeat-y`
/// represented as the combination of `no-repeat` and `repeat` in the opposite
/// axes.
///
/// https://drafts.csswg.org/css-backgrounds/#the-background-repeat
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
pub struct BackgroundRepeat(pub BackgroundRepeatKeyword, pub BackgroundRepeatKeyword);

impl BackgroundRepeat {
    /// Returns the `repeat repeat` value.
    pub fn repeat() -> Self {
        BackgroundRepeat(
            BackgroundRepeatKeyword::Repeat,
            BackgroundRepeatKeyword::Repeat,
        )
    }
}

impl ToCss for BackgroundRepeat {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match (self.0, self.1) {
            (BackgroundRepeatKeyword::Repeat, BackgroundRepeatKeyword::NoRepeat) => {
                dest.write_str("repeat-x")
            },
            (BackgroundRepeatKeyword::NoRepeat, BackgroundRepeatKeyword::Repeat) => {
                dest.write_str("repeat-y")
            },
            (horizontal, vertical) => {
                horizontal.to_css(dest)?;
                if horizontal != vertical {
                    dest.write_char(' ')?;
                    vertical.to_css(dest)?;
                }
                Ok(())
            },
        }
    }
}

impl Parse for BackgroundRepeat {
    fn parse<'i, 't>(
        _context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let ident = input.expect_ident_cloned()?;

        match_ignore_ascii_case! { &ident,
            "repeat-x" => {
                return Ok(BackgroundRepeat(BackgroundRepeatKeyword::Repeat, BackgroundRepeatKeyword::NoRepeat));
            },
            "repeat-y" => {
                return Ok(BackgroundRepeat(BackgroundRepeatKeyword::NoRepeat, BackgroundRepeatKeyword::Repeat));
            },
            _ => {},
        }

        let horizontal = match BackgroundRepeatKeyword::from_ident(&ident) {
            Ok(h) => h,
            Err(()) => {
                return Err(
                    input.new_custom_error(SelectorParseErrorKind::UnexpectedIdent(ident.clone()))
                );
            },
        };

        let vertical = input.try_parse(BackgroundRepeatKeyword::parse).ok();
        Ok(BackgroundRepeat(horizontal, vertical.unwrap_or(horizontal)))
    }
}

#[cfg(all(test, feature = "servo"))]
mod tests {
    use super::*;
    use cssparser::ParserInput;

    fn parse_background_clip(css: &str) -> Result<BackgroundClip, ()> {
        let mut input = ParserInput::new(css);
        Parser::new(&mut input)
            .parse_entirely(BackgroundClip::parse_value)
            .map_err(|_| ())
    }

    #[test]
    fn background_clip_parses_every_level_four_value() {
        assert_eq!(
            parse_background_clip("border-box"),
            Ok(BackgroundClip::BorderBox)
        );
        assert_eq!(
            parse_background_clip("padding-box"),
            Ok(BackgroundClip::PaddingBox)
        );
        assert_eq!(
            parse_background_clip("content-box"),
            Ok(BackgroundClip::ContentBox)
        );
        assert_eq!(parse_background_clip("text"), Ok(BackgroundClip::Text));
        assert_eq!(
            parse_background_clip("border-area"),
            Ok(BackgroundClip::BorderArea)
        );
        assert_eq!(
            parse_background_clip("border-area text"),
            Ok(BackgroundClip::BorderAreaText),
        );
        assert_eq!(
            parse_background_clip("text border-area"),
            Ok(BackgroundClip::BorderAreaText),
        );
    }

    #[test]
    fn background_clip_rejects_duplicate_and_foreign_components() {
        for css in [
            "text text",
            "border-area border-area",
            "border-area text text",
            "border-box text",
            "fill-box",
        ] {
            assert_eq!(parse_background_clip(css), Err(()), "{css}");
        }
    }

    #[test]
    fn background_clip_union_has_canonical_serialisation() {
        assert_eq!(
            BackgroundClip::BorderAreaText.to_css_string(),
            "border-area text"
        );
    }
}
