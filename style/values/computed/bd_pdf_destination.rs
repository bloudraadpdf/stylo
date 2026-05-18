/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for moegoe PDF attachment / destination /
//! page-label surface (F9).

use crate::derives::*;
use crate::values::computed::url::ComputedUrl;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_pdf_destination as specified;

pub use specified::{
    BdDestinationArea, BdPdfAttachmentLocation, BdPdfAttachmentModificationDate,
    BdPdfAttachmentRelationship, BdPdfStringSlot,
};

/// Computed value of `-bd-pdf-attachment-url`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum BdPdfAttachmentUrl {
    /// `none` — no embedded file.
    None,
    /// `url(<file-url>)` — embedded file source.
    Url(ComputedUrl),
}

impl BdPdfAttachmentUrl {
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

impl ToComputedValue for specified::BdPdfAttachmentUrl {
    type ComputedValue = BdPdfAttachmentUrl;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdPdfAttachmentUrl::None => BdPdfAttachmentUrl::None,
            specified::BdPdfAttachmentUrl::Url(u) => {
                BdPdfAttachmentUrl::Url(u.to_computed_value(ctx))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdPdfAttachmentUrl::None => specified::BdPdfAttachmentUrl::None,
            BdPdfAttachmentUrl::Url(u) => {
                specified::BdPdfAttachmentUrl::Url(ToComputedValue::from_computed_value(u))
            }
        }
    }
}
