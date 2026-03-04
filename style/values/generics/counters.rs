/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Generic types for counters-related CSS values.

#[cfg(feature = "servo")]
use crate::computed_values::list_style_type::T as ListStyleType;
#[cfg(feature = "gecko")]
use crate::counter_style::CounterStyle;
use crate::derives::*;
use crate::values::specified::Attr;
use crate::values::CustomIdent;
use std::fmt::{self, Write};
use std::ops::Deref;
use style_traits::{CssWriter, ToCss};

/// A name / value pair for counters.
#[derive(
    Clone,
    Debug,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C)]
pub struct GenericCounterPair<Integer> {
    /// The name of the counter.
    pub name: CustomIdent,
    /// The value of the counter / increment / etc.
    pub value: Integer,
    /// If true, then this represents `reversed(name)`.
    /// NOTE: It can only be true on `counter-reset` values.
    pub is_reversed: bool,
}
pub use self::GenericCounterPair as CounterPair;

impl<Integer> ToCss for CounterPair<Integer>
where
    Integer: ToCss + PartialEq<i32>,
{
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if self.is_reversed {
            dest.write_str("reversed(")?;
        }
        self.name.to_css(dest)?;
        if self.is_reversed {
            dest.write_char(')')?;
            if self.value == i32::min_value() {
                return Ok(());
            }
        }
        dest.write_char(' ')?;
        self.value.to_css(dest)
    }
}

/// A generic value for the `counter-increment` property.
#[derive(
    Clone,
    Debug,
    Default,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(transparent)]
pub struct GenericCounterIncrement<I>(#[css(field_bound)] pub GenericCounters<I>);
pub use self::GenericCounterIncrement as CounterIncrement;

impl<I> CounterIncrement<I> {
    /// Returns a new value for `counter-increment`.
    #[inline]
    pub fn new(counters: Vec<CounterPair<I>>) -> Self {
        CounterIncrement(Counters(counters.into()))
    }
}

impl<I> Deref for CounterIncrement<I> {
    type Target = [CounterPair<I>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &(self.0).0
    }
}

/// A generic value for the `counter-set` property.
#[derive(
    Clone,
    Debug,
    Default,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(transparent)]
pub struct GenericCounterSet<I>(#[css(field_bound)] pub GenericCounters<I>);
pub use self::GenericCounterSet as CounterSet;

impl<I> CounterSet<I> {
    /// Returns a new value for `counter-set`.
    #[inline]
    pub fn new(counters: Vec<CounterPair<I>>) -> Self {
        CounterSet(Counters(counters.into()))
    }
}

impl<I> Deref for CounterSet<I> {
    type Target = [CounterPair<I>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &(self.0).0
    }
}

/// A generic value for the `counter-reset` property.
#[derive(
    Clone,
    Debug,
    Default,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(transparent)]
pub struct GenericCounterReset<I>(#[css(field_bound)] pub GenericCounters<I>);
pub use self::GenericCounterReset as CounterReset;

impl<I> CounterReset<I> {
    /// Returns a new value for `counter-reset`.
    #[inline]
    pub fn new(counters: Vec<CounterPair<I>>) -> Self {
        CounterReset(Counters(counters.into()))
    }
}

impl<I> Deref for CounterReset<I> {
    type Target = [CounterPair<I>];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &(self.0).0
    }
}

/// A generic value for lists of counters.
///
/// Keyword `none` is represented by an empty vector.
#[derive(
    Clone,
    Debug,
    Default,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
#[repr(transparent)]
pub struct GenericCounters<I>(
    #[css(field_bound)]
    #[css(iterable, if_empty = "none")]
    crate::OwnedSlice<GenericCounterPair<I>>,
);
pub use self::GenericCounters as Counters;

#[cfg(feature = "servo")]
type CounterStyleType = ListStyleType;

#[cfg(feature = "gecko")]
type CounterStyleType = CounterStyle;

#[cfg(feature = "servo")]
#[inline]
fn is_decimal(counter_type: &CounterStyleType) -> bool {
    *counter_type == ListStyleType::Decimal
}

#[cfg(feature = "gecko")]
#[inline]
fn is_decimal(counter_type: &CounterStyleType) -> bool {
    *counter_type == CounterStyle::decimal()
}

