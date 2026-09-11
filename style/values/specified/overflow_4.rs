/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Typed line-clamp longhands and CSS Inline 3 text box edges.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::specified::{Integer, PositiveInteger};
use crate::OwnedStr;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, ToCss};

/// Specified value of the `block-ellipsis` property
/// (<https://drafts.csswg.org/css-overflow-4/#block-ellipsis>).
///
/// Grammar: `no-ellipsis | ellipsis | <string>`. Selects the ellipsis glyph
/// inserted at the truncation boundary. `ellipsis` defers to the UA's
/// content-language-aware default.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToCss,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum BlockEllipsis {
    /// No glyph is inserted at truncation.
    NoEllipsis,
    /// UA-selected default glyph.
    Ellipsis,
    /// `<string>` — author-supplied ellipsis glyph string.
    String(OwnedStr),
}

impl Parse for BlockEllipsis {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("no-ellipsis"))
            .is_ok()
        {
            return Ok(Self::NoEllipsis);
        }
        if input
            .try_parse(|i| i.expect_ident_matching("ellipsis"))
            .is_ok()
        {
            return Ok(Self::Ellipsis);
        }
        let s = input.expect_string()?;
        Ok(Self::String(s.as_ref().to_owned().into()))
    }
}

/// A parser-validated positive specified line count.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped)]
#[repr(transparent)]
pub struct PositiveLineCount(Integer);

impl PositiveLineCount {
    /// Retains the positive-integer proof established by the CSS parser.
    #[inline]
    pub(crate) fn from_positive(value: PositiveInteger) -> Self {
        Self(value.0)
    }

    /// Returns the underlying specified integer for computed-value conversion.
    #[inline]
    pub(crate) fn integer(&self) -> &Integer {
        &self.0
    }

    /// Reconstructs a specified value from its proof-carrying computed value.
    #[inline]
    pub(crate) fn from_computed(
        value: crate::values::computed::overflow_4::PositiveLineCount,
    ) -> Self {
        Self(Integer::new(value.raw()))
    }
}

impl Parse for PositiveLineCount {
    #[inline]
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        PositiveInteger::parse(context, input).map(Self::from_positive)
    }
}

/// Specified maximum line count: `auto | <integer [1,∞]> || auto`.
pub type MaxLines = crate::values::generics::box_::GenericMaxLines<PositiveLineCount>;

impl Parse for MaxLines {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let leading_auto = input.try_parse(|i| i.expect_ident_matching("auto")).is_ok();
        let count = input.try_parse(|i| PositiveLineCount::parse(context, i));
        match count {
            Ok(count) => {
                let automatic =
                    leading_auto || input.try_parse(|i| i.expect_ident_matching("auto")).is_ok();
                Ok(if automatic {
                    Self::LinesAuto(count)
                } else {
                    Self::Lines(count)
                })
            },
            Err(_) if leading_auto => Ok(Self::Auto),
            Err(error) => Err(error),
        }
    }
}

/// Specified value of the `continue` property
/// (<https://drafts.csswg.org/css-overflow-4/#continue>).
///
/// Controls continuation after the line or block-size limit.
///
/// The Rust type is named [`Continue`] but the longhand keyword
/// `continue` is a Rust reserved word; the generated property module
/// is renamed to `continue_` by `data.py::to_rust_ident`.
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
pub enum Continue {
    /// Overflow continues into subsequent fragments.
    #[default]
    Normal,
    /// `discard` — overflow is dropped at the truncation boundary.
    Discard,
    /// Excess content has no layout or paint extent.
    Collapse,
    /// Applies the legacy vertical box clamping rules.
    #[css(keyword = "-webkit-legacy")]
    WebkitLegacy,
}

/// Specified value of the `text-box-trim` property
/// (<https://drafts.csswg.org/css-inline-3/#text-box-trim>).
///
/// Grammar: `none | trim-start | trim-end | trim-both`.
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
pub enum LeadingTrim {
    /// `none` — preserve leading on both edges (default).
    #[default]
    #[css(keyword = "none")]
    Normal,
    /// `trim-start` — trim leading from the block-start edge.
    #[css(keyword = "trim-start")]
    Start,
    /// `trim-end` — trim leading from the block-end edge.
    #[css(keyword = "trim-end")]
    End,
    /// `trim-both` — trim leading from both edges.
    #[css(keyword = "trim-both")]
    Both,
}

macro_rules! text_edge_metric {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[repr(u8)]
        #[derive(
            Clone, Copy, Debug, Eq, MallocSizeOf, Parse, PartialEq, SpecifiedValueInfo,
            ToComputedValue, ToCss, ToResolvedValue, ToShmem, ToTyped,
        )]
        #[allow(missing_docs)]
        pub enum $name {
            $($variant),+
        }
    };
}

text_edge_metric!(TextEdgeOver {
    Text,
    Ideographic,
    IdeographicInk,
    Cap,
    Ex
});
text_edge_metric!(TextEdgeUnder {
    Text,
    Ideographic,
    IdeographicInk,
    Alphabetic
});

