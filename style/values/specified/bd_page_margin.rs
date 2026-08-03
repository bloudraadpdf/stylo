/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! moegoe asymmetric / alternating page-margin surface (F29).
//!
//! - `-bd-margin-inside`, `-bd-margin-outside` — verso/recto
//!   asymmetric `@page` margins.
//! - `-bd-margin-alt` — alternating margin opposite to current
//!   margin-left/right (Prince's spelling).
//! - `-bd-inset-inside`, `-bd-inset-outside` — equivalent inset
//!   pair for the binding edge.
//!
//! All values accept the standard `Margin` grammar
//! (`<length-percentage> | auto`).

use crate::derives::*;
use crate::values::specified::length::LengthPercentageOrAuto;

/// Specified value of `-bd-margin-{inside,outside,alt}` and
/// `-bd-inset-{inside,outside}`.
#[derive(
    Clone, Debug, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToCss, ToShmem, ToTyped, Parse,
)]
#[repr(C)]
pub struct BdPageMarginEdge(pub LengthPercentageOrAuto);

impl BdPageMarginEdge {
    /// Initial value (`auto`).
    #[inline]
    pub fn auto() -> Self {
        Self(LengthPercentageOrAuto::Auto)
    }
}
