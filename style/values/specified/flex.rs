/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Specified types for CSS values related to flexbox.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::generics::flex::FlexBasis as GenericFlexBasis;
use crate::values::specified::Size;
use cssparser::Parser;
use style_traits::ParseError;

/// A specified value for the `flex-basis` property.
pub type FlexBasis = GenericFlexBasis<Size>;

/// The wrapping and balancing mode of a flex container.
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
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum FlexWrap {
    /// A single flex line.
    Nowrap,
    /// Ordinary line wrapping.
    Wrap,
    /// Ordinary wrapping with reversed line stacking.
    WrapReverse,
    /// Balanced line wrapping.
    Balance,
    /// Balanced wrapping with reversed line stacking.
    #[css(keyword = "wrap-reverse balance")]
    BalanceReverse,
}

impl Parse for FlexWrap {
    fn parse<'i>(_: &ParserContext, input: &mut Parser<'i, '_>) -> Result<Self, ParseError<'i>> {
        let first = try_match_ident_ignore_ascii_case! {input,
            "nowrap" => Self::Nowrap,
            "wrap" => Self::Wrap,
            "wrap-reverse" => Self::WrapReverse,
            "balance" => Self::Balance,
        };
        Ok(match first {
            Self::Wrap | Self::WrapReverse
                if input
                    .try_parse(|i| i.expect_ident_matching("balance"))
                    .is_ok() =>
            {
                if first == Self::Wrap {
                    Self::Balance
                } else {
                    Self::BalanceReverse
                }
            },
            Self::Balance => input
                .try_parse(|i| -> Result<Self, ParseError<'i>> {
                    Ok(try_match_ident_ignore_ascii_case! {i,
                        "wrap" => Self::Balance,
                        "wrap-reverse" => Self::BalanceReverse,
                    })
                })
                .unwrap_or(Self::Balance),
            value => value,
        })
    }
}

impl FlexBasis {
    /// `auto`
    #[inline]
    pub fn auto() -> Self {
        GenericFlexBasis::Size(Size::auto())
    }

    /// `0%`
    #[inline]
    pub fn zero_percent() -> Self {
        GenericFlexBasis::Size(Size::zero_percent())
    }
}

impl Parse for FlexBasis {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let v = input.try_parse(|i| {
            Ok(try_match_ident_ignore_ascii_case! {i, "content" => Self::Content, })
        });
        if v.is_ok() {
            return v;
        }
        Ok(Self::Size(Size::parse_size_for_flex_basis_width(
            context, input,
        )?))
    }
}
