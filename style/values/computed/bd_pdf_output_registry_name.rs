/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed value of `-bd-pdf-output-registry-name`.
//!
//! The `<url>` variant requires `SpecifiedUrl` -> `ComputedUrl`
//! conversion, so this type cannot be a plain re-export of the
//! specified-side enum.

use crate::derives::*;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_pdf_output_registry_name as specified;
use crate::OwnedStr;

/// Computed value of the `-bd-pdf-output-registry-name` property.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfOutputRegistryName {
    /// `none` — no `RegistryName` entry is emitted.
    None,
    /// `<url>` — registry identifier URL.
    Url(ComputedUrl),
    /// `<string>` — literal registry-name string.
    String(OwnedStr),
}

impl BdPdfOutputRegistryName {
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

impl ToComputedValue for specified::BdPdfOutputRegistryName {
    type ComputedValue = BdPdfOutputRegistryName;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfOutputRegistryName::None => BdPdfOutputRegistryName::None,
            specified::BdPdfOutputRegistryName::Url(u) => {
                BdPdfOutputRegistryName::Url(u.to_computed_value(ctx))
            }
            specified::BdPdfOutputRegistryName::String(s) => {
                BdPdfOutputRegistryName::String(s.clone())
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfOutputRegistryName::None => specified::BdPdfOutputRegistryName::None,
            BdPdfOutputRegistryName::Url(u) => {
                specified::BdPdfOutputRegistryName::Url(ToComputedValue::from_computed_value(u))
            }
            BdPdfOutputRegistryName::String(s) => {
                specified::BdPdfOutputRegistryName::String(s.clone())
            }
        }
    }
}
