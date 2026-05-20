/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! Computed values for CSS Text Decoration 4 longhands wired in
//! `style/values/specified/text_decor_4.rs`.
//!
//! `TextEmphasisSkip` and `TextDecorationSkipKind` are identity-computed;
//! the specified types derive `ToComputedValue`. `TextDecorationTrim`
//! computes its `Length` payload through `ToComputedValue`.

use crate::derives::*;
use crate::values::computed::length::Length;
use crate::values::computed::{Context, ToComputedValue};
use crate::values::specified::text_decor_4 as specified;
use to_shmem::ToShmem;

pub use specified::{TextDecorationSkipKind, TextEmphasisSkip};

/// Computed value of `text-decoration-trim`.
#[derive(Clone, Debug, MallocSizeOf, PartialEq, ToCss, ToResolvedValue, ToTyped)]
#[repr(C, u8)]
pub enum TextDecorationTrim {
    /// `auto` — UA-chosen trim distances (initial).
    Auto,
    /// `<length>{1,2}` — explicit trim distances. `end` defaults to
    /// `start` when only one length is authored.
    Length {
        /// Trim distance at the start edge of the line.
        start: Length,
        /// Trim distance at the end edge of the line.
        end: Length,
    },
}

impl TextDecorationTrim {
    /// `auto` value.
    #[inline]
    pub fn auto() -> Self {
        Self::Auto
    }

    /// Whether the value is `auto`.
    #[inline]
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

impl ToComputedValue for specified::TextDecorationTrim {
    type ComputedValue = TextDecorationTrim;

    fn to_computed_value(&self, ctx: &Context) -> Self::ComputedValue {
        match self {
            specified::TextDecorationTrim::Auto => TextDecorationTrim::Auto,
            specified::TextDecorationTrim::Length { start, end } => TextDecorationTrim::Length {
                start: start.to_computed_value(ctx),
                end: end.to_computed_value(ctx),
            },
        }
    }

    fn from_computed_value(computed: &Self::ComputedValue) -> Self {
        match computed {
            TextDecorationTrim::Auto => specified::TextDecorationTrim::Auto,
            TextDecorationTrim::Length { start, end } => specified::TextDecorationTrim::Length {
                start: ToComputedValue::from_computed_value(start),
                end: ToComputedValue::from_computed_value(end),
            },
        }
    }
}

// `ToShmem` is implemented manually because the inner `Length` is a
// computed-side type that does not implement `ToShmem`. The clone is
// safe — `Length` is a small `Copy`-equivalent payload.
impl ToShmem for TextDecorationTrim {
    fn to_shmem(&self, _: &mut to_shmem::SharedMemoryBuilder) -> to_shmem::Result<Self> {
        Ok(std::mem::ManuallyDrop::new(self.clone()))
    }
}
