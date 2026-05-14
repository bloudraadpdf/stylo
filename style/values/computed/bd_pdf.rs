/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values of moegoe `-bd-pdf-*` document metadata properties.
//!
//! No real computation — specified strings round-trip identically to
//! computed values. The shared specified-side type carries the
//! `ToComputedValue` derive that performs the identity conversion.

pub use crate::values::specified::bd_pdf::BdPdfMetaValue;
