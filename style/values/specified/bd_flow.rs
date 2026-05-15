/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-flow-from` / `-bd-flow-into` properties (Family 17).
//!
//! **SHELVED — PARSE-ONLY THIN SHIM.** CSS Regions has been
//! essentially withdrawn at W3C; PDFreactor's surface
//! (`-ro-flow-from` / `-ro-flow-into`) is a smaller subset, and
//! moegoe's paginator does not currently support a re-flow loop.
//! This module ships the parse + serialise round-trip so authored
//! content does not hit `unknown property` warnings; the
//! `moegoe-css` boundary emits a
//! `RenderWarning::UnsupportedCssFeature` until the paginator
//! grows region awareness.
//!
//! Source: `docs/reference-manuals/pdfreactor.md:14995–15016` and
//! `docs/audits/CSS-COVERAGE-AUDIT-2026-05-14/stylo-push-plan.md` §F17.

use crate::derives::*;
use crate::OwnedStr;

/// Specified value of `-bd-flow-from`.
///
/// `none` (initial) — the element is not a region-content sink.
/// `<ident>` — pulls fragments from the named flow.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdFlowFrom {
    /// `none`.
    None,
    /// `<ident>`.
    Ident(OwnedStr),
}

impl Default for BdFlowFrom {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdFlowFrom {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl crate::parser::Parse for BdFlowFrom {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let ident = input.expect_ident()?;
        Ok(Self::Ident(ident.as_ref().to_owned().into()))
    }
}

/// Specified value of `-bd-flow-into`.
///
/// `none` (initial) — the element does not contribute to any
/// named flow. `<ident> [content | element]?` — feeds the named
/// flow with the element's content or the element itself.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToComputedValue,
    ToResolvedValue, ToShmem, ToTyped,
)]
#[repr(C, u8)]
pub enum BdFlowInto {
    /// `none`.
    None,
    /// `<ident> [content | element]?`.
    Flow {
        /// Named flow identifier.
        name: OwnedStr,
        /// Source mode — `content` (default) or `element`.
        mode: BdFlowIntoMode,
    },
}

/// Sub-keyword of `-bd-flow-into`.
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
pub enum BdFlowIntoMode {
    #[default]
    Content,
    Element,
}

impl Default for BdFlowInto {
    #[inline]
    fn default() -> Self {
        Self::None
    }
}

impl BdFlowInto {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl crate::parser::Parse for BdFlowInto {
    fn parse<'i, 't>(
        _: &crate::parser::ParserContext,
        input: &mut cssparser::Parser<'i, 't>,
    ) -> Result<Self, style_traits::ParseError<'i>> {
        if input
            .try_parse(|i| i.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Self::None);
        }
        let ident = input.expect_ident()?.as_ref().to_owned();
        let mode = input
            .try_parse(BdFlowIntoMode::parse)
            .unwrap_or_default();
        Ok(Self::Flow {
            name: ident.into(),
            mode,
        })
    }
}
