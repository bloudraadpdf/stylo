/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for `-bd-tab-stops`.
//!
//! Alignment and leader keywords are identity-computed (re-exported
//! from the specified module). The list itself swaps the specified
//! `NonNegativeLength` for its computed equivalent via a manual
//! `ToComputedValue` walk.

use crate::derives::*;
use crate::values::computed::length::NonNegativeLength;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::bd_tab_stops as specified;
use crate::OwnedSlice;
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

pub use specified::{BdTabStopAlignment, BdTabStopLeader};

/// Computed value of one entry in a `-bd-tab-stops` list.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C)]
pub struct BdTabStop {
    /// Position of the tab stop, computed to a concrete length.
    pub position: NonNegativeLength,
    /// Alignment applied to text at this stop.
    pub alignment: BdTabStopAlignment,
    /// Leader glyph repeated up to this stop.
    pub leader: BdTabStopLeader,
}

impl ToCss for BdTabStop {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        self.position.to_css(dest)?;
        dest.write_char(' ')?;
        self.alignment.to_css(dest)?;
        if !self.leader.is_none() {
            dest.write_char(' ')?;
            self.leader.to_css(dest)?;
        }
        Ok(())
    }
}

/// Computed value of `-bd-tab-stops`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToResolvedValue, ToShmem, ToTyped)]
#[repr(C, u8)]
pub enum BdTabStops {
    /// `none` — no positional stops; fall back to interval `tab-size`.
    None,
    /// Comma-separated list of computed positional stops.
    Stops(OwnedSlice<BdTabStop>),
}

impl BdTabStops {
    /// `none` value.
    #[inline]
    pub fn none() -> Self {
        Self::None
    }

    /// Whether the value is `none`.
    #[inline]
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl ToCss for BdTabStops {
    fn to_css<W: Write>(&self, dest: &mut CssWriter<W>) -> fmt::Result {
        match self {
            Self::None => dest.write_str("none"),
            Self::Stops(stops) => {
                let mut first = true;
                for stop in stops.iter() {
                    if !first {
                        dest.write_str(", ")?;
                    }
                    stop.to_css(dest)?;
                    first = false;
                }
                Ok(())
            }
        }
    }
}

impl ToComputedValue for specified::BdTabStop {
    type ComputedValue = BdTabStop;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        BdTabStop {
            position: self.position.to_computed_value(ctx),
            alignment: self.alignment,
            leader: self.leader.clone(),
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        specified::BdTabStop {
            position: ToComputedValue::from_computed_value(&computed.position),
            alignment: computed.alignment,
            leader: computed.leader.clone(),
        }
    }
}

impl ToComputedValue for specified::BdTabStops {
    type ComputedValue = BdTabStops;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::BdTabStops::None => BdTabStops::None,
            specified::BdTabStops::Stops(stops) => {
                let computed: Vec<BdTabStop> =
                    stops.iter().map(|s| s.to_computed_value(ctx)).collect();
                BdTabStops::Stops(OwnedSlice::from(computed))
            }
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            BdTabStops::None => specified::BdTabStops::None,
            BdTabStops::Stops(stops) => {
                let specified: Vec<specified::BdTabStop> = stops
                    .iter()
                    .map(ToComputedValue::from_computed_value)
                    .collect();
                specified::BdTabStops::Stops(OwnedSlice::from(specified))
            }
        }
    }
}
