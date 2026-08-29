/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Generic values for CSS gap decorations.

use crate::derives::*;
use crate::values::animated::{Animate, Procedure, ToAnimatedZero};
use crate::values::distance::{ComputeSquaredDistance, SquaredDistance};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

/// A non-empty owned sequence used by gap-decoration lists and repeaters.
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
pub struct NonEmptyGapRuleValues<Value>(crate::OwnedSlice<Value>);

impl<Value> NonEmptyGapRuleValues<Value> {
    /// Construct a sequence, rejecting an empty input.
    pub fn from_vec(values: Vec<Value>) -> Option<Self> {
        (!values.is_empty()).then(|| Self(crate::OwnedSlice::from(values)))
    }

    /// Iterate over every value in order.
    pub fn iter(&self) -> impl Iterator<Item = &Value> {
        self.0.iter()
    }

    /// Return the number of values.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

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
        values: NonEmptyGapRuleValues<Value>,
    },
}

impl<Value, Integer> GenericGapRuleListItem<Value, Integer> {
    /// Construct a repeater, rejecting an empty body.
    pub fn repeat(count: GapRuleRepeatCount<Integer>, values: Vec<Value>) -> Option<Self> {
        Some(Self::Repeat {
            count,
            values: NonEmptyGapRuleValues::from_vec(values)?,
        })
    }
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
    NonEmptyGapRuleValues<GenericGapRuleListItem<Value, Integer>>,
);

impl<Value, Integer> GenericGapRuleList<Value, Integer> {
    /// Construct a one-value list.
    pub fn single(value: Value) -> Self {
        Self(NonEmptyGapRuleValues(crate::OwnedSlice::from(vec![
            GenericGapRuleListItem::Value(value),
        ])))
    }

    /// Construct a list, rejecting an empty input.
    pub fn from_vec(items: Vec<GenericGapRuleListItem<Value, Integer>>) -> Option<Self> {
        Some(Self(NonEmptyGapRuleValues::from_vec(items)?))
    }

    /// Iterate over every list item in order.
    pub fn iter(&self) -> impl Iterator<Item = &GenericGapRuleListItem<Value, Integer>> {
        self.0.iter()
    }

    /// Return the number of list items before fixed-repeater expansion.
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

enum ExpandedGapRuleList<Value> {
    Fixed(Vec<Value>),
    Auto {
        leading: Vec<Value>,
        repeated: Vec<Value>,
        trailing: Vec<Value>,
    },
}

impl<Value, Integer> GenericGapRuleList<Value, Integer>
where
    Value: Clone,
    Integer: Copy + TryInto<usize>,
{
    fn expanded(&self) -> Result<ExpandedGapRuleList<Value>, ()> {
        let mut leading = Vec::new();
        let mut repeated = None;
        let mut trailing = Vec::new();

        for item in self.iter() {
            let destination = if repeated.is_some() {
                &mut trailing
            } else {
                &mut leading
            };
            match item {
                GenericGapRuleListItem::Value(value) => destination.push(value.clone()),
                GenericGapRuleListItem::Repeat {
                    count: GapRuleRepeatCount::Number(count),
                    values,
                } => {
                    let count = (*count).try_into().map_err(|_| ())?;
                    let additional = values.len().checked_mul(count).ok_or(())?;
                    destination.try_reserve(additional).map_err(|_| ())?;
                    for _ in 0..count {
                        destination.extend(values.iter().cloned());
                    }
                },
                GenericGapRuleListItem::Repeat {
                    count: GapRuleRepeatCount::Auto,
                    values,
                } => {
                    if repeated.is_some() {
                        return Err(());
                    }
                    repeated = Some(values.iter().cloned().collect());
                },
            }
        }

        match repeated {
            Some(repeated) => Ok(ExpandedGapRuleList::Auto {
                leading,
                repeated,
                trailing,
            }),
            None if leading.is_empty() => Err(()),
            None => Ok(ExpandedGapRuleList::Fixed(leading)),
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
                values: NonEmptyGapRuleValues::from_vec(
                    values
                        .iter()
                        .map(ToAnimatedZero::to_animated_zero)
                        .collect::<Result<Vec<_>, _>>()?,
                )
                .ok_or(())?,
            }),
        }
    }
}

