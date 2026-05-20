/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-trapped` property (K7).
//!
//! Document-level descriptor projecting onto the PDF info-dictionary
//! `/Trapped` entry (ISO 32000-2 §14.11.3). krilla maps the moegoe
//! values onto `krilla::Metadata::trapped(Trapping::*)`:
//!
//! - `unknown` (initial) — `Trapping::Unknown`, equivalent to omitting
//!   the `/Trapped` key.
//! - `true` — `Trapping::Trapped`, the document is fully trapped.
//! - `false` — `Trapping::NotTrapped`, the document has not been
//!   trapped.
//!
//! The cascade reader only honours declarations on `:root`.

use crate::derives::*;

/// Specified value of the `-bd-pdf-trapped` property.
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
pub enum BdPdfTrapped {
    /// `unknown` — `/Trapped` omitted from the info dictionary.
    #[default]
    Unknown,
    /// `true` — `/Trapped /True`; document is fully trapped.
    True,
    /// `false` — `/Trapped /False`; document is not trapped.
    False,
}

impl BdPdfTrapped {
    /// Whether the value is `unknown` (the initial value).
    #[inline]
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}
