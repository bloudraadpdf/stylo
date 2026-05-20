/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for CSS Regions Module Level 1 longhands.
//!
//! Both `flow-into` and `flow-from` are identity-computed; the
//! specified types derive `ToComputedValue`.

pub use crate::values::specified::regions_1::{FlowFrom, FlowInto, FlowIntoMode};
