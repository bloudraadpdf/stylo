/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values of moegoe `-bd-pdf-script` and
//! `-bd-pdf-event-scripts` (G64).
//!
//! Specified-to-computed is the identity for both — the strings round
//! through unchanged, and the event-key enum is `Copy`.

pub use crate::values::specified::bd_pdf_script::{
    BdPdfEventKind, BdPdfEventScript, BdPdfEventScripts, BdPdfScript, BdPdfWidgetActionScript,
};
