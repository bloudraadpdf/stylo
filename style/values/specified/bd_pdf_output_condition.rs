/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-output-condition` property (K10).
//!
//! Document-level descriptor projecting onto the PDF output-intent
//! `OutputCondition` entry (ISO 32000-2 §14.11.5, Table 401).
//!
//! Grammar: `none | <string>`. `none` (initial) leaves the slot
//! unset — bladsy will not emit an `OutputCondition` key on the
//! output-intent dictionary. A literal `<string>` is round-tripped
//! verbatim.
//!
//! The cascade reader only honours declarations on `:root`.

use crate::derives::*;
use crate::parser::{Parse, ParserContext};
use crate::OwnedStr;
use cssparser::Parser;
use style_traits::ParseError;

/// Specified value of the `-bd-pdf-output-condition` property.
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
pub enum BdPdfOutputCondition {
    /// `none` — no `OutputCondition` entry is emitted.
    None,
    /// `<string>` — literal output-condition descriptor.
    String(OwnedStr),
}

impl BdPdfOutputCondition {
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

impl Parse for BdPdfOutputCondition {
    fn parse<'i, 't>(
        _: &ParserContext,
        input: &mut Parser<'i, 't>,
    ) -> Result<Self, ParseError<'i>> {
        if input.try_parse(|i| i.expect_ident_matching("none")).is_ok() {
            return Ok(Self::None);
        }
        let s = input.expect_string()?;
        Ok(Self::String(s.as_ref().to_owned().into()))
    }
}
