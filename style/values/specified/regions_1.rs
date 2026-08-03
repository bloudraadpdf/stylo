/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! CSS Regions Module Level 1 longhands.
//!
//! Implements the standard `flow-into` (§2.1) and `flow-from` (§2.2)
//! properties. The moegoe `-bd-flow-into` / `-bd-flow-from` shims in
//! `bd_flow.rs` remain in place because they have a slightly different
//! grammar inherited from PDFreactor's `-ro-*` surface and the cascade
//! reader already consumes them; the standard `flow-into` / `flow-from`
//! defined here are wired alongside so authored content using the
//! W3C-spec property names parses correctly.
//!
//! Per spec §2.1, the named-flow identifier must not be one of the
//! reserved CSS-wide values (`none`, `inherit`, `initial`, `unset`),
//! the CSS-shorthand `default` placeholder, or `auto`. Attempting to
//! use any of those as the flow name is a parse error.
//!
//! Both properties cascade through the `box` style struct.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::OwnedStr;
use cssparser::Parser;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ParseError, StyleParseErrorKind, ToCss};

/// Whether `<custom-ident>` is reserved by the CSS Regions spec
/// and therefore invalid as a named-flow identifier.
fn is_reserved_flow_ident(ident: &str) -> bool {
    matches!(
        ident,
        "none" | "inherit" | "initial" | "unset" | "default" | "auto"
    )
}

/// Whether an element's flow contributes its own box or its content.
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
pub enum FlowIntoMode {
    /// The element's flow contributes its element (the box itself).
    #[default]
    Element,
    /// The element's flow contributes only its content.
    Content,
}

/// Specified value of the standard `flow-into` property
/// (<https://drafts.csswg.org/css-regions-1/#flow-into>).
///
/// `none` (initial) — the element does not contribute to any named flow.
/// `<custom-ident> [element | content]?` — feeds the named flow with
/// the element's box (`element`, default) or its content (`content`).
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum FlowInto {
    /// `none` — initial value.
    None,
    /// `<custom-ident> [element | content]?`.
    Named {
        /// Named-flow identifier (validated against the reserved list).
        name: OwnedStr,
        /// Source mode — `element` (default) or `content`.
        mode: FlowIntoMode,
    },
}

impl Default for FlowInto {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl FlowInto {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for FlowInto {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Named { name, mode } => {
                dest.write_str(name)?;
                // Default `element` is the spec initial — emit only when non-default.
                if !matches!(mode, FlowIntoMode::Element) {
                    dest.write_char(' ')?;
                    mode.to_css(dest)?;
                }
                Ok(())
            },
        }
    }
}

impl Parse for FlowInto {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let ident = input.expect_ident()?.as_ref().to_owned();
        if is_reserved_flow_ident(&ident) {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        let mode = input.try_parse(FlowIntoMode::parse).unwrap_or_default();
        Ok(Self::Named {
            name: ident.into(),
            mode,
        })
    }
}

/// Specified value of the standard `flow-from` property
/// (<https://drafts.csswg.org/css-regions-1/#flow-from>).
///
/// `none` (initial) — the element is not a region.
/// `<custom-ident>` — receives content from the named flow.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum FlowFrom {
    /// `none` — initial value.
    None,
    /// `<custom-ident>` — named flow this region pulls content from.
    Named(OwnedStr),
}

impl Default for FlowFrom {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl FlowFrom {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for FlowFrom {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Named(name) => dest.write_str(name),
        }
    }
}

impl Parse for FlowFrom {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let ident = input.expect_ident()?.as_ref().to_owned();
        if is_reserved_flow_ident(&ident) {
            return Err(input.new_custom_error(StyleParseErrorKind::UnspecifiedError));
        }
        Ok(Self::Named(ident.into()))
    }
}

/// Specified value of the standard `region-fragment` property
/// (<https://drafts.csswg.org/css-regions-1/#region-fragment>).
///
/// `auto` (initial) — content overflows the last region in the chain
/// and remains visible. `break` — content past the last region's
/// capacity is dropped. The moegoe-native `-bd-region-fragment`
/// longhand (`bd_flow.rs`) carries the same two-keyword grammar; this
/// type is the standard-spec twin so authored content using the
/// W3C-spec property name parses correctly.
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
pub enum RegionFragment {
    /// `auto` — content overflows the last region visibly (initial).
    #[default]
    Auto,
    /// `break` — content past the last region's capacity is dropped.
    Break,
}
