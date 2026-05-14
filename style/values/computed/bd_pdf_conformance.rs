/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-pdf-conformance` and `-bd-pdf-version`.
//!
//! Specified-to-computed is the identity; the specified-side types
//! derive `ToComputedValue` for the round-trip.

pub use crate::values::specified::bd_pdf_conformance::{BdPdfConformanceValue, BdPdfVersionValue};