/// Lookup keyword used by GCPM string()/element() functions.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum StringLookupKeyword {
    /// `first`
    First,
    /// `start`
    Start,
    /// `last`
    Last,
    /// `first-except`
    FirstExcept,
}

#[inline]
fn is_first_lookup(keyword: &StringLookupKeyword) -> bool {
    *keyword == StringLookupKeyword::First
}

/// Selector for content() in `string-set`.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    MallocSizeOf,
    Parse,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum StringSetContentKeyword {
    /// `text`
    Text,
    /// `before`
    Before,
    /// `after`
    After,
    /// `first-letter`
    FirstLetter,
}

/// Keyword for `target-text()` second argument.
///
/// https://www.w3.org/TR/css-gcpm-3/#funcdef-target-text
#[derive(
    Clone,
    Copy,
    Debug,
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
#[repr(u8)]
pub enum TargetTextKeyword {
    /// `content`
    Content,
    /// `before`
    Before,
    /// `after`
    After,
    /// `first-letter`
    FirstLetter,
}

impl Default for TargetTextKeyword {
    fn default() -> Self {
        Self::Content
    }
}

#[inline]
fn is_content_keyword(keyword: &TargetTextKeyword) -> bool {
    *keyword == TargetTextKeyword::Content
}

/// Type of leader pattern for `leader()` function.
///
/// https://www.w3.org/TR/css-gcpm-3/#funcdef-leader
#[derive(
    Clone,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
pub enum LeaderType {
    /// `dotted`
    Dotted,
    /// `solid`
    Solid,
    /// `space`
    Space,
    /// A custom string pattern.
    String(crate::OwnedStr),
}

impl ToCss for LeaderType {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        match self {
            Self::Dotted => dest.write_str("dotted"),
            Self::Solid => dest.write_str("solid"),
            Self::Space => dest.write_str("space"),
            Self::String(s) => s.to_css(dest),
        }
    }
}

/// The non-normal, non-none values of the content property.
#[derive(
    Clone, Debug, Eq, MallocSizeOf, PartialEq, SpecifiedValueInfo, ToComputedValue, ToShmem,
)]
#[repr(C)]
pub struct GenericContentItems<Image> {
    /// The actual content items. Note that, past the alt marker, only some subset (strings,
    /// attr(), counter())
    pub items: thin_vec::ThinVec<GenericContentItem<Image>>,
    /// The index at which alt text starts, always non-zero. If equal to items.len(), no alt text
    /// exists.
    pub alt_start: usize,
}

impl<Image> ToCss for GenericContentItems<Image>
where
    Image: ToCss,
{
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        for (i, item) in self.items.iter().enumerate() {
            if i == self.alt_start {
                dest.write_str(" /")?;
            }
            if i != 0 {
                dest.write_str(" ")?;
            }
            item.to_css(dest)?;
        }
        Ok(())
    }
}

/// The specified value for the `content` property.
///
/// https://drafts.csswg.org/css-content/#propdef-content
#[derive(
    Clone,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToCss,
    ToShmem,
    ToTyped,
)]
#[repr(u8)]
pub enum GenericContent<Image> {
    /// `normal` reserved keyword.
    Normal,
    /// `none` reserved keyword.
    None,
    /// Content items.
    Items(GenericContentItems<Image>),
}

pub use self::GenericContent as Content;

impl<Image> Content<Image> {
    /// Whether `self` represents list of items.
    #[inline]
    pub fn is_items(&self) -> bool {
        matches!(*self, Self::Items(..))
    }

    /// Set `content` property to `normal`.
    #[inline]
    pub fn normal() -> Self {
        Content::Normal
    }
}

