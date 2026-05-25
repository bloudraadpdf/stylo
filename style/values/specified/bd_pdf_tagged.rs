/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe `-bd-pdf-tagged` document-level toggle.
//!
//! Forces (or suppresses) Tagged-PDF structure-tree emission from the
//! CSS surface. The renderer's `PdfConfig::tagged` API field today is
//! the only way to opt in; `-bd-pdf-tagged` exposes the toggle to
//! authors so a stylesheet can request a tagged output without the
//! embedder reconfiguring the renderer.
//!
//! - `auto` (initial) — defer to the embedder's `PdfConfig::tagged`
//!   value. No cascade-side override is applied.
//! - `yes`             — force `PdfConfig::tagged = true`.
//! - `no`              — force `PdfConfig::tagged = false`.
//!
//! The cascade reader only honours declarations on `:root`.
//!
//! Prince spells this `prince-pdf-tagged: auto | yes | no` (Prince
//! manual, `prince.md`); the translator at
//! `crates/moegoe-css/src/compat/translate.rs` aliases the Prince
//! property name onto this native longhand under
//! `CompatMode::Prince`.

use crate::derives::*;

/// Specified value of the `-bd-pdf-tagged` property.
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
pub enum BdPdfTagged {
    /// `auto` — defer to the embedder's `PdfConfig::tagged` value.
    #[default]
    Auto,
    /// `yes` — force tagged PDF emission on.
    Yes,
    /// `no` — force tagged PDF emission off.
    No,
}

impl BdPdfTagged {
    /// Whether the value is `auto` (initial — no cascade override).
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}
