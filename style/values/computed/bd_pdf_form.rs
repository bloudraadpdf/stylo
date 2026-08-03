/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-pdf-form-field-*` /
//! `-bd-pdf-signature-field-*` (G3 — Family 26).
//!
//! Specified-to-computed is the identity for every keyword / integer
//! variant. The colour and url variants follow the same pattern as
//! `BdPdfMetaValue` — collapse the inner type at compute time and
//! resolve `currentcolor` against the cascade context.

use crate::derives::*;
use crate::values::computed::color::Color as ComputedColor;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, Percentage, ToComputedValue};
use crate::values::specified::bd_pdf_form as specified;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use crate::values::specified::bd_pdf_form::{
    BdPdfAnnotationHidden, BdPdfFormFieldFlags, BdPdfFormFieldMaxLength,
    BdPdfFormFieldMkIconFitScaleType, BdPdfFormFieldMkIconFitScaleWhen, BdPdfFormFieldMkRotation,
    BdPdfFormFieldMkTextPosition, BdPdfSignatureFieldLock, BdPdfSignatureFieldLockFields,
    BdPdfSignatureFieldName,
};

/// Computed value of `-bd-pdf-form-field-mk-{border,background}-colour`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfFormFieldMkColour {
    /// `none` — entry omitted from `/MK`.
    None,
    /// Explicit colour.
    Colour(ComputedColor),
}

impl BdPdfFormFieldMkColour {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for BdPdfFormFieldMkColour {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Colour(c) => c.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdPdfFormFieldMkColour {
    type ComputedValue = BdPdfFormFieldMkColour;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfFormFieldMkColour::None => BdPdfFormFieldMkColour::None,
            specified::BdPdfFormFieldMkColour::Colour(c) => {
                BdPdfFormFieldMkColour::Colour(c.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfFormFieldMkColour::None => specified::BdPdfFormFieldMkColour::None,
            BdPdfFormFieldMkColour::Colour(c) => {
                specified::BdPdfFormFieldMkColour::Colour(ToComputedValue::from_computed_value(c))
            },
        }
    }
}

/// Computed value of `-bd-pdf-form-field-mk-{rollover,down}-caption`.
///
/// Identity over the [`specified`] enum (the string variant carries
/// `OwnedStr` which already implements every required derive).
pub use crate::values::specified::bd_pdf_form::BdPdfFormFieldMkCaption;

/// Computed value of `-bd-pdf-form-field-mk-{rollover,alternate}-icon`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfFormFieldMkIcon {
    /// `none` — entry omitted from `/MK`.
    None,
    /// `url(<href>)` — external icon image.
    Url(ComputedUrl),
}

impl BdPdfFormFieldMkIcon {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for BdPdfFormFieldMkIcon {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Url(u) => u.to_css(dest),
        }
    }
}

impl ToComputedValue for specified::BdPdfFormFieldMkIcon {
    type ComputedValue = BdPdfFormFieldMkIcon;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfFormFieldMkIcon::None => BdPdfFormFieldMkIcon::None,
            specified::BdPdfFormFieldMkIcon::Url(u) => {
                BdPdfFormFieldMkIcon::Url(u.to_computed_value(ctx))
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfFormFieldMkIcon::None => specified::BdPdfFormFieldMkIcon::None,
            BdPdfFormFieldMkIcon::Url(u) => {
                specified::BdPdfFormFieldMkIcon::Url(ToComputedValue::from_computed_value(u))
            },
        }
    }
}

/// Computed value of `-bd-pdf-form-field-mk-icon-fit`. The two
/// percentages are computed identically; the keyword components are
/// already identity-computed.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfFormFieldMkIconFit {
    /// `none` — entry omitted from `/MK`.
    None,
    /// Explicit icon-fit triple.
    Fit(BdPdfFormFieldMkIconFitValue),
}

/// The non-`none` payload of [`BdPdfFormFieldMkIconFit`].
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToTyped)]
#[repr(C)]
pub struct BdPdfFormFieldMkIconFitValue {
    /// `/IF /S`.
    pub scale_type: BdPdfFormFieldMkIconFitScaleType,
    /// `/IF /SW`.
    pub scale_when: BdPdfFormFieldMkIconFitScaleWhen,
    /// `/IF /A[0]`.
    pub align_x: Percentage,
    /// `/IF /A[1]`.
    pub align_y: Percentage,
}

impl BdPdfFormFieldMkIconFit {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for BdPdfFormFieldMkIconFit {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Fit(value) => {
                value.scale_type.to_css(dest)?;
                dest.write_char(' ')?;
                value.scale_when.to_css(dest)?;
                dest.write_char(' ')?;
                value.align_x.to_css(dest)?;
                dest.write_char(' ')?;
                value.align_y.to_css(dest)
            },
        }
    }
}

impl ToComputedValue for specified::BdPdfFormFieldMkIconFit {
    type ComputedValue = BdPdfFormFieldMkIconFit;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfFormFieldMkIconFit::None => BdPdfFormFieldMkIconFit::None,
            specified::BdPdfFormFieldMkIconFit::Fit(v) => {
                BdPdfFormFieldMkIconFit::Fit(BdPdfFormFieldMkIconFitValue {
                    scale_type: v.scale_type,
                    scale_when: v.scale_when,
                    align_x: v.align_x.to_computed_value(ctx),
                    align_y: v.align_y.to_computed_value(ctx),
                })
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfFormFieldMkIconFit::None => specified::BdPdfFormFieldMkIconFit::None,
            BdPdfFormFieldMkIconFit::Fit(v) => {
                specified::BdPdfFormFieldMkIconFit::Fit(specified::BdPdfFormFieldMkIconFitValue {
                    scale_type: v.scale_type,
                    scale_when: v.scale_when,
                    align_x: ToComputedValue::from_computed_value(&v.align_x),
                    align_y: ToComputedValue::from_computed_value(&v.align_y),
                })
            },
        }
    }
}
