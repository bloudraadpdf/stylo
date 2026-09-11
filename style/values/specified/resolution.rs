/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Resolution values:
//!
//! https://drafts.csswg.org/css-values/#resolution

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::CalcNode;
use crate::values::CSSFloat;
use cssparser::{match_ignore_ascii_case, Parser, Token};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// A literal resolution dimension.
#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem)]
pub struct ResolutionDimension {
    value: CSSFloat,
    unit: ResolutionUnit,
}

#[derive(Clone, Copy, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToShmem)]
enum ResolutionUnit {
    Dpi,
    X,
    Dppx,
    Dpcm,
}

impl ResolutionDimension {
    /// Creates a resolution in dots per pixel.
    pub fn from_dppx(value: CSSFloat) -> Self {
        Self {
            value,
            unit: ResolutionUnit::Dppx,
        }
    }

    /// Converts this resolution to dots per pixel.
    pub fn dppx(self) -> CSSFloat {
        match self.unit {
            ResolutionUnit::X | ResolutionUnit::Dppx => self.value,
            ResolutionUnit::Dpi => self.value / 96.0,
            ResolutionUnit::Dpcm => self.value * 2.54 / 96.0,
        }
    }

    /// Parses a literal resolution dimension.
    pub fn parse_dimension(value: CSSFloat, unit: &str) -> Result<Self, ()> {
        let unit = match_ignore_ascii_case! { unit,
            "dpi" => ResolutionUnit::Dpi,
            "dppx" => ResolutionUnit::Dppx,
            "dpcm" => ResolutionUnit::Dpcm,
            "x" => ResolutionUnit::X,
            _ => return Err(())
        };
        Ok(Self { value, unit })
    }
}

impl ToCss for ResolutionDimension {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        let unit = match self.unit {
            ResolutionUnit::Dpi => "dpi",
            ResolutionUnit::X => "x",
            ResolutionUnit::Dppx => "dppx",
            ResolutionUnit::Dpcm => "dpcm",
        };
        crate::values::serialize_specified_dimension(self.value, unit, false, dest)
    }
}

/// A specified resolution, retained until its calculation context is available.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
pub struct Resolution(ResolutionValue);

impl style_traits::SpecifiedValueInfo for Resolution {}

#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToShmem)]
enum ResolutionValue {
    Dimension(ResolutionDimension),
    Calc(Box<CalcNode>),
}

impl Resolution {
    /// Creates a resolution in dots per pixel.
    pub fn from_dppx(value: CSSFloat) -> Self {
        Self(ResolutionValue::Dimension(ResolutionDimension::from_dppx(
            value,
        )))
    }

    /// Creates a resolution using the x unit.
    pub fn from_x(value: CSSFloat) -> Self {
        Self(ResolutionValue::Dimension(ResolutionDimension {
            value,
            unit: ResolutionUnit::X,
        }))
    }

    pub(crate) fn from_calc_node(node: CalcNode) -> Self {
        Self(ResolutionValue::Calc(Box::new(node)))
    }
}

impl ToComputedValue for Resolution {
    type ComputedValue = crate::values::computed::Resolution;

    fn to_computed_value(&self, context: &Context) -> Self::ComputedValue {
        let dppx = match &self.0 {
            ResolutionValue::Dimension(value) => value.dppx(),
            ResolutionValue::Calc(node) => node
                .resolve_resolution(context)
                .expect("a validated resolution calculation must resolve in its element context"),
        };
        Self::ComputedValue::from_dppx(crate::values::normalize(dppx).clamp(0.0, f32::MAX))
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        Self::from_dppx(computed.dppx())
    }
}

impl ToCss for Resolution {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match &self.0 {
            ResolutionValue::Dimension(value) => value.to_css(dest),
            ResolutionValue::Calc(node) => node.to_css(dest),
        }
    }
}

impl Parse for Resolution {
    fn parse<'i, 't>(
        context: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        let location = input.current_source_location();
        match *input.next()? {
            Token::Dimension {
                value, ref unit, ..
            } if value >= 0. => ResolutionDimension::parse_dimension(value, unit)
                .map(|value| Self(ResolutionValue::Dimension(value)))
                .map_err(|()| location.new_custom_error(StyleParseErrorKind::UnspecifiedError)),
            Token::Function(ref name) => {
                let function = CalcNode::math_function(context, name, location)?;
                CalcNode::parse_resolution(context, input, function)
            },
            ref token => Err(location.new_unexpected_token_error(token.clone())),
        }
    }
}
