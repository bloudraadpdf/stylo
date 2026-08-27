/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Generic values for CSS gap decorations.

use crate::derives::*;
use crate::values::animated::{Animate, Procedure, ToAnimatedZero};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

/// The repetition count in a gap-decoration `repeat()` item.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum GapRuleRepeatCount<Integer> {
    /// A positive integer repetition count.
    Number(Integer),
    /// The auto repeater which fills gaps not reserved by surrounding items.
    Auto,
}

impl<Integer: ToCss> ToCss for GapRuleRepeatCount<Integer> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Number(count) => count.to_css(dest),
            Self::Auto => dest.write_str("auto"),
        }
    }
}

/// One value or repeater in a gap-decoration property.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C, u8)]
pub enum GenericGapRuleListItem<Value, Integer> {
    /// A single decoration value.
    Value(Value),
    /// A fixed or automatic repetition of one or more values.
    Repeat {
        /// The fixed count or `auto` keyword.
        count: GapRuleRepeatCount<Integer>,
        /// The non-empty body of the repeater.
        values: crate::OwnedSlice<Value>,
    },
}

impl<Value: ToCss, Integer: ToCss> ToCss for GenericGapRuleListItem<Value, Integer> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Value(value) => value.to_css(dest),
            Self::Repeat { count, values } => {
                dest.write_str("repeat(")?;
                count.to_css(dest)?;
                for value in values.iter() {
                    dest.write_str(", ")?;
                    value.to_css(dest)?;
                }
                dest.write_char(')')
            },
        }
    }
}

/// A non-empty comma-separated list of gap-decoration values and repeaters.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToAnimatedValue,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(C)]
pub struct GenericGapRuleList<Value, Integer>(
    pub crate::OwnedSlice<GenericGapRuleListItem<Value, Integer>>,
);

impl<Value, Integer> GenericGapRuleList<Value, Integer> {
    /// Construct a one-value list.
    pub fn single(value: Value) -> Self {
        Self(crate::OwnedSlice::from(vec![
            GenericGapRuleListItem::Value(value),
        ]))
    }
}

impl<Value, Integer> Animate for GenericGapRuleListItem<Value, Integer>
where
    Value: Animate,
    Integer: Clone + PartialEq,
{
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        match (self, other) {
            (Self::Value(left), Self::Value(right)) => {
                Ok(Self::Value(left.animate(right, procedure)?))
            },
            (
                Self::Repeat {
                    count: left_count,
                    values: left_values,
                },
                Self::Repeat {
                    count: right_count,
                    values: right_values,
                },
            ) if left_count == right_count => Ok(Self::Repeat {
                count: left_count.clone(),
                values: crate::OwnedSlice::from(
                    crate::values::animated::lists::by_computed_value::animate::<_, Vec<_>>(
                        left_values,
                        right_values,
                        procedure,
                    )?,
                ),
            }),
            _ => Err(()),
        }
    }
}

impl<Value, Integer> ComputeSquaredDistance for GenericGapRuleListItem<Value, Integer>
where
    Value: ComputeSquaredDistance,
    Integer: PartialEq,
{
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        match (self, other) {
            (Self::Value(left), Self::Value(right)) => left.compute_squared_distance(right),
            (
                Self::Repeat {
                    count: left_count,
                    values: left_values,
                },
                Self::Repeat {
                    count: right_count,
                    values: right_values,
                },
            ) if left_count == right_count => {
                crate::values::animated::lists::by_computed_value::squared_distance(
                    left_values,
                    right_values,
                )
            },
            _ => Err(()),
        }
    }
}

impl<Value, Integer> ToAnimatedZero for GenericGapRuleListItem<Value, Integer>
where
    Value: ToAnimatedZero,
    Integer: Clone,
{
    fn to_animated_zero(&self) -> Result<Self, ()> {
        match self {
            Self::Value(value) => Ok(Self::Value(value.to_animated_zero()?)),
            Self::Repeat { count, values } => Ok(Self::Repeat {
                count: count.clone(),
                values: crate::OwnedSlice::from(
                    values
                        .iter()
                        .map(ToAnimatedZero::to_animated_zero)
                        .collect::<Result<Vec<_>, _>>()?,
                ),
            }),
        }
    }
}

impl<Value, Integer> Animate for GenericGapRuleList<Value, Integer>
where
    GenericGapRuleListItem<Value, Integer>: Animate,
{
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        Ok(Self(crate::OwnedSlice::from(
            crate::values::animated::lists::repeatable_list::animate::<_, Vec<_>>(
                &self.0, &other.0, procedure,
            )?,
        )))
    }
}

impl<Value, Integer> ComputeSquaredDistance for GenericGapRuleList<Value, Integer>
where
    GenericGapRuleListItem<Value, Integer>: ComputeSquaredDistance,
{
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        crate::values::animated::lists::repeatable_list::squared_distance(&self.0, &other.0)
    }
}

impl<Value, Integer> ToAnimatedZero for GenericGapRuleList<Value, Integer>
where
    GenericGapRuleListItem<Value, Integer>: ToAnimatedZero,
{
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Ok(Self(crate::OwnedSlice::from(
            self.0
                .iter()
                .map(ToAnimatedZero::to_animated_zero)
                .collect::<Result<Vec<_>, _>>()?,
        )))
    }
}

impl<Value: ToCss, Integer: ToCss> ToCss for GenericGapRuleList<Value, Integer> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        for (index, item) in self.0.iter().enumerate() {
            if index != 0 {
                dest.write_str(", ")?;
            }
            item.to_css(dest)?;
        }
        Ok(())
    }
}

pub use self::GenericGapRuleList as GapRuleList;
pub use self::GenericGapRuleListItem as GapRuleListItem;
