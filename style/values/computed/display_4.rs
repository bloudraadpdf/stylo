/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for CSS Display Module Level 4 longhands.
//!
//! `reading-flow` is identity-computed; `reading-order` uses the
//! pre-existing `Integer` machinery and therefore needs no computed
//! re-export here.

pub use crate::values::specified::display_4::ReadingFlow;
