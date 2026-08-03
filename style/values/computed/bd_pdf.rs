/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values of moegoe `-bd-pdf-*` document metadata properties.
//!
//! `None` / `Strings` round-trip identically; the `Url` variant
//! collapses `SpecifiedUrl -> ComputedUrl` so the cascade reader
//! receives the resolved URL bytes downstream. `ComputedUrl` does
//! not implement `ToShmem`, so the computed enum follows the same
//! pattern the F1 output-intent property uses (
//! [`crate::values::computed::bd_pdf_output_intent::BdPdfOutputIntent`]
//! ).

use crate::derives::*;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_pdf as specified;
use crate::{OwnedSlice, OwnedStr};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

/// Computed value of a `-bd-pdf-*` document-metadata property.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfMetaValue {
    /// `none` — no contribution to the PDF metadata slot.
    None,
    /// `<string>+` — one or more author strings.
    Strings(OwnedSlice<OwnedStr>),
    /// `url(<href>)` — external packet (XMP only). Other metadata
    /// slots reject the variant at the cascade boundary.
    Url(ComputedUrl),
}

impl BdPdfMetaValue {
    /// `none` value (initial).
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

impl ToCss for BdPdfMetaValue {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Strings(strings) => {
                let mut first = true;
                for s in strings.iter() {
                    if !first {
                        dest.write_char(' ')?;
                    }
                    s.to_css(dest)?;
                    first = false;
                }
                Ok(())
            },
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdPdfMetaValue {
    type ComputedValue = BdPdfMetaValue;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::None => BdPdfMetaValue::None,
            Self::Strings(strings) => BdPdfMetaValue::Strings(strings.clone()),
            Self::Url(url) => BdPdfMetaValue::Url(url.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfMetaValue::None => Self::None,
            BdPdfMetaValue::Strings(strings) => Self::Strings(strings.clone()),
            BdPdfMetaValue::Url(url) => Self::Url(ToComputedValue::from_computed_value(url)),
        }
    }
}