/// Items for the `content` property.
#[derive(
    Clone,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    ToComputedValue,
    SpecifiedValueInfo,
    ToCss,
    ToResolvedValue,
    ToShmem,
)]
#[repr(u8)]
pub enum GenericContentItem<I> {
    /// Literal string content.
    String(crate::OwnedStr),
    /// `counter(name, style)`.
    #[css(comma, function)]
    Counter(CustomIdent, #[css(skip_if = "is_decimal")] CounterStyleType),
    /// `counters(name, separator, style)`.
    #[css(comma, function)]
    Counters(
        CustomIdent,
        crate::OwnedStr,
        #[css(skip_if = "is_decimal")] CounterStyleType,
    ),
    /// `string(name[, keyword])`.
    #[css(comma, function = "string")]
    StringFunction(
        CustomIdent,
        #[css(skip_if = "is_first_lookup")] StringLookupKeyword,
    ),
    /// `element(name[, keyword])`.
    #[css(comma, function = "element")]
    ElementFunction(
        CustomIdent,
        #[css(skip_if = "is_first_lookup")] StringLookupKeyword,
    ),
    /// `content([text|before|after|first-letter])`.
    #[css(function = "content")]
    ContentFunction(StringSetContentKeyword),
    /// `open-quote`.
    OpenQuote,
    /// `close-quote`.
    CloseQuote,
    /// `no-open-quote`.
    NoOpenQuote,
    /// `no-close-quote`.
    NoCloseQuote,
    /// `-moz-alt-content`.
    #[cfg(feature = "gecko")]
    MozAltContent,
    /// `-moz-label-content`.
    /// This is needed to make `accesskey` work for XUL labels. It's basically
    /// attr(value) otherwise.
    #[cfg(feature = "gecko")]
    MozLabelContent,
    /// `attr([namespace? `|`]? ident)`
    Attr(Attr),
    /// image-set(url) | url(url)
    Image(I),
    /// `target-counter(<url>, <ident>, <counter-style>?)`
    ///
    /// https://www.w3.org/TR/css-gcpm-3/#funcdef-target-counter
    #[css(comma, function = "target-counter")]
    TargetCounter(crate::OwnedStr, CustomIdent, #[css(skip_if = "is_decimal")] CounterStyleType),
    /// `target-text(<url>, <keyword>?)`
    ///
    /// https://www.w3.org/TR/css-gcpm-3/#funcdef-target-text
    #[css(comma, function = "target-text")]
    TargetText(crate::OwnedStr, #[css(skip_if = "is_content_keyword")] TargetTextKeyword),
    /// `leader(dotted | solid | space | <string>)`
    ///
    /// https://www.w3.org/TR/css-gcpm-3/#funcdef-leader
    #[css(function)]
    Leader(LeaderType),
}

pub use self::GenericContentItem as ContentItem;

/// A single named string assignment in `string-set`.
#[derive(
    Clone,
    Debug,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
)]
#[repr(C)]
pub struct GenericStringSetAssignment<I> {
    /// Named string identifier.
    pub name: CustomIdent,
    /// Content list used to compute the string value.
    #[css(field_bound)]
    pub value: crate::OwnedSlice<GenericContentItem<I>>,
}

impl<I: ToCss> ToCss for GenericStringSetAssignment<I> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        self.name.to_css(dest)?;
        for item in &*self.value {
            dest.write_str(" ")?;
            item.to_css(dest)?;
        }
        Ok(())
    }
}

/// Value of the `string-set` property.
#[derive(
    Clone,
    Debug,
    Default,
    Eq,
    MallocSizeOf,
    PartialEq,
    SpecifiedValueInfo,
    ToComputedValue,
    ToResolvedValue,
    ToShmem,
    ToTyped,
)]
#[repr(transparent)]
pub struct GenericStringSet<I>(
    #[css(field_bound)] pub crate::OwnedSlice<GenericStringSetAssignment<I>>,
);

impl<I> GenericStringSet<I> {
    /// `none` value.
    #[inline]
    pub fn none() -> Self {
        Self(Default::default())
    }

    /// Whether this is the `none` value.
    #[inline]
    pub fn is_none(&self) -> bool {
        self.0.is_empty()
    }
}

impl<I: ToCss> ToCss for GenericStringSet<I> {
    fn to_css<W>(&self, dest: &mut CssWriter<W>) -> fmt::Result
    where
        W: Write,
    {
        if self.0.is_empty() {
            return dest.write_str("none");
        }

        let mut first = true;
        for assignment in &*self.0 {
            if !first {
                dest.write_str(", ")?;
            }
            assignment.to_css(dest)?;
            first = false;
        }
        Ok(())
    }
}

pub use self::GenericStringSet as StringSet;
pub use self::GenericStringSetAssignment as StringSetAssignment;