const fn implicit_under(over: TextEdgeOver) -> TextEdgeUnder {
    match over {
        TextEdgeOver::Text => TextEdgeUnder::Text,
        TextEdgeOver::Ideographic => TextEdgeUnder::Ideographic,
        TextEdgeOver::IdeographicInk => TextEdgeUnder::IdeographicInk,
        TextEdgeOver::Cap | TextEdgeOver::Ex => TextEdgeUnder::Text,
    }
}

#[repr(C, u8)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[allow(missing_docs)]
pub enum TextBoxEdge {
    #[default]
    Auto,
    Edges(TextEdgeOver, TextEdgeUnder),
}

impl TextBoxEdge {
    /// Creates an explicit over and under metric pair.
    #[inline]
    pub const fn edges(over: TextEdgeOver, under: TextEdgeUnder) -> Self {
        Self::Edges(over, under)
    }
}

impl Parse for TextBoxEdge {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input
            .try_parse(|input| input.expect_ident_matching("auto"))
            .is_ok()
        {
            return Ok(Self::Auto);
        }

        if input
            .try_parse(|input| input.expect_ident_matching("alphabetic"))
            .is_ok()
        {
            return Ok(Self::Edges(TextEdgeOver::Text, TextEdgeUnder::Alphabetic));
        }

        let over = TextEdgeOver::parse(input)?;
        let under = input
            .try_parse(TextEdgeUnder::parse)
            .unwrap_or_else(|_| implicit_under(over));
        Ok(Self::Edges(over, under))
    }
}

impl ToCss for TextBoxEdge {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::Edges(TextEdgeOver::Text, TextEdgeUnder::Alphabetic) => {
                dest.write_str("alphabetic")
            },
            Self::Edges(over, under) => {
                over.to_css(dest)?;
                if *under != implicit_under(*over) {
                    dest.write_char(' ')?;
                    under.to_css(dest)?;
                }
                Ok(())
            },
        }
    }
}

#[cfg(test)]
mod leading_trim_tests {
    use super::*;
    use crate::context::QuirksMode;
    use crate::stylesheets::{CssRuleType, Origin, UrlExtraData};
    use cssparser::{Parser, ParserInput};
    use style_traits::ParsingMode;

    fn parser_context(url_data: &UrlExtraData) -> ParserContext<'_> {
        ParserContext::new(
            Origin::Author,
            url_data,
            Some(CssRuleType::Style),
            ParsingMode::DEFAULT,
            QuirksMode::NoQuirks,
            Default::default(),
            None,
            None,
        )
    }

    fn parse_leading_trim(css: &str) -> LeadingTrim {
        let mut input = ParserInput::new(css);
        Parser::new(&mut input)
            .parse_entirely(LeadingTrim::parse)
            .expect("text-box-trim value should parse")
    }

    #[test]
    fn current_text_box_trim_keywords_map_to_the_existing_typed_states() {
        for (css, expected) in [
            ("none", LeadingTrim::Normal),
            ("trim-start", LeadingTrim::Start),
            ("trim-end", LeadingTrim::End),
            ("trim-both", LeadingTrim::Both),
        ] {
            assert_eq!(parse_leading_trim(css), expected);
        }
    }

    #[test]
    fn text_box_edge_preserves_typed_over_and_under_metrics() {
        let url_data = UrlExtraData::from(url::Url::parse("https://example.invalid/").unwrap());
        let context = parser_context(&url_data);
        for (css, expected) in [
            ("auto", TextBoxEdge::Auto),
            (
                "text",
                TextBoxEdge::edges(TextEdgeOver::Text, TextEdgeUnder::Text),
            ),
            (
                "cap",
                TextBoxEdge::edges(TextEdgeOver::Cap, TextEdgeUnder::Text),
            ),
            (
                "ex text",
                TextBoxEdge::edges(TextEdgeOver::Ex, TextEdgeUnder::Text),
            ),
            (
                "text alphabetic",
                TextBoxEdge::edges(TextEdgeOver::Text, TextEdgeUnder::Alphabetic),
            ),
            (
                "cap alphabetic",
                TextBoxEdge::edges(TextEdgeOver::Cap, TextEdgeUnder::Alphabetic),
            ),
            (
                "ideographic-ink",
                TextBoxEdge::edges(TextEdgeOver::IdeographicInk, TextEdgeUnder::IdeographicInk),
            ),
        ] {
            let mut input = ParserInput::new(css);
            let actual = Parser::new(&mut input)
                .parse_entirely(|input| TextBoxEdge::parse(&context, input))
                .expect("text-box-edge value should parse");
            assert_eq!(actual, expected);
        }

        for invalid in ["alphabetic text", "text cap", "auto text"] {
            let mut input = ParserInput::new(invalid);
            assert!(
                Parser::new(&mut input)
                    .parse_entirely(|input| TextBoxEdge::parse(&context, input))
                    .is_err(),
                "{invalid} must be rejected"
            );
        }
    }
}