impl<Value, Integer> Animate for GenericGapRuleList<Value, Integer>
where
    Value: Animate + Clone,
    Integer: Copy + TryInto<usize>,
{
    fn animate(&self, other: &Self, procedure: Procedure) -> Result<Self, ()> {
        use crate::values::animated::lists::{by_computed_value, repeatable_list};

        let items: Vec<GenericGapRuleListItem<Value, Integer>> =
            match (self.expanded()?, other.expanded()?) {
                (ExpandedGapRuleList::Fixed(left), ExpandedGapRuleList::Fixed(right)) => {
                    repeatable_list::animate::<_, Vec<_>>(&left, &right, procedure)?
                        .into_iter()
                        .map(GenericGapRuleListItem::Value)
                        .collect()
                },
                (
                    ExpandedGapRuleList::Auto {
                        leading: left_leading,
                        repeated: left_repeated,
                        trailing: left_trailing,
                    },
                    ExpandedGapRuleList::Auto {
                        leading: right_leading,
                        repeated: right_repeated,
                        trailing: right_trailing,
                    },
                ) => {
                    let leading = by_computed_value::animate::<_, Vec<_>>(
                        &left_leading,
                        &right_leading,
                        procedure,
                    )?;
                    let repeated = repeatable_list::animate::<_, Vec<_>>(
                        &left_repeated,
                        &right_repeated,
                        procedure,
                    )?;
                    let trailing = by_computed_value::animate::<_, Vec<_>>(
                        &left_trailing,
                        &right_trailing,
                        procedure,
                    )?;
                    leading
                        .into_iter()
                        .map(GenericGapRuleListItem::Value)
                        .chain(std::iter::once(GenericGapRuleListItem::Repeat {
                            count: GapRuleRepeatCount::Auto,
                            values: NonEmptyGapRuleValues::from_vec(repeated).ok_or(())?,
                        }))
                        .chain(trailing.into_iter().map(GenericGapRuleListItem::Value))
                        .collect()
                },
                (ExpandedGapRuleList::Fixed(_), ExpandedGapRuleList::Auto { .. })
                | (ExpandedGapRuleList::Auto { .. }, ExpandedGapRuleList::Fixed(_)) => {
                    return Err(())
                },
            };
        Self::from_vec(items).ok_or(())
    }
}

impl<Value, Integer> ComputeSquaredDistance for GenericGapRuleList<Value, Integer>
where
    Value: Clone + ComputeSquaredDistance,
    Integer: Copy + TryInto<usize>,
{
    fn compute_squared_distance(&self, other: &Self) -> Result<SquaredDistance, ()> {
        use crate::values::animated::lists::{by_computed_value, repeatable_list};

        match (self.expanded()?, other.expanded()?) {
            (ExpandedGapRuleList::Fixed(left), ExpandedGapRuleList::Fixed(right)) => {
                repeatable_list::squared_distance(&left, &right)
            },
            (
                ExpandedGapRuleList::Auto {
                    leading: left_leading,
                    repeated: left_repeated,
                    trailing: left_trailing,
                },
                ExpandedGapRuleList::Auto {
                    leading: right_leading,
                    repeated: right_repeated,
                    trailing: right_trailing,
                },
            ) => Ok(
                by_computed_value::squared_distance(&left_leading, &right_leading)?
                    + repeatable_list::squared_distance(&left_repeated, &right_repeated)?
                    + by_computed_value::squared_distance(&left_trailing, &right_trailing)?,
            ),
            (ExpandedGapRuleList::Fixed(_), ExpandedGapRuleList::Auto { .. })
            | (ExpandedGapRuleList::Auto { .. }, ExpandedGapRuleList::Fixed(_)) => Err(()),
        }
    }
}

impl<Value, Integer> ToAnimatedZero for GenericGapRuleList<Value, Integer>
where
    GenericGapRuleListItem<Value, Integer>: ToAnimatedZero,
{
    fn to_animated_zero(&self) -> Result<Self, ()> {
        Self::from_vec(
            self.iter()
                .map(ToAnimatedZero::to_animated_zero)
                .collect::<Result<Vec<_>, _>>()?,
        )
        .ok_or(())
    }
}

impl<Value: ToCss, Integer: ToCss> ToCss for GenericGapRuleList<Value, Integer> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        for (index, item) in self.iter().enumerate() {
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

#[cfg(test)]
mod tests {
    use super::{GapRuleList, GapRuleListItem, GapRuleRepeatCount};
    use crate::values::animated::{Animate, Procedure};

    type TestList = GapRuleList<f64, i32>;
    type TestItem = GapRuleListItem<f64, i32>;

    fn list(items: Vec<TestItem>) -> TestList {
        GapRuleList::from_vec(items).expect("test list is non-empty")
    }

    fn auto(items: Vec<f64>) -> TestItem {
        TestItem::repeat(GapRuleRepeatCount::Auto, items).expect("test repeater is non-empty")
    }

    #[test]
    fn constructors_reject_empty_gap_rule_sequences() {
        assert!(TestList::from_vec(Vec::new()).is_none());
        assert!(TestItem::repeat(GapRuleRepeatCount::Auto, Vec::new()).is_none());
    }

    #[test]
    fn integer_repeaters_expand_before_repeatable_list_interpolation() {
        let from = list(vec![TestItem::repeat(
            GapRuleRepeatCount::Number(2),
            vec![10.0, 20.0],
        )
        .expect("test repeater is non-empty")]);
        let to = list(vec![TestItem::Value(20.0)]);

        assert_eq!(
            from.animate(&to, Procedure::Interpolate { progress: 0.5 }),
            Ok(list(vec![
                TestItem::Value(15.0),
                TestItem::Value(20.0),
                TestItem::Value(15.0),
                TestItem::Value(20.0),
            ])),
        );
    }

    #[test]
    fn auto_repeater_bodies_use_repeatable_list_interpolation() {
        let from = list(vec![
            TestItem::Value(10.0),
            auto(vec![20.0]),
            TestItem::Value(30.0),
        ]);
        let to = list(vec![
            TestItem::Value(20.0),
            auto(vec![40.0, 50.0]),
            TestItem::Value(40.0),
        ]);

        assert_eq!(
            from.animate(&to, Procedure::Interpolate { progress: 0.5 }),
            Ok(list(vec![
                TestItem::Value(15.0),
                auto(vec![30.0, 35.0]),
                TestItem::Value(35.0),
            ])),
        );
    }
}
