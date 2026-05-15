/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for PDF/X output intent + colour-conversion (F1).
//!
//! `BdPdfColourConversion` is identity-computed (keyword enum).
//! `BdPdfColourOptions` is identity-computed (bitset). The
//! output-intent and fallback-CMYK-profile values swap their
//! specified `SpecifiedUrl` for the computed `ComputedUrl`.

use crate::derives::*;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_pdf_output_intent as specified;
use crate::values::AtomIdent;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::bd_pdf_output_intent::{
    BdPdfColourConversion, BdPdfColourOption, BdPdfColourOptions,
};

/// Computed value of `-bd-pdf-output-intent`.
///
/// `ComputedUrl` (servo) does not implement `ToShmem`, so the
/// computed value cannot either. Pattern matches the other
/// computed types that hold a URL.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfOutputIntent {
    /// `auto` — defer to the conformance default.
    Auto,
    /// `none` — clear any registered OutputIntent.
    None,
    /// `<icc-profile-name>` — well-known profile identifier.
    Named(AtomIdent),
    /// `url(<profile-url>)` — fetched and embedded.
    Url(ComputedUrl),
}

impl BdPdfOutputIntent {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToCss for BdPdfOutputIntent {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::Auto => dest.write_str("auto"),
            Self::None => dest.write_str("none"),
            Self::Named(name) => name.to_css(dest),
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdPdfOutputIntent {
    type ComputedValue = BdPdfOutputIntent;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::Auto => BdPdfOutputIntent::Auto,
            Self::None => BdPdfOutputIntent::None,
            Self::Named(name) => BdPdfOutputIntent::Named(name.clone()),
            Self::Url(url) => BdPdfOutputIntent::Url(url.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfOutputIntent::Auto => Self::Auto,
            BdPdfOutputIntent::None => Self::None,
            BdPdfOutputIntent::Named(name) => Self::Named(name.clone()),
            BdPdfOutputIntent::Url(url) => {
                Self::Url(ToComputedValue::from_computed_value(url))
            },
        }
    }
}

/// Computed value of `-bd-pdf-fallback-cmyk-profile`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfFallbackCmykProfile {
    /// `none` — no fallback profile.
    None,
    /// `url(<profile-url>)` — fetched, embedded, and applied to CMYK.
    Url(ComputedUrl),
}

impl BdPdfFallbackCmykProfile {
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

impl ToCss for BdPdfFallbackCmykProfile {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Url(url) => url.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdPdfFallbackCmykProfile {
    type ComputedValue = BdPdfFallbackCmykProfile;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            Self::None => BdPdfFallbackCmykProfile::None,
            Self::Url(url) => BdPdfFallbackCmykProfile::Url(url.to_computed_value(ctx)),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfFallbackCmykProfile::None => Self::None,
            BdPdfFallbackCmykProfile::Url(url) => {
                Self::Url(ToComputedValue::from_computed_value(url))
            },
        }
    }
}
