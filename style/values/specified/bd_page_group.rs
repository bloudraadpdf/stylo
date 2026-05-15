/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe page-group surface (F30).
//!
//! `-bd-page-group: start | auto` declares that an element opens a
//! new page group (Prince-for-Books semantics). The `:first-of-group`
//! page pseudo-class lives separately in
//! `style/selector_parser/`.

use crate::derives::*;

/// Specified value of `-bd-page-group`.
#[repr(u8)]
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    Hash,
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
pub enum BdPageGroup {
    #[default]
    Auto,
    Start,
}
