/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-format` property (G3).
//!
//! AcroForm interactive-form opt-in. `pdf` declares that the
//! document contains widget annotations driven by HTML form
//! controls (ISO 32000-2 §12.7). v1 ships the parse surface
//! only; full widget emission via krilla is deferred to a
//! separate workstream. The moegoe renderer emits a
//! `RenderWarning::UnsupportedPdfFeature` when any computed
//! `-bd-pdf-format: pdf` is observed in the cascade.

use crate::derives::*;

/// Specified value of `-bd-pdf-format`.
///
/// `none` (initial) — the document opts out of AcroForm widgets.
/// `pdf` — the document is flagged as carrying AcroForms; the
/// runtime emits an "unsupported" warning until widget plumbing
/// lands.
#[repr(u8)]
#[derive(
    Clone,
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
pub enum BdPdfFormat {
    #[default]
    None,
    Pdf,
}

impl BdPdfFormat {
    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Whether the value is `pdf` (AcroForm requested).
    #[inline]
    pub fn is_pdf(&self) -> bool {
        matches!(self, Self::Pdf)
    }
}
