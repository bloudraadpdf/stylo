/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for CSS Backgrounds 4 §5.5 `corner-shape`.
//!
//! `CornerShape` is its own computed value — the specified-side enum
//! already carries a `CSSFloat` (`f32`) Lamé exponent inside
//! `Superellipse`, with parse-time clamping applied so the value is
//! always finite and strictly positive. No further computation is
//! required at cascade time.

pub use crate::values::specified::corner_shape::CornerShape;
