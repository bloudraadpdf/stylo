/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for counter properties

use crate::derives::*;
use crate::values::computed::image::Image;
use crate::values::generics::counters as generics;
use crate::values::generics::counters::CounterIncrement as GenericCounterIncrement;
use crate::values::generics::counters::CounterReset as GenericCounterReset;
use crate::values::generics::counters::CounterSet as GenericCounterSet;
use crate::values::generics::counters::StringSet as GenericStringSet;
use crate::values::resolved::{Context as ResolvedContext, ToResolvedValue};
use std::fmt::{self, Write};
use style_traits::{CssWriter, ToCss};

/// A computed integer used by CSS counters.
#[derive(Clone, Copy, Debug, Default, MallocSizeOf, PartialEq, ToShmem, ToTyped)]
pub struct CounterInteger(i64);

impl CounterInteger {
    /// Construct a computed counter integer.
    pub fn new(value: i64) -> Self {
        Self(value)
    }

    /// Return the computed counter integer.
    pub fn value(self) -> i64 {
        self.0
    }
}

impl PartialEq<i64> for CounterInteger {
    fn eq(&self, value: &i64) -> bool {
        self.0 == *value
    }
}

impl ToCss for CounterInteger {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        write!(dest, "{}", self.0)
    }
}

impl ToResolvedValue for CounterInteger {
    type ResolvedValue = Self;

    fn to_resolved_value(self, _: &ResolvedContext) -> Self::ResolvedValue {
        self
    }

    fn from_resolved_value(resolved: Self::ResolvedValue) -> Self {
        resolved
    }
}

/// A computed value for the `counter-increment` property.
pub type CounterIncrement = GenericCounterIncrement<CounterInteger>;

/// A computed value for the `counter-reset` property.
pub type CounterReset = GenericCounterReset<CounterInteger>;

/// A computed value for the `counter-set` property.
pub type CounterSet = GenericCounterSet<CounterInteger>;

/// A computed value for the `content` property.
pub type Content = generics::GenericContent<Image>;

/// A computed content item.
pub type ContentItem = generics::GenericContentItem<Image>;

/// A computed value for the `string-set` property.
pub type StringSet = GenericStringSet<Image>;

use crate::values::generics::counters::BookmarkLabel as GenericBookmarkLabel;

/// A computed value for the `bookmark-label` property.
pub type BookmarkLabel = GenericBookmarkLabel<Image>;
