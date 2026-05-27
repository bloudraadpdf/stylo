/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for the `-bd-pdf-stamp-*` cluster (V1-20).
//!
//! Every specified value derives `ToComputedValue` directly (the
//! variants carry plain `OwnedStr` strings, no length/colour
//! resolution required), so the computed surface re-exports the
//! specified types verbatim.

pub use crate::values::specified::bd_pdf_stamp::{BdPdfStampIcon, BdPdfStampIntent, BdPdfStampString};
