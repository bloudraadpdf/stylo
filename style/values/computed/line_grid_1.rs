/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for CSS Line Grid Level 1 standard longhands.
//!
//! `line-grid` and `box-snap` are identity-computed; the specified
//! types derive `ToComputedValue`.

pub use crate::values::specified::line_grid_1::{BoxSnap, LineGrid};
